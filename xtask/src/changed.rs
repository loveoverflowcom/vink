//! Diff-aware verification (#501). `verify changed --plan` reads pinned Git inputs and the
//! checkout's Cargo/Tailwind/Kani/browser-module facts, then selects existing xtask selectors
//! with a reason per changed path. `verify changed` computes the same plan and executes exactly
//! those selectors, stopping as soon as the inputs drift from the plan.
mod exec;
mod git;
mod graph;
mod rules;

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
    path::Path,
    process::ExitCode,
    time::{Duration, Instant},
};

use sha2::{Digest, Sha256};

/// The integration branch's remote-tracking ref; resolved locally, never fetched.
pub const DEFAULT_BASE: &str = "origin/develop";
const USAGE: &str = "usage: cargo xtask verify changed [--base REV] [--head REV | --worktree] [--only CHECK]... [--expect PLAN_ID] [--plan]";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Args {
    pub base: Option<String>,
    /// Committed scope ending at this revision; `None` is the worktree scope.
    pub head: Option<String>,
    pub plan: bool,
    pub only: Vec<String>,
    pub expect: Option<String>,
}

/// `cargo xtask verify static docs` and `verify  static docs` name the same check.
fn normalize(check: &str) -> String {
    let words: Vec<_> = check.split_ascii_whitespace().collect();
    let words = match words.as_slice() {
        ["cargo", "xtask", rest @ ..] => rest,
        rest => rest,
    };
    words.join(" ")
}

pub fn parse(args: &[&str]) -> Result<Args, String> {
    let mut parsed = Args::default();
    let mut worktree = false;
    let mut i = 0;
    while i < args.len() {
        let flag = args[i];
        let mut value = || {
            i += 1;
            args.get(i)
                .filter(|v| !v.trim().is_empty() && !v.starts_with("--"))
                .map(|v| v.to_string())
                .ok_or_else(|| format!("{flag} needs a value; {USAGE}"))
        };
        match flag {
            "--base" if parsed.base.is_none() => parsed.base = Some(value()?),
            "--head" if parsed.head.is_none() && !worktree => parsed.head = Some(value()?),
            "--worktree" if parsed.head.is_none() && !worktree => worktree = true,
            "--plan" if !parsed.plan => parsed.plan = true,
            "--expect" if parsed.expect.is_none() => parsed.expect = Some(value()?),
            "--only" => {
                let check = normalize(&value()?);
                if parsed.only.contains(&check) {
                    return Err(format!("duplicate --only `{check}`"));
                }
                parsed.only.push(check);
            }
            other => return Err(format!("unsupported or repeated option `{other}`; {USAGE}")),
        }
        i += 1;
    }
    Ok(parsed)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Cost {
    Static,
    Compile,
    Unit,
    Formal,
    Postgres,
    Browser,
    Lane,
}

impl Cost {
    fn describe(self) -> &'static str {
        match self {
            Self::Static => "static: xtask/Node checker, no product compile",
            Self::Compile => {
                "compile: cargo check of the selected target graph (cold minutes, warm seconds)"
            }
            Self::Unit => "unit: compile+link one owner test target, then run it",
            Self::Formal => "formal: pinned Quint (Node) or Kani/CBMC plus Rust replay",
            Self::Postgres => "postgres: test binaries plus a real disposable cluster",
            Self::Browser => "browser: WASM/bindgen/App (or Gradle) build plus Chrome per scenario",
            Self::Lane => {
                "full lane: workspace clippy (server + 3 WASM), doctests, unit and static"
            }
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Compile => "compile",
            Self::Unit => "unit",
            Self::Formal => "formal",
            Self::Postgres => "postgres",
            Self::Browser => "browser",
            Self::Lane => "lane",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Check {
    pub command: String,
    pub entry: rules::Entry,
    pub cost: Cost,
    pub needs: String,
}

/// Validates a selector against the real CLI parser and summarizes its prerequisites.
pub fn describe_check(command: &str) -> Result<(Cost, String), String> {
    let words: Vec<_> = command.split_ascii_whitespace().collect();
    let fixed = match words.as_slice() {
        ["verify", "quick"] => Some((Cost::Lane, "rustfmt, wasm32 target; no database or browser")),
        ["verify", "discussion"] => Some((
            Cost::Formal,
            "Node 22+/npm (pinned Quint); vink-app and vink-web replay tests",
        )),
        ["verify", "expressions"] => Some((
            Cost::Formal,
            "Node 22+/npm (pinned Quint); vink-expressions replay",
        )),
        ["verify", "kani"] => Some((Cost::Formal, "pinned cargo-kani")),
        ["verify", "postgres"] => Some((
            Cost::Postgres,
            "VINK_TEST_DATABASE_URL, psql 15+, pg_dump/pg_restore; four owners then lifecycle",
        )),
        ["ui-preview", "cmp", "verify"] => Some((
            Cost::Browser,
            "JDK 17 and the Gradle wrapper (Compose desktop preview)",
        )),
        _ => None,
    };
    if let Some((cost, needs)) = fixed {
        return Ok((cost, needs.to_string()));
    }
    let mut args = words.clone();
    args.push("--plan");
    let invocation = crate::cli::parse(&args)
        .map_err(|e| format!("`{command}`: {e}"))?
        .filter(|i| i.plan.request.operation == crate::plan::Operation::Verify)
        .ok_or_else(|| format!("`{command}` is not a verify selector"))?;
    let plan = invocation.plan;
    use crate::plan::Target as T;
    let cost = match plan.request.target {
        T::Static(_) => Cost::Static,
        T::Check(_, _) => Cost::Compile,
        T::Unit(_) => Cost::Unit,
        T::Postgres(_) => Cost::Postgres,
        _ => Cost::Browser,
    };
    let mut needs = vec![format!("tools {:?}", plan.tools)];
    if !plan.artifacts.is_empty() {
        needs.push(format!("artifacts {:?}", plan.artifacts));
    }
    if !plan.services.is_empty() {
        needs.push(format!("services {:?}", plan.services));
    }
    if !plan.databases.is_empty() {
        needs.push(format!("databases {:?}", plan.databases));
    }
    Ok((cost, needs.join("; ")))
}

/// Merges Web scenarios, folds subsumed commands into the broader one and validates every
/// selector through the real CLI parser. Ordered cheapest first.
pub fn checks(
    selection: &rules::Selection,
) -> Result<(Vec<Check>, BTreeMap<String, &'static str>), String> {
    let mut commands = selection.commands();
    let subsumed = rules::subsumed(&commands);
    for (cmd, by) in &subsumed {
        if let Some(entry) = commands.remove(cmd) {
            let target = commands
                .get_mut(*by)
                .expect("subsuming command is selected");
            for reason in entry.reasons {
                target.reasons.insert(format!("covers `{cmd}`: {reason}"));
            }
            target.paths.extend(entry.paths);
        }
    }
    let mut checks = Vec::new();
    for (command, entry) in commands {
        let (cost, needs) = describe_check(&command)
            .map_err(|e| format!("planner emitted an invalid selector: {e}"))?;
        checks.push(Check {
            command,
            entry,
            cost,
            needs,
        });
    }
    checks.sort_by(|a, b| (a.cost, &a.command).cmp(&(b.cost, &b.command)));
    Ok((checks, subsumed))
}

pub struct ChangedPlan {
    pub snapshot: git::Snapshot,
    pub base_defaulted: bool,
    pub selection: rules::Selection,
    pub checks: Vec<Check>,
    pub subsumed: BTreeMap<String, &'static str>,
    /// Short digest of the input fingerprint and the selected commands.
    pub id: String,
    pub graph: Option<String>,
}

impl ChangedPlan {
    pub fn is_no_work(&self) -> bool {
        self.snapshot.is_empty()
    }
}

/// Builds the plan. The graph loader runs only when the diff is nonempty.
pub fn compute(
    root: &Path,
    args: &Args,
    load: impl FnOnce() -> Result<graph::Graph, String>,
) -> Result<ChangedPlan, String> {
    let base_ref = args.base.as_deref().unwrap_or(DEFAULT_BASE);
    let snapshot = git::snapshot(root, base_ref, args.head.as_deref())?;
    let mut selection = rules::Selection::default();
    let mut summary = None;
    // The dependency scanner reads real Cargo manifests, source and module imports.
    // A different or dirty checkout cannot supply facts about a committed head.
    let foreign_graph = !snapshot.is_empty()
        && snapshot.mode == git::Mode::Committed
        && !git::clean_checkout_of(root, &snapshot.head)?;
    if foreign_graph {
        let reason = "committed dependency graph unavailable in this checkout; replan on its clean head for exact owner and integration targets";
        summary = Some(reason.into());
        for path in snapshot.changes.iter().flat_map(|change| &change.paths) {
            selection.fallback(reason, path);
        }
    } else if !snapshot.is_empty() {
        let graph = load()?;
        summary = Some(format!(
            "cargo metadata --no-deps --offline ({} workspace packages); {} embedded inputs; {} browser import edges; {} Tailwind entries; {} Kani inputs",
            graph.packages.len(),
            graph.includes.len(),
            graph.importers.values().map(BTreeSet::len).sum::<usize>(),
            graph.stylesheets.len(),
            graph.kani_inputs.len()
        ));
        for change in &snapshot.changes {
            for path in &change.paths {
                if change.gitlink {
                    selection
                        .fallback("submodule/gitlink content is outside this repository", path);
                }
                if change.status.starts_with(['T', 'U']) {
                    selection.fallback("file type changed or the index is unmerged", path);
                }
                rules::classify(&mut selection, &graph, path, &graph.includers(path));
            }
        }
        selection.expand_broad(&graph);
    }
    let (checks, subsumed) = checks(&selection)?;
    let mut hasher = Sha256::new();
    hasher.update(&snapshot.fingerprint);
    for check in &checks {
        hasher.update(format!("\n{}", check.command));
    }
    let id: String = hasher.finalize()[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok(ChangedPlan {
        base_defaulted: args.base.is_none(),
        snapshot,
        selection,
        checks,
        subsumed,
        id,
        graph: summary,
    })
}

fn list(paths: &BTreeSet<String>, limit: usize) -> String {
    let mut shown: Vec<_> = paths.iter().take(limit).map(String::as_str).collect();
    let more = paths.len().saturating_sub(limit);
    let suffix = if more > 0 {
        format!(" (+{more} more)")
    } else {
        String::new()
    };
    if shown.is_empty() {
        shown.push("—");
    }
    format!("{}{suffix}", shown.join(", "))
}

pub fn render(plan: &ChangedPlan, elapsed: Duration, executing: bool) -> String {
    let s = &plan.snapshot;
    let mut out = String::new();
    let scope = match s.mode {
        git::Mode::Worktree => {
            "worktree: merge-base..HEAD + staged + unstaged + non-ignored untracked"
        }
        git::Mode::Committed => "committed: merge-base..head commits only (dirty files excluded)",
    };
    let mode = match (plan.is_no_work(), executing) {
        (true, _) => "NO_WORK",
        (false, false) => "plan only; nothing executed",
        (false, true) => "executing the checks below in order",
    };
    writeln!(out, "verify changed plan {} — {mode}", plan.id).unwrap();
    writeln!(out, "scope: {scope}").unwrap();
    writeln!(
        out,
        "base: {} = {}{}",
        s.base_ref,
        s.base,
        if plan.base_defaulted {
            " (default --base; local ref, not fetched by xtask)"
        } else {
            ""
        }
    )
    .unwrap();
    let excluded = if s.base_only > 0 {
        format!("; {} commit(s) only on base are excluded", s.base_only)
    } else {
        String::new()
    };
    writeln!(
        out,
        "merge-base: {} ({} commit(s) to head{excluded})",
        s.merge_base, s.head_only
    )
    .unwrap();
    writeln!(out, "head: {} = {}", s.head_ref, s.head).unwrap();
    writeln!(out, "inputs fingerprint: {}", s.fingerprint).unwrap();
    if plan.is_no_work() {
        out.push_str("status: NO_WORK — no changed paths in this scope; no checks selected and no coverage established (this is not a PASS).\n");
        return out;
    }
    if let Some(graph) = &plan.graph {
        writeln!(out, "dependency facts: {graph}").unwrap();
    }
    writeln!(
        out,
        "\nchanged paths ({}):",
        s.changes.iter().map(|c| c.paths.len()).sum::<usize>()
    )
    .unwrap();
    for change in &s.changes {
        let owners: BTreeSet<_> = change
            .paths
            .iter()
            .flat_map(|p| plan.selection.owners.get(p).into_iter().flatten())
            .map(String::as_str)
            .collect();
        writeln!(
            out,
            "  {:<9} {:<4} {}  [{}]",
            change.layer.name(),
            change.status,
            change.paths.join(" -> "),
            owners.into_iter().collect::<Vec<_>>().join(", ")
        )
        .unwrap();
    }
    writeln!(
        out,
        "\nselected checks ({}), cheapest first:",
        plan.checks.len()
    )
    .unwrap();
    for (i, check) in plan.checks.iter().enumerate() {
        writeln!(out, "{:>3}. cargo xtask {}", i + 1, check.command).unwrap();
        writeln!(out, "     cost: {}", check.cost.describe()).unwrap();
        writeln!(out, "     needs: {}", check.needs).unwrap();
        for reason in check.entry.reasons.iter().take(6) {
            writeln!(out, "     why: {reason}").unwrap();
        }
        if check.entry.reasons.len() > 6 {
            writeln!(
                out,
                "     why: … {} more reason(s)",
                check.entry.reasons.len() - 6
            )
            .unwrap();
        }
        writeln!(out, "     from: {}", list(&check.entry.paths, 4)).unwrap();
    }
    if !plan.selection.artifacts.is_empty() {
        out.push_str(
            "\nartifacts the selected checks rebuild (verified receipts may be reused, #500):\n",
        );
        for (artifact, entry) in &plan.selection.artifacts {
            writeln!(
                out,
                "  - {artifact}: {} [{}]",
                list(&entry.reasons, 2),
                list(&entry.paths, 3)
            )
            .unwrap();
        }
    }
    if !plan.selection.manual.is_empty() {
        out.push_str("\nmanual obligations (no xtask selector; reported NOT_RUN on execution):\n");
        for (what, entry) in &plan.selection.manual {
            writeln!(out, "  - {what} [{}]", list(&entry.paths, 3)).unwrap();
        }
    }
    out.push_str("\nnot selected:\n");
    for (cmd, by) in &plan.subsumed {
        writeln!(out, "  - {cmd}: executed inside `{by}`").unwrap();
    }
    let selected = |cmd: &str| plan.checks.iter().any(|c| c.command == cmd);
    for (check, entry) in &plan.selection.consumers {
        let covered = selected(check)
            || (check.starts_with("verify web --scenario <") && selected("verify web"));
        if !covered {
            writeln!(
                out,
                "  - {check}: {} [{}]",
                list(&entry.reasons, 2),
                list(&entry.paths, 3)
            )
            .unwrap();
        }
    }
    for (lane, prefix) in [
        ("verify static", "verify static"),
        ("verify check", "verify check"),
        ("verify unit", "verify unit"),
        ("verify postgres", "verify postgres"),
        ("verify web", "verify web"),
        ("verify preview", "verify preview"),
        ("verify discussion", "verify discussion"),
        ("verify expressions", "verify expressions"),
        ("verify kani", "verify kani"),
        ("verify quick", "verify quick"),
        ("ui-preview cmp verify", "ui-preview cmp"),
    ] {
        let used = plan.checks.iter().any(|c| c.command.starts_with(prefix))
            || plan.subsumed.keys().any(|c| c.starts_with(prefix));
        if !used {
            writeln!(out, "  - {lane}: no changed input maps to this lane").unwrap();
        }
    }
    if plan.selection.unknown.is_empty() {
        out.push_str("\nunknown inputs: none\n");
    } else {
        writeln!(
            out,
            "\nunknown inputs ({}): {}",
            plan.selection.unknown.len(),
            list(&plan.selection.unknown, 8)
        )
        .unwrap();
    }
    if plan.selection.fallback.is_empty() {
        out.push_str("fallback: none\n");
    } else {
        for (reason, paths) in &plan.selection.fallback {
            writeln!(
                out,
                "fallback: {reason} [{}] -> every CI lane plus Web opt-ins",
                list(paths, 4)
            )
            .unwrap();
        }
    }
    let mut classes: BTreeMap<Cost, usize> = BTreeMap::new();
    for check in &plan.checks {
        *classes.entry(check.cost).or_default() += 1;
    }
    let classes: Vec<_> = classes
        .iter()
        .map(|(c, n)| format!("{n} {}", c.name()))
        .collect();
    writeln!(
        out,
        "cost: {} check(s): {}; highest class {}",
        plan.checks.len(),
        classes.join(", "),
        plan.checks
            .iter()
            .map(|c| c.cost)
            .max()
            .map_or("none", Cost::name)
    )
    .unwrap();
    writeln!(out, "planning took {:.3} s (Git + read-only manifest/entry scan; no build, network, database or browser)", elapsed.as_secs_f64()).unwrap();
    writeln!(
        out,
        "evidence: NOT_RUN. Execute this exact plan: cargo xtask verify changed{} --expect {}",
        plan.snapshot_args(),
        plan.id
    )
    .unwrap();
    out.push_str(
        "A selected PASS covers only these checks; it is not a full-lane or merge-gate PASS.\n",
    );
    out
}

impl ChangedPlan {
    fn snapshot_args(&self) -> String {
        let mut args = String::new();
        if !self.base_defaulted {
            write!(args, " --base {}", self.snapshot.base_ref).unwrap();
        }
        if self.snapshot.mode == git::Mode::Committed {
            write!(args, " --head {}", self.snapshot.head_ref).unwrap();
        }
        args
    }
}

/// Resolves `--only` against the plan: a typo, an unselected check or an empty plan is an error.
pub fn select_only(
    plan: &ChangedPlan,
    only: &[String],
) -> Result<Option<BTreeSet<String>>, String> {
    if only.is_empty() {
        return Ok(None);
    }
    if plan.checks.is_empty() {
        return Err("--only matched nothing: this scope selects no checks (NO_WORK)".into());
    }
    let selected: Vec<_> = plan.checks.iter().map(|c| c.command.as_str()).collect();
    let mut chosen = BTreeSet::new();
    for check in only {
        if selected.contains(&check.as_str()) {
            chosen.insert(check.clone());
        } else if let Some(by) = plan.subsumed.get(check) {
            return Err(format!(
                "`{check}` runs inside `{by}` in this plan; pass --only \"{by}\""
            ));
        } else if let Err(error) = describe_check(check) {
            return Err(format!("unknown check {error}"));
        } else {
            return Err(format!(
                "`{check}` is a valid selector but this diff does not select it; selected: {selected:?}"
            ));
        }
    }
    Ok(Some(chosen))
}

pub fn run(root: &Path, raw: &[&str]) -> ExitCode {
    let args = match parse(raw) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("xtask: {error}");
            return ExitCode::from(2);
        }
    };
    let started = Instant::now();
    let plan = match compute(root, &args, || graph::Graph::load(root)) {
        Ok(plan) => plan,
        Err(error) => {
            eprintln!("xtask: {error}");
            return ExitCode::FAILURE;
        }
    };
    let elapsed = started.elapsed();
    let stale = args
        .expect
        .as_ref()
        .filter(|expected| **expected != plan.id);
    let only = select_only(&plan, &args.only);
    let runs = !args.plan && stale.is_none() && only.is_ok();
    print!("{}", render(&plan, elapsed, runs));
    if let Some(expected) = stale {
        eprintln!(
            "xtask: plan changed since it was reviewed: expected {expected}, current {}; review the plan above and pass its id",
            plan.id
        );
        return ExitCode::FAILURE;
    }
    let only = match only {
        Ok(only) => only,
        Err(error) => {
            eprintln!("xtask: {error}");
            return ExitCode::from(2);
        }
    };
    if args.plan {
        return ExitCode::SUCCESS;
    }
    exec::execute(root, &args, &plan, only.as_ref())
}

/// `--from-diff` suffix kept from #512: revalidates one explicit selector against the current
/// diff before the normal dispatcher runs it. `verify changed --only` supersedes it.
pub fn revalidate(
    root: &Path,
    command: &[&str],
    base: &str,
    head: &str,
    mode: &str,
) -> Result<(), String> {
    if !matches!(mode, "committed" | "worktree") {
        return Err("expected --from-diff BASE_SHA HEAD_SHA committed|worktree".into());
    }
    if git::commit(root, base)? != base || git::commit(root, "HEAD")? != head {
        return Err("pinned base or checked-out HEAD differs; generate a new diff plan".into());
    }
    let args = Args {
        base: Some(base.into()),
        ..Args::default()
    };
    let plan = compute(root, &args, || graph::Graph::load(root))?;
    if mode == "committed"
        && plan
            .snapshot
            .changes
            .iter()
            .any(|c| c.layer != git::Layer::Committed)
    {
        return Err("committed plan has dirty inputs; replan with --worktree".into());
    }
    let wanted = normalize(&command.join(" "));
    if !plan.checks.iter().any(|c| c.command == wanted) {
        return Err("command is not selected by the current diff; generate a new plan".into());
    }
    println!(
        "revalidated {mode} at {head} (plan {}); executing only: {wanted}",
        plan.id
    );
    Ok(())
}

#[cfg(test)]
mod tests;
