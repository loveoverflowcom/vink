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
        (vec!["verify", "web", "--scenario", ""], 2),
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_xtask"))
            .args(args)
            .env("PATH", &dir)
            .env("CARGO", dir.join("cargo"))
            .env("DATABASE_URL", "postgres://secret:private-marker@host/dev")
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
