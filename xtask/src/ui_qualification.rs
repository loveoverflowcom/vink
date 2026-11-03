//! Admission checks for the migrated UI owners, run by the existing `verify quick` lane.
//! Public component discovery is deliberately bounded to the repository's declaration syntax;
//! registration is independent of the catalog so adding an API cannot silently bypass preview.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

const COVERAGE: &str = "packages/ui-system/shared/api-coverage.json";

fn files(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                result.extend(files(&path, extension));
            } else if path.extension().is_some_and(|e| e == extension) {
                result.push(path);
            }
        }
    }
    result.sort();
    result
}

fn components(text: &str, renderer: &str) -> BTreeSet<String> {
    let marker = if renderer == "leptos" {
        "#[component"
    } else {
        "@Composable"
    };
    text.split(marker)
        .skip(1)
        .filter_map(|part| {
            let declaration = part.split('{').next()?;
            let prefix = if renderer == "leptos" {
                "pub fn "
            } else {
                "fun "
            };
            declaration.lines().find_map(|line| {
                let name = line.strip_prefix(prefix)?.split('(').next()?;
                (name.chars().next()?.is_uppercase() && !name.contains('.'))
                    .then(|| name.to_string())
            })
        })
        .collect()
}

pub fn check(root: &Path, problems: &mut Vec<String>) {
    let read = |path: &str| std::fs::read_to_string(root.join(path)).unwrap_or_default();
    let coverage: Value = serde_json::from_str(&read(COVERAGE)).unwrap_or(Value::Null);
    if coverage["format"] != 1 || coverage["components"].as_array().is_none_or(Vec::is_empty) {
        problems.push(format!("{COVERAGE}: missing or invalid API admission map"));
    }
    let catalog: Value =
        serde_json::from_str(&read(crate::ui_tokens::CATALOG)).unwrap_or(Value::Null);
    let entries: BTreeSet<&str> = catalog["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| e["id"].as_str())
        .collect();
    let mut declared = BTreeSet::new();
    for (renderer, dirs, extension) in [
        (
            "leptos",
            vec![
                "packages/ui-system/leptos/src",
                "packages/vink-patterns/leptos/src",
            ],
            "rs",
        ),
        (
            "cmp",
            vec![
                "packages/ui-system/cmp/ui/src/commonMain",
                "packages/vink-patterns/cmp/src/commonMain",
            ],
            "kt",
        ),
    ] {
        for file in dirs
            .iter()
            .flat_map(|dir| files(&root.join(dir), extension))
        {
            let path = file
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .to_string();
            let text = std::fs::read_to_string(&file).unwrap_or_default();
            for name in components(&text, renderer) {
                declared.insert((renderer.to_string(), path.clone(), name));
            }
            // Production sources cannot depend on diagnostic fixtures, effect adapters or hosts.
            for forbidden in [
                "vink.ui.preview",
                "VinkFixture",
                "VinkIdentityVectors",
                "::catalog::fixture",
                "vink_app::",
                "vink_web::",
                "import retrofit",
                "import io.ktor",
                "import java.sql",
            ] {
                if !file
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .contains("Vectors")
                    && !path.ends_with("catalog.rs")
                    && !path.ends_with("identity_vectors.rs")
                    && !path.ends_with("VinkCatalog.kt")
                    && text.lines().any(|l| {
                        let l = l.trim();
                        (l.starts_with("use ") || l.starts_with("import ")) && l.contains(forbidden)
                    })
                {
                    problems.push(format!("{path}: production UI imports `{forbidden}`"));
                }
            }
        }
    }
    let mut admitted = BTreeSet::new();
    for component in coverage["components"].as_array().into_iter().flatten() {
        let get = |k: &str| component[k].as_str().unwrap_or_default().to_string();
        let key = (get("renderer"), get("source"), get("symbol"));
        if !admitted.insert(key.clone())
            || !declared.contains(&key)
            || !entries.contains(get("entry").as_str())
        {
            problems.push(format!(
                "{COVERAGE}: duplicate/stale API or missing catalog entry: {key:?}"
            ));
        }
    }
    for key in declared.difference(&admitted) {
        problems.push(format!(
            "{COVERAGE}: public component lacks scenario mapping: {key:?}"
        ));
    }
    for manifest in [
        "apps/vink-web/Cargo.toml",
        "packages/vink-patterns/leptos/Cargo.toml",
        "packages/ui-system/leptos/Cargo.toml",
        "packages/ui-system/cmp/ui/build.gradle.kts",
        "packages/vink-patterns/cmp/build.gradle.kts",
    ] {
        for forbidden in [
            "vink-ui-preview",
            "project(\":preview\")",
            "project(\":diagnostics\")",
            "vink-app",
            "vink-web",
            "sqlx",
        ] {
            if read(manifest).lines().any(|l| {
                !l.trim_start().starts_with('#')
                    && (l.trim_start().starts_with(forbidden)
                        || forbidden.starts_with("project(") && l.contains(forbidden))
            }) {
                problems.push(format!("{manifest}: production dependency on {forbidden}"));
            }
        }
    }
    for file in files(&root.join("apps/vink-web/src"), "rs") {
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        for (i, line) in text.lines().enumerate() {
            if ["<button", "<input", "<select", "<textarea"]
                .iter()
                .any(|tag| line.contains(tag))
            {
                problems.push(format!(
                    "{}:{}: migrated app must use shared controls",
                    file.display(),
                    i + 1
                ));
            }
        }
    }
    let exceptions: Vec<(&str, &str)> = coverage["style_exceptions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let path = e["path"].as_str()?;
            let declaration = e["declaration"].as_str()?;
            if e["owner"].as_str().is_none_or(str::is_empty)
                || e["reason"].as_str().is_none_or(str::is_empty)
                || !read(path).contains(declaration)
            {
                problems.push(format!(
                    "{COVERAGE}: stale or unexplained style exception: {path}: {declaration}"
                ));
            }
            Some((path, declaration))
        })
        .collect();
    for path in [
        "apps/vink-web/style/vink.css",
        "packages/ui-system/leptos/style/vink-ui.css",
        "packages/vink-patterns/leptos/patterns.css",
    ] {
        for chunk in read(path).split(';') {
            let declaration = chunk.rsplit(['{', '}']).next().unwrap_or("").trim();
            if style_literal(declaration) && !exceptions.contains(&(path, declaration)) {
                problems.push(format!(
                    "{path}: authored UI value outside tokens: {declaration}"
                ));
            }
        }
    }
    // Native literal colour/type/motion values; geometry and colour arithmetic have other owners.
    for dir in [
        "packages/ui-system/cmp/ui/src/commonMain",
        "packages/vink-patterns/cmp/src/commonMain",
    ] {
        for file in files(&root.join(dir), "kt") {
            if file.file_name().is_some_and(|n| n == "VinkTokens.kt") {
                continue;
            }
            for (i, line) in std::fs::read_to_string(&file)
                .unwrap_or_default()
                .lines()
                .enumerate()
            {
                if line.contains("Color(0x")
                    || [
                        "FontWeight.SemiBold",
                        "FontWeight.Bold",
                        "FontWeight.Normal",
                        "FontWeight.Medium",
                    ]
                    .iter()
                    .any(|weight| line.contains(weight))
                    || ["tween(", "delay(", "durationMillis = "]
                        .iter()
                        .any(|marker| {
                            line.split(marker).skip(1).any(|after| {
                                after.trim_start().starts_with(|c: char| c.is_ascii_digit())
                            })
                        })
                    || line
                        .split(".sp")
                        .take(line.matches(".sp").count())
                        .any(|prefix| prefix.chars().last().is_some_and(|c| c.is_ascii_digit()))
                {
                    problems.push(format!(
                        "{}:{}: native colour/type literal outside tokens",
                        file.display(),
                        i + 1
                    ));
                }
            }
        }
    }
}

fn style_literal(declaration: &str) -> bool {
    let Some((property, value)) = declaration.split_once(':') else {
        return false;
    };
    let value = value.trim();
    if property.trim() == "font"
        && value
            .split_whitespace()
            .any(|part| part.starts_with(|c: char| c.is_ascii_digit()))
    {
        return true;
    }
    if property.trim().starts_with("font-") && property.trim() != "font-style" {
        return !value.contains("var(") && !["inherit", "normal", "bold"].contains(&value);
    }
    value.contains("rgb(")
        || value.contains("rgba(")
        || value
            .split('#')
            .skip(1)
            .any(|hex| hex.chars().take_while(char::is_ascii_hexdigit).count() >= 3)
        || (property.trim().ends_with("duration") && !value.contains("var("))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovers_unregistered_public_components_independently_of_catalog() {
        assert_eq!(
            components(
                "#[component]\npub fn NewControl(x: String) -> impl IntoView {}\n#[component]\nfn Private() {}",
                "leptos"
            ),
            BTreeSet::from(["NewControl".into()])
        );
        assert_eq!(
            components(
                "@Composable\nfun NewNative() {}\n@Composable\nprivate fun Hidden() {}",
                "cmp"
            ),
            BTreeSet::from(["NewNative".into()])
        );
    }
    #[test]
    fn rejects_authored_colour_type_and_duration_but_allows_layout() {
        for s in [
            "color: #ffeeff",
            "background: rgb(0 0 0 / .5)",
            "font-size: 14px",
            "transition-duration: 123ms",
        ] {
            assert!(style_literal(s), "{s}");
        }
        for s in [
            "font-size: var(--vk-type-body-size)",
            "width: 320px",
            "background: var(--vk-surface)",
        ] {
            assert!(!style_literal(s), "{s}");
        }
    }

    #[test]
    fn admission_fails_on_missing_apis_stale_entries_and_preview_dependencies() {
        let root = std::env::temp_dir().join(format!("vink-ui-admission-{}", std::process::id()));
        for dir in [
            "packages/ui-system/shared",
            "packages/ui-system/leptos/src",
            "apps/vink-web",
        ] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::write(
            root.join("packages/ui-system/leptos/src/new.rs"),
            "#[component]\npub fn Unregistered() -> impl IntoView {}\n",
        )
        .unwrap();
        std::fs::write(root.join(COVERAGE), r#"{"format":1,"components":[{"renderer":"leptos","source":"missing.rs","symbol":"Stale","entry":"missing"}]}"#).unwrap();
        std::fs::write(
            root.join("apps/vink-web/Cargo.toml"),
            "[dependencies]\nvink-ui-preview = { path = \"preview\" }\n",
        )
        .unwrap();
        let mut problems = Vec::new();
        check(&root, &mut problems);
        std::fs::remove_dir_all(root).unwrap();
        assert!(problems.iter().any(|p| p.contains("Unregistered")));
        assert!(problems.iter().any(|p| p.contains("Stale")));
        assert!(
            problems
                .iter()
                .any(|p| p.contains("production dependency on vink-ui-preview"))
        );
    }
}
