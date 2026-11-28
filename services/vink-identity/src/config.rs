//! Process configuration read from the environment. [`ServeConfig`] uses the Identity runtime
//! credential; [`MigrateConfig`] uses only the Identity migration credential. Variables are
//! prefixed `VINK_IDENTITY_` so one environment can also hold the App's settings without a
//! process reading the other's credential.
//!
//! Validation collects every problem at once and never echoes a database URL.

use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use crate::mail::ResendSettings;

/// `sun_path` is 104 bytes on macOS and 108 on Linux, including the terminator.
pub(crate) const MAX_SOCKET_PATH: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Text,
    Json,
}

/// Which authority `serve` runs (D-05, ACC-03).
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
    /// `VINK_IDENTITY_AUTHORITY=password`: native accounts with username/password credentials,
    /// no mail transport required.
    Password,
    /// `VINK_IDENTITY_AUTHORITY=native`: native accounts with both password and email-code methods.
    Native {
        mail: MailChoice,
        global_challenge_limit: Option<i64>,
    },
}

/// `VINK_IDENTITY_MAIL_TRANSPORT`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MailChoice {
    /// `local-outbox` (`VINK_IDENTITY_LOCAL_OUTBOX`): files on this machine; development only.
    LocalOutbox(PathBuf),
    /// `resend-api`: HTTPS email submission using an on-disk sending key.
    Resend(ResendSettings),
}

impl MailChoice {
    pub fn label(&self) -> &'static str {
        match self {
            MailChoice::LocalOutbox(_) => "local_outbox",
            MailChoice::Resend(_) => "resend_api",
        }
    }
}

const RESEND_KEYS: [&str; 4] = [
    "VINK_IDENTITY_RESEND_API_KEY_FILE",
    "VINK_IDENTITY_RESEND_FROM",
    "VINK_IDENTITY_RESEND_TIMEOUT_SECONDS",
    "VINK_IDENTITY_RESEND_PROXY",
];

fn resend_settings(
    value: &dyn Fn(&str) -> Option<String>,
    problems: &mut Vec<String>,
) -> Option<ResendSettings> {
    let key_file = value("VINK_IDENTITY_RESEND_API_KEY_FILE").map(PathBuf::from);
    if key_file.is_none() {
        problems.push("VINK_IDENTITY_RESEND_API_KEY_FILE: missing (private secret file)".into());
    }
    let from = value("VINK_IDENTITY_RESEND_FROM");
    if from.is_none() {
        problems.push(
            "VINK_IDENTITY_RESEND_FROM: missing (mailbox on a verified sender domain)".into(),
        );
    }
    let timeout = match value("VINK_IDENTITY_RESEND_TIMEOUT_SECONDS") {
        None => 10,
        Some(raw) => match raw.parse::<u64>() {
            Ok(seconds) if (1..=30).contains(&seconds) => seconds,
            _ => {
                problems.push("VINK_IDENTITY_RESEND_TIMEOUT_SECONDS: expected 1–30".into());
                10
            }
        },
    };
    Some(ResendSettings {
        key_file: key_file?,
        from: from?,
        timeout: Duration::from_secs(timeout),
        proxy: value("VINK_IDENTITY_RESEND_PROXY"),
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

/// Reads the authority and its explicit mail transport. Inline API keys are never accepted.
fn authority(
    value: &dyn Fn(&str) -> Option<String>,
    problems: &mut Vec<String>,
) -> AuthorityChoice {
    if value("VINK_IDENTITY_RESEND_API_KEY").is_some() {
        problems.push(
            "VINK_IDENTITY_RESEND_API_KEY: not accepted; use VINK_IDENTITY_RESEND_API_KEY_FILE"
                .into(),
        );
    }
    if [
        "VINK_IDENTITY_SMTP_PROVIDER",
        "VINK_IDENTITY_SMTP_HOST",
        "VINK_IDENTITY_SMTP_PORT",
        "VINK_IDENTITY_SMTP_SECURITY",
        "VINK_IDENTITY_SMTP_USERNAME",
        "VINK_IDENTITY_SMTP_PASSWORD",
        "VINK_IDENTITY_SMTP_PASSWORD_FILE",
        "VINK_IDENTITY_SMTP_TIMEOUT_SECONDS",
        "VINK_IDENTITY_MAIL_FROM",
        "VINK_IDENTITY_RESEND_ENDPOINT",
    ]
    .iter()
    .any(|key| value(key).is_some())
    {
        problems.push("legacy SMTP settings and custom Resend endpoints are not supported".into());
    }
    let outbox = value("VINK_IDENTITY_LOCAL_OUTBOX");
    let resend_set = RESEND_KEYS.iter().any(|key| value(key).is_some());
    let transport = value("VINK_IDENTITY_MAIL_TRANSPORT").map(|t| t.trim().to_string());
    let global = value("VINK_IDENTITY_GLOBAL_CHALLENGE_LIMIT");
    match value("VINK_IDENTITY_AUTHORITY").as_deref().map(str::trim) {
        None => {
            for (set, key) in [
                (outbox.is_some(), "VINK_IDENTITY_LOCAL_OUTBOX"),
                (transport.is_some(), "VINK_IDENTITY_MAIL_TRANSPORT"),
                (resend_set, "VINK_IDENTITY_RESEND_*"),
                (global.is_some(), "VINK_IDENTITY_GLOBAL_CHALLENGE_LIMIT"),
            ] {
                if set {
                    problems.push(format!(
                        "{key}: only valid with VINK_IDENTITY_AUTHORITY=email-code or native"
                    ));
                }
            }
            AuthorityChoice::NotConfigured
        }
        Some("password") => {
            for (set, key) in [
                (outbox.is_some(), "VINK_IDENTITY_LOCAL_OUTBOX"),
                (transport.is_some(), "VINK_IDENTITY_MAIL_TRANSPORT"),
                (resend_set, "VINK_IDENTITY_RESEND_*"),
                (global.is_some(), "VINK_IDENTITY_GLOBAL_CHALLENGE_LIMIT"),
            ] {
                if set {
                    problems.push(format!(
                        "{key}: not used with VINK_IDENTITY_AUTHORITY=password (password does not send mail)"
                    ));
                }
            }
            AuthorityChoice::Password
        }
        Some(kind @ ("email-code" | "native")) => {
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
            let choice = transport.as_deref().or(outbox
                .as_ref()
                .filter(|_| !resend_set)
                .map(|_| "local-outbox"));
            let mail = match choice {
                Some("local-outbox") => {
                    if resend_set {
                        problems
                            .push("VINK_IDENTITY_RESEND_*: not used with the local outbox".into());
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
                Some("resend-api") => {
                    if outbox.is_some() {
                        problems.push(
                            "VINK_IDENTITY_LOCAL_OUTBOX: not used with Resend; remove it".into(),
                        );
                    }
                    resend_settings(value, problems).map(MailChoice::Resend)
                }
                Some(_) => {
                    problems.push(
                        "VINK_IDENTITY_MAIL_TRANSPORT: expected `resend-api` or `local-outbox`"
                            .into(),
                    );
                    None
                }
                None => {
                    problems.push(
                        "VINK_IDENTITY_MAIL_TRANSPORT: missing (`resend-api`, or `local-outbox` for \
                         development)"
                            .into(),
                    );
                    None
                }
            };
            match mail {
                Some(mail) if kind == "email-code" => AuthorityChoice::EmailCode {
                    mail,
                    global_challenge_limit,
                },
                Some(mail) => AuthorityChoice::Native {
                    mail,
                    global_challenge_limit,
                },
                None => AuthorityChoice::NotConfigured,
            }
        }
        Some(other) => {
            problems.push(format!(
                "VINK_IDENTITY_AUTHORITY: expected `password`, `email-code`, `native` or unset, got `{other}`"
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
            AuthorityChoice::EmailCode { mail, .. } | AuthorityChoice::Native { mail, .. } if problems.is_empty() => {
                Ok(Self { mail, log_format })
            }
            _ if problems.is_empty() => Err(ConfigError {
                problems: vec![
                    "VINK_IDENTITY_AUTHORITY: `native` or `email-code` with a mail transport is needed to \
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
        assert_eq!(
            with(&[("VINK_IDENTITY_AUTHORITY", "password")])
                .unwrap()
                .authority,
            AuthorityChoice::Password
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
                &[("VINK_IDENTITY_RESEND_FROM", "mail@example.vn")][..],
                "only valid with",
            ),
            (
                &[
                    ("VINK_IDENTITY_AUTHORITY", "password"),
                    ("VINK_IDENTITY_LOCAL_OUTBOX", "/run/vink/outbox"),
                ][..],
                "not used with VINK_IDENTITY_AUTHORITY=password",
            ),
            (
                &[("VINK_IDENTITY_AUTHORITY", "unsupported")][..],
                "VINK_IDENTITY_AUTHORITY: expected",
            ),
            (
                &[
                    ("VINK_IDENTITY_AUTHORITY", "email-code"),
                    ("VINK_IDENTITY_MAIL_TRANSPORT", "sendgrid"),
                ][..],
                "expected `resend-api` or `local-outbox`",
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
    fn resend_needs_file_and_sender_with_no_implicit_fallback() {
        let base = [
            ("VINK_IDENTITY_AUTHORITY", "native"),
            ("VINK_IDENTITY_MAIL_TRANSPORT", "resend-api"),
            ("VINK_IDENTITY_RESEND_API_KEY_FILE", "/run/secrets/resend"),
            ("VINK_IDENTITY_RESEND_FROM", "Vink <no-reply@vink.vn>"),
        ];
        let with = |extra: &[(&str, &str)]| {
            let mut pairs = base.to_vec();
            pairs.extend_from_slice(extra);
            let vars = env(&pairs);
            MailConfig::from_lookup(|k| vars.get(k).cloned())
        };
        assert!(matches!(with(&[]).unwrap().mail, MailChoice::Resend(_)));
        for (key, raw, expected) in [
            (
                "VINK_IDENTITY_RESEND_API_KEY_FILE",
                "",
                "missing (private secret file)",
            ),
            ("VINK_IDENTITY_RESEND_FROM", "", "missing (mailbox"),
            (
                "VINK_IDENTITY_RESEND_API_KEY",
                "re_DO_NOT_PRINT",
                "not accepted",
            ),
            (
                "VINK_IDENTITY_RESEND_TIMEOUT_SECONDS",
                "31",
                "expected 1–30",
            ),
            (
                "VINK_IDENTITY_MAIL_TRANSPORT",
                "smtp",
                "expected `resend-api`",
            ),
            ("VINK_IDENTITY_SMTP_PASSWORD", "DO_NOT_PRINT", "legacy SMTP"),
            (
                "VINK_IDENTITY_LOCAL_OUTBOX",
                "/tmp/mail",
                "not used with Resend",
            ),
            (
                "VINK_IDENTITY_MAIL_TRANSPORT",
                "",
                "MAIL_TRANSPORT: missing",
            ),
        ] {
            let error = with(&[(key, raw)]).unwrap_err().to_string();
            assert!(error.contains(expected), "{key}: {error}");
            assert!(!error.contains("DO_NOT_PRINT"), "{error}");
        }
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
