//! Cheap repository checks: skill metadata, relative Markdown links, layer boundaries, the
//! served font assets against their provenance record, the editor runtime's pinned packages and
//! the safety lints every crate inherits.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Self-hosted font faces, their OFL licenses and `provenance.json`.
pub const FONTS_DIR: &str = "apps/vink-web/assets/fonts";
const DESKTOP_FONTS_DIR: &str = "packages/ui-system/cmp/preview/fonts";
pub const BRAND_DIR: &str = "apps/vink-web/assets/brand";

pub fn run(root: &Path) -> Result<(), String> {
    let mut problems = Vec::new();
    skills(root, &mut problems);
    links(root, &mut problems);
    boundaries(root, &mut problems);
    fonts(root, &mut problems);
    brand(root, &mut problems);
    editor_packages(root, &mut problems);
    crate::ui_qualification::check(root, &mut problems);
    secrets(root, &mut problems);
    safety(root, &mut problems);
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

/// Preserve the approved V-pen source and reject stale/unrecorded served artwork.
pub(crate) fn brand(root: &Path, problems: &mut Vec<String>) {
    let dir = root.join(BRAND_DIR);
    let record = read(&dir.join("provenance.json"))
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
    let Some(record) = record else {
        problems.push(format!("{BRAND_DIR}/provenance.json: invalid record"));
        return;
    };
    let Some(files) = record["files"].as_object() else {
        problems.push(format!("{BRAND_DIR}/provenance.json: missing files"));
        return;
    };
    // Pin the approved ZIP and its original manifest independently of the runtime record.
    // Editing a served checksum alone cannot bless altered or historical artwork.
    for (path_key, hash_key, approved) in [
        (
            "reference_path",
            "reference_sha256",
            "3e490cb95171139ab23404fb0d01e7a8a7505ea25c5a0e07ab60df7cdcc01b9e",
        ),
        (
            "source_manifest_path",
            "source_manifest_sha256",
            "96d781333a71d1ce1c501c52c087c0962c2116313d949522f2a7e06ea5350cc2",
        ),
    ] {
        let path = record[path_key].as_str().unwrap_or_default();
        if record[hash_key].as_str() != Some(approved)
            || std::fs::read(root.join(path))
                .map(|b| sha256_hex(&b))
                .ok()
                .as_deref()
                != Some(approved)
        {
            problems.push(format!(
                "{BRAND_DIR}: {path_key} differs from approved V-pen R1"
            ));
        }
    }
    let source = record["source_manifest_path"]
        .as_str()
        .and_then(|path| read(&root.join(path)))
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .unwrap_or_default();
    for (name, entry) in files {
        match std::fs::read(dir.join(name)) {
            Ok(bytes)
                if entry["sha256"].as_str() == Some(sha256_hex(&bytes).as_str())
                    && entry["bytes"].as_u64() == Some(bytes.len() as u64) => {}
            _ => problems.push(format!(
                "{BRAND_DIR}/{name}: missing or bytes/SHA-256 differs"
            )),
        }
        if let Some(path) = entry["source_path"].as_str() {
            if source["assets"][path]["sha256"] != entry["sha256"]
                || source["assets"][path]["bytes"] != entry["bytes"]
            {
                problems.push(format!(
                    "{BRAND_DIR}/{name}: differs from approved source manifest"
                ));
            }
        } else if !matches!(
            name.as_str(),
            "vpen-r1-favicon.svg"
                | "vpen-r1-manifest-light.webmanifest"
                | "vpen-r1-manifest-dark.webmanifest"
        ) {
            problems.push(format!(
                "{BRAND_DIR}/{name}: artwork must have a source_path"
            ));
        }
    }
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if matches!(
                path.extension().and_then(|s| s.to_str()),
                Some("svg" | "png" | "webmanifest")
            ) && !files.contains_key(
                path.file_name()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap_or_default(),
            ) {
                problems.push(format!(
                    "{}: unrecorded served brand asset",
                    rel(root, &path)
                ));
            }
        }
    }
    for variant in ["mark", "lockup"] {
        for theme in ["light", "dark", "mono"] {
            let name = format!("vpen-r1-{variant}-{theme}.svg");
            if !files.contains_key(&name) {
                problems.push(format!("{BRAND_DIR}/{name}: not recorded"));
            }
        }
    }
    for theme in ["light", "dark"] {
        for size in [24, 32, 48, 96, 192, 512] {
            let name = format!("vpen-r1-mark-{theme}-{size}.png");
            if !files.contains_key(&name) {
                problems.push(format!("{BRAND_DIR}/{name}: not recorded"));
            }
        }
        let name = format!("vpen-r1-manifest-{theme}.webmanifest");
        if !files.contains_key(&name) {
            problems.push(format!("{BRAND_DIR}/{name}: not recorded"));
        }
    }
    if !files.contains_key("vpen-r1-favicon.svg") {
        problems.push(format!("{BRAND_DIR}: fallback favicon not recorded"));
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
        if !root
            .join(".claude/skills")
            .join(&name)
            .join("SKILL.md")
            .is_file()
        {
            problems.push(format!(
                "{name}: Claude Code cannot load it; add symlink .claude/skills/{name} -> ../../.agents/skills/{name}"
            ));
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
    kotlin_files(
        &root.join("packages/ui-system/cmp/diagnostics/src/commonMain"),
        &mut common,
    );
    kotlin_files(
        &root.join("packages/vink-patterns/cmp/src/commonMain"),
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
    desktop_fonts(root, &record, problems);
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

/// The Desktop adapter follows the web face list; derivatives have one provenance owner.
fn desktop_fonts(root: &Path, record: &serde_json::Value, problems: &mut Vec<String>) {
    let dir = root.join(DESKTOP_FONTS_DIR);
    if record["desktop"]["directory"].as_str() != Some(DESKTOP_FONTS_DIR) {
        problems.push(format!(
            "{FONTS_DIR}/provenance.json: invalid Desktop directory"
        ));
    }
    let mut recorded = std::collections::BTreeSet::new();
    for (source, entry) in record["files"].as_object().into_iter().flatten() {
        let expected = format!("{}.ttf", source.trim_end_matches(".woff2"));
        let derivative = &entry["desktop"];
        if derivative["file"].as_str() != Some(expected.as_str()) {
            problems.push(format!(
                "{source}: missing or mismatched Desktop derivative"
            ));
            continue;
        }
        recorded.insert(expected.clone());
        match std::fs::read(dir.join(&expected)) {
            Ok(bytes) => {
                if derivative["sha256"].as_str() != Some(sha256_hex(&bytes).as_str())
                    || derivative["bytes"].as_u64() != Some(bytes.len() as u64)
                    || !bytes.starts_with(&[0, 1, 0, 0])
                {
                    problems.push(format!("{DESKTOP_FONTS_DIR}/{expected}: format/length/SHA-256 differs from provenance.json"));
                }
            }
            Err(_) => problems.push(format!(
                "{DESKTOP_FONTS_DIR}/{expected}: listed but missing"
            )),
        }
    }
    for entry in std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".ttf") && !recorded.contains(&name) {
            problems.push(format!(
                "{DESKTOP_FONTS_DIR}/{name}: not recorded in provenance.json"
            ));
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

/// No crate needs `unsafe`, so the workspace forbids it once, including code macros expand into
/// a crate. Every member must inherit those lints unchanged, and every owner-local Kani manifest,
/// which compiles production sources outside the workspace, repeats the forbid. A `compile_fail`
/// doctest needs a lawful twin with the same setup: stable rustdoc ignores its error code, so a
/// broken import or a renamed constructor would also "fail to compile" and pass.
fn safety(root: &Path, problems: &mut Vec<String>) {
    let workspace = read(&root.join("Cargo.toml")).unwrap_or_default();
    if !table(&workspace, "workspace.lints.rust")
        .into_iter()
        .any(forbids_unsafe)
    {
        problems
            .push("Cargo.toml: [workspace.lints.rust] must set unsafe_code = \"forbid\"".into());
    }
    let members = workspace_members(&workspace);
    if members.is_empty() {
        problems.push("Cargo.toml: no [workspace] members found".into());
    }
    for member in members {
        let path = format!("{member}/Cargo.toml");
        let manifest = read(&root.join(&path)).unwrap_or_default();
        if let Some(problem) = member_lints(&manifest) {
            problems.push(format!("{path}: {problem}"));
        }
        let mut files = Vec::new();
        rust_files(&root.join(&member).join("src"), &mut files);
        for file in files {
            let text = read(&file).unwrap_or_default();
            if text.contains("```compile_fail") && !text.contains("Lawful twin") {
                problems.push(format!(
                    "{}: a compile_fail doctest needs a \"Lawful twin\" doctest with the same setup",
                    rel(root, &file)
                ));
            }
        }
    }
    for (manifest, _) in crate::kani::MANIFESTS {
        let text = read(&root.join(manifest)).unwrap_or_default();
        if !table(&text, "lints.rust").into_iter().any(forbids_unsafe) {
            problems.push(format!(
                "{manifest}: [lints.rust] must repeat unsafe_code = \"forbid\""
            ));
        }
    }
}

fn forbids_unsafe(line: &str) -> bool {
    line.replace(' ', "") == "unsafe_code=\"forbid\""
}

fn member_lints(manifest: &str) -> Option<&'static str> {
    let inherits = table(manifest, "lints")
        .into_iter()
        .any(|line| line.replace(' ', "") == "workspace=true");
    let overrides = manifest
        .lines()
        .any(|line| line.trim_start().starts_with("[lints."));
    (!inherits || overrides).then_some(
        "must inherit the workspace lints: `[lints] workspace = true`, no [lints.*] table",
    )
}

/// Settings of the TOML table `[name]` up to the next header, without blanks and comments.
fn table<'a>(text: &'a str, name: &str) -> Vec<&'a str> {
    let header = format!("[{name}]");
    text.lines()
        .map(str::trim)
        .skip_while(|line| *line != header)
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

fn workspace_members(workspace: &str) -> Vec<String> {
    let Some((_, rest)) = workspace.split_once("members = [") else {
        return Vec::new();
    };
    rest.split(']')
        .next()
        .unwrap_or_default()
        .split(',')
        .map(|entry| entry.trim().trim_matches('"').to_string())
        .filter(|entry| !entry.is_empty())
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
    fn members_must_inherit_the_workspace_lints_unchanged() {
        let ok = "[package]\nname = \"a\"\n\n[lints]\nworkspace = true\n\n[dependencies]\n";
        assert_eq!(member_lints(ok), None);
        assert!(member_lints("[package]\nname = \"a\"\n").is_some());
        assert!(member_lints("[lints]\nworkspace = false\n").is_some());
        let overridden = "[lints]\nworkspace = true\n[lints.rust]\nunsafe_code = \"allow\"\n";
        assert!(member_lints(overridden).is_some());
        // A setting under another table is not the lints opt-in.
        assert!(member_lints("[package]\nworkspace = true\n[lints]\n").is_some());
    }

    #[test]
    fn the_workspace_member_list_and_forbid_are_read_from_their_tables() {
        let workspace = "[workspace]\nmembers = [\n    \"apps/a\",\n    \"xtask\",\n]\n\n\
            [workspace.lints.rust]\n# no crate needs it\nunsafe_code = \"forbid\"\n[x]\n";
        assert_eq!(workspace_members(workspace), ["apps/a", "xtask"]);
        assert!(
            table(workspace, "workspace.lints.rust")
                .into_iter()
                .any(forbids_unsafe)
        );
        assert!(!table(workspace, "x").into_iter().any(forbids_unsafe));
        assert!(!forbids_unsafe("unsafe_code = \"deny\""));
    }

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
    fn brand_rejects_rehashed_artwork_and_unrecorded_legacy_assets() {
        let root = std::env::temp_dir().join(format!("vink-brand-check-{}", std::process::id()));
        let dir = root.join(BRAND_DIR);
        std::fs::create_dir_all(&dir).unwrap();
        let source_root = crate::root();
        let mut record: serde_json::Value = serde_json::from_str(
            &read(&source_root.join(BRAND_DIR).join("provenance.json")).unwrap(),
        )
        .unwrap();
        for key in ["reference_path", "source_manifest_path"] {
            let path = record[key].as_str().unwrap();
            std::fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
            std::fs::copy(source_root.join(path), root.join(path)).unwrap();
        }
        for name in record["files"].as_object().unwrap().keys() {
            std::fs::copy(source_root.join(BRAND_DIR).join(name), dir.join(name)).unwrap();
        }
        let write_record = |record: &serde_json::Value| {
            std::fs::write(dir.join("provenance.json"), record.to_string()).unwrap();
        };
        write_record(&record);
        let mut problems = Vec::new();
        brand(&root, &mut problems);
        assert!(problems.is_empty(), "{problems:?}");

        // Simulate the old hash-only shortcut: runtime and its record agree, source does not.
        let name = "vpen-r1-mark-light.svg";
        std::fs::write(dir.join(name), b"redrawn artwork").unwrap();
        record["files"][name]["sha256"] = sha256_hex(b"redrawn artwork").into();
        record["files"][name]["bytes"] = 15.into();
        write_record(&record);
        std::fs::write(dir.join("vink-mark-light.svg"), b"historical circle").unwrap();
        brand(&root, &mut problems);
        assert!(
            problems
                .iter()
                .any(|p| p.contains("differs from approved source manifest")),
            "{problems:?}"
        );
        assert!(
            problems
                .iter()
                .any(|p| p.contains("unrecorded served brand asset")),
            "{problems:?}"
        );
        std::fs::remove_dir_all(root).unwrap();
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
        let desktop_dir = root.join(DESKTOP_FONTS_DIR);
        std::fs::create_dir_all(&desktop_dir).unwrap();
        let desktop_bytes = [0, 1, 0, 0, 7];
        std::fs::write(desktop_dir.join("a.ttf"), desktop_bytes).unwrap();
        let record = format!(
            r#"{{"desktop": {{"directory": "{DESKTOP_FONTS_DIR}"}},
                "files": {{"a.woff2": {{"sha256": "{}", "desktop": {{"file": "a.ttf", "bytes": 5, "sha256": "{}"}}}}}},
                "families": {{"A": {{"license_file": "OFL.txt", "license_sha256": "0"}}}}}}"#,
            sha256_hex(b"face"),
            sha256_hex(&desktop_bytes),
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
    fn desktop_font_derivative_checks_detect_corruption_and_unowned_files() {
        let root =
            std::env::temp_dir().join(format!("vink-desktop-fonts-check-{}", std::process::id()));
        let dir = root.join(DESKTOP_FONTS_DIR);
        std::fs::create_dir_all(&dir).unwrap();
        let bytes = [0, 1, 0, 0, 7];
        let record = serde_json::json!({
            "desktop": {"directory": DESKTOP_FONTS_DIR},
            "files": {"a.woff2": {"desktop": {"file": "a.ttf", "bytes": bytes.len(), "sha256": sha256_hex(&bytes)}}}
        });
        std::fs::write(dir.join("a.ttf"), bytes).unwrap();
        let mut problems = Vec::new();
        desktop_fonts(&root, &record, &mut problems);
        assert!(problems.is_empty(), "{problems:?}");
        // Valid header and unchanged byte count: only the pinned hash detects this mutation.
        std::fs::write(dir.join("a.ttf"), [0, 1, 0, 0, 8]).unwrap();
        std::fs::write(dir.join("stray.ttf"), bytes).unwrap();
        desktop_fonts(&root, &record, &mut problems);
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems[0].contains("a.ttf: format/length/SHA-256"));
        assert!(problems[1].contains("stray.ttf: not recorded"));
        std::fs::remove_dir_all(root).unwrap();
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
