//! Artifact adapters: no migrations, servers or tests.
use crate::plan::Artifact;
use crate::process::cargo;
use crate::{checks, doctor, env, process, root, ui_tokens};
use std::path::Path;
const EDITOR_RUNTIME_DIR: &str = "apps/vink-web/editor";

pub fn binary(artifact: Artifact, release: bool) -> Result<(), String> {
    process::run(
        &mut binary_command(artifact, release)?,
        &format!("build {artifact:?}"),
    )
}
fn binary_command(artifact: Artifact, release: bool) -> Result<std::process::Command, String> {
    let (package, bin) = match artifact {
        Artifact::App => ("vink-app", "vink-app"),
        Artifact::Worker => ("vink-app", "vink-worker"),
        Artifact::Identity => ("vink-identity", "vink-identity"),
        _ => return Err("expected a server binary".into()),
    };
    let mut command = cargo(&root());
    command.args(["build", "--locked", "-p", package, "--bin", bin]);
    if artifact == Artifact::Worker {
        command.arg("--no-default-features");
    }
    if release {
        command.arg("--release");
    }
    Ok(command)
}

pub fn bundle(release: bool, perf_probe: bool) -> Result<(), String> {
    bundle_at(
        release,
        if perf_probe { "perf-probe" } else { "hydrate" },
        &root().join("target/site/pkg"),
        None,
    )
}

pub fn api(release: bool) -> Result<(), String> {
    let mut command = binary_command(Artifact::App, release)?;
    command.arg("--no-default-features");
    process::run(&mut command, "build API without SSR")
}

pub const SPA_SCRATCH_SITE: &str = "target/verify-spa";

pub fn spa(release: bool, scratch: bool) -> Result<(), String> {
    let root = root();
    let revision = match std::env::var("VINK_SPA_REVISION") {
        Ok(revision) => revision,
        Err(_) => {
            let out = std::process::Command::new("git")
                .current_dir(&root)
                .args(["rev-parse", "HEAD"])
                .output()
                .map_err(|e| e.to_string())?;
            String::from_utf8_lossy(&out.stdout).trim().to_owned()
        }
    };
    if revision.len() != 40
        || !revision
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err("VINK_SPA_REVISION requires a full lowercase Git SHA".into());
    }
    let site = root.join(if scratch {
        SPA_SCRATCH_SITE
    } else {
        "target/spa"
    });
    let prefix = format!("/_v/{revision}");
    let pkg = site.join("_v").join(&revision).join("pkg");
    // These public presentation hints never grant access. The API enforces both flags.
    let mut features = String::new();
    for (key, name) in [
        ("VINK_SPA_SIGN_IN", "vink-sign-in"),
        ("VINK_SPA_FOLLOWING_FEED", "vink-following-feed"),
    ] {
        let value = if scratch {
            Ok("enabled".to_owned())
        } else {
            std::env::var(key)
        };
        match value.as_deref() {
            Ok("enabled") => {
                features.push_str(&format!(r#"<meta name="{name}" content="enabled">"#))
            }
            Ok("disabled") | Err(std::env::VarError::NotPresent) => (),
            _ => return Err(format!("{key} must be enabled or disabled")),
        }
    }
    let index = include_str!("../../apps/vink-web/index.html")
        .replace("{{PREFIX}}", &prefix)
        .replace("{{FEATURES}}", &features)
        .replace(
            "{{APPEARANCE}}",
            include_str!("../../apps/vink-web/appearance.js"),
        );
    // The artifact lock serializes builders. A failed rebuild cannot leave a
    // publishable entry that refers to a partially replaced package.
    let entry = site.join("index.html");
    match std::fs::remove_file(&entry) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(format!("clear {}: {e}", entry.display())),
    }
    bundle_at(release, "csr", &pkg, Some(&prefix))?;
    let pending = site.join("index.tmp");
    std::fs::write(&pending, index)
        .and_then(|()| std::fs::rename(pending, entry))
        .map_err(|e| e.to_string())
}

fn bundle_at(release: bool, feature: &str, pkg: &Path, prefix: Option<&str>) -> Result<(), String> {
    let root = root();
    crate::web_boundary::check(&root)?;
    doctor::require_wasm_bindgen(&root)?;
    doctor::node_version()?;
    doctor::npm_version()?;
    let profile = if release { "wasm-release" } else { "dev" };
    let mut wasm = cargo(&root);
    if let Some(prefix) = prefix {
        wasm.env("VINK_ASSET_PREFIX", prefix);
    }
    wasm.args([
        "build",
        "--locked",
        "-p",
        "vink-web",
        "--lib",
        "--target",
        "wasm32-unknown-unknown",
        "--features",
        feature,
        "--profile",
        profile,
    ]);
    process::run(
        &mut wasm,
        &format!("build browser bundle (vink-web, {feature})"),
    )?;
    let out_dir = if release { "wasm-release" } else { "debug" };
    let wasm_file = root
        .join("target/wasm32-unknown-unknown")
        .join(out_dir)
        .join("vink_web.wasm");
    // Both are generated. Bindgen does not prune obsolete snippet hashes, and previous
    // Web builds could have left the complete Preview payload here.
    for stale in [pkg.join("expressions"), pkg.join("snippets")] {
        match std::fs::remove_dir_all(&stale) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("clear generated {}: {error}", stale.display())),
        }
    }
    std::fs::create_dir_all(pkg).map_err(|e| format!("create {}: {e}", pkg.display()))?;
    let mut bindgen = std::process::Command::new("wasm-bindgen");
    bindgen
        .args([
            "--target",
            "web",
            "--no-typescript",
            "--out-name",
            "vink",
            "--out-dir",
        ])
        .arg(pkg)
        .arg(&wasm_file);
    if release {
        bindgen.arg("--remove-name-section");
    }
    process::run(&mut bindgen, "wasm-bindgen public package")?;
    editor_runtime(&root, &pkg.join("vink-editor.js"))?;
    stylesheet(&root, &pkg.join("vink.css"), release)?;
    copy_fonts(&root.join(checks::FONTS_DIR), &pkg.join("fonts"))?;
    copy_brand(&root.join(checks::BRAND_DIR), &pkg.join("brand"))?;
    copy_empty_artwork(&root, &pkg.join("empty"))?;
    crate::web_boundary::artifacts(pkg)
}

/// Server binaries plus
/// `target/site/pkg/{vink.js, vink_bg.wasm, snippets/, vink-editor.js, vink.css, fonts/}`;
/// `vink.css` is compiled by the pinned Tailwind v4 CLI from `packages/ui-system/leptos/tailwind/web.css`
/// (layering generated `tailwind-theme.css` + `tokens.css`, legacy CSS, and allowlisted utilities).
pub fn legacy(env: &env::Env, release: bool) -> Result<(), String> {
    let _ = env;
    bundle(release, false)?;
    for artifact in [Artifact::App, Artifact::Worker, Artifact::Identity] {
        binary(artifact, release)?;
    }
    Ok(())
}

/// `/pkg/vink.css`: checks generated UI tokens first (so a stale `tokens.css` or
/// `tailwind-theme.css` is never served), then compiles `packages/ui-system/leptos/tailwind/web.css`
/// with the pinned Tailwind v4 CLI into one stylesheet (`--minify` in release).
fn stylesheet(root: &Path, out: &Path, release: bool) -> Result<(), String> {
    ui_tokens::run(root, true)?;
    crate::tailwind::compile(root, &root.join(crate::tailwind::WEB_ENTRY), out, release)
}

/// The CodeMirror runtime (`apps/vink-web/editor`): installs the locked npm dependencies when the
/// lockfile changed since the last install, then type-checks and bundles it to `outfile`.
/// Install scripts are not run; esbuild and TypeScript ship their binaries as packages.
fn editor_runtime(root: &Path, outfile: &Path) -> Result<(), String> {
    let dir = root.join(EDITOR_RUNTIME_DIR);
    let lock = std::fs::read(dir.join("package-lock.json"))
        .map_err(|e| format!("read {EDITOR_RUNTIME_DIR}/package-lock.json: {e}"))?;
    let stamp = dir.join("node_modules/.vink-installed-lock.json");
    if std::fs::read(&stamp).ok().as_ref() != Some(&lock) {
        let mut install = std::process::Command::new("npm");
        install
            .current_dir(&dir)
            .args(["ci", "--ignore-scripts", "--no-audit", "--no-fund"]);
        process::run(&mut install, "npm ci (editor runtime, locked)")?;
        std::fs::write(&stamp, &lock).map_err(|e| format!("write {}: {e}", stamp.display()))?;
    }
    let mut bundle = std::process::Command::new("node");
    bundle
        .current_dir(&dir)
        .arg("scripts/build.mjs")
        .arg(outfile);
    process::run(
        &mut bundle,
        "editor runtime: type check + bundle → vink-editor.js",
    )
}

/// Font faces and their licenses, served next to `vink.css` at `/pkg/fonts/`. The directory is
/// recreated so a removed face is not served from an earlier build.
fn copy_fonts(from: &Path, to: &Path) -> Result<(), String> {
    if to.exists() {
        std::fs::remove_dir_all(to).map_err(|e| format!("clear {}: {e}", to.display()))?;
    }
    std::fs::create_dir_all(to).map_err(|e| format!("create {}: {e}", to.display()))?;
    let entries = std::fs::read_dir(from).map_err(|e| format!("read {}: {e}", from.display()))?;
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.ends_with(".woff2") || name.starts_with("OFL") {
            std::fs::copy(entry.path(), to.join(name.as_ref()))
                .map_err(|e| format!("copy font {name}: {e}"))?;
        }
    }
    Ok(())
}

/// Serve committed brand assets only; the selected ZIP and review boards stay outside the site.
fn copy_brand(from: &Path, to: &Path) -> Result<(), String> {
    let mut problems = Vec::new();
    checks::brand(&root(), &mut problems);
    if !problems.is_empty() {
        return Err(problems.join("\n"));
    }
    if to.exists() {
        std::fs::remove_dir_all(to).map_err(|e| format!("clear {}: {e}", to.display()))?;
    }
    std::fs::create_dir_all(to).map_err(|e| format!("create {}: {e}", to.display()))?;
    for entry in std::fs::read_dir(from).map_err(|e| format!("read brand: {e}"))? {
        let path = entry.map_err(|e| format!("read brand entry: {e}"))?.path();
        if matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("png" | "svg" | "webmanifest")
        ) {
            std::fs::copy(&path, to.join(path.file_name().expect("brand file name")))
                .map_err(|e| format!("copy brand {}: {e}", path.display()))?;
        }
    }
    Ok(())
}

/// Only verified runtime derivatives enter /pkg; sources, boards and scripts stay out.
pub(crate) fn check_empty_artwork(root: &Path) -> Result<(), String> {
    process::run(
        std::process::Command::new("node").arg(root.join("apps/vink-web/assets/empty/verify.mjs")),
        "empty artwork: WebP container, dimensions, SHA-256 and byte budget",
    )
}

fn copy_empty_artwork(root: &Path, to: &Path) -> Result<(), String> {
    check_empty_artwork(root)?;
    if to.exists() {
        std::fs::remove_dir_all(to).map_err(|e| format!("clear {}: {e}", to.display()))?;
    }
    std::fs::create_dir_all(to).map_err(|e| format!("create {}: {e}", to.display()))?;
    for name in ["no-results", "no-public-works", "first-draft"] {
        let file = format!("vink-empty-{name}.webp");
        std::fs::copy(
            root.join("apps/vink-web/assets/empty").join(&file),
            to.join(&file),
        )
        .map_err(|e| format!("copy empty artwork {file}: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn builds_exactly_one_requested_binary_without_frontend_commands() {
        for (artifact, package, bin) in [
            (Artifact::App, "vink-app", "vink-app"),
            (Artifact::Worker, "vink-app", "vink-worker"),
            (Artifact::Identity, "vink-identity", "vink-identity"),
        ] {
            let cmd = binary_command(artifact, false).unwrap();
            let args: Vec<_> = cmd.get_args().map(|a| a.to_str().unwrap()).collect();
            let mut expected = vec!["build", "--locked", "-p", package, "--bin", bin];
            if artifact == Artifact::Worker {
                expected.push("--no-default-features");
            }
            assert_eq!(args, expected);
        }
    }
}
