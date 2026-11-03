//! The structural D2T-01 model and real Rust seam share one local/CI command.
//! npm installs locked tool dependencies, never Rust toolchains or a model-checker backend.

use std::process::Command;

use crate::process::{self, Report, cargo};

const MODEL_DIR: &str = "services/vink-app/behavior/discussion";

pub fn verify(report: &mut Report) -> bool {
    let root = crate::root();
    let dir = root.join(MODEL_DIR);
    if !report.step("discussion: Node.js 22+ and npm", || {
        crate::doctor::node_version()?;
        crate::doctor::npm_version()?;
        Ok(())
    }) {
        return false;
    }
    let mut install = Command::new("npm");
    install
        .current_dir(&dir)
        .args(["ci", "--ignore-scripts", "--no-audit", "--no-fund"]);
    if !report.step("discussion: locked Quint dependencies", || {
        process::run(&mut install, "install pinned structural-model tools")
    }) {
        return false;
    }
    let mut model = Command::new("node");
    model.current_dir(&dir).arg("verify.mjs");
    if !report.step("discussion: Quint structural model and trace drift", || {
        process::run(
            &mut model,
            "verify the structural model and generated traces",
        )
    }) {
        return false;
    }
    let mut replay = cargo(&root);
    replay.args([
        "test",
        "--locked",
        "-p",
        "vink-app",
        "--lib",
        "domain::discussion::tests",
    ]);
    report.step(
        "discussion: production Rust structural tests and trace replay",
        || {
            process::run(
                &mut replay,
                "replay model inputs through the real Rust creation seam",
            )
        },
    )
}
