//! Process configuration read from the environment. [`Config`] serves `vink-app serve|dev-seed`
//! and `vink-worker` with the runtime credential; [`MigrateConfig`] serves `vink-app migrate`
//! with the separate migration credential.
//!
//! Validation collects every problem at once and never echoes a database URL.

use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;

use crate::domain::abuse::TrustedProxies;
use crate::domain::ids::PrincipalId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// Developer machine. Allows the loopback-only identity fixture and the development seed.
    Local,
    /// Default. Private routes stay closed until an auth authority is selected (D-05).
    Production,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Text,
    Json,
}

#[derive(Clone)]
pub struct Config {
    pub profile: Profile,
    pub database_url: String,
    pub bind: SocketAddr,
    /// Origin the browser uses, e.g. `http://127.0.0.1:3000`; canonical URLs and the
    /// Origin check for unsafe API methods use it.
    pub public_origin: String,
    /// Directory containing `pkg/` (wasm, JS and CSS built by `cargo xtask build`).
    pub site_root: PathBuf,
    /// Local-profile identity fixture; see [`crate::identity`].
    pub dev_principal: Option<PrincipalId>,
    /// The separate Identity process, when configured; see [`crate::identity::IdentityService`].
    pub identity: Option<IdentitySettings>,
    /// `VINK_AUTH_SIGN_IN`: whether the sign-in routes may start new sessions. Default `enabled`
    /// in the local profile and `disabled` in production, where D-05 is Provisional: production
    /// activation is an explicit operator decision, not a side effect of configuring Identity.
    /// Sessions already issued keep being verified either way.
    pub sign_in_enabled: bool,
    /// `VINK_TRUSTED_PROXIES`: proxies whose `X-Forwarded-For` attributes a request to a client
    /// network for rate limits. Empty: only the TCP peer counts.
    pub trusted_proxies: TrustedProxies,
    pub log_format: LogFormat,
}

/// Where the Identity process listens and which OS user must be serving that socket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentitySettings {
    pub socket: PathBuf,
    pub server_uid: u32,
}

/// `sun_path` is 104 bytes on macOS and 108 on Linux, including the terminator.
const MAX_SOCKET_PATH: usize = 100;

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("profile", &self.profile)
            .field("database_url", &"<redacted>")
            .field("bind", &self.bind)
            .field("public_origin", &self.public_origin)
            .field("site_root", &self.site_root)
            .field("dev_principal", &self.dev_principal)
            .field("identity", &self.identity)
            .field("sign_in_enabled", &self.sign_in_enabled)
            .field("trusted_proxies", &self.trusted_proxies)
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
        write!(f, "see .env.example and docs/development/README.md")
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let mut problems = Vec::new();
        let value = |key: &str| get(key).filter(|v| !v.trim().is_empty());

        let profile = match value("VINK_PROFILE").as_deref() {
            None | Some("production") => Profile::Production,
            Some("local") => Profile::Local,
            Some(other) => {
                problems.push(format!(
                    "VINK_PROFILE: expected `local` or `production`, got `{other}`"
                ));
                Profile::Production
            }
        };

        let database_url = database_url(
            "DATABASE_URL",
            value("DATABASE_URL"),
            "PostgreSQL URL of the runtime role",
            &mut problems,
        );

        let bind = match value("VINK_BIND")
            .unwrap_or_else(|| "127.0.0.1:3000".into())
            .parse()
        {
            Ok(addr) => addr,
            Err(_) => {
                problems.push("VINK_BIND: expected host:port such as 127.0.0.1:3000".into());
                SocketAddr::from(([127, 0, 0, 1], 3000))
            }
        };

        let public_origin = match value("VINK_PUBLIC_ORIGIN") {
            Some(origin) => match normalize_origin(&origin) {
                Some(origin) => origin,
                None => {
                    problems.push(
                        "VINK_PUBLIC_ORIGIN: expected scheme://host[:port] without a path".into(),
                    );
                    String::new()
                }
            },
            None if profile == Profile::Local => format!("http://{bind}"),
            None => {
                problems.push("VINK_PUBLIC_ORIGIN: required in the production profile".into());
                String::new()
            }
        };

        let site_root =
            PathBuf::from(value("VINK_SITE_ROOT").unwrap_or_else(|| "target/site".into()));

        let dev_principal = match value("VINK_DEV_PRINCIPAL") {
            None => None,
            Some(raw) => {
                if profile != Profile::Local {
                    problems.push(
                        "VINK_DEV_PRINCIPAL: identity fixture is only allowed with VINK_PROFILE=local"
                            .into(),
                    );
                }
                if !bind.ip().is_loopback() {
                    problems.push(
                        "VINK_DEV_PRINCIPAL: identity fixture requires a loopback VINK_BIND".into(),
                    );
                }
                match PrincipalId::parse(&raw) {
                    Some(id) => Some(id),
                    None => {
                        problems.push("VINK_DEV_PRINCIPAL: expected a UUID".into());
                        None
                    }
                }
            }
        };

        let identity = match (
            value("VINK_IDENTITY_SOCKET"),
            value("VINK_IDENTITY_SERVER_UID"),
        ) {
            (None, None) => None,
            (Some(_), None) => {
                problems.push(
                    "VINK_IDENTITY_SERVER_UID: required with VINK_IDENTITY_SOCKET (user id that must serve the socket)"
                        .into(),
                );
                None
            }
            (None, Some(_)) => {
                problems.push("VINK_IDENTITY_SERVER_UID: set without VINK_IDENTITY_SOCKET".into());
                None
            }
            (Some(socket), Some(uid)) => {
                if socket.len() > MAX_SOCKET_PATH {
                    problems.push(format!(
                        "VINK_IDENTITY_SOCKET: longer than {MAX_SOCKET_PATH} bytes, the Unix socket path limit"
                    ));
                }
                match uid.trim().parse::<u32>() {
                    Ok(server_uid) => Some(IdentitySettings {
                        socket: PathBuf::from(socket),
                        server_uid,
                    }),
                    Err(_) => {
                        problems
                            .push("VINK_IDENTITY_SERVER_UID: expected a numeric user id".into());
                        None
                    }
                }
            }
        };

        let sign_in_enabled = match value("VINK_AUTH_SIGN_IN").as_deref().map(str::trim) {
            None => profile == Profile::Local,
            Some("enabled") => true,
            Some("disabled") => false,
            Some(other) => {
                problems.push(format!(
                    "VINK_AUTH_SIGN_IN: expected `enabled` or `disabled`, got `{other}`"
                ));
                false
            }
        };
        if sign_in_enabled && identity.is_none() && value("VINK_AUTH_SIGN_IN").is_some() {
            problems.push(
                "VINK_AUTH_SIGN_IN: `enabled` needs VINK_IDENTITY_SOCKET (the Identity process)"
                    .into(),
            );
        }
        let trusted_proxies = match value("VINK_TRUSTED_PROXIES") {
            None => TrustedProxies::default(),
            Some(raw) => TrustedProxies::parse(&raw).unwrap_or_else(|entry| {
                problems.push(format!(
                    "VINK_TRUSTED_PROXIES: `{entry}` is not an IP address or CIDR range"
                ));
                TrustedProxies::default()
            }),
        };

        let log_format = log_format(value("VINK_LOG_FORMAT"), &mut problems);

        if problems.is_empty() {
            Ok(Self {
                profile,
                database_url,
                bind,
                public_origin,
                site_root,
                dev_principal,
                identity,
                sign_in_enabled,
                trusted_proxies,
                log_format,
            })
        } else {
            Err(ConfigError { problems })
        }
    }
}

/// Configuration of `vink-app migrate`. It reads only `VINK_MIGRATE_DATABASE_URL`, never
/// `DATABASE_URL`: the runtime role gets no DDL rights by accident, and a missing migration
/// credential stops the command instead of falling back to another role.
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

impl MigrateConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let mut problems = Vec::new();
        let value = |key: &str| get(key).filter(|v| !v.trim().is_empty());
        let database_url = database_url(
            "VINK_MIGRATE_DATABASE_URL",
            value("VINK_MIGRATE_DATABASE_URL"),
            "PostgreSQL URL of the migration role, separate from DATABASE_URL",
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

fn normalize_origin(raw: &str) -> Option<String> {
    let origin = raw.trim_end_matches('/');
    let rest = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))?;
    (!rest.is_empty() && !rest.contains(['/', '?', '#', ' '])).then(|| origin.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn load(pairs: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let env: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Config::from_lookup(|key| env.get(key).cloned())
    }

    const URL: &str = "postgres://vink:secret@127.0.0.1:5432/vink";
    const DEV: &str = "11111111-1111-4111-8111-111111111111";

    #[test]
    fn missing_settings_are_all_reported_without_secrets() {
        let error = load(&[]).unwrap_err();
        let text = error.to_string();
        assert!(text.contains("DATABASE_URL: missing"), "{text}");
        assert!(text.contains("VINK_PUBLIC_ORIGIN: required"), "{text}");

        let error = load(&[("DATABASE_URL", "mysql://vink:secret@db/vink")]).unwrap_err();
        assert!(!error.to_string().contains("secret"));
    }

    #[test]
    fn production_is_the_default_and_rejects_the_identity_fixture() {
        let error = load(&[
            ("DATABASE_URL", URL),
            ("VINK_PUBLIC_ORIGIN", "https://vink.example"),
            ("VINK_DEV_PRINCIPAL", DEV),
        ])
        .unwrap_err();
        assert_eq!(error.problems.len(), 1, "{error}");
        assert!(error.problems[0].contains("only allowed with VINK_PROFILE=local"));
    }

    #[test]
    fn identity_fixture_requires_loopback_bind() {
        let error = load(&[
            ("DATABASE_URL", URL),
            ("VINK_PROFILE", "local"),
            ("VINK_BIND", "0.0.0.0:3000"),
            ("VINK_DEV_PRINCIPAL", DEV),
        ])
        .unwrap_err();
        assert!(error.to_string().contains("requires a loopback VINK_BIND"));
    }

    #[test]
    fn local_profile_defaults() {
        let config = load(&[
            ("DATABASE_URL", URL),
            ("VINK_PROFILE", "local"),
            ("VINK_DEV_PRINCIPAL", DEV),
        ])
        .unwrap();
        assert_eq!(config.profile, Profile::Local);
        assert_eq!(config.public_origin, "http://127.0.0.1:3000");
        assert_eq!(config.dev_principal, PrincipalId::parse(DEV));
        assert!(!format!("{config:?}").contains("secret"));
    }

    #[test]
    fn identity_socket_needs_the_server_uid() {
        let base = [("DATABASE_URL", URL), ("VINK_PROFILE", "local")];
        let with = |extra: &[(&str, &str)]| {
            let pairs: Vec<(&str, &str)> = base.iter().chain(extra).copied().collect();
            load(&pairs)
        };
        assert_eq!(with(&[]).unwrap().identity, None);
        let error = with(&[("VINK_IDENTITY_SOCKET", "/run/vink/identity.sock")]).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("VINK_IDENTITY_SERVER_UID: required"),
            "{error}"
        );
        let error = with(&[("VINK_IDENTITY_SERVER_UID", "1000")]).unwrap_err();
        assert!(
            error.to_string().contains("without VINK_IDENTITY_SOCKET"),
            "{error}"
        );
        let config = with(&[
            ("VINK_IDENTITY_SOCKET", "/run/vink/identity.sock"),
            ("VINK_IDENTITY_SERVER_UID", "1000"),
        ])
        .unwrap();
        assert_eq!(
            config.identity,
            Some(IdentitySettings {
                socket: PathBuf::from("/run/vink/identity.sock"),
                server_uid: 1000,
            })
        );
    }

    /// Production activation of sign-in is explicit (D-05 Provisional); local development keeps
    /// it on. Trusted proxies are validated entry by entry.
    #[test]
    fn sign_in_activation_is_explicit_in_production_and_proxies_are_validated() {
        let identity = [
            ("VINK_IDENTITY_SOCKET", "/run/vink/identity.sock"),
            ("VINK_IDENTITY_SERVER_UID", "1000"),
        ];
        let with = |extra: &[(&str, &str)]| {
            let mut pairs = vec![
                ("DATABASE_URL", URL),
                ("VINK_PUBLIC_ORIGIN", "https://vink.example"),
            ];
            pairs.extend_from_slice(&identity);
            pairs.extend_from_slice(extra);
            load(&pairs)
        };
        let production = with(&[]).unwrap();
        assert!(
            !production.sign_in_enabled,
            "closed by default in production"
        );
        assert!(production.trusted_proxies.is_empty());
        assert!(
            with(&[("VINK_AUTH_SIGN_IN", "enabled")])
                .unwrap()
                .sign_in_enabled
        );
        assert!(with(&[("VINK_PROFILE", "local")]).unwrap().sign_in_enabled);
        assert!(
            !with(&[("VINK_PROFILE", "local"), ("VINK_AUTH_SIGN_IN", "disabled")])
                .unwrap()
                .sign_in_enabled
        );
        let error = with(&[("VINK_AUTH_SIGN_IN", "yes")]).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("expected `enabled` or `disabled`")
        );
        let error = load(&[
            ("DATABASE_URL", URL),
            ("VINK_PUBLIC_ORIGIN", "https://vink.example"),
            ("VINK_AUTH_SIGN_IN", "enabled"),
        ])
        .unwrap_err();
        assert!(error.to_string().contains("needs VINK_IDENTITY_SOCKET"));
        let proxies = with(&[("VINK_TRUSTED_PROXIES", "10.0.0.0/8, 2001:db8::/32")]).unwrap();
        assert!(proxies.trusted_proxies.trusts("10.9.9.9".parse().unwrap()));
        let error = with(&[("VINK_TRUSTED_PROXIES", "10.0.0.0/8, *")]).unwrap_err();
        assert!(
            error.to_string().contains("`*` is not an IP address"),
            "{error}"
        );
    }

    #[test]
    fn migrate_reads_only_its_own_credential() {
        let migrate = |pairs: &[(&str, &str)]| {
            let env: HashMap<String, String> = pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            MigrateConfig::from_lookup(|key| env.get(key).cloned())
        };
        let error = migrate(&[("DATABASE_URL", URL)]).unwrap_err();
        assert_eq!(error.problems.len(), 1, "{error}");
        assert!(error.problems[0].starts_with("VINK_MIGRATE_DATABASE_URL: missing"));

        let error =
            migrate(&[("VINK_MIGRATE_DATABASE_URL", "mysql://m:secret@db/vink")]).unwrap_err();
        assert!(!error.to_string().contains("secret"), "{error}");

        let config = migrate(&[("VINK_MIGRATE_DATABASE_URL", URL)]).unwrap();
        assert_eq!(config.database_url, URL);
        assert!(!format!("{config:?}").contains("secret"));
    }

    #[test]
    fn origin_must_not_have_a_path() {
        let error = load(&[
            ("DATABASE_URL", URL),
            ("VINK_PUBLIC_ORIGIN", "https://vink.example/app"),
        ])
        .unwrap_err();
        assert!(error.to_string().contains("VINK_PUBLIC_ORIGIN: expected"));
        assert_eq!(
            normalize_origin("https://vink.example/"),
            Some("https://vink.example".into())
        );
    }
}
