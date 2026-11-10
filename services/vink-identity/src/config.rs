//! Process configuration read from the environment. [`ServeConfig`] uses the Identity runtime
//! credential; [`MigrateConfig`] uses only the Identity migration credential. Variables are
//! prefixed `VINK_IDENTITY_` so one environment can also hold the App's settings without a
//! process reading the other's credential.
//!
//! Validation collects every problem at once and never echoes a database URL.

use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use crate::mail::{SmtpSecurity, SmtpSettings};

/// `sun_path` is 104 bytes on macOS and 108 on Linux, including the terminator.
pub const MAX_SOCKET_PATH: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Text,
    Json,
}

/// Which authority `serve` runs (D-05).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorityChoice {
    /// Default: every verification answers `authority_not_configured`.
    NotConfigured,
    /// `VINK_IDENTITY_AUTHORITY=email-code`: native accounts proven by a one-time code, mailed
    /// by the delivery worker through `mail`.
    EmailCode {
        mail: MailChoice,
        /// `VINK_IDENTITY_GLOBAL_CHALLENGE_LIMIT`: codes per 10 minutes across all addresses.
        global_challenge_limit: Option<i64>,
    },
}

/// `VINK_IDENTITY_MAIL_TRANSPORT`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MailChoice {
    /// `local-outbox` (`VINK_IDENTITY_LOCAL_OUTBOX`): files on this machine; development only.
    LocalOutbox(PathBuf),
    /// `smtp`: authenticated submission over TLS (`VINK_IDENTITY_SMTP_*`).
    Smtp(SmtpSettings),
}

impl MailChoice {
    pub fn label(&self) -> &'static str {
        match self {
            MailChoice::LocalOutbox(_) => "local_outbox",
            MailChoice::Smtp(_) => "smtp",
        }
    }
}

const SMTP_KEYS: [&str; 8] = [
    "VINK_IDENTITY_SMTP_PROVIDER",
    "VINK_IDENTITY_SMTP_HOST",
    "VINK_IDENTITY_SMTP_PORT",
    "VINK_IDENTITY_SMTP_SECURITY",
    "VINK_IDENTITY_SMTP_USERNAME",
    "VINK_IDENTITY_SMTP_PASSWORD_FILE",
    "VINK_IDENTITY_SMTP_TIMEOUT_SECONDS",
    "VINK_IDENTITY_MAIL_FROM",
];

/// Defaults of a known SMTP provider: `VINK_IDENTITY_SMTP_PROVIDER=<name>` fills host, security
/// and username; explicit `VINK_IDENTITY_SMTP_*` values still win. Switching to another SMTP
/// provider is one row here (or explicit settings) — the outbox, worker and messages do not change.
struct SmtpPreset {
    name: &'static str,
    host: &'static str,
    security: SmtpSecurity,
    username: &'static str,
}

const SMTP_PRESETS: [SmtpPreset; 1] = [SmtpPreset {
    // https://resend.com/docs/send-with-smtp — the password is an API key (sending access).
    name: "resend",
    host: "smtp.resend.com",
    security: SmtpSecurity::ImplicitTls,
    username: "resend",
}];

/// Longest SMTP conversation allowed; the delivery lease (45 s) must outlast one send.
pub const MAX_SMTP_TIMEOUT_SECONDS: u64 = 30;

fn loopback_host(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "::1" | "[::1]")
}

fn smtp_settings(
    value: &dyn Fn(&str) -> Option<String>,
    problems: &mut Vec<String>,
) -> Option<SmtpSettings> {
    let before = problems.len();
    let preset = match value("VINK_IDENTITY_SMTP_PROVIDER")
        .as_deref()
        .map(str::trim)
    {
        None => None,
        Some(name) => match SMTP_PRESETS.iter().find(|preset| preset.name == name) {
            Some(preset) => Some(preset),
            None => {
                let known: Vec<&str> = SMTP_PRESETS.iter().map(|p| p.name).collect();
                problems.push(format!(
                    "VINK_IDENTITY_SMTP_PROVIDER: unknown `{name}` (known: {}); or leave it unset \
                     and give VINK_IDENTITY_SMTP_HOST",
                    known.join(", ")
                ));
                None
            }
        },
    };
    let host = value("VINK_IDENTITY_SMTP_HOST")
        .map(|h| h.trim().to_string())
        .or_else(|| preset.map(|p| p.host.to_string()));
    if host.is_none() {
        problems.push("VINK_IDENTITY_SMTP_HOST: missing (SMTP relay host name)".into());
    }
    let security = match value("VINK_IDENTITY_SMTP_SECURITY")
        .as_deref()
        .map(str::trim)
    {
        None if preset.is_some() => preset.map_or(SmtpSecurity::StartTls, |p| p.security),
        None | Some("starttls") => SmtpSecurity::StartTls,
        Some("tls") => SmtpSecurity::ImplicitTls,
        Some("none") => SmtpSecurity::Plaintext,
        Some(other) => {
            problems.push(format!(
                "VINK_IDENTITY_SMTP_SECURITY: expected `starttls`, `tls` or `none`, got `{other}`"
            ));
            SmtpSecurity::StartTls
        }
    };
    if security == SmtpSecurity::Plaintext && !host.as_deref().is_some_and(loopback_host) {
        problems.push(
            "VINK_IDENTITY_SMTP_SECURITY: `none` is only allowed for a loopback relay              (localhost, 127.0.0.1, ::1); use `starttls` or `tls`"
                .into(),
        );
    }
    let port = match value("VINK_IDENTITY_SMTP_PORT") {
        None => match security {
            SmtpSecurity::ImplicitTls => 465,
            SmtpSecurity::StartTls => 587,
            SmtpSecurity::Plaintext => 25,
        },
        Some(raw) => match raw.trim().parse::<u16>() {
            Ok(port) if port > 0 => port,
            _ => {
                problems.push("VINK_IDENTITY_SMTP_PORT: expected a TCP port".into());
                0
            }
        },
    };
    let username =
        value("VINK_IDENTITY_SMTP_USERNAME").or_else(|| preset.map(|p| p.username.to_string()));
    let password_file = value("VINK_IDENTITY_SMTP_PASSWORD_FILE").map(PathBuf::from);
    if username.is_some() != password_file.is_some() {
        problems.push(
            "VINK_IDENTITY_SMTP_USERNAME and VINK_IDENTITY_SMTP_PASSWORD_FILE: set both or neither"
                .into(),
        );
    }
    let from = value("VINK_IDENTITY_MAIL_FROM");
    if from.is_none() {
        problems.push(
            "VINK_IDENTITY_MAIL_FROM: missing (sender mailbox such as `Vink <no-reply@vink.vn>`)"
                .into(),
        );
    }
    let timeout = match value("VINK_IDENTITY_SMTP_TIMEOUT_SECONDS") {
        None => 10,
        Some(raw) => match raw.trim().parse::<u64>() {
            Ok(seconds) if (1..=MAX_SMTP_TIMEOUT_SECONDS).contains(&seconds) => seconds,
            _ => {
                problems.push(format!(
                    "VINK_IDENTITY_SMTP_TIMEOUT_SECONDS: expected 1–{MAX_SMTP_TIMEOUT_SECONDS}"
                ));
                10
            }
        },
    };
    (problems.len() == before).then(|| SmtpSettings {
        host: host.unwrap_or_default(),
        port,
        security,
        username,
        password_file,
        from: from.unwrap_or_default(),
        timeout: Duration::from_secs(timeout),
    })
}

#[derive(Clone)]
pub struct ServeConfig {
    pub database_url: String,
    /// Unix socket the private contract is served on; there is no TCP listener.
    pub socket: PathBuf,
    /// The only OS user whose connections may call the contract. `None` keeps the contract
    /// closed: every call is refused and readiness reports `callers: not_configured`.
    pub caller_uid: Option<u32>,
    pub authority: AuthorityChoice,
    pub log_format: LogFormat,
}

impl fmt::Debug for ServeConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServeConfig")
            .field("database_url", &"<redacted>")
            .field("socket", &self.socket)
            .field("caller_uid", &self.caller_uid)
            .field("authority", &self.authority)
            .field("log_format", &self.log_format)
            .finish()
    }
}

/// `vink-identity account …`: operator commands on the runtime credential.
#[derive(Clone)]
pub struct OperatorConfig {
    pub database_url: String,
    pub log_format: LogFormat,
}

impl fmt::Debug for OperatorConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OperatorConfig")
            .field("database_url", &"<redacted>")
            .field("log_format", &self.log_format)
            .finish()
    }
}

impl OperatorConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let mut problems = Vec::new();
        let value = |key: &str| get(key).filter(|v| !v.trim().is_empty());
        let database_url = database_url(
            "VINK_IDENTITY_DATABASE_URL",
            value("VINK_IDENTITY_DATABASE_URL"),
            "PostgreSQL URL of the Identity runtime role",
            &mut problems,
        );
        let log_format = log_format(value("VINK_LOG_FORMAT"), &mut problems);
        if problems.is_empty() {
            Ok(Self {
                database_url,
                log_format,
            })
        } else {
            Err(ConfigError { problems })
        }
    }
}

#[derive(Clone)]
pub struct MigrateConfig {
    pub database_url: String,
    pub log_format: LogFormat,
}

impl fmt::Debug for MigrateConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MigrateConfig")
            .field("database_url", &"<redacted>")
            .field("log_format", &self.log_format)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    pub problems: Vec<String>,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "invalid configuration:")?;
        for problem in &self.problems {
            writeln!(f, "  - {problem}")?;
        }
        write!(f, "see services/vink-identity/README.md")
    }
}

impl std::error::Error for ConfigError {}

impl ServeConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let mut problems = Vec::new();
        let value = |key: &str| get(key).filter(|v| !v.trim().is_empty());
        let database_url = database_url(
            "VINK_IDENTITY_DATABASE_URL",
            value("VINK_IDENTITY_DATABASE_URL"),
            "PostgreSQL URL of the Identity runtime role",
            &mut problems,
        );
        let socket = match value("VINK_IDENTITY_SOCKET") {
            None => {
                problems
                    .push("VINK_IDENTITY_SOCKET: missing (path of the private Unix socket)".into());
                PathBuf::new()
            }
            Some(path) if path.len() > MAX_SOCKET_PATH => {
                problems.push(format!(
                    "VINK_IDENTITY_SOCKET: longer than {MAX_SOCKET_PATH} bytes, the Unix socket path limit"
                ));
                PathBuf::new()
            }
            Some(path) => PathBuf::from(path),
        };
        let caller_uid = match value("VINK_IDENTITY_CALLER_UID") {
            None => None,
            Some(raw) => match raw.trim().parse::<u32>() {
                Ok(uid) => Some(uid),
                Err(_) => {
                    problems.push("VINK_IDENTITY_CALLER_UID: expected a numeric user id".into());
                    None
                }
            },
        };
        let authority = authority(&value, &mut problems);
        let log_format = log_format(value("VINK_LOG_FORMAT"), &mut problems);
        if problems.is_empty() {
            Ok(Self {
                database_url,
                socket,
                caller_uid,
                authority,
                log_format,
            })
        } else {
            Err(ConfigError { problems })
        }
    }
}

/// Reads the authority and, for `email-code`, its mail transport. A secret is never accepted as a
/// value: the SMTP password comes only from a file.
fn authority(
    value: &dyn Fn(&str) -> Option<String>,
    problems: &mut Vec<String>,
) -> AuthorityChoice {
    if value("VINK_IDENTITY_SMTP_PASSWORD").is_some() {
        problems.push(
            "VINK_IDENTITY_SMTP_PASSWORD: not accepted; put the password in a file readable only \
             by the Identity user and set VINK_IDENTITY_SMTP_PASSWORD_FILE"
                .into(),
        );
    }
    let outbox = value("VINK_IDENTITY_LOCAL_OUTBOX");
    let smtp_set = SMTP_KEYS.iter().any(|key| value(key).is_some());
    let transport = value("VINK_IDENTITY_MAIL_TRANSPORT").map(|t| t.trim().to_string());
    let global = value("VINK_IDENTITY_GLOBAL_CHALLENGE_LIMIT");
    match value("VINK_IDENTITY_AUTHORITY").as_deref().map(str::trim) {
        None => {
            for (set, key) in [
                (outbox.is_some(), "VINK_IDENTITY_LOCAL_OUTBOX"),
                (transport.is_some(), "VINK_IDENTITY_MAIL_TRANSPORT"),
                (smtp_set, "VINK_IDENTITY_SMTP_*"),
                (global.is_some(), "VINK_IDENTITY_GLOBAL_CHALLENGE_LIMIT"),
            ] {
                if set {
                    problems.push(format!(
                        "{key}: only valid with VINK_IDENTITY_AUTHORITY=email-code"
                    ));
                }
            }
            AuthorityChoice::NotConfigured
        }
        Some("email-code") => {
            let global_challenge_limit = match global {
                None => None,
                Some(raw) => match raw.trim().parse::<i64>() {
                    Ok(limit) if limit > 0 => Some(limit),
                    _ => {
                        problems.push(
                            "VINK_IDENTITY_GLOBAL_CHALLENGE_LIMIT: expected a positive integer \
                             (codes per 10 minutes)"
                                .into(),
                        );
                        None
                    }
                },
            };
            // Without an explicit transport, a local outbox keeps the spike's configuration.
            let choice = transport
                .as_deref()
                .or(outbox.as_ref().map(|_| "local-outbox"));
            let mail = match choice {
                Some("local-outbox") => {
                    if smtp_set {
                        problems.push(
                            "VINK_IDENTITY_SMTP_*: not used with the local outbox; remove them or \
                             set VINK_IDENTITY_MAIL_TRANSPORT=smtp"
                                .into(),
                        );
                    }
                    match outbox {
                        Some(dir) => Some(MailChoice::LocalOutbox(PathBuf::from(dir))),
                        None => {
                            problems.push(
                                "VINK_IDENTITY_LOCAL_OUTBOX: missing (directory of the local \
                                 outbox)"
                                    .into(),
                            );
                            None
                        }
                    }
                }
                Some("smtp") => {
                    if outbox.is_some() {
                        problems.push(
                            "VINK_IDENTITY_LOCAL_OUTBOX: not used with SMTP; remove it".into(),
                        );
                    }
                    smtp_settings(value, problems).map(MailChoice::Smtp)
                }
                Some(other) => {
                    problems.push(format!(
                        "VINK_IDENTITY_MAIL_TRANSPORT: expected `smtp` or `local-outbox`, got `{other}`"
                    ));
                    None
                }
                None => {
                    problems.push(
                        "VINK_IDENTITY_MAIL_TRANSPORT: missing (`smtp`, or `local-outbox` for \
                         development)"
                            .into(),
                    );
                    None
                }
            };
            match mail {
                Some(mail) => AuthorityChoice::EmailCode {
                    mail,
                    global_challenge_limit,
                },
                None => AuthorityChoice::NotConfigured,
            }
        }
        Some(other) => {
            problems.push(format!(
                "VINK_IDENTITY_AUTHORITY: expected `email-code` or unset, got `{other}`"
            ));
            AuthorityChoice::NotConfigured
        }
    }
}

/// `vink-identity mail test <address>`: only the mail settings of `serve`.
#[derive(Debug, Clone)]
pub struct MailConfig {
    pub mail: MailChoice,
    pub log_format: LogFormat,
}

impl MailConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let mut problems = Vec::new();
        let value = |key: &str| get(key).filter(|v| !v.trim().is_empty());
        let authority = authority(&value, &mut problems);
        let log_format = log_format(value("VINK_LOG_FORMAT"), &mut problems);
        match authority {
            AuthorityChoice::EmailCode { mail, .. } if problems.is_empty() => {
                Ok(Self { mail, log_format })
            }
            AuthorityChoice::NotConfigured if problems.is_empty() => Err(ConfigError {
                problems: vec![
                    "VINK_IDENTITY_AUTHORITY: `email-code` with a mail transport is needed to \
                     test mail"
                        .into(),
                ],
            }),
            _ => Err(ConfigError { problems }),
        }
    }
}

impl MigrateConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// Reads only `VINK_IDENTITY_MIGRATE_DATABASE_URL`; the runtime credential is never a
    /// fallback.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let mut problems = Vec::new();
        let value = |key: &str| get(key).filter(|v| !v.trim().is_empty());
        let database_url = database_url(
            "VINK_IDENTITY_MIGRATE_DATABASE_URL",
            value("VINK_IDENTITY_MIGRATE_DATABASE_URL"),
            "PostgreSQL URL of the Identity migration role",
            &mut problems,
        );
        let log_format = log_format(value("VINK_LOG_FORMAT"), &mut problems);
        if problems.is_empty() {
            Ok(Self {
                database_url,
                log_format,
            })
        } else {
            Err(ConfigError { problems })
        }
    }
}

fn database_url(
    key: &str,
    raw: Option<String>,
    purpose: &str,
    problems: &mut Vec<String>,
) -> String {
    match raw {
        Some(url) if url.starts_with("postgres://") || url.starts_with("postgresql://") => url,
        Some(_) => {
            problems.push(format!("{key}: must be a postgres:// or postgresql:// URL"));
            String::new()
        }
        None => {
            problems.push(format!("{key}: missing ({purpose})"));
            String::new()
        }
    }
}

fn log_format(raw: Option<String>, problems: &mut Vec<String>) -> LogFormat {
    match raw.as_deref() {
        None | Some("text") => LogFormat::Text,
        Some("json") => LogFormat::Json,
        Some(other) => {
            problems.push(format!(
                "VINK_LOG_FORMAT: expected `text` or `json`, got `{other}`"
            ));
            LogFormat::Text
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    const URL: &str = "postgres://vink_identity_runtime:secret@127.0.0.1:5432/vink_identity";

    #[test]
    fn serve_reports_every_missing_setting_without_secrets() {
        let vars = env(&[("VINK_IDENTITY_CALLER_UID", "app")]);
        let error = ServeConfig::from_lookup(|k| vars.get(k).cloned()).unwrap_err();
        let text = error.to_string();
        assert!(
            text.contains("VINK_IDENTITY_DATABASE_URL: missing"),
            "{text}"
        );
        assert!(text.contains("VINK_IDENTITY_SOCKET: missing"), "{text}");
        assert!(
            text.contains("VINK_IDENTITY_CALLER_UID: expected"),
            "{text}"
        );

        let vars = env(&[("VINK_IDENTITY_DATABASE_URL", "mysql://u:secret@db/x")]);
        let error = ServeConfig::from_lookup(|k| vars.get(k).cloned()).unwrap_err();
        assert!(!error.to_string().contains("secret"), "{error}");
    }

    #[test]
    fn serve_does_not_read_app_credentials_and_caller_is_optional() {
        let vars = env(&[
            ("DATABASE_URL", URL),
            ("VINK_IDENTITY_SOCKET", "/run/vink/identity.sock"),
        ]);
        let error = ServeConfig::from_lookup(|k| vars.get(k).cloned()).unwrap_err();
        assert_eq!(error.problems.len(), 1, "{error}");

        let vars = env(&[
            ("VINK_IDENTITY_DATABASE_URL", URL),
            ("VINK_IDENTITY_SOCKET", "/run/vink/identity.sock"),
        ]);
        let config = ServeConfig::from_lookup(|k| vars.get(k).cloned()).unwrap();
        assert_eq!(config.caller_uid, None);
        assert!(!format!("{config:?}").contains("secret"));
    }

    #[test]
    fn socket_path_must_fit_the_unix_limit() {
        let long = format!("/{}", "s".repeat(MAX_SOCKET_PATH));
        let vars = env(&[
            ("VINK_IDENTITY_DATABASE_URL", URL),
            ("VINK_IDENTITY_SOCKET", &long),
        ]);
        let error = ServeConfig::from_lookup(|k| vars.get(k).cloned()).unwrap_err();
        assert!(
            error.to_string().contains("Unix socket path limit"),
            "{error}"
        );
    }

    #[test]
    fn the_email_code_authority_needs_a_mail_transport_and_nothing_else_is_accepted() {
        let base = [
            ("VINK_IDENTITY_DATABASE_URL", URL),
            ("VINK_IDENTITY_SOCKET", "/run/vink/identity.sock"),
        ];
        let with = |extra: &[(&str, &str)]| {
            let mut pairs = base.to_vec();
            pairs.extend_from_slice(extra);
            let vars = env(&pairs);
            ServeConfig::from_lookup(|k| vars.get(k).cloned())
        };
        assert_eq!(with(&[]).unwrap().authority, AuthorityChoice::NotConfigured);
        // The spike's configuration still means the local outbox.
        let local = AuthorityChoice::EmailCode {
            mail: MailChoice::LocalOutbox(PathBuf::from("/run/vink/outbox")),
            global_challenge_limit: None,
        };
        assert_eq!(
            with(&[
                ("VINK_IDENTITY_AUTHORITY", "email-code"),
                ("VINK_IDENTITY_LOCAL_OUTBOX", "/run/vink/outbox"),
            ])
            .unwrap()
            .authority,
            local
        );
        assert_eq!(
            with(&[
                ("VINK_IDENTITY_AUTHORITY", "email-code"),
                ("VINK_IDENTITY_MAIL_TRANSPORT", "local-outbox"),
                ("VINK_IDENTITY_LOCAL_OUTBOX", "/run/vink/outbox"),
            ])
            .unwrap()
            .authority,
            local
        );
        for (extra, expected) in [
            (
                &[("VINK_IDENTITY_AUTHORITY", "email-code")][..],
                "VINK_IDENTITY_MAIL_TRANSPORT: missing",
            ),
            (
                &[("VINK_IDENTITY_LOCAL_OUTBOX", "/run/vink/outbox")][..],
                "only valid with",
            ),
            (
                &[("VINK_IDENTITY_SMTP_HOST", "smtp.example.vn")][..],
                "only valid with",
            ),
            (
                &[("VINK_IDENTITY_AUTHORITY", "password")][..],
                "VINK_IDENTITY_AUTHORITY: expected",
            ),
            (
                &[
                    ("VINK_IDENTITY_AUTHORITY", "email-code"),
                    ("VINK_IDENTITY_MAIL_TRANSPORT", "sendgrid"),
                ][..],
                "expected `smtp` or `local-outbox`",
            ),
            (
                &[
                    ("VINK_IDENTITY_AUTHORITY", "email-code"),
                    ("VINK_IDENTITY_LOCAL_OUTBOX", "/run/vink/outbox"),
                    ("VINK_IDENTITY_GLOBAL_CHALLENGE_LIMIT", "0"),
                ][..],
                "GLOBAL_CHALLENGE_LIMIT: expected a positive",
            ),
        ] {
            let error = with(extra).unwrap_err().to_string();
            assert!(error.contains(expected), "{extra:?}: {error}");
        }
    }

    #[test]
    fn smtp_requires_tls_off_loopback_and_never_takes_a_password_value() {
        let base = [
            ("VINK_IDENTITY_DATABASE_URL", URL),
            ("VINK_IDENTITY_SOCKET", "/run/vink/identity.sock"),
            ("VINK_IDENTITY_AUTHORITY", "email-code"),
            ("VINK_IDENTITY_MAIL_TRANSPORT", "smtp"),
        ];
        let with = |extra: &[(&str, &str)]| {
            let mut pairs = base.to_vec();
            pairs.extend_from_slice(extra);
            let vars = env(&pairs);
            ServeConfig::from_lookup(|k| vars.get(k).cloned())
        };
        let good = [
            ("VINK_IDENTITY_SMTP_HOST", "smtp.example.vn"),
            ("VINK_IDENTITY_SMTP_USERNAME", "vink"),
            ("VINK_IDENTITY_SMTP_PASSWORD_FILE", "/run/secrets/smtp"),
            ("VINK_IDENTITY_MAIL_FROM", "Vink <no-reply@vink.vn>"),
        ];
        let config = with(&good).unwrap();
        let AuthorityChoice::EmailCode {
            mail: MailChoice::Smtp(smtp),
            ..
        } = &config.authority
        else {
            panic!("{:?}", config.authority);
        };
        assert_eq!(
            (smtp.port, smtp.security, smtp.timeout),
            (587, SmtpSecurity::StartTls, Duration::from_secs(10))
        );
        let mut implicit = good.to_vec();
        implicit.push(("VINK_IDENTITY_SMTP_SECURITY", "tls"));
        implicit.push(("VINK_IDENTITY_GLOBAL_CHALLENGE_LIMIT", "50"));
        let config = with(&implicit).unwrap();
        assert!(matches!(
            config.authority,
            AuthorityChoice::EmailCode {
                mail: MailChoice::Smtp(SmtpSettings { port: 465, .. }),
                global_challenge_limit: Some(50),
            }
        ));
        let mut plain = good.to_vec();
        plain.push(("VINK_IDENTITY_SMTP_SECURITY", "none"));
        assert!(
            with(&plain)
                .unwrap_err()
                .to_string()
                .contains("only allowed for a loopback relay")
        );
        let loopback = [
            ("VINK_IDENTITY_SMTP_HOST", "127.0.0.1"),
            ("VINK_IDENTITY_SMTP_SECURITY", "none"),
            ("VINK_IDENTITY_SMTP_PORT", "2525"),
            ("VINK_IDENTITY_MAIL_FROM", "Vink <no-reply@vink.test>"),
        ];
        assert!(with(&loopback).is_ok());
        let mut secret = good.to_vec();
        secret.push(("VINK_IDENTITY_SMTP_PASSWORD", "hunter2-value"));
        let error = with(&secret).unwrap_err().to_string();
        assert!(
            error.contains("VINK_IDENTITY_SMTP_PASSWORD: not accepted"),
            "{error}"
        );
        assert!(!error.contains("hunter2-value"), "{error}");
        for (key, raw, expected) in [
            ("VINK_IDENTITY_SMTP_TIMEOUT_SECONDS", "31", "expected 1–30"),
            ("VINK_IDENTITY_SMTP_PORT", "smtp", "expected a TCP port"),
            ("VINK_IDENTITY_SMTP_SECURITY", "ssl", "expected `starttls`"),
        ] {
            let mut bad = good.to_vec();
            bad.push((key, raw));
            let error = with(&bad).unwrap_err().to_string();
            assert!(error.contains(expected), "{key}: {error}");
        }
        let error = with(&[("VINK_IDENTITY_SMTP_USERNAME", "vink")])
            .unwrap_err()
            .to_string();
        for needle in [
            "VINK_IDENTITY_SMTP_HOST: missing",
            "set both or neither",
            "VINK_IDENTITY_MAIL_FROM: missing",
        ] {
            assert!(error.contains(needle), "{error}");
        }
        // A provider preset fills host, security, port and username; explicit values win.
        let resend = [
            ("VINK_IDENTITY_SMTP_PROVIDER", "resend"),
            ("VINK_IDENTITY_SMTP_PASSWORD_FILE", "/run/secrets/resend"),
            ("VINK_IDENTITY_MAIL_FROM", "Vink <no-reply@vink.vn>"),
        ];
        let AuthorityChoice::EmailCode {
            mail: MailChoice::Smtp(preset),
            ..
        } = with(&resend).unwrap().authority
        else {
            panic!("resend preset");
        };
        assert_eq!(
            (
                preset.host.as_str(),
                preset.port,
                preset.security,
                preset.username.as_deref()
            ),
            (
                "smtp.resend.com",
                465,
                SmtpSecurity::ImplicitTls,
                Some("resend")
            )
        );
        let mut starttls = resend.to_vec();
        starttls.push(("VINK_IDENTITY_SMTP_SECURITY", "starttls"));
        let AuthorityChoice::EmailCode {
            mail: MailChoice::Smtp(preset),
            ..
        } = with(&starttls).unwrap().authority
        else {
            panic!("resend preset with STARTTLS");
        };
        assert_eq!(
            (preset.port, preset.security),
            (587, SmtpSecurity::StartTls)
        );
        let error = with(&[
            ("VINK_IDENTITY_SMTP_PROVIDER", "mailgun"),
            ("VINK_IDENTITY_MAIL_FROM", "Vink <no-reply@vink.vn>"),
        ])
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("unknown `mailgun` (known: resend)"),
            "{error}"
        );
        let mut mixed = good.to_vec();
        mixed.push(("VINK_IDENTITY_LOCAL_OUTBOX", "/run/vink/outbox"));
        assert!(
            with(&mixed)
                .unwrap_err()
                .to_string()
                .contains("not used with SMTP")
        );
        // Debug never prints the account name.
        assert!(!format!("{:?}", with(&good).unwrap()).contains("\"vink\""));
    }

    #[test]
    fn operator_commands_use_the_runtime_credential_only() {
        let vars = env(&[("VINK_IDENTITY_MIGRATE_DATABASE_URL", URL)]);
        assert!(OperatorConfig::from_lookup(|k| vars.get(k).cloned()).is_err());
        let vars = env(&[("VINK_IDENTITY_DATABASE_URL", URL)]);
        let config = OperatorConfig::from_lookup(|k| vars.get(k).cloned()).unwrap();
        assert!(!format!("{config:?}").contains("secret"));
    }

    #[test]
    fn migrate_reads_only_its_own_credential() {
        let vars = env(&[("VINK_IDENTITY_DATABASE_URL", URL), ("DATABASE_URL", URL)]);
        let error = MigrateConfig::from_lookup(|k| vars.get(k).cloned()).unwrap_err();
        assert_eq!(error.problems.len(), 1, "{error}");
        assert!(error.problems[0].starts_with("VINK_IDENTITY_MIGRATE_DATABASE_URL: missing"));

        let vars = env(&[("VINK_IDENTITY_MIGRATE_DATABASE_URL", URL)]);
        let config = MigrateConfig::from_lookup(|k| vars.get(k).cloned()).unwrap();
        assert!(!format!("{config:?}").contains("secret"));
    }
}
