//! Process configuration read from the environment. [`ServeConfig`] uses the Identity runtime
//! credential; [`MigrateConfig`] uses only the Identity migration credential. Variables are
//! prefixed `VINK_IDENTITY_` so one environment can also hold the App's settings without a
//! process reading the other's credential.
//!
//! Validation collects every problem at once and never echoes a database URL.

use std::fmt;
use std::path::PathBuf;

/// `sun_path` is 104 bytes on macOS and 108 on Linux, including the terminator.
pub const MAX_SOCKET_PATH: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Text,
    Json,
}

#[derive(Clone)]
pub struct ServeConfig {
    pub database_url: String,
    /// Unix socket the private contract is served on; there is no TCP listener.
    pub socket: PathBuf,
    /// The only OS user whose connections may call the contract. `None` keeps the contract
    /// closed: every call is refused and readiness reports `callers: not_configured`.
    pub caller_uid: Option<u32>,
    pub log_format: LogFormat,
}

impl fmt::Debug for ServeConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServeConfig")
            .field("database_url", &"<redacted>")
            .field("socket", &self.socket)
            .field("caller_uid", &self.caller_uid)
            .field("log_format", &self.log_format)
            .finish()
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
        let log_format = log_format(value("VINK_LOG_FORMAT"), &mut problems);
        if problems.is_empty() {
            Ok(Self {
                database_url,
                socket,
                caller_uid,
                log_format,
            })
        } else {
            Err(ConfigError { problems })
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
