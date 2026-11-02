//! Process configuration read from the environment. [`Config`] serves `vink-app serve|dev-seed`
//! and `vink-worker` with the runtime credential; [`MigrateConfig`] serves `vink-app migrate`
//! with the separate migration credential.
//!
//! Validation collects every problem at once and never echoes a database URL.

use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;

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
    pub log_format: LogFormat,
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("profile", &self.profile)
            .field("database_url", &"<redacted>")
            .field("bind", &self.bind)
            .field("public_origin", &self.public_origin)
            .field("site_root", &self.site_root)
            .field("dev_principal", &self.dev_principal)
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

        let log_format = log_format(value("VINK_LOG_FORMAT"), &mut problems);

        if problems.is_empty() {
            Ok(Self {
                profile,
                database_url,
                bind,
                public_origin,
                site_root,
                dev_principal,
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
