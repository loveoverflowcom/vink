//! Artifact adapters: no migrations, servers or tests.
use crate::plan::Artifact;
use crate::process::cargo;
use crate::web_dag::WebArtifactId;
use crate::{checks, doctor, env, process, root, web_assets::Package};
use std::path::Path;
use std::time::Instant;

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
    crate::web_assets::require_local_path(&root, &pkg)?;
    let entry = site.join("index.html");
    match std::fs::remove_file(&entry) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(format!("clear {}: {e}", entry.display())),
    }
    bundle_at(release, "csr", &pkg, Some(&prefix))?;
    let index_started = Instant::now();
    let pending = site.join("index.tmp");
    // Unlink a pending entry left by an interrupted run (including a symlink).
    match std::fs::remove_file(&pending) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(format!("clear {}: {e}", pending.display())),
    }
    use std::io::Write;
    std::fs::File::create_new(&pending)
        .and_then(|mut file| file.write_all(index.as_bytes()))
        .and_then(|()| std::fs::rename(pending, entry))
        .map_err(|e| e.to_string())?;
    let index_digest = checks::sha256_hex(index.as_bytes());
    println!(
        "==> {}: UPDATED ({} ms, sha256={})",
        WebArtifactId::SpaIndex.name(),
        index_started.elapsed().as_millis(),
        &index_digest[..12]
    );
    Ok(())
}

fn bundle_at(release: bool, feature: &str, pkg: &Path, prefix: Option<&str>) -> Result<(), String> {
    let root = root();
    let rustc_version = doctor::toolchain(&root)?;
    let bindgen = doctor::require_wasm_bindgen(&root)?;
    let node_version = doctor::node_version()?;
    doctor::npm_version()?;
    let profile = if release { "wasm-release" } else { "dev" };
    let effective_prefix = prefix
        .map(str::to_owned)
        .or_else(|| std::env::var("VINK_ASSET_PREFIX").ok());
    let mut package = Package::new(
        &root,
        pkg,
        profile,
        feature,
        effective_prefix.as_deref(),
        &rustc_version,
        &node_version,
        &bindgen,
    )?;

    package.tokens(&root)?;
    let boundary_started = Instant::now();
    crate::web_boundary::check(&root)?;
    package.record_step(
        WebArtifactId::WebBoundary,
        "VERIFIED",
        boundary_started.elapsed().as_millis() as u64,
        String::new(),
        "routes and five Cargo graphs checked".into(),
    );

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
        "--message-format=json-render-diagnostics",
    ]);
    let freshness = process::run_cargo_json(
        &mut wasm,
        &format!("build browser bundle (vink-web, {feature}, {profile})"),
    )?;
    let out_dir = if release { "wasm-release" } else { "debug" };
    let wasm_file = root
        .join("target/wasm32-unknown-unknown")
        .join(out_dir)
        .join("vink_web.wasm");
    let raw = package.record_rust_wasm(&root, &wasm_file, freshness)?;
    package.bindgen(&wasm_file, raw, release)?;
    package.editor_js(&root, &node_version)?;
    package.tailwind_css(&root, release, &node_version)?;

    package.static_files(
        WebArtifactId::StaticFonts,
        "fonts",
        &root.join(checks::FONTS_DIR),
        |path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".woff2") || name.starts_with("OFL"))
        },
    )?;
    let mut problems = Vec::new();
    checks::brand(&root, &mut problems);
    if !problems.is_empty() {
        return Err(problems.join("\n"));
    }
    package.static_files(
        WebArtifactId::StaticBrand,
        "brand",
        &root.join(checks::BRAND_DIR),
        |path| {
            matches!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("png" | "svg" | "webmanifest")
            )
        },
    )?;
    check_empty_artwork(&root)?;
    package.static_files(
        WebArtifactId::StaticEmpty,
        "empty",
        &root.join("apps/vink-web/assets/empty"),
        |path| {
            matches!(
                path.file_name().and_then(|n| n.to_str()),
                Some(
                    "vink-empty-no-results.webp"
                        | "vink-empty-no-public-works.webp"
                        | "vink-empty-first-draft.webp"
                )
            )
        },
    )?;
    package.publish()
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

/// Only verified runtime derivatives enter /pkg; sources, boards and scripts stay out.
pub(crate) fn check_empty_artwork(root: &Path) -> Result<(), String> {
    process::run(
        std::process::Command::new("node").arg(root.join("apps/vink-web/assets/empty/verify.mjs")),
        "empty artwork: WebP container, dimensions, SHA-256 and byte budget",
    )
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
