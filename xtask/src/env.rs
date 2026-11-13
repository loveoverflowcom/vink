//! Process environment plus defaults from the repository's `.env` (if present).
//! Real environment variables always win over `.env`.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

pub struct Env {
    from_file: BTreeMap<String, String>,
}

impl Env {
    pub fn load(root: &Path) -> Self {
        let mut from_file = BTreeMap::new();
        if let Ok(text) = std::fs::read_to_string(root.join(".env")) {
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((key, value)) = line.split_once('=') {
                    let value = value.trim().trim_matches('"').trim_matches('\'');
                    from_file.insert(key.trim().to_string(), value.to_string());
                }
            }
        }
        Self { from_file }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.resolve(key, std::env::var(key).ok())
    }

    fn resolve(&self, key: &str, process: Option<String>) -> Option<String> {
        process
            .or_else(|| self.from_file.get(key).cloned())
            .filter(|v| !v.is_empty())
    }

    pub fn has_file(&self) -> bool {
        !self.from_file.is_empty()
    }

    /// Adds `.env` values that the real environment does not define.
    pub fn apply(&self, command: &mut Command) {
        for (key, value) in &self.from_file {
            if std::env::var_os(key).is_none() {
                command.env(key, value);
            }
        }
    }
}

/// Remove capabilities inherited from the shell as well as `.env`. A child receives only the
/// owner-specific settings explicitly selected afterwards. Ordinary OS/tool variables survive.
pub fn clear_capabilities(command: &mut Command) {
    for (key, _) in std::env::vars_os() {
        if key
            .to_str()
            .is_some_and(|s| s.starts_with("VINK_") || s == "DATABASE_URL")
        {
            command.env_remove(key);
        }
    }
    for key in [
        "DATABASE_URL",
        "VINK_DEV_PRINCIPAL",
        "VINK_IDENTITY_SOCKET",
        "VINK_AUTH_SIGN_IN",
        "VINK_MIGRATE_DATABASE_URL",
        "VINK_IDENTITY_MIGRATE_DATABASE_URL",
        crate::TEST_DATABASE_URL,
    ] {
        command.env_remove(key);
    }
}
impl Env {
    pub fn copy(&self, command: &mut Command, keys: &[&str]) {
        for key in keys {
            if let Some(value) = self.get(key) {
                command.env(key, value);
            }
        }
    }
    #[cfg(test)]
    pub fn fixture(values: &[(&str, &str)]) -> Self {
        Self {
            from_file: values
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }
}

#[cfg(test)]
mod precedence_tests {
    use super::*;
    #[test]
    fn process_wins_and_empty_process_disables_file_default() {
        let env = Env::fixture(&[("X", "file")]);
        assert_eq!(env.resolve("X", None), Some("file".into()));
        assert_eq!(
            env.resolve("X", Some("process".into())),
            Some("process".into())
        );
        assert_eq!(env.resolve("X", Some(String::new())), None);
    }
}
