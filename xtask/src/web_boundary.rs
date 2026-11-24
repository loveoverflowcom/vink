//! Admission of Web product routes, the resolved normal dependency graph and generated files.
use crate::process::cargo;
use std::{collections::BTreeSet, path::Path};

const APPROVED_ROUTES: &[&str] = &[
    "/",
    "/the/:slug",
    "/tac-pham/:id",
    "/ban-viet",
    "/ban-viet/:id",
    "/ban-viet/:id/kiem-duyet",
    "/kiem-duyet",
    "/kiem-duyet/ca/:id",
    "/kiem-duyet/phan-doi/:id",
    "/toi",
    "/toi/chinh-ho-so",
    "/toi/dia-chi",
    "/:author",
    "/:author/nguoi-theo-doi",
    "/:author/dang-theo-doi",
];

pub fn check(root: &Path) -> Result<(), String> {
    let router = root.join("apps/vink-web/src/lib.rs");
    let source =
        std::fs::read_to_string(&router).map_err(|e| format!("read {}: {e}", router.display()))?;
    routes(&source)?;
    // Cargo resolves aliases, feature-activated edges and transitive normal dependencies.
    // Preview workspace members and lawful build/test dependencies are outside these trees.
    for (label, args) in [
        (
            "Web SSR",
            &["-p", "vink-web", "--no-default-features", "-F", "ssr"][..],
        ),
        (
            "Web hydrate",
            &[
                "-p",
                "vink-web",
                "--no-default-features",
                "-F",
                "hydrate",
                "--target",
                "wasm32-unknown-unknown",
            ],
        ),
        ("App default", &["-p", "vink-app"]),
    ] {
        let output = cargo(root)
            .args([
                "tree", "--locked", "--edges", "normal", "--prefix", "none", "--format", "{p}",
            ])
            .args(args)
            .output()
            .map_err(|e| format!("resolve {label} normal dependencies: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "{label} cargo tree: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        graph(root, &String::from_utf8_lossy(&output.stdout))
            .map_err(|error| format!("{label}: {error}"))?;
    }
    Ok(())
}

fn routes(source: &str) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for tag in source
        .split("<Route")
        .skip(1)
        .filter(|suffix| suffix.starts_with(char::is_whitespace))
    {
        let path = tag
            .split_once("/>")
            .and_then(|(tag, _)| tag.split_once("path=path!(\""))
            .and_then(|(_, path)| path.split_once('"'))
            .map(|(path, _)| path)
            .ok_or("a product Route must declare a literal path!(...)")?;
        if !APPROVED_ROUTES.contains(&path) || !seen.insert(path) {
            return Err(format!("unreviewed or repeated product route {path}"));
        }
    }
    if seen.len() != APPROVED_ROUTES.len() {
        return Err("router and product ownership inventory differ".into());
    }
    // Fail closed for paths outside `<Route …/>` (ParentRoute, nested routers, helpers).
    if source.matches("path!(").count() != seen.len() {
        return Err("a product path!(...) is declared outside a reviewed <Route>".into());
    }
    Ok(())
}

fn graph(root: &Path, tree: &str) -> Result<(), String> {
    let mut count = 0;
    for line in tree.lines() {
        let Some(package) = line.split_whitespace().next() else {
            continue;
        };
        count += 1;
        let test_owner = line
            .split_once(" (")
            .and_then(|(_, path)| path.split_once(')'))
            .and_then(|(path, _)| Path::new(path).strip_prefix(root).ok())
            .is_some_and(|path| {
                path.iter()
                    .any(|part| matches!(part.to_str(), Some("preview" | "examples" | "fixtures")))
            });
        if matches!(package, "vink-expressions" | "vink-ui-preview") || test_owner {
            return Err(format!(
                "Preview/Test package in product normal dependencies: {line}"
            ));
        }
    }
    if count == 0 {
        return Err("Cargo tree returned no packages".into());
    }
    Ok(())
}

/// Inspect the actual generated tree, including output left by older builds.
pub fn artifacts(pkg: &Path) -> Result<(), String> {
    fn walk(root: &Path, dir: &Path) -> Result<(), String> {
        for entry in std::fs::read_dir(dir).map_err(|e| format!("read {}: {e}", dir.display()))? {
            let entry = entry.map_err(|e| format!("read {}: {e}", dir.display()))?;
            let path = entry.path();
            let name = path
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_string_lossy();
            let kind = entry
                .file_type()
                .map_err(|e| format!("inspect {}: {e}", path.display()))?;
            if kind.is_symlink() || forbidden_artifact(&name) {
                return Err(format!("forbidden product artifact {name}"));
            }
            if kind.is_dir() {
                walk(root, &path)?;
            }
        }
        Ok(())
    }
    walk(pkg, pkg)
}

fn forbidden_artifact(name: &str) -> bool {
    let name = name.replace('\\', "/");
    let parts: Vec<_> = name.split('/').collect();
    parts.iter().any(|part| {
        matches!(*part, "expressions" | "preview" | "fixtures")
            || part.starts_with("vink-ui-preview-")
            || part.starts_with("vink_ui_preview")
            || part.starts_with("expression_hydration")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_routes_require_explicit_admission() {
        let source = APPROVED_ROUTES
            .iter()
            .map(|route| format!("<Route path=path!(\"{route}\") view=RealPage />"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(routes(&source).is_ok());
        assert!(
            routes(&format!(
                "{source}<Route path=path!(\"/san-pham\") view=Preview />"
            ))
            .is_err()
        );
        assert!(routes(&format!("{source}<Route path=SOME_FIXTURE view=Preview />")).is_err());
        assert!(
            routes(&format!(
                "{source}<ParentRoute path=path!(\"/mau\") view=Preview></ParentRoute>"
            ))
            .is_err()
        );
    }

    #[test]
    fn normal_graph_accepts_shared_packages_and_rejects_preview_edges() {
        let lawful = "vink-web v0.1\nvink-ui v0.1\nvink-patterns v0.1";
        let root = Path::new("/repo");
        assert!(graph(root, lawful).is_ok());
        for injected in [
            "vink-expressions v0.1",
            "vink-ui-preview v0.1",
            "specimen v0.1 (/repo/preview/specimen)",
        ] {
            assert!(graph(root, &format!("{lawful}\n{injected}")).is_err());
        }
    }

    #[test]
    fn generated_snippets_and_retired_pilot_assets_are_detected() {
        for file in [
            "expressions/pilot.js",
            "snippets/vink-web-f00/expressions/bridge.js",
            "snippets/vink-ui-preview-f00/runtime.js",
            "expression_hydration_bg.wasm",
        ] {
            assert!(forbidden_artifact(file));
        }
        for file in [
            "vink_bg.wasm",
            "snippets/vink-web-f00/editor/bridge.js",
            "vink.css",
        ] {
            assert!(!forbidden_artifact(file));
        }
    }
}
