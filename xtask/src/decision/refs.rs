use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde_json::{Value, json};

use super::record::{self, Key, Origin, Registry};

const ROOTS: &[&str] = &["prd", "docs", ".agents/skills"];
const PREFIX: &str = "<!-- vdr:";

pub(super) struct Report {
    declared: Vec<Value>,
    problems: Vec<String>,
}

impl Report {
    pub fn len(&self) -> usize {
        self.declared.len()
    }

    pub fn output(&self, registry: &Registry) -> Value {
        json!({
            "snapshot_sha256": registry.snapshot,
            "scanned_roots": ROOTS,
            "coverage": "declared_markdown_only",
            "semantic_review": "NOT_CHECKED",
            "references": self.declared,
            "problems": self.problems,
        })
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.problems.is_empty() {
            Ok(())
        } else {
            Err(self.problems.join("\n"))
        }
    }
}

pub(super) fn report(root: &Path, registry: &Registry) -> Result<Report, String> {
    let mut files = Vec::new();
    for dir in ROOTS {
        let path = root.join(dir);
        if record::is_directory(&path)? {
            markdown_files(&path, &mut files)?;
        }
    }
    files.sort();
    let mut report = Report {
        declared: Vec::new(),
        problems: Vec::new(),
    };
    for file in files {
        let display = file
            .strip_prefix(root)
            .expect("reference inside repository")
            .display();
        let mut seen = BTreeSet::new();
        for (index, line) in record::read_file(&file)?.lines().enumerate() {
            if !line.contains(PREFIX) {
                continue;
            }
            let number = index + 1;
            match check_marker(line, registry, &mut seen) {
                Ok((mode, key)) => report.declared.push(json!({
                    "path": display.to_string(), "line": number, "mode": mode, "pin": key.to_string(),
                })),
                Err(error) => report.problems.push(format!("{display}:{number}: {error}")),
            }
        }
    }
    Ok(report)
}

fn check_marker<'a>(
    line: &'a str,
    registry: &Registry,
    seen: &mut BTreeSet<(String, Key)>,
) -> Result<(&'a str, Key), String> {
    let body = line
        .trim()
        .strip_prefix(PREFIX)
        .and_then(|body| body.strip_suffix("-->"))
        .ok_or("expected a standalone VDR marker")?;
    let parts: Vec<_> = body.split_whitespace().collect();
    let [mode @ ("active" | "historical"), pin] = parts.as_slice() else {
        return Err("expected a VDR marker with active|historical and ID@rN".into());
    };
    let key = Key::parse(pin)?;
    if !seen.insert(((*mode).into(), key.clone())) {
        return Err(format!("duplicate {mode} reference to {key}"));
    }
    let entry = registry
        .entries
        .get(&key)
        .ok_or_else(|| format!("unknown VDR reference {key}"))?;
    if !matches!(entry.origin, Origin::Legacy(_)) {
        return Err(format!(
            "{key}: pending proposal cannot back a Markdown decision reference"
        ));
    }
    if *mode == "active"
        && registry
            .current(&key.id)
            .is_none_or(|(current, _)| *current != key)
    {
        let current = registry
            .current(&key.id)
            .map_or_else(|| "none".into(), |(key, _)| key.to_string());
        return Err(format!(
            "stale active reference {key}; current is {current}"
        ));
    }
    Ok((mode, key))
}

fn markdown_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| format!("{}: {e}", dir.display()))?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if kind.is_dir() {
            markdown_files(&path, files)?;
        } else if kind.is_file() && path.extension().is_some_and(|ext| ext == "md") {
            files.push(path);
        }
    }
    Ok(())
}
