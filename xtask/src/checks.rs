//! Cheap repository checks: skill metadata, relative Markdown links and layer boundaries.

use std::path::{Path, PathBuf};

pub fn run(root: &Path) -> Result<(), String> {
    let mut problems = Vec::new();
    skills(root, &mut problems);
    links(root, &mut problems);
    boundaries(root, &mut problems);
    if problems.is_empty() {
        println!("==> repository checks: ok");
        Ok(())
    } else {
        for problem in &problems {
            eprintln!("  - {problem}");
        }
        Err(format!("{} repository problem(s)", problems.len()))
    }
}

fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn skills(root: &Path, problems: &mut Vec<String>) {
    let dir = root.join(".agents/skills");
    let index = read(&dir.join("README.md")).unwrap_or_default();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        problems.push(".agents/skills is missing".into());
        return;
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    for name in names {
        let skill = dir.join(&name).join("SKILL.md");
        let Some(text) = read(&skill) else {
            problems.push(format!("{name}: SKILL.md missing"));
            continue;
        };
        problems.extend(skill_frontmatter(&name, &text));
        let yaml = read(&dir.join(&name).join("agents/openai.yaml")).unwrap_or_default();
        for key in ["display_name:", "short_description:", "default_prompt:"] {
            if !yaml.lines().any(|l| l.trim_start().starts_with(key)) {
                problems.push(format!("{name}: agents/openai.yaml lacks `{key}`"));
            }
        }
        if !yaml.contains(&format!("${name}")) {
            problems.push(format!("{name}: default_prompt must invoke `${name}`"));
        }
        if !index.contains(&format!("]({name}/SKILL.md)")) {
            problems.push(format!("{name}: not listed in .agents/skills/README.md"));
        }
    }
}

/// Frontmatter needs `name` equal to the directory and a non-empty `description`.
fn skill_frontmatter(name: &str, text: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let mut lines = text.lines();
    if lines.next() != Some("---") {
        return vec![format!("{name}: SKILL.md must start with YAML frontmatter")];
    }
    let front: Vec<&str> = lines.take_while(|l| *l != "---").collect();
    if !front.iter().any(|l| *l == format!("name: {name}")) {
        problems.push(format!("{name}: frontmatter `name` must be `{name}`"));
    }
    let description = front.iter().position(|l| l.starts_with("description:"));
    let has_text = description.is_some_and(|i| {
        let inline = front[i].trim_start_matches("description:").trim();
        let folded = front[i + 1..]
            .iter()
            .take_while(|l| l.starts_with(' '))
            .any(|l| !l.trim().is_empty());
        (!inline.is_empty() && inline != ">-" && inline != ">" && inline != "|") || folded
    });
    if !has_text {
        problems.push(format!("{name}: frontmatter `description` is empty"));
    }
    problems
}

fn markdown_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if !matches!(name.as_ref(), ".git" | "target" | "node_modules") {
                markdown_files(&path, out);
            }
        } else if name.ends_with(".md") {
            out.push(path);
        }
    }
}

fn links(root: &Path, problems: &mut Vec<String>) {
    let mut files = Vec::new();
    markdown_files(root, &mut files);
    files.sort();
    for file in files {
        let Some(text) = read(&file) else { continue };
        let mut fenced = false;
        for (number, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("```") {
                fenced = !fenced;
                continue;
            }
            if fenced {
                continue;
            }
            for target in link_targets(line) {
                if let Some(missing) = broken(&file, &target) {
                    problems.push(format!(
                        "{}:{}: broken link `{missing}`",
                        rel(root, &file),
                        number + 1
                    ));
                }
            }
        }
    }
}

fn link_targets(line: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find("](") {
        let after = &rest[start + 2..];
        let Some(end) = after.find(')') else { break };
        targets.push(after[..end].trim().trim_matches(['<', '>']).to_string());
        rest = &after[end..];
    }
    targets
}

fn broken(file: &Path, target: &str) -> Option<String> {
    if target.is_empty()
        || target.starts_with('#')
        || target.contains("://")
        || target.starts_with("mailto:")
    {
        return None;
    }
    let path = target.split('#').next().unwrap_or(target);
    // `file.rs:42` line references point at the file.
    let path = match path.rsplit_once(':') {
        Some((head, line)) if line.chars().all(|c| c.is_ascii_digit()) => head,
        _ => path,
    };
    let resolved = file.parent()?.join(path);
    (!resolved.exists()).then(|| target.to_string())
}

/// Layer rules from AGENTS.md, checked by imports.
fn boundaries(root: &Path, problems: &mut Vec<String>) {
    let rules: [(&str, &[&str]); 2] = [
        (
            "services/vink-app/src/domain",
            &[
                "axum",
                "sqlx",
                "leptos",
                "tower",
                "tokio",
                "vink_web",
                "crate::http",
                "crate::persistence",
                "crate::application",
            ],
        ),
        (
            "services/vink-app/src/application",
            &[
                "axum",
                "sqlx",
                "leptos",
                "tower",
                "vink_web",
                "crate::http",
                "crate::persistence",
            ],
        ),
    ];
    for (dir, forbidden) in rules {
        let mut files = Vec::new();
        rust_files(&root.join(dir), &mut files);
        for file in files {
            let Some(text) = read(&file) else { continue };
            for (number, line) in text.lines().enumerate() {
                let code = line.split("//").next().unwrap_or("");
                for name in forbidden {
                    let uses = code.contains(&format!("{name}::"))
                        || code.contains(&format!("use {name}"));
                    if uses {
                        problems.push(format!(
                            "{}:{}: `{name}` is not allowed in {dir}",
                            rel(root, &file),
                            number + 1
                        ));
                    }
                }
            }
        }
    }
    let manifest = read(&root.join("apps/vink-web/Cargo.toml")).unwrap_or_default();
    for forbidden in ["vink-app", "sqlx", "tokio-postgres"] {
        if manifest
            .lines()
            .any(|l| l.trim_start().starts_with(forbidden))
        {
            problems.push(format!(
                "apps/vink-web/Cargo.toml: the browser-facing crate must not depend on `{forbidden}`"
            ));
        }
    }
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_inline_link_targets() {
        assert_eq!(
            link_targets("see [a](x.md) and [b](<y z.md#top>), not `code`"),
            vec!["x.md".to_string(), "y z.md#top".to_string()]
        );
    }

    #[test]
    fn frontmatter_requires_matching_name_and_description() {
        let ok = "---\nname: vink-x\ndescription: >-\n  Does x.\n---\n# X\n";
        assert!(skill_frontmatter("vink-x", ok).is_empty());
        let bad = "---\nname: other\ndescription: >-\n---\n";
        assert_eq!(skill_frontmatter("vink-x", bad).len(), 2);
    }

    #[test]
    fn external_anchor_and_line_links_are_handled() {
        let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        assert_eq!(broken(&file, "https://example.com/x"), None);
        assert_eq!(broken(&file, "#section"), None);
        assert_eq!(broken(&file, "src/main.rs:12"), None);
        assert_eq!(broken(&file, "missing.md"), Some("missing.md".into()));
    }
}
