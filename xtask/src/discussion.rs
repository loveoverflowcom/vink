//! The structural D2T-01, lifecycle D2T-02 (with submission receipts), reaction D2T-INV-08,
//! literary endorsement LVE (#73) and admission/post-review (#120) models and their real Rust
//! seams share one local/CI command.
//! npm installs locked tool dependencies, never Rust toolchains or a model-checker backend.
//! The canonical publication model and remaining target specifications share the Quint pin.
//! Publication author actions, evaluator/moderation decisions and scripted mock vectors have
//! scoped Rust replay; delete/remove lifecycle actions and the remaining target models have
//! model evidence only.

use std::process::Command;

use crate::process::{self, Report, cargo};

const MODEL_DIR: &str = "services/vink-app/behavior/discussion";
const TARGET_DIR: &str = "services/vink-app/behavior/target";
const FILTERS: [&str; 13] = [
    "domain::discussion::tests",
    "domain::discussion::admission::tests",
    "domain::discussion::admission_replay",
    "domain::discussion::outbox::tests",
    "domain::discussion::outbox_replay",
    "domain::discussion::post_review",
    "domain::discussion::lifecycle_replay",
    "domain::discussion::reaction::tests",
    "domain::discussion::reaction_replay",
    "domain::endorsement::",
    "domain::moderation::",
    "domain::publication::",
    "application::moderation::",
];
/// The tests that feed model inputs to production code. Their absence fails the lane.
const REPLAYS: [&str; 15] = [
    "domain::publication::replay::quint_publication_author_inputs_and_admitted_decision_effects_replay_through_production",
    "domain::publication::replay::the_withdrawn_intent_counterexample_is_refused_by_the_production_publication_guard",
    "domain::moderation::replay::quint_evaluator_vectors_replay_publication_steps_and_check_comment_contract",
    "domain::moderation::replay::the_admin_late_result_counterexample_is_refused_by_the_production_moderation_guard",
    "domain::discussion::tests::quint_generated_inputs_replay_against_the_production_creation_seam",
    "domain::discussion::lifecycle_replay::quint_lifecycle_inputs_replay_against_the_production_decisions",
    "domain::discussion::lifecycle_replay::the_no_op_mode_counterexample_is_refused_by_production",
    "domain::discussion::reaction_replay::quint_reaction_inputs_replay_against_the_production_decisions",
    "domain::discussion::reaction_replay::the_no_op_reaction_counterexample_is_refused_by_production",
    "domain::endorsement::replay::quint_endorsement_inputs_replay_against_production",
    "domain::discussion::admission_replay::quint_admission_inputs_replay_against_the_production_decisions",
    "domain::discussion::admission_replay::the_admission_counterexamples_are_refused_by_production",
    "domain::discussion::outbox_replay::quint_outbox_inputs_replay_against_the_production_decisions",
    "domain::discussion::outbox_replay::the_unfenced_completion_counterexample_is_refused_by_production",
    "domain::discussion::post_review_replay::quint_operator_review_inputs_replay_through_production_guards",
];

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
    if !report.step(
        "discussion: Quint structure/lifecycle/reaction/endorsement/admission/outbox/post-review models and trace drift",
        || {
            process::run(
                &mut model,
                "verify the discussion models and generated traces",
            )
        },
    ) {
        return false;
    }
    let mut target = Command::new("node");
    target.current_dir(root.join(TARGET_DIR)).arg("verify.mjs");
    if !report.step(
        "discussion: publication canonical model + remaining target specifications and vector drift",
        || {
            process::run(
                &mut target,
                "verify the target specifications and generated vectors",
            )
        },
    ) {
        return false;
    }
    let replay = || {
        let mut command = cargo(&root);
        command.args(["test", "--locked", "-p", "vink-app", "--lib", "--"]);
        command.args(FILTERS);
        command
    };
    report.step(
        "discussion: production Rust structural tests and trace replays",
        || {
            process::require_tests(&mut replay(), "discussion replays", &REPLAYS)?;
            process::run(
                &mut replay(),
                "replay model inputs through the real Rust decisions",
            )
        },
    )
}
