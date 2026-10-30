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
        std::env::var(key)
            .ok()
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
