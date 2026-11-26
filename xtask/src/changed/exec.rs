//! Executes the computed plan through the existing dispatchers. Inputs are re-fingerprinted
//! before every check and once after the last one; any drift stops the run.
use super::{Args, ChangedPlan, Check, git};
use crate::{cli, env::Env, lifecycle, process, root, ui_preview, verify, workflow};
use std::{collections::BTreeSet, path::Path, process::ExitCode, time::Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Pass,
    Fail,
    NotRun,
    Unsupported,
}

impl Status {
    pub fn name(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::NotRun => "NOT_RUN",
            Self::Unsupported => "UNSUPPORTED",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub command: String,
    pub status: Status,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overall {
    Pass,
    Fail,
    Partial,
    NoWork,
}

impl Overall {
    pub fn name(self) -> &'static str {
        match self {
            Self::Pass => "PASS (selected checks only; not a full-lane or merge-gate PASS)",
            Self::Fail => "FAIL",
            Self::Partial => "PARTIAL (some selected checks NOT_RUN or UNSUPPORTED; not a PASS)",
            Self::NoWork => "NO_WORK (nothing selected or executed; not a PASS)",
        }
    }

    pub fn code(self) -> ExitCode {
        match self {
            Self::Pass => ExitCode::SUCCESS,
            Self::Fail => ExitCode::FAILURE,
            Self::Partial | Self::NoWork => ExitCode::from(3),
        }
    }
}

/// Browser and Gradle lanes are UNSUPPORTED inside the local resource launcher (#250); the
/// planner must not bypass that policy by running them from a supported parent command.
pub fn unsupported(check: &Check, locally_managed: bool) -> Option<&'static str> {
    let browser = check.command.starts_with("verify web")
        || check.command.starts_with("verify preview")
        || check.command.starts_with("ui-preview");
    (locally_managed && browser).then_some(
        "browser/Gradle process ownership is UNSUPPORTED under tools/local-budget; run this command directly and sequentially",
    )
}

/// Runs selected checks in plan order. `drift` returns `Some(reason)` when the inputs no
/// longer match the plan. Checks after a failure or drift are NOT_RUN.
pub fn run_checks(
    plan: &ChangedPlan,
    only: Option<&BTreeSet<String>>,
    drift: &mut dyn FnMut() -> Result<Option<String>, String>,
    runner: &mut dyn FnMut(&Check) -> Result<(), String>,
    unsupported: &dyn Fn(&Check) -> Option<&'static str>,
) -> (Vec<Outcome>, Overall) {
    let mut outcomes = Vec::new();
    let mut stop: Option<String> = None;
    let mut executed = false;
    let mut failed = false;
    for (i, check) in plan.checks.iter().enumerate() {
        let outcome = |status, detail: String| Outcome {
            command: check.command.clone(),
            status,
            detail,
        };
        if let Some(reason) = &stop {
            outcomes.push(outcome(Status::NotRun, reason.clone()));
            continue;
        }
        if only.is_some_and(|o| !o.contains(&check.command)) {
            outcomes.push(outcome(Status::NotRun, "excluded by --only".into()));
            continue;
        }
        match drift() {
            Ok(None) => {}
            Ok(Some(reason)) | Err(reason) => {
                failed = true;
                let reason = format!("inputs drifted before this check: {reason}; replan");
                eprintln!("xtask: {reason}");
                outcomes.push(outcome(Status::NotRun, reason.clone()));
                stop = Some("not run after input drift".into());
                continue;
            }
        }
        if let Some(reason) = unsupported(check) {
            outcomes.push(outcome(Status::Unsupported, reason.into()));
            continue;
        }
        if lifecycle::cancelled() {
            stop = Some("not run after cancellation".into());
            outcomes.push(outcome(Status::NotRun, "cancelled".into()));
            continue;
        }
        println!(
            "\n==> [{}/{}] cargo xtask {}",
            i + 1,
            plan.checks.len(),
            check.command
        );
        let started = Instant::now();
        executed = true;
        let result = runner(check);
        let seconds = started.elapsed().as_secs_f64();
        match result {
            Ok(()) => outcomes.push(outcome(Status::Pass, format!("{seconds:.1} s"))),
            Err(error) => {
                failed = true;
                outcomes.push(outcome(Status::Fail, format!("{seconds:.1} s: {error}")));
                stop = Some("not run after an earlier failure".into());
            }
        }
    }
    if executed && !failed {
        match drift() {
            Ok(None) => {}
            Ok(Some(reason)) | Err(reason) => {
                failed = true;
                outcomes.push(Outcome {
                    command: "inputs after the last check".into(),
                    status: Status::Fail,
                    detail: format!(
                        "{reason}: results apply to an unknown mixture of inputs; replan"
                    ),
                });
            }
        }
    }
    let overall = if failed {
        Overall::Fail
    } else if outcomes.iter().all(|o| o.status == Status::Pass) && !outcomes.is_empty() {
        Overall::Pass
    } else {
        Overall::Partial
    };
    (outcomes, overall)
}

/// Mirrors main's dispatch for every command the planner can emit.
fn dispatch(command: &str, env: &Env) -> Result<(), String> {
    use verify::Lane;
    let words: Vec<_> = command.split_ascii_whitespace().collect();
    let lane = match words.as_slice() {
        ["verify", "quick"] => Some(Lane::Quick),
        ["verify", "discussion"] => Some(Lane::Discussion),
        ["verify", "expressions"] => Some(Lane::Expressions),
        ["verify", "kani"] => Some(Lane::Kani),
        ["verify", "postgres"] => Some(Lane::Postgres),
        _ => None,
    };
    let artifacts = || lifecycle::Lock::acquire(&root().join("target/.xtask-artifacts.lock"));
    if let Some(lane) = lane {
        let _lock = artifacts()?;
        return (verify::verify(&[lane], env) == ExitCode::SUCCESS)
            .then_some(())
            .ok_or_else(|| format!("{command} failed; see its summary above"));
    }
    if words == ["ui-preview", "cmp", "verify"] {
        let _lock = artifacts()?;
        return (ui_preview::verify(env, "cmp") == ExitCode::SUCCESS)
            .then_some(())
            .ok_or_else(|| format!("{command} failed; see its output above"));
    }
    match cli::parse(&words)? {
        Some(invocation) if !invocation.inspect && !invocation.doctor => {
            workflow::execute(&invocation.plan, env)
        }
        _ => Err(format!("`{command}` has no executor")),
    }
}

/// Recomputes the snapshot with the plan's own refs and compares fingerprints.
pub(super) fn drift_of(
    root: &Path,
    args: &Args,
    plan: &ChangedPlan,
) -> Result<Option<String>, String> {
    let base = args.base.as_deref().unwrap_or(super::DEFAULT_BASE);
    let now = git::snapshot(root, base, args.head.as_deref())?;
    Ok((now.fingerprint != plan.snapshot.fingerprint).then(|| {
        format!(
            "fingerprint {} -> {} (base {}, head {})",
            &plan.snapshot.fingerprint[..12],
            &now.fingerprint[..12],
            now.base,
            now.head
        )
    }))
}

/// A committed-scope plan describes commits only: it runs on a clean checkout of `head`.
fn committed_tree_matches(root: &Path, plan: &ChangedPlan) -> Result<(), String> {
    if !git::clean_checkout_of(root, &plan.snapshot.head)? {
        return Err(
            "a committed-scope plan executes only on a clean checkout of its head; omit --head to plan the worktree".into(),
        );
    }
    Ok(())
}

pub fn summary(plan: &ChangedPlan, outcomes: &[Outcome], overall: Overall) -> String {
    let mut out = format!("\nverify changed summary (plan {})\n", plan.id);
    for o in outcomes {
        out.push_str(&format!(
            "  {:<11} {}: {}\n",
            o.status.name(),
            o.command,
            o.detail
        ));
    }
    for what in plan.selection.manual.keys() {
        out.push_str(&format!("  {:<11} manual: {what}\n", Status::NotRun.name()));
    }
    out.push_str(&format!("overall: {}\n", overall.name()));
    out
}

pub fn execute(
    root: &Path,
    args: &Args,
    plan: &ChangedPlan,
    only: Option<&BTreeSet<String>>,
) -> ExitCode {
    if plan.checks.is_empty() {
        // Changed inputs whose only obligations are manual are not "no work".
        let overall = if plan.is_no_work() {
            Overall::NoWork
        } else {
            Overall::Partial
        };
        print!("{}", summary(plan, &[], overall));
        return overall.code();
    }
    let prepared = if plan.snapshot.mode == git::Mode::Committed {
        committed_tree_matches(root, plan)
    } else {
        Ok(())
    }
    .and_then(|()| lifecycle::install_signals());
    if let Err(error) = prepared {
        eprintln!("xtask: {error}");
        return ExitCode::FAILURE;
    }
    let env = Env::load(root);
    let managed = process::locally_managed();
    let committed = plan.snapshot.mode == git::Mode::Committed;
    let (outcomes, overall) = run_checks(
        plan,
        only,
        &mut || match committed.then(|| committed_tree_matches(root, plan)) {
            Some(Err(reason)) => Ok(Some(reason)),
            _ => drift_of(root, args, plan),
        },
        &mut |check| dispatch(&check.command, &env),
        &|check| unsupported(check, managed),
    );
    print!("{}", summary(plan, &outcomes, overall));
    overall.code()
}
