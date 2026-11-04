//! Owner-local Kani harnesses. xtask never installs the verifier or its bundled toolchain; CI
//! installs the pinned release first. Each manifest compiles production sources in place. The
//! lane requires every `#[kani::proof]` in the harness file to be verified, so a harness module
//! that is compiled out or a manifest that finds nothing cannot pass silently.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::Stdio;

use crate::process::{Report, cargo};

pub const VERSION: &str = "0.67.0";

/// Each owner-local manifest and the file holding its harnesses.
pub const MANIFESTS: [(&str, &str); 2] = [
    (
        "packages/expressions/kani/Cargo.toml",
        "packages/expressions/src/proofs.rs",
    ),
    (
        "services/vink-app/kani/Cargo.toml",
        "services/vink-app/src/domain/discussion/proofs.rs",
    ),
];

pub fn verify(report: &mut Report) -> bool {
    let root = crate::root();
    if !report.step(&format!("kani: cargo-kani {VERSION}"), || version(&root)) {
        return false;
    }
    for (manifest, harnesses) in MANIFESTS {
        let name = format!("kani: {manifest}");
        if !report.step(&name, || run(&root, manifest, harnesses)) {
            return false;
        }
    }
    true
}

fn version(root: &Path) -> Result<(), String> {
    let output = cargo(root)
        .args(["kani", "--version"])
        .output()
        .map_err(|e| format!("cannot start cargo: {e}"))?;
    let printed = String::from_utf8_lossy(&output.stdout);
    let found = printed.trim();
    let install =
        format!("cargo install --locked kani-verifier --version {VERSION} && cargo kani setup");
    if output.status.success() && found == format!("cargo-kani {VERSION}") {
        Ok(())
    } else if output.status.success() {
        Err(format!(
            "found `{found}`, the pin is {VERSION}; reinstall with `{install}`"
        ))
    } else {
        Err(format!("cargo-kani is not installed; run `{install}`"))
    }
}

/// Streams Kani's output and requires its own summary to count exactly the harnesses in the
/// source file, all verified.
fn run(root: &Path, manifest: &str, harnesses: &str) -> Result<(), String> {
    let source = std::fs::read_to_string(root.join(harnesses))
        .map_err(|e| format!("cannot read {harnesses}: {e}"))?;
    let expected = source.matches("#[kani::proof]").count();
    if expected == 0 {
        return Err(format!("{harnesses} declares no #[kani::proof] harness"));
    }
    println!("==> cargo kani --manifest-path {manifest} ({expected} harnesses)");
    let mut child = cargo(root)
        .args(["kani", "--manifest-path", manifest])
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot start cargo kani: {e}"))?;
    let mut summary = None;
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            println!("{line}");
            if line.starts_with("Complete - ") {
                summary = Some(line);
            }
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    let wanted = format!(
        "Complete - {expected} successfully verified harnesses, 0 failures, {expected} total."
    );
    match summary {
        _ if !status.success() => Err(format!("cargo kani exited with {status}")),
        Some(line) if line == wanted => Ok(()),
        Some(line) => Err(format!("expected `{wanted}`, Kani reported `{line}`")),
        None => Err(format!(
            "Kani verified nothing, though {harnesses} declares {expected} harnesses"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_manifest_and_harness_file_exists() {
        let root = crate::root();
        for (manifest, harnesses) in MANIFESTS {
            assert!(root.join(manifest).is_file(), "{manifest}");
            let source = std::fs::read_to_string(root.join(harnesses)).unwrap();
            assert!(source.contains("#[kani::proof]"), "{harnesses}");
        }
    }
}
