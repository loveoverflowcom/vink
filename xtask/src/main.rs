//! `cargo xtask` — Vink's repository command surface for local development and CI.
//!
//! Commands are documented in docs/development/README.md. xtask never installs toolchains,
//! starts containers, pushes, publishes or deploys. A missing prerequisite is reported as a
//! failure with a fix hint, never as a pass.

mod checks;
mod discussion;
mod doctor;
mod env;
mod expressions;
mod identity;
mod kani;
mod pgurl;
mod process;
mod provision;
mod ui_navigation;
mod ui_preview;
mod ui_qualification;
mod ui_states;
mod ui_tokens;
mod web;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use process::{Report, cargo};

/// Test harness credential for `verify postgres|web`: may create and drop scratch databases and
/// roles. Never passed to a binary as its own credential outside those scratch databases.
pub const TEST_DATABASE_URL: &str = "VINK_TEST_DATABASE_URL";
pub const TEST_DATABASE_URL_MISSING: &str = "VINK_TEST_DATABASE_URL is not set; point it at a superuser of a local or CI \
     PostgreSQL that may create scratch databases and roles (services/postgres/README.md)";

/// The browser editor runtime: TypeScript sources, locked npm dependencies and build script.
const EDITOR_RUNTIME_DIR: &str = "apps/vink-web/editor";

const USAGE: &str = "\
usage: cargo xtask <command>

  doctor                 check prerequisites and configuration (no secrets printed)
  build [--release]      build the server binaries and the browser bundle into target/site
  db migrate             apply App migrations (VINK_MIGRATE_DATABASE_URL), then Identity's
                         when VINK_IDENTITY_MIGRATE_DATABASE_URL is set
  db seed                load the synthetic development fixture (VINK_PROFILE=local)
  dev                    build, migrate, start Identity if VINK_IDENTITY_SOCKET is set,
                         serve on VINK_BIND (Ctrl-C to stop)
  verify quick           fmt, clippy (server + wasm), unit/contract tests, repository checks
  verify expressions     Noto manifest, Quint lifecycle/negative control and Rust replay
  verify discussion      pinned Quint structure/lifecycle checks, trace drift and Rust replays
  verify kani            owner-local Kani harnesses (expressions policy, discussion core);
                         needs the pinned cargo-kani, which xtask never installs
  verify postgres        migration lifecycle, persistence/concurrency/HTTP tests, provisioning
                         and role isolation, worker and vink-identity processes
                         (VINK_TEST_DATABASE_URL, psql)
  verify web             SSR + hydration + desk interactions in headless Chrome
  verify all             quick, discussion, expressions, postgres and web in order
  ui-tokens [--check]    regenerate UI token CSS/Rust/Kotlin from packages/ui-system/shared
                         (--check: write nothing, fail on missing or drifted output)
  ui-preview leptos|cmp|android|ios build|verify|run
                         UI system previews: Leptos (wasm + headless Chrome checks) and
                         Compose Multiplatform desktop (Gradle; VINK_UI_NATIVE_TARGETS=1 adds
                         Android/iOS compilation). android/ios run isolated diagnostic hosts;
                         ios requires VINK_UI_IOS_DEVICE (simulator UDID), Xcode and XcodeGen.";

pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives inside the repository")
        .to_path_buf()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let env = env::Env::load(&root());
    match args.as_slice() {
        ["doctor"] => doctor::run(&env),
        ["build"] => finish(build(&env, false)),
        ["build", "--release"] => finish(build(&env, true)),
        ["db", "migrate"] => finish(migrate_owners(&env)),
        ["db", "seed"] => finish(app_command(&env, "dev-seed")),
        ["dev"] => finish(dev(&env)),
        ["ui-tokens"] => finish(ui_tokens::run(&root(), false)),
        ["ui-tokens", "--check"] => finish(ui_tokens::run(&root(), true)),
        [
            "ui-preview",
            renderer @ ("leptos" | "cmp" | "android" | "ios"),
            "verify",
        ] => ui_preview::verify(&env, renderer),
        [
            "ui-preview",
            renderer @ ("leptos" | "cmp" | "android" | "ios"),
            action @ ("build" | "run"),
        ] => finish(ui_preview::run(&env, renderer, action)),
        ["verify", lane] => match *lane {
            "quick" => verify(&[Lane::Quick], &env),
            "expressions" => verify(&[Lane::Expressions], &env),
            "discussion" => verify(&[Lane::Discussion], &env),
            "kani" => verify(&[Lane::Kani], &env),
            "postgres" => verify(&[Lane::Postgres], &env),
            "web" => verify(&[Lane::Web], &env),
            "all" => verify(
                &[
                    Lane::Quick,
                    Lane::Discussion,
                    Lane::Expressions,
                    Lane::Postgres,
                    Lane::Web,
                ],
                &env,
            ),
            _ => usage(),
        },
        _ => usage(),
    }
}

fn usage() -> ExitCode {
    eprintln!("{USAGE}");
    ExitCode::from(2)
}

fn finish(result: Result<(), String>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("xtask: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Server binaries plus
/// `target/site/pkg/{vink.js, vink_bg.wasm, snippets/, vink-editor.js, vink.css, fonts/}`;
/// `vink.css` is the generated `tokens.css`, the UI system's `vink-ui.css`, then the app's `vink.css`.
pub fn build(env: &env::Env, release: bool) -> Result<(), String> {
    let root = root();
    doctor::require_wasm_bindgen(&root)?;
    doctor::node_version()?;
    doctor::npm_version()?;
    let profile = if release { "wasm-release" } else { "dev" };
    let mut wasm = cargo(&root);
    wasm.args([
        "build",
        "--locked",
        "-p",
        "vink-web",
        "--lib",
        "--target",
        "wasm32-unknown-unknown",
        "--features",
        "hydrate",
        "--profile",
        profile,
    ]);
    process::run(&mut wasm, "build browser bundle (vink-web, hydrate)")?;
    let out_dir = if release { "wasm-release" } else { "debug" };
    let wasm_file = root
        .join("target/wasm32-unknown-unknown")
        .join(out_dir)
        .join("vink_web.wasm");
    let pkg = root.join("target/site/pkg");
    std::fs::create_dir_all(&pkg).map_err(|e| format!("create {}: {e}", pkg.display()))?;
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
        .arg(&pkg)
        .arg(&wasm_file);
    process::run(&mut bindgen, "wasm-bindgen → target/site/pkg")?;
    editor_runtime(&root, &pkg.join("vink-editor.js"))?;
    expressions::build(&root, &pkg.join("expressions"))?;
    stylesheet(&root, &pkg.join("vink.css"))?;
    copy_fonts(&root.join(checks::FONTS_DIR), &pkg.join("fonts"))?;
    copy_brand(&root.join(checks::BRAND_DIR), &pkg.join("brand"))?;
    let mut server = cargo(&root);
    server.args([
        "build",
        "--locked",
        "-p",
        "vink-app",
        "-p",
        "vink-identity",
        "--bins",
    ]);
    if release {
        server.arg("--release");
    }
    env.apply(&mut server);
    process::run(
        &mut server,
        "build server binaries (vink-app, vink-worker, vink-identity)",
    )
}

/// The UI system renderer styles (scoped `.vk-ui`/`vk-ui-*`; the app shell uses its navigation).
const UI_CSS: &str = "packages/ui-system/leptos/style/vink-ui.css";

/// `/pkg/vink.css`: the generated UI tokens (checked against their source first, so a stale
/// file is never served), the UI system's renderer styles, then the web app's own styles.
/// One request, one order.
fn stylesheet(root: &Path, out: &Path) -> Result<(), String> {
    ui_tokens::run(root, true)?;
    let mut css = String::new();
    for part in [
        ui_tokens::CSS_OUT,
        UI_CSS,
        "packages/vink-patterns/leptos/patterns.css",
        "apps/vink-web/style/vink.css",
    ] {
        let text =
            std::fs::read_to_string(root.join(part)).map_err(|e| format!("read {part}: {e}"))?;
        css.push_str(&text);
        css.push('\n');
    }
    std::fs::write(out, css).map_err(|e| format!("write {}: {e}", out.display()))
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

fn app_command(env: &env::Env, command: &str) -> Result<(), String> {
    let root = root();
    let mut run = cargo(&root);
    run.args([
        "run", "--locked", "-q", "-p", "vink-app", "--bin", "vink-app", "--", command,
    ]);
    local_identity_env(env, &mut run)?;
    process::run(&mut run, &format!("vink-app {command}"))
}

/// Migrates each owner with its own migration credential: the App always, Identity when `.env`
/// configures it. There is no cross-owner transaction; a failure leaves the earlier owner migrated.
fn migrate_owners(env: &env::Env) -> Result<(), String> {
    app_command(env, "migrate")?;
    if env.get("VINK_IDENTITY_MIGRATE_DATABASE_URL").is_some() {
        let root = root();
        let mut run = cargo(&root);
        run.args([
            "run",
            "--locked",
            "-q",
            "-p",
            "vink-identity",
            "--bin",
            "vink-identity",
            "--",
            "migrate",
        ]);
        env.apply(&mut run);
        process::run(&mut run, "vink-identity migrate")?;
    }
    Ok(())
}

/// Local stack only: when `.env` names `VINK_IDENTITY_SOCKET`, create its directory (mode 0700)
/// and, where unset, make the developer's own user both the allowed caller and the expected server.
/// Production configures both user ids explicitly.
fn local_identity_env(env: &env::Env, command: &mut std::process::Command) -> Result<(), String> {
    env.apply(command);
    let Some(socket) = env.get("VINK_IDENTITY_SOCKET") else {
        return Ok(());
    };
    let socket = root().join(socket);
    let dir = socket
        .parent()
        .ok_or("VINK_IDENTITY_SOCKET has no directory")?;
    if !dir.exists() {
        std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        std::fs::set_permissions(dir, std::os::unix::fs::PermissionsExt::from_mode(0o700))
            .map_err(|e| format!("chmod {}: {e}", dir.display()))?;
    }
    let uid =
        std::os::unix::fs::MetadataExt::uid(&std::fs::metadata(dir).map_err(|e| e.to_string())?)
            .to_string();
    for key in ["VINK_IDENTITY_CALLER_UID", "VINK_IDENTITY_SERVER_UID"] {
        if env.get(key).is_none() {
            command.env(key, &uid);
        }
    }
    Ok(())
}

/// Starts Identity when configured and waits until it reports ready on its socket.
fn start_identity(env: &env::Env) -> Result<Option<web::Guard>, String> {
    let Some(socket) = env.get("VINK_IDENTITY_SOCKET") else {
        return Ok(None);
    };
    let root = root();
    let socket = root.join(socket);
    let mut command = std::process::Command::new(root.join("target/debug/vink-identity"));
    command.arg("serve").current_dir(&root);
    local_identity_env(env, &mut command)?;
    let child = command
        .spawn()
        .map_err(|e| format!("spawn vink-identity: {e}"))?;
    let mut guard = web::Guard(Some(child));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let mut last = String::new();
    while std::time::Instant::now() < deadline {
        if let Some(Ok(Some(status))) = guard.0.as_mut().map(|c| c.try_wait()) {
            return Err(format!(
                "vink-identity exited with {status}; see its log above"
            ));
        }
        match identity::request(&socket, "GET", "/health/ready", &[], b"") {
            Ok((200, _)) => {
                println!("==> vink-identity ready on {}", socket.display());
                return Ok(Some(guard));
            }
            Ok((_, body)) => last = body,
            Err(_) => {}
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    let _ = guard.terminate(std::time::Duration::from_secs(10));
    Err(format!(
        "vink-identity not ready within 20s: {last} (provisioned and migrated? see services/vink-identity/README.md)"
    ))
}

/// Build, migrate every configured owner, start Identity (if configured) and wait for it, then
/// serve the App in the foreground. Ctrl-C reaches both processes; Identity is stopped with
/// SIGTERM when the App exits on its own.
fn dev(env: &env::Env) -> Result<(), String> {
    build(env, false)?;
    migrate_owners(env)?;
    let root = root();
    let identity = start_identity(env)?;
    let origin = env
        .get("VINK_PUBLIC_ORIGIN")
        .unwrap_or_else(|| "http://127.0.0.1:3000".into());
    println!("==> serving on {origin}  (sample data: `cargo xtask db seed`; Ctrl-C to stop)");
    let mut serve = std::process::Command::new(root.join("target/debug/vink-app"));
    serve.arg("serve").current_dir(&root);
    if env.get("VINK_SITE_ROOT").is_none() {
        serve.env("VINK_SITE_ROOT", root.join("target/site"));
    }
    local_identity_env(env, &mut serve)?;
    let served = process::run(&mut serve, "vink-app serve");
    if let Some(mut identity) = identity {
        let _ = identity.terminate(std::time::Duration::from_secs(10));
    }
    served
}

#[derive(Clone, Copy)]
enum Lane {
    Quick,
    Discussion,
    Expressions,
    /// Not part of `verify all`: it needs a separately installed, pinned verifier.
    Kani,
    Postgres,
    Web,
}

fn verify(lanes: &[Lane], env: &env::Env) -> ExitCode {
    let mut report = Report::default();
    for lane in lanes {
        let ok = match lane {
            Lane::Quick => quick(&mut report),
            Lane::Discussion => discussion::verify(&mut report),
            Lane::Expressions => expressions::verify(&mut report),
            Lane::Kani => kani::verify(&mut report),
            Lane::Postgres => postgres(&mut report, env),
            Lane::Web => web::verify(&mut report, env),
        };
        if !ok {
            break;
        }
    }
    report.finish()
}

fn quick(report: &mut Report) -> bool {
    let root = root();
    let steps: [(&str, &[&str]); 6] = [
        ("fmt", &["fmt", "--all", "--", "--check"]),
        (
            "clippy server (workspace, ssr)",
            &[
                "clippy",
                "--locked",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
        ),
        (
            "clippy browser (vink-web, hydrate, wasm32)",
            &[
                "clippy",
                "--locked",
                "-p",
                "vink-web",
                "--lib",
                "--target",
                "wasm32-unknown-unknown",
                "--features",
                "hydrate",
                "--",
                "-D",
                "warnings",
            ],
        ),
        (
            "clippy UI preview (vink-ui-preview, csr, wasm32)",
            &[
                "clippy",
                "--locked",
                "-p",
                "vink-ui-preview",
                "--lib",
                "--target",
                "wasm32-unknown-unknown",
                "--features",
                "csr",
                "--",
                "-D",
                "warnings",
            ],
        ),
        (
            "literary core type-boundary doctests",
            &["test", "--locked", "-p", "vink-app", "--doc"],
        ),
        (
            "unit + contract tests (no database)",
            &["test", "--locked", "--workspace", "--lib", "--bins"],
        ),
    ];
    if !report.step(
        "ui tokens: source valid, generated CSS/Rust/Kotlin match",
        || ui_tokens::run(&root, true),
    ) {
        return false;
    }
    if !report.step("expressions: manifest/hash/Unicode/drift", || {
        expressions::check(&root)
    }) {
        return false;
    }
    for (name, args) in steps {
        let mut command = cargo(&root);
        command.args(args);
        if !report.step(name, || process::run(&mut command, name)) {
            return false;
        }
    }
    report.step(
        "repository checks (skills, links, boundaries, fonts, editor packages)",
        || checks::run(&root),
    )
}

fn postgres(report: &mut Report, env: &env::Env) -> bool {
    let root = root();
    let Some(url) = env.get(TEST_DATABASE_URL) else {
        report.fail(
            "PostgreSQL test harness credential",
            TEST_DATABASE_URL_MISSING,
        );
        return false;
    };
    if !report.step("PostgreSQL available (test harness)", || {
        doctor::probe_database(&url)
    }) {
        return false;
    }
    // `#[sqlx::test]` creates a database per test from DATABASE_URL.
    for (name, package) in [
        (
            "shared store: migration lifecycle and error classes",
            "vink-store-postgres",
        ),
        (
            "migrations + persistence/concurrency/HTTP tests",
            "vink-app",
        ),
        (
            "identity: migrations and readiness on PostgreSQL",
            "vink-identity",
        ),
    ] {
        let mut tests = cargo(&root);
        tests.args(["test", "--locked", "-p", package, "--test", "postgres"]);
        env.apply(&mut tests);
        tests.env("DATABASE_URL", &url);
        if !report.step(name, || process::run(&mut tests, name)) {
            return false;
        }
    }
    let mut bins = cargo(&root);
    bins.args([
        "build",
        "--locked",
        "-p",
        "vink-app",
        "-p",
        "vink-identity",
        "--bins",
    ]);
    if !report.step("build binaries", || {
        process::run(&mut bins, "build binaries")
    }) {
        return false;
    }
    report.step("invalid configuration exits 2", || {
        web::config_rejection(&root)
    }) && provision::verify(report, &root, &url)
}
