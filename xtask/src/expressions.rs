//! Owner-local expression catalog/runtime/model checks. No discussion state or I/O proof.
use crate::process::{self, Report, cargo};
use std::path::Path;
use std::process::Command;
const WEB: &str = "apps/vink-web/expressions";
const MODEL: &str = "packages/expressions/behavior";

pub fn check(root: &Path) -> Result<(), String> {
    crate::doctor::node_version()?;
    let mut node = Command::new("node");
    node.current_dir(root)
        .args(["--test", "packages/expressions/validate.test.mjs"]);
    process::run(
        &mut node,
        "expression manifest, Unicode, safety and generated drift",
    )
}

fn install(root: &Path, relative: &str) -> Result<(), String> {
    let dir = root.join(relative);
    let lock = std::fs::read(dir.join("package-lock.json")).map_err(|e| e.to_string())?;
    let stamp = dir.join("node_modules/.vink-installed-lock.json");
    if std::fs::read(&stamp).ok().as_ref() != Some(&lock) {
        let mut npm = Command::new("npm");
        npm.current_dir(&dir)
            .args(["ci", "--ignore-scripts", "--no-audit", "--no-fund"]);
        process::run(&mut npm, "expression tools: locked npm dependencies")?;
        std::fs::write(stamp, lock).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn build(root: &Path, out: &Path) -> Result<(), String> {
    install(root, WEB)?;
    let mut node = Command::new("node");
    node.current_dir(root.join(WEB)).arg("build.mjs").arg(out);
    process::run(
        &mut node,
        "expression runtime: pinned, self-hosted software renderer",
    )
}

pub fn verify(report: &mut Report) -> bool {
    let root = crate::root();
    if !report.step("expressions: manifest/hash/Unicode/drift", || check(&root)) {
        return false;
    }
    if !report.step("expressions: pinned Quint dependencies", || {
        install(&root, MODEL)
    }) {
        return false;
    }
    let mut model = Command::new("node");
    model.current_dir(root.join(MODEL)).arg("verify.mjs");
    if !report.step(
        "expressions: Quint simulation, trace drift and negative control",
        || process::run(&mut model, "expression lifecycle model"),
    ) {
        return false;
    }
    let mut replay = cargo(&root);
    replay.args(["test", "--locked", "-p", "vink-expressions", "--lib"]);
    report.step(
        "expressions: production Rust policy and trace replay",
        || process::run(&mut replay, "expression production seam"),
    )
}
