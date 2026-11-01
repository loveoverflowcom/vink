//! `cargo xtask` — Vink's repository command surface for local development and CI.
//!
//! Commands are documented in docs/development/README.md. xtask never installs toolchains,
//! starts containers, pushes, publishes or deploys. A missing prerequisite is reported as a
//! failure with a fix hint, never as a pass.

mod checks;
mod doctor;
mod env;
mod process;
mod web;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use process::{Report, cargo};

/// The browser editor runtime: TypeScript sources, locked npm dependencies and build script.
const EDITOR_RUNTIME_DIR: &str = "apps/vink-web/editor";

const USAGE: &str = "\
usage: cargo xtask <command>

  doctor                 check prerequisites and configuration (no secrets printed)
  build [--release]      build the server binaries and the browser bundle into target/site
  db migrate             apply migrations to DATABASE_URL
  db seed                load the synthetic development fixture (VINK_PROFILE=local)
  dev                    build, migrate and serve on VINK_BIND (Ctrl-C to stop)
  verify quick           fmt, clippy (server + wasm), unit/contract tests, repository checks
  verify postgres        migrations, persistence/concurrency/HTTP tests, worker lifecycle
  verify web             SSR + hydration + desk interactions in headless Chrome
  verify all             quick, postgres and web in order";

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
        ["db", "migrate"] => finish(app_command(&env, "migrate")),
        ["db", "seed"] => finish(app_command(&env, "dev-seed")),
        ["dev"] => finish(dev(&env)),
        ["verify", lane] => match *lane {
            "quick" => verify(&[Lane::Quick], &env),
            "postgres" => verify(&[Lane::Postgres], &env),
            "web" => verify(&[Lane::Web], &env),
            "all" => verify(&[Lane::Quick, Lane::Postgres, Lane::Web], &env),
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
/// `target/site/pkg/{vink.js, vink_bg.wasm, snippets/, vink-editor.js, vink.css, fonts/}`.
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
    std::fs::copy(
        root.join("apps/vink-web/style/vink.css"),
        pkg.join("vink.css"),
    )
    .map_err(|e| format!("copy vink.css: {e}"))?;
    copy_fonts(&root.join(checks::FONTS_DIR), &pkg.join("fonts"))?;
    let mut server = cargo(&root);
    server.args(["build", "--locked", "-p", "vink-app", "--bins"]);
    if release {
        server.arg("--release");
    }
    env.apply(&mut server);
    process::run(&mut server, "build server binaries (vink-app, vink-worker)")
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

fn app_command(env: &env::Env, command: &str) -> Result<(), String> {
    let root = root();
    let mut run = cargo(&root);
    run.args([
        "run", "--locked", "-q", "-p", "vink-app", "--bin", "vink-app", "--", command,
    ]);
    env.apply(&mut run);
    process::run(&mut run, &format!("vink-app {command}"))
}

fn dev(env: &env::Env) -> Result<(), String> {
    build(env, false)?;
    app_command(env, "migrate")?;
    let root = root();
    let origin = env
        .get("VINK_PUBLIC_ORIGIN")
        .unwrap_or_else(|| "http://127.0.0.1:3000".into());
    println!("==> serving on {origin}  (sample data: `cargo xtask db seed`; Ctrl-C to stop)");
    let mut serve = std::process::Command::new(root.join("target/debug/vink-app"));
    serve.arg("serve").current_dir(&root);
    if env.get("VINK_SITE_ROOT").is_none() {
        serve.env("VINK_SITE_ROOT", root.join("target/site"));
    }
    env.apply(&mut serve);
    process::run(&mut serve, "vink-app serve")
}

#[derive(Clone, Copy)]
enum Lane {
    Quick,
    Postgres,
    Web,
}

fn verify(lanes: &[Lane], env: &env::Env) -> ExitCode {
    let mut report = Report::default();
    for lane in lanes {
        let ok = match lane {
            Lane::Quick => quick(&mut report),
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
    let steps: [(&str, &[&str]); 4] = [
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
            "unit + contract tests (no database)",
            &["test", "--locked", "--workspace", "--lib", "--bins"],
        ),
    ];
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
    let Some(url) = env.get("DATABASE_URL") else {
        report.fail(
            "PostgreSQL available",
            "DATABASE_URL is not set; start infra/dev/compose.yaml and copy .env.example to .env",
        );
        return false;
    };
    if !report.step("PostgreSQL available", || doctor::probe_database(&url)) {
        return false;
    }
    let mut tests = cargo(&root);
    tests.args(["test", "--locked", "-p", "vink-app", "--test", "postgres"]);
    env.apply(&mut tests);
    if !report.step("migrations + persistence/concurrency/HTTP tests", || {
        process::run(&mut tests, "postgres integration tests")
    }) {
        return false;
    }
    let mut bins = cargo(&root);
    bins.args(["build", "--locked", "-p", "vink-app", "--bins"]);
    if !report.step("build binaries", || {
        process::run(&mut bins, "build binaries")
    }) {
        return false;
    }
    report.step("invalid configuration exits 2", || {
        web::config_rejection(&root)
    }) && report.step("worker starts, probes schema, stops on SIGTERM", || {
        web::worker_lifecycle(&root, env)
    })
}
