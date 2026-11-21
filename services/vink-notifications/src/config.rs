//! Process configuration read from the environment. [`ServeConfig`] uses the Notification runtime
//! credential; [`MigrateConfig`] uses only the Notification migration credential. Variables are
//! prefixed `VINK_NOTIFICATIONS_` so one environment can also hold the App's and Identity's
//! settings without a process reading another's credential.
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

/// One private socket and the only OS user allowed to call it (T-07 CHOICE-21 A).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocketScope {
    pub path: PathBuf,
    /// `None` keeps the socket closed: every call is refused and readiness stays not_ready.
    pub caller_uid: Option<u32>,
}

#[derive(Clone)]
pub struct ServeConfig {
    pub database_url: String,
    /// The App relay's socket (`POST /v1/ingest`), `VINK_NOTIFICATIONS_INGEST_SOCKET` +
    /// `VINK_NOTIFICATIONS_RELAY_UID`.
    pub ingest: SocketScope,
    /// The App Gateway's socket (`/v1/inboxes/…`), `VINK_NOTIFICATIONS_INBOX_SOCKET` +
    /// `VINK_NOTIFICATIONS_GATEWAY_UID`.
    pub inbox: SocketScope,
    pub log_format: LogFormat,
}

impl fmt::Debug for ServeConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServeConfig")
            .field("database_url", &"<redacted>")
            .field("ingest", &self.ingest)
            .field("inbox", &self.inbox)
            .field("log_format", &self.log_format)
            .finish()
    }
}

/// `vink-notifications maintenance …`: operator commands on the runtime credential.
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
        write!(f, "invalid configuration: {}", self.problems.join("; "))
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
            "VINK_NOTIFICATIONS_DATABASE_URL",
            value("VINK_NOTIFICATIONS_DATABASE_URL"),
            "PostgreSQL URL of the Notification runtime role",
            &mut problems,
        );
        let ingest = scope(
            &value,
            "VINK_NOTIFICATIONS_INGEST_SOCKET",
            "VINK_NOTIFICATIONS_RELAY_UID",
            &mut problems,
        );
        let inbox = scope(
            &value,
            "VINK_NOTIFICATIONS_INBOX_SOCKET",
            "VINK_NOTIFICATIONS_GATEWAY_UID",
            &mut problems,
        );
        if !ingest.path.as_os_str().is_empty() && ingest.path == inbox.path {
            problems.push(
                "VINK_NOTIFICATIONS_INBOX_SOCKET: must differ from VINK_NOTIFICATIONS_INGEST_SOCKET (one socket per caller scope)"
                    .into(),
            );
        }
        let log_format = log_format(value("VINK_LOG_FORMAT"), &mut problems);
        if problems.is_empty() {
            Ok(Self {
                database_url,
                ingest,
                inbox,
                log_format,
            })
        } else {
            Err(ConfigError { problems })
        }
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
            "VINK_NOTIFICATIONS_DATABASE_URL",
            value("VINK_NOTIFICATIONS_DATABASE_URL"),
            "PostgreSQL URL of the Notification runtime role",
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

impl MigrateConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// Reads only `VINK_NOTIFICATIONS_MIGRATE_DATABASE_URL`; the runtime credential is never a
    /// fallback.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let mut problems = Vec::new();
        let value = |key: &str| get(key).filter(|v| !v.trim().is_empty());
        let database_url = database_url(
            "VINK_NOTIFICATIONS_MIGRATE_DATABASE_URL",
            value("VINK_NOTIFICATIONS_MIGRATE_DATABASE_URL"),
            "PostgreSQL URL of the Notification migration role",
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

fn scope(
    value: &dyn Fn(&str) -> Option<String>,
    socket_key: &str,
    uid_key: &str,
    problems: &mut Vec<String>,
) -> SocketScope {
    let path = match value(socket_key) {
        None => {
            problems.push(format!(
                "{socket_key}: missing (path of a private Unix socket)"
            ));
            PathBuf::new()
        }
        Some(path) if path.len() > MAX_SOCKET_PATH => {
            problems.push(format!(
                "{socket_key}: longer than {MAX_SOCKET_PATH} bytes, the Unix socket path limit"
            ));
            PathBuf::new()
        }
        Some(path) => PathBuf::from(path),
    };
    let caller_uid = match value(uid_key) {
        None => None,
        Some(raw) => match raw.trim().parse::<u32>() {
            Ok(uid) => Some(uid),
            Err(_) => {
                problems.push(format!("{uid_key}: expected a numeric user id"));
                None
            }
        },
    };
    SocketScope { path, caller_uid }
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

    const URL: &str =
        "postgres://vink_notifications_runtime:secret@127.0.0.1:5432/vink_notifications";

    fn with(pairs: &[(&str, &str)]) -> Result<ServeConfig, ConfigError> {
        let env: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        ServeConfig::from_lookup(|key| env.get(key).cloned())
    }

    #[test]
    fn serve_needs_both_sockets_and_reads_each_caller() {
        let config = with(&[
            ("VINK_NOTIFICATIONS_DATABASE_URL", URL),
            (
                "VINK_NOTIFICATIONS_INGEST_SOCKET",
                "/run/vink-n/ingest.sock",
            ),
            ("VINK_NOTIFICATIONS_RELAY_UID", "1001"),
            ("VINK_NOTIFICATIONS_INBOX_SOCKET", "/run/vink-n/inbox.sock"),
            ("VINK_NOTIFICATIONS_GATEWAY_UID", " 1002 "),
        ])
        .unwrap();
        assert_eq!(config.ingest.caller_uid, Some(1001));
        assert_eq!(config.inbox.caller_uid, Some(1002));
        assert!(!format!("{config:?}").contains("secret"));

        let error = with(&[("VINK_NOTIFICATIONS_DATABASE_URL", "mysql://x")]).unwrap_err();
        let text = error.to_string();
        assert!(text.contains("must be a postgres://"), "{text}");
        assert!(!text.contains("mysql://x"), "{text}");
        assert!(text.contains("VINK_NOTIFICATIONS_INGEST_SOCKET: missing"));
        assert!(text.contains("VINK_NOTIFICATIONS_INBOX_SOCKET: missing"));
    }

    #[test]
    fn one_socket_cannot_serve_both_scopes_and_uids_are_numeric() {
        let error = with(&[
            ("VINK_NOTIFICATIONS_DATABASE_URL", URL),
            ("VINK_NOTIFICATIONS_INGEST_SOCKET", "/run/vink-n/n.sock"),
            ("VINK_NOTIFICATIONS_INBOX_SOCKET", "/run/vink-n/n.sock"),
            ("VINK_NOTIFICATIONS_RELAY_UID", "app"),
        ])
        .unwrap_err()
        .to_string();
        assert!(error.contains("must differ"), "{error}");
        assert!(error.contains("RELAY_UID: expected a numeric"), "{error}");
        let long = format!("/{}", "s".repeat(MAX_SOCKET_PATH));
        let error = with(&[
            ("VINK_NOTIFICATIONS_DATABASE_URL", URL),
            ("VINK_NOTIFICATIONS_INGEST_SOCKET", &long),
            ("VINK_NOTIFICATIONS_INBOX_SOCKET", "/run/vink-n/inbox.sock"),
        ])
        .unwrap_err()
        .to_string();
        assert!(error.contains("Unix socket path limit"), "{error}");
    }

    #[test]
    fn migrate_never_falls_back_to_the_runtime_credential() {
        let env: HashMap<String, String> = [("VINK_NOTIFICATIONS_DATABASE_URL", URL)]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let error = MigrateConfig::from_lookup(|key| env.get(key).cloned()).unwrap_err();
        assert!(error.to_string().contains("MIGRATE_DATABASE_URL: missing"));
    }
}
