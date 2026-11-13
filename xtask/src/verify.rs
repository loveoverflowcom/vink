//! Existing regression lanes; selected checks live with their owning adapter.
use crate::process::{Report, cargo};
use crate::{
    TEST_DATABASE_URL, TEST_DATABASE_URL_MISSING, checks, discussion, doctor, env, expressions,
    kani, lifecycle, process, provision, root, ui_tokens, web,
};
use std::process::ExitCode;
#[derive(Clone, Copy)]
pub enum Lane {
    Quick,
    Discussion,
    Expressions,
    /// Not part of `verify all`: it needs a separately installed, pinned verifier.
    Kani,
    Postgres,
    Web,
}

pub fn verify(lanes: &[Lane], env: &env::Env) -> ExitCode {
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
        "repository checks (decisions, skills, links, boundaries, fonts, editor packages)",
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
    let _scratch_lock = match lifecycle::ScratchLock::acquire(&url) {
        Ok(lock) => lock,
        Err(e) => {
            report.fail("scratch ownership", &e);
            return false;
        }
    };
    // Each SQLx test provisions its own database and pool. Bound libtest's scheduling
    // so fewer of those databases are active at once.
    let test_threads = env.get("RUST_TEST_THREADS").unwrap_or_else(|| "2".into());
    let threads_arg = format!("--test-threads={test_threads}");
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
        tests
            .args(["test", "--locked", "-p", package, "--test", "postgres"])
            .args(["--", &threads_arg]);
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
        lifecycle::config_rejection(&root)
    }) && provision::verify(report, &root, &url)
}
