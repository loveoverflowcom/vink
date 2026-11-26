//! Actual binary boundary: inspection and parse rejection must not run even a tool probe.
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
#[test]
fn inspection_and_invalid_selection_never_call_executor_or_reveal_credentials() {
    let dir = std::env::temp_dir().join(format!("vink-cli-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let marker = dir.join("called");
    for tool in [
        "cargo",
        "rustc",
        "node",
        "npm",
        "wasm-bindgen",
        "psql",
        "google-chrome",
    ] {
        let path = dir.join(tool);
        std::fs::write(
            &path,
            format!("#!/bin/sh\n: > '{}'\nexit 99\n", marker.display()),
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    for (args, code) in [
        (vec!["dev", "api", "--plan"], 0),
        (vec!["verify", "web", "--scenario", "restart", "--plan"], 0),
        (vec!["verify", "static", "docs", "--plan"], 0),
        (
            vec!["verify", "check", "web", "--target", "hydrate", "--plan"],
            0,
        ),
        (
            vec![
                "verify",
                "postgres",
                "store",
                "--filter",
                "fresh_database",
                "--plan",
            ],
            0,
        ),
        (vec!["verify", "postgres", "--plan"], 0),
        (vec!["verify", "web", "--scenario", ""], 2),
        (vec!["verify", "postgres", "lifecycle", "--filter", "x"], 2),
        (vec!["verify", "check", "store", "--target", "csr"], 2),
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_xtask"))
            .args(args)
            .env("PATH", &dir)
            .env("CARGO", dir.join("cargo"))
            // Documented local-only placeholder; the assertion below still detects its marker.
            .env(
                "DATABASE_URL",
                "postgres://secret:private-marker-local-only@host/dev",
            )
            .env(
                "VINK_TEST_DATABASE_URL",
                "postgres://private-marker@host/test",
            )
            .env("VINK_BIND", "private-marker")
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(code),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(!String::from_utf8_lossy(&out.stdout).contains("private-marker"));
        assert!(!marker.exists(), "inspection/parse unexpectedly ran a tool");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn changed_plan_and_guard_reject_redirected_git_inputs() {
    let bin = env!("CARGO_BIN_EXE_xtask");
    for key in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
    ] {
        for args in [
            &[
                "verify",
                "changed",
                "--base",
                "HEAD",
                "--worktree",
                "--plan",
            ][..],
            &[
                "verify",
                "static",
                "docs",
                "--from-diff",
                "HEAD",
                "HEAD",
                "committed",
            ],
        ] {
            let out = Command::new(bin)
                .args(args)
                .env(key, "a-different-checkout-or-index")
                .output()
                .unwrap();
            assert!(!out.status.success());
            assert!(
                String::from_utf8_lossy(&out.stderr)
                    .contains(&format!("{key} overrides Git inputs"))
            );
            assert!(out.stdout.is_empty());
        }
    }
}

#[test]
fn changed_reports_no_work_and_rejects_invalid_selection_before_execution() {
    let bin = env!("CARGO_BIN_EXE_xtask");
    let run = |args: &[&str]| {
        let out = Command::new(bin).args(args).output().unwrap();
        (
            out.status.code(),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    };
    let empty = ["verify", "changed", "--base", "HEAD", "--head", "HEAD"];
    let (code, stdout, _) = run(&[&empty[..], &["--plan"]].concat());
    assert_eq!(code, Some(0));
    assert!(stdout.contains("NO_WORK") && stdout.contains("not a PASS"));
    let (code, stdout, _) = run(&empty);
    assert_eq!(code, Some(3), "NO_WORK must not exit like a PASS");
    assert!(stdout.contains("overall: NO_WORK") && !stdout.contains("overall: PASS"));

    let (code, _, stderr) = run(&["verify", "changed", "--base", "no-such-ref", "--plan"]);
    assert_eq!(code, Some(1));
    assert!(stderr.contains("unknown revision `no-such-ref`"));
    let (code, _, _) = run(&["verify", "changed", "--head", "HEAD", "--worktree"]);
    assert_eq!(code, Some(2));

    // A real committed range: these all fail before any selected check could start.
    let parent = Command::new("git")
        .args(["rev-parse", "--verify", "--quiet", "HEAD~1"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    if !parent.status.success() {
        eprintln!("shallow checkout without HEAD~1: committed-range assertions NOT_RUN");
        return;
    }
    let last = ["verify", "changed", "--base", "HEAD~1", "--head", "HEAD"];
    let (code, stdout, stderr) = run(&[&last[..], &["--only", "verify unit apx"]].concat());
    assert_eq!(code, Some(2), "{stderr}");
    assert!(stderr.contains("unknown check") && !stdout.contains("==> ["));
    let (code, stdout, stderr) = run(&[&last[..], &["--expect", "0000000000000000"]].concat());
    assert_eq!(code, Some(1));
    assert!(stderr.contains("plan changed since it was reviewed") && !stdout.contains("==> ["));
}
