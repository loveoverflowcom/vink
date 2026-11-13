//! `cargo xtask` — Vink's repository command surface for local development and CI.
//!
//! Commands are documented in docs/development/README.md. xtask never installs toolchains,
//! starts service containers, pushes, publishes or deploys. The opt-in `infra build-images`
//! adapter only builds OCI artifacts; it never starts runtime services. A missing prerequisite is reported as a
//! failure with a fix hint, never as a pass.

mod build;
mod checks;
mod cli;
mod decision;
mod discussion;
mod doctor;
mod env;
mod expressions;
mod identity;
mod infra;
mod kani;
mod legacy;
mod lifecycle;
mod pgurl;
mod plan;
mod process;
mod provision;
mod ui_navigation;
mod ui_preview;
mod ui_qualification;
mod ui_states;
mod ui_toggle;
mod ui_tokens;
mod verify;
mod web;
mod workflow;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use verify::{Lane, verify};

/// Test harness credential for `verify postgres|web`: may create and drop scratch databases and
/// roles. Never passed to a binary as its own credential outside those scratch databases.
pub const TEST_DATABASE_URL: &str = "VINK_TEST_DATABASE_URL";
pub const TEST_DATABASE_URL_MISSING: &str = "VINK_TEST_DATABASE_URL is not set; point it at a superuser of a local or CI \
     PostgreSQL that may create scratch databases and roles (services/postgres/README.md)";

const USAGE: &str = "\
usage: cargo xtask <command>

  infra --help           test inventory, OCI build and scoped DNS plans (no deploy)
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
  verify kani            all owner-local Kani harnesses (expressions and App domain)
                         requires the pinned cargo-kani
  verify postgres        migration lifecycle, persistence/concurrency/HTTP tests, provisioning
                         and role isolation, worker and vink-identity processes
                         (VINK_TEST_DATABASE_URL, psql)
                         integration tests default to 2 threads (RUST_TEST_THREADS overrides)
  verify web             SSR + hydration + desk interactions in headless Chrome
  verify all             quick, discussion, expressions, postgres and web in order
  decision --help        inspect, compare and propose versioned decisions
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
    if matches!(args.as_slice(), ["--help"] | ["help"] | ["-h"]) {
        println!("{}\n{USAGE}", cli::HELP);
        return ExitCode::SUCCESS;
    }
    if let ["infra", rest @ ..] = args.as_slice() {
        if matches!(rest, ["build-images", ..]) {
            if let Err(error) = lifecycle::install_signals() {
                return finish(Err(error));
            }
            let _lock = match lifecycle::Lock::acquire(&root().join("target/.xtask-artifacts.lock"))
            {
                Ok(lock) => lock,
                Err(error) => return finish(Err(error)),
            };
            return finish(infra::run(&root(), rest));
        }
        return finish(infra::run(&root(), rest));
    }
    let env = env::Env::load(&root());
    match cli::parse(&args) {
        Err(error) => {
            eprintln!("xtask: {error}\n{}", cli::HELP);
            return ExitCode::from(2);
        }
        Ok(Some(invocation)) => {
            if invocation.inspect {
                print!("{}", invocation.plan.describe());
                for key in ["VINK_BIND", "VINK_UI_PREVIEW_BIND"] {
                    let default = if key == "VINK_BIND" {
                        "127.0.0.1:3000"
                    } else {
                        "127.0.0.1:3019"
                    };
                    let address = env.get(key).unwrap_or_else(|| default.into());
                    println!(
                        "{key}: {}",
                        address
                            .parse::<std::net::SocketAddr>()
                            .map(|v| v.to_string())
                            .unwrap_or_else(|_| "<invalid address; value hidden>".into())
                    );
                }
                return ExitCode::SUCCESS;
            }
            if invocation.doctor {
                return finish(doctor::scoped(&invocation.plan, &env));
            }
            if let Err(error) = lifecycle::install_signals() {
                return finish(Err(error));
            }
            return finish(workflow::execute(&invocation.plan, &env));
        }
        Ok(None) => {}
    }
    // Reject legacy typos too, before locks or signal handlers are installed.
    if !matches!(
        args.as_slice(),
        ["decision", ..]
            | ["doctor"]
            | ["build"]
            | ["build", "--release"]
            | ["db", "migrate" | "seed"]
            | ["dev"]
            | ["ui-tokens"]
            | ["ui-tokens", "--check"]
            | [
                "ui-preview",
                "leptos" | "cmp" | "android" | "ios",
                "build" | "run" | "verify"
            ]
            | [
                "verify",
                "quick" | "discussion" | "expressions" | "kani" | "postgres" | "all"
            ]
    ) {
        return usage();
    }
    if let Err(error) = lifecycle::install_signals() {
        return finish(Err(error));
    }
    let _artifacts = if matches!(
        args.first(),
        Some(&"build" | &"dev" | &"verify" | &"ui-preview" | &"db")
    ) {
        match lifecycle::Lock::acquire(&root().join("target/.xtask-artifacts.lock")) {
            Ok(lock) => Some(lock),
            Err(error) => return finish(Err(error)),
        }
    } else {
        None
    };
    match args.as_slice() {
        ["decision", commands @ ..] => finish(decision::run(
            &root(),
            commands,
            &mut std::io::stdout().lock(),
        )),
        ["doctor"] => doctor::run(&env),
        ["build"] => finish(build::legacy(&env, false)),
        ["build", "--release"] => finish(build::legacy(&env, true)),
        ["db", "migrate"] => finish(legacy::migrate_owners(&env)),
        ["db", "seed"] => finish(legacy::app_command(&env, "dev-seed")),
        ["dev"] => finish(legacy::dev(&env)),
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
