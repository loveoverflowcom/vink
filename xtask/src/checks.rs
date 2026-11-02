//! Cheap repository checks: skill metadata, relative Markdown links, layer boundaries, the
//! served font assets against their provenance record and the editor runtime's pinned packages.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Self-hosted font faces, their OFL licenses and `provenance.json`.
pub const FONTS_DIR: &str = "apps/vink-web/assets/fonts";
pub const BRAND_DIR: &str = "apps/vink-web/assets/brand";

pub fn run(root: &Path) -> Result<(), String> {
    let mut problems = Vec::new();
    skills(root, &mut problems);
    links(root, &mut problems);
    boundaries(root, &mut problems);
    fonts(root, &mut problems);
    brand(root, &mut problems);
    editor_packages(root, &mut problems);
    secrets(root, &mut problems);
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

/// Preserve the approved source derivatives and catch missing/corrupt release assets.
fn brand(root: &Path, problems: &mut Vec<String>) {
    let dir = root.join(BRAND_DIR);
    let record = read(&dir.join("provenance.json"))
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
    let Some(files) = record.as_ref().and_then(|value| value["files"].as_object()) else {
        problems.push(format!(
            "{BRAND_DIR}/provenance.json: missing or invalid files"
        ));
        return;
    };
    for (name, entry) in files {
        match std::fs::read(dir.join(name)) {
            Ok(bytes) if entry["sha256"].as_str() == Some(sha256_hex(&bytes).as_str()) => {}
            _ => problems.push(format!("{BRAND_DIR}/{name}: missing or SHA-256 differs")),
        }
    }
    for theme in ["light", "dark"] {
        for size in [24, 32, 48, 96, 192, 512] {
            let name = format!("enso-{theme}-{size}.avif");
            if !files.contains_key(&name) {
                problems.push(format!(
                    "{BRAND_DIR}/{name}: not recorded in provenance.json"
                ));
            }
        }
    }
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

/// Tracked files carry no private key and no PostgreSQL URL with a real-looking password. Allowed
/// passwords are the documented throwaway values (`*-only`), short test placeholders and
/// templates. A lint, not a scanner: it cannot recognise every secret.
fn secrets(root: &Path, problems: &mut Vec<String>) {
    let listed = std::process::Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(root)
        .output();
    let Some(listed) = listed.ok().filter(|out| out.status.success()) else {
        problems.push("secrets: cannot list tracked files with git".into());
        return;
    };
    for name in String::from_utf8_lossy(&listed.stdout).split('\0') {
        if name.is_empty() {
            continue;
        }
        let file = Path::new(name);
        let base = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if base == ".env" || base.ends_with(".env") {
            problems.push(format!(
                "{name}: an environment file is tracked; commit a .example instead"
            ));
        }
        let Some(text) = read(&root.join(file)) else {
            continue;
        };
        // Built at run time so this file does not match itself.
        if text.contains(&["PRIVATE", " KEY-----"].concat()) {
            problems.push(format!("{name}: contains a private key"));
        }
        for (index, _) in text.match_indices("postgres") {
            let rest = &text[index..];
            let Some(after) = rest
                .strip_prefix("postgresql://")
                .or_else(|| rest.strip_prefix("postgres://"))
            else {
                continue;
            };
            let token: String = after
                .chars()
                .take_while(|c| !c.is_whitespace() && !matches!(c, '"' | '`' | ')' | '\''))
                .collect();
            let Some((userinfo, _)) = token.split_once('@') else {
                continue;
            };
            let Some((_, password)) = userinfo.split_once(':') else {
                continue;
            };
            let placeholder = password.len() <= 6
                || password.ends_with("-only")
                || password.contains(['{', '$', '<', '*', '…']);
            if !placeholder {
                problems.push(format!(
                    "{name}: a PostgreSQL URL carries a password that is not a documented placeholder"
                ));
            }
        }
    }
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
    // Shared infrastructure packages (`packages/<name>/Cargo.toml`) sit below every service: no
    // service, web or HTTP framework dependency. The UI system renderer has its own rule below.
    if let Ok(entries) = std::fs::read_dir(root.join("packages")) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path().join("Cargo.toml");
            let manifest = read(&path).unwrap_or_default();
            for line in manifest.lines().map(str::trim_start) {
                let forbidden = ["vink-app", "vink-web", "vink-identity", "axum", "leptos"]
                    .into_iter()
                    .find(|name| line.starts_with(name))
                    .or_else(|| line.contains("../../services/").then_some("a service path"));
                if let Some(name) = forbidden {
                    problems.push(format!(
                        "{}: a shared package must not depend on `{name}`",
                        rel(root, &path)
                    ));
                }
            }
        }
    }
    ui_system(root, problems);
    // Services never import each other; each keeps its own representation of the other's contract.
    // Any mention in the manifest counts: `[dependencies.x]`, renamed packages and paths too.
    for (manifest, forbidden) in [
        ("services/vink-app/Cargo.toml", "vink-identity"),
        ("services/vink-identity/Cargo.toml", "vink-app"),
        ("services/vink-identity/Cargo.toml", "vink-web"),
    ] {
        let text = read(&root.join(manifest)).unwrap_or_default();
        if text.lines().any(|l| l.contains(forbidden)) {
            problems.push(format!("{manifest}: must not depend on `{forbidden}`"));
        }
    }
    // Identity's code and migrations never touch App tables. A lint only: database roles that
    // cannot connect across owners are the actual guard (`verify postgres`).
    let mut identity = Vec::new();
    rust_files(&root.join("services/vink-identity/src"), &mut identity);
    if let Ok(entries) = std::fs::read_dir(root.join("services/vink-identity/migrations")) {
        identity.extend(entries.filter_map(Result::ok).map(|e| e.path()));
    }
    for file in identity {
        // Quotes and escapes dropped, whitespace collapsed: `FROM\n  public."works"` still counts.
        let lowered = read(&file)
            .unwrap_or_default()
            .to_lowercase()
            .replace(['"', '\\'], "");
        let text = lowered
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .replace("public.", "");
        for table in [
            "working_drafts",
            "works",
            "principals",
            "revisions",
            "publications",
        ] {
            for verb in ["from", "join", "into", "update", "table"] {
                if text.contains(&format!("{verb} {table}")) {
                    problems.push(format!(
                        "{}: Identity must not use the App table `{table}`",
                        rel(root, &file)
                    ));
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

/// The UI system renders facts and dispatches intents: its Rust crates name no service, web app,
/// transport, router or database, read no browser storage and fetch nothing; its Kotlin common
/// source uses no JVM, Android or Apple platform API, so every target compiles the same code.
fn ui_system(root: &Path, problems: &mut Vec<String>) {
    const FORBIDDEN_CRATES: [&str; 10] = [
        "vink-app",
        "vink-web",
        "vink-identity",
        "vink-store-postgres",
        "axum",
        "sqlx",
        "tokio",
        "leptos_router",
        "leptos_axum",
        "gloo-net",
    ];
    for manifest in [
        "packages/ui-system/leptos/Cargo.toml",
        "packages/ui-system/leptos/preview/Cargo.toml",
    ] {
        let text = read(&root.join(manifest)).unwrap_or_default();
        for line in text.lines().map(str::trim_start) {
            if let Some(name) = FORBIDDEN_CRATES.iter().find(|name| line.starts_with(*name)) {
                problems.push(format!(
                    "{manifest}: the UI system must not depend on `{name}`"
                ));
            }
        }
    }
    let mut sources = Vec::new();
    rust_files(&root.join("packages/ui-system/leptos/src"), &mut sources);
    rust_files(
        &root.join("packages/ui-system/leptos/preview/src"),
        &mut sources,
    );
    for file in sources {
        let Some(text) = read(&file) else { continue };
        for (number, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            for name in [
                "local_storage",
                "session_storage",
                "localStorage",
                "fetch(",
                "Request::",
            ] {
                if code.contains(name) {
                    problems.push(format!(
                        "{}:{}: `{name}` is not allowed in the UI system (callers own storage and I/O)",
                        rel(root, &file),
                        number + 1
                    ));
                }
            }
        }
    }
    let mut common = Vec::new();
    kotlin_files(
        &root.join("packages/ui-system/cmp/ui/src/commonMain"),
        &mut common,
    );
    for file in common {
        let Some(text) = read(&file) else { continue };
        for (number, line) in text.lines().enumerate() {
            let line = line.trim_start();
            for prefix in [
                "import java.",
                "import javax.",
                "import android.",
                "import platform.",
                "import kotlinx.coroutines.swing",
            ] {
                if line.starts_with(prefix) {
                    problems.push(format!(
                        "{}:{}: `{}` is platform-specific; commonMain must compile for every target",
                        rel(root, &file),
                        number + 1,
                        prefix.trim_start_matches("import ")
                    ));
                }
            }
        }
    }
}

fn kotlin_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            kotlin_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "kt") {
            out.push(path);
        }
    }
}

/// Every served face and license matches `provenance.json` byte for byte, every face on disk is
/// recorded, and every font URL the stylesheet or the shell requests names a recorded face.
fn fonts(root: &Path, problems: &mut Vec<String>) {
    let dir = root.join(FONTS_DIR);
    let record: serde_json::Value =
        match read(&dir.join("provenance.json")).map(|t| serde_json::from_str(&t)) {
            Some(Ok(record)) => record,
            Some(Err(error)) => {
                problems.push(format!("{FONTS_DIR}/provenance.json: {error}"));
                return;
            }
            None => {
                problems.push(format!("{FONTS_DIR}/provenance.json is missing"));
                return;
            }
        };
    let files = record["files"].as_object();
    let recorded = |name: &str| files.is_some_and(|files| files.contains_key(name));
    let faces = files
        .into_iter()
        .flatten()
        .map(|(name, entry)| (name.as_str(), &entry["sha256"]));
    let licenses = record["families"]
        .as_object()
        .into_iter()
        .flat_map(|families| families.values())
        .map(|family| {
            (
                family["license_file"].as_str().unwrap_or_default(),
                &family["license_sha256"],
            )
        });
    for (name, sha256) in faces.chain(licenses) {
        match std::fs::read(dir.join(name)) {
            Ok(bytes) if sha256.as_str() == Some(sha256_hex(&bytes).as_str()) => {}
            Ok(_) => problems.push(format!(
                "{FONTS_DIR}/{name}: SHA-256 differs from provenance.json"
            )),
            Err(_) => problems.push(format!(
                "{FONTS_DIR}/{name}: listed in provenance.json but missing"
            )),
        }
    }
    for entry in std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".woff2") && !recorded(&name) {
            problems.push(format!(
                "{FONTS_DIR}/{name}: not recorded in provenance.json"
            ));
        }
    }
    let consumers = [
        ("apps/vink-web/style/vink.css", "url(\"fonts/"),
        ("packages/ui-system/leptos/style/tokens.css", "url(\"fonts/"),
        ("apps/vink-web/src/lib.rs", "\"/pkg/fonts/"),
    ];
    for (file, prefix) in consumers {
        let text = read(&root.join(file)).unwrap_or_default();
        for (number, line) in text.lines().enumerate() {
            let Some(start) = line.find(prefix) else {
                continue;
            };
            let name = line[start + prefix.len()..]
                .split(['"', ')'])
                .next()
                .unwrap_or_default();
            if !recorded(name) {
                problems.push(format!(
                    "{file}:{}: font `{name}` is not a recorded asset",
                    number + 1
                ));
            }
        }
    }
}

/// The editor runtime pins exact package versions, and its lockfile was produced from the same
/// manifest, so `npm ci` installs what was reviewed.
fn editor_packages(root: &Path, problems: &mut Vec<String>) {
    const DIR: &str = "apps/vink-web/editor";
    let mut parse = |name: &str| -> Option<serde_json::Value> {
        match read(&root.join(DIR).join(name)).map(|t| serde_json::from_str(&t)) {
            Some(Ok(value)) => Some(value),
            Some(Err(error)) => {
                problems.push(format!("{DIR}/{name}: {error}"));
                None
            }
            None => {
                problems.push(format!("{DIR}/{name} is missing"));
                None
            }
        }
    };
    let (Some(manifest), Some(lock)) = (parse("package.json"), parse("package-lock.json")) else {
        return;
    };
    for section in ["dependencies", "devDependencies"] {
        let declared = manifest[section].as_object().cloned().unwrap_or_default();
        for (name, version) in &declared {
            let version = version.as_str().unwrap_or_default();
            let exact = version.split('.').count() == 3
                && version.split('.').all(|part| part.parse::<u32>().is_ok());
            if !exact {
                problems.push(format!(
                    "{DIR}/package.json: {name} must pin an exact version, not `{version}`"
                ));
            }
        }
        if lock["packages"][""][section] != manifest[section] {
            problems.push(format!(
                "{DIR}/package-lock.json: {section} differ from package.json; run `npm install` there"
            ));
        }
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
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

    #[test]
    fn font_assets_must_match_their_provenance() {
        let root = std::env::temp_dir().join(format!("vink-fonts-check-{}", std::process::id()));
        let dir = root.join(FONTS_DIR);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::create_dir_all(root.join("apps/vink-web/src")).unwrap();
        std::fs::create_dir_all(root.join("apps/vink-web/style")).unwrap();
        std::fs::write(dir.join("a.woff2"), b"face").unwrap();
        std::fs::write(dir.join("stray.woff2"), b"x").unwrap();
        std::fs::write(dir.join("OFL.txt"), b"license").unwrap();
        let record = format!(
            r#"{{"files": {{"a.woff2": {{"sha256": "{}"}}}},
                "families": {{"A": {{"license_file": "OFL.txt", "license_sha256": "0"}}}}}}"#,
            sha256_hex(b"face")
        );
        std::fs::write(dir.join("provenance.json"), record).unwrap();
        std::fs::write(
            root.join("apps/vink-web/style/vink.css"),
            "src: url(\"fonts/a.woff2\");\nsrc: url(\"fonts/b.woff2\");\n",
        )
        .unwrap();
        std::fs::write(
            root.join("apps/vink-web/src/lib.rs"),
            "href=\"/pkg/fonts/a.woff2\"\n",
        )
        .unwrap();
        let mut problems = Vec::new();
        fonts(&root, &mut problems);
        std::fs::remove_dir_all(&root).unwrap();
        problems.sort();
        assert_eq!(problems.len(), 3, "{problems:?}");
        assert!(problems[0].ends_with("OFL.txt: SHA-256 differs from provenance.json"));
        assert!(problems[1].ends_with("stray.woff2: not recorded in provenance.json"));
        assert!(problems[2].contains("vink.css:2: font `b.woff2`"));
    }

    #[test]
    fn ui_system_rejects_services_storage_and_platform_imports() {
        let root = std::env::temp_dir().join(format!("vink-ui-check-{}", std::process::id()));
        let src = root.join("packages/ui-system/leptos/src");
        let common = root.join("packages/ui-system/cmp/ui/src/commonMain/kotlin/vink/ui");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&common).unwrap();
        std::fs::write(
            root.join("packages/ui-system/leptos/Cargo.toml"),
            "[dependencies]\nleptos.workspace = true\nleptos_router = \"0.8\"\n",
        )
        .unwrap();
        std::fs::write(
            src.join("lib.rs"),
            "let s = window().local_storage(); // local_storage ok here\n",
        )
        .unwrap();
        std::fs::write(
            common.join("A.kt"),
            "import androidx.compose.ui.unit.dp\nimport java.io.File\n",
        )
        .unwrap();
        let mut problems = Vec::new();
        ui_system(&root, &mut problems);
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(problems.len(), 3, "{problems:?}");
        assert!(problems[0].contains("must not depend on `leptos_router`"));
        assert!(problems[1].contains("lib.rs:1: `local_storage`"));
        assert!(problems[2].contains("A.kt:2: `java.`"));
    }

    #[test]
    fn editor_packages_are_pinned_and_locked() {
        let root = std::env::temp_dir().join(format!("vink-editor-check-{}", std::process::id()));
        let dir = root.join("apps/vink-web/editor");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            r#"{"dependencies": {"a": "1.2.3", "b": "^2.0.0"}, "devDependencies": {"c": "3.0.0"}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("package-lock.json"),
            r#"{"packages": {"": {"dependencies": {"a": "1.2.3", "b": "^2.0.0"},
                "devDependencies": {"c": "3.0.1"}}}}"#,
        )
        .unwrap();
        let mut problems = Vec::new();
        editor_packages(&root, &mut problems);
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems[0].contains("b must pin an exact version, not `^2.0.0`"));
        assert!(problems[1].contains("devDependencies differ from package.json"));
    }
}
