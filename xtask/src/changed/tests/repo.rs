//! Disposable Git repositories with real repository paths. The dependency graph is the real
//! checkout's; Git inputs come from the fixture. No fetch, build or network.
use super::super::exec::{self, Overall, Status};
use super::super::*;
use super::graph;
use std::{
    fs,
    path::{Path, PathBuf},
};

struct Repo(PathBuf);

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl Repo {
    fn new(name: &str) -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "vink-changed-{name}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        let repo = Self(dir);
        repo.git(&["init", "--quiet"]);
        repo.git(&["config", "user.name", "fixture"]);
        repo.git(&["config", "user.email", "fixture@invalid.example"]);
        repo.git(&["config", "commit.gpgsign", "false"]);
        repo
    }

    fn git(&self, args: &[&str]) -> String {
        String::from_utf8(git::git(&self.0, args).unwrap()).unwrap()
    }

    fn write(&self, path: &str, content: &str) {
        let file = self.0.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, content).unwrap();
    }

    fn commit(&self, message: &str) -> String {
        self.git(&["add", "-A"]);
        self.git(&["commit", "--quiet", "--allow-empty", "-m", message]);
        self.git(&["rev-parse", "HEAD"]).trim().to_string()
    }

    fn plan(&self, args: &[&str]) -> Result<ChangedPlan, String> {
        compute(&self.0, &parse(args)?, || Ok(graph().clone()))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

fn rows(plan: &ChangedPlan) -> Vec<String> {
    plan.snapshot
        .changes
        .iter()
        .map(|c| {
            format!(
                "{} {} {}",
                c.layer.name(),
                &c.status[..1],
                c.paths.join(" -> ")
            )
        })
        .collect()
}

fn commands(plan: &ChangedPlan) -> Vec<&str> {
    plan.checks.iter().map(|c| c.command.as_str()).collect()
}

const DOMAIN: &str = "services/vink-app/src/domain/discussion/reaction.rs";
const PAGINATION: &str = "apps/vink-web/e2e/draft-pagination-smoke.mjs";
/// A cross-area move: the old name is pure domain (Kani), the new one is application code.
const OLD: &str = "services/vink-app/src/domain/draft.rs";
const MOVED: &str = "services/vink-app/src/application/draft_moved.rs";
const MIGRATION: &str = "services/vink-identity/migrations/20261120000000_resend_payload_fence.sql";
const CSS: &str = "apps/vink-web/style/vink.css";
const CONTRACT: &str = "services/vink-app/contracts/http/v1/README.md";

/// A base on `origin/develop`, a feature branch with committed changes and a base that
/// advanced after the branch point.
fn branched() -> (Repo, String, String) {
    let repo = Repo::new("matrix");
    repo.write(".gitignore", "ignored/\n");
    repo.write("AGENTS.md", "policy\n");
    for path in [DOMAIN, PAGINATION, OLD, MIGRATION, CSS] {
        repo.write(path, &format!("base {path}\n"));
    }
    let fork = repo.commit("base");
    repo.git(&["update-ref", "refs/remotes/origin/develop", &fork]);
    repo.write(DOMAIN, "changed domain\n");
    fs::create_dir_all(repo.path().join(MOVED).parent().unwrap()).unwrap();
    repo.git(&["mv", OLD, MOVED]);
    repo.git(&["rm", "--quiet", MIGRATION]);
    let head = repo.commit("feature");
    // Advance the base on a side branch: its commit must stay out of the diff.
    repo.git(&["checkout", "--quiet", "-b", "side", &fork]);
    repo.write("docs/only-on-base.md", "base-only\n");
    let advanced = repo.commit("base advanced");
    repo.git(&["update-ref", "refs/remotes/origin/develop", &advanced]);
    repo.git(&["checkout", "--quiet", &head]);
    (repo, fork, head)
}

#[test]
fn empty_scope_is_no_work_without_loading_the_graph() {
    let repo = Repo::new("empty");
    repo.write("AGENTS.md", "policy\n");
    let head = repo.commit("base");
    repo.git(&["update-ref", "refs/remotes/origin/develop", &head]);
    for args in [&["--plan"][..], &["--base", "HEAD", "--head", "HEAD"]] {
        let plan = compute(repo.path(), &parse(args).unwrap(), || {
            panic!("the graph must not load for an empty diff")
        })
        .unwrap();
        assert!(plan.is_no_work() && plan.checks.is_empty());
        let text = render(&plan, std::time::Duration::ZERO, false);
        assert!(
            text.contains("NO_WORK") && text.contains("not a PASS"),
            "{text}"
        );
        assert!(!text.contains("overall: PASS"));
        assert!(select_only(&plan, &["verify static docs".into()]).is_err());
        let summary = exec::summary(&plan, &[], Overall::NoWork);
        assert!(summary.contains("NO_WORK") && !summary.contains("overall: PASS"));
    }
}

#[test]
fn worktree_scope_pins_merge_base_and_every_dirty_layer() {
    let (repo, fork, head) = branched();
    repo.write(CONTRACT, "staged contract\n");
    repo.git(&["add", CONTRACT]);
    repo.write(CSS, "unstaged css\n");
    repo.write("ignored/secret.txt", "ignored\n");
    let status_before = repo.git(&["status", "--porcelain=v1", "-z"]);
    let index_before = fs::read(repo.path().join(".git/index")).unwrap();

    let plan = repo.plan(&["--plan"]).unwrap();
    assert!(plan.base_defaulted);
    assert_eq!(plan.snapshot.merge_base, fork);
    assert_eq!(plan.snapshot.head, head);
    assert_eq!((plan.snapshot.base_only, plan.snapshot.head_only), (1, 1));
    let listed = rows(&plan);
    for expected in [
        format!("committed M {DOMAIN}"),
        format!("committed R {OLD} -> {MOVED}"),
        format!("committed D {MIGRATION}"),
        format!("staged A {CONTRACT}"),
        format!("unstaged M {CSS}"),
    ] {
        assert!(listed.contains(&expected), "{expected} not in {listed:#?}");
    }
    assert!(
        !listed
            .iter()
            .any(|r| r.contains("only-on-base") || r.contains("ignored/"))
    );
    let selected = commands(&plan);
    for required in [
        "verify discussion",
        "verify kani",
        "verify unit app",
        "verify postgres app",
        "verify unit identity",
        "verify postgres identity",
        "verify postgres lifecycle",
        "verify unit notifications",
        "verify static tokens",
        "verify web",
        "verify web --scenario standalone",
    ] {
        assert!(
            selected.contains(&required),
            "{required} not in {selected:#?}"
        );
    }
    // Both sides of the move are classified: Kani for the old domain path, the App
    // PostgreSQL target for the new application path. Nothing broadened.
    assert!(
        plan.checks
            .iter()
            .any(|c| c.command == "verify kani" && c.entry.paths.contains(OLD))
    );
    assert!(
        plan.checks
            .iter()
            .any(|c| c.command == "verify postgres app" && c.entry.paths.contains(MOVED))
    );
    assert!(
        plan.selection.fallback.is_empty(),
        "{:?}",
        plan.selection.fallback
    );
    let text = render(&plan, std::time::Duration::from_millis(5), false);
    for part in [
        "merge-base:",
        "1 commit(s) only on base are excluded",
        "selected checks",
        "why:",
        "not selected:",
        "cost:",
        "--expect",
    ] {
        assert!(text.contains(part), "missing {part}:\n{text}");
    }
    // Planning wrote nothing: index, status and target are untouched.
    assert_eq!(status_before, repo.git(&["status", "--porcelain=v1", "-z"]));
    assert_eq!(
        index_before,
        fs::read(repo.path().join(".git/index")).unwrap()
    );
    assert!(!repo.path().join("target").exists());

    // Committed scope reads only commits.
    let committed = repo.plan(&["--head", "HEAD", "--plan"]).unwrap();
    assert!(
        committed
            .snapshot
            .changes
            .iter()
            .all(|c| c.layer == git::Layer::Committed)
    );
    assert!(!commands(&committed).contains(&"verify unit notifications"));

    assert!(
        committed
            .graph
            .as_deref()
            .unwrap()
            .contains("committed dependency graph unavailable")
    );

    // A browser fixture moved to an unregistered name keeps its old owner and broadens.
    repo.git(&["mv", PAGINATION, "apps/vink-web/e2e/pagination-renamed.mjs"]);
    let moved = repo.plan(&[]).unwrap();
    assert!(moved.selection.owners[PAGINATION].contains("browser fixture"));
    assert!(
        moved
            .selection
            .fallback
            .contains_key("browser module with no registered caller")
    );
    assert!(commands(&moved).contains(&"verify quick"));
    repo.git(&["mv", "apps/vink-web/e2e/pagination-renamed.mjs", PAGINATION]);

    // An unknown untracked file broadens instead of selecting nothing.
    repo.write("new/area/file.txt", "?\n");
    let broad = repo.plan(&[]).unwrap();
    assert!(rows(&broad).contains(&"untracked ? new/area/file.txt".to_string()));
    assert!(broad.selection.unknown.contains("new/area/file.txt"));
    assert!(commands(&broad).contains(&"verify quick"));
}

#[test]
fn drift_is_detected_for_every_input_kind() {
    let (repo, _, _) = branched();
    repo.write(CSS, "dirty once\n");
    let plan = repo.plan(&[]).unwrap();
    let args = parse(&[]).unwrap();
    let drift = || exec::drift_of(repo.path(), &args, &plan).unwrap();
    assert_eq!(drift(), None);
    // A second edit of an already dirty file changes no raw diff record, only its bytes.
    repo.write(CSS, "dirty twice\n");
    assert!(drift().is_some());
    repo.write(CSS, "dirty once\n");
    assert_eq!(drift(), None);
    repo.write("untracked.md", "new\n");
    assert!(drift().is_some());
    fs::remove_file(repo.path().join("untracked.md")).unwrap();
    repo.git(&["add", CSS]);
    assert!(
        drift().is_some(),
        "staging moves the change to another layer"
    );
    repo.git(&["reset", "--quiet", "--", CSS]);
    assert_eq!(drift(), None);
    repo.commit("new head");
    assert!(drift().is_some(), "HEAD moved");
    // The plan id follows the inputs, so --expect rejects a plan reviewed before the change.
    assert_ne!(repo.plan(&[]).unwrap().id, plan.id);
}

#[test]
fn invalid_references_and_unsafe_states_fail_explicitly() {
    let (repo, _, _) = branched();
    for (args, message) in [
        (&["--base", "missing"][..], "unknown revision `missing`"),
        (
            &["--base", "HEAD", "--head", "nope"],
            "unknown revision `nope`",
        ),
        (&["--base", "HEAD~1..HEAD"], "expected one commit reference"),
    ] {
        let error = repo.plan(args).err().unwrap();
        assert!(error.contains(message), "{args:?}: {error}");
    }
    repo.git(&["checkout", "--quiet", "--orphan", "unrelated"]);
    repo.git(&["rm", "-rf", "--quiet", "."]);
    repo.write("x.md", "x\n");
    repo.commit("unrelated root");
    assert!(
        repo.plan(&[])
            .err()
            .unwrap()
            .contains("share no merge base")
    );
    repo.git(&["update-index", "--assume-unchanged", "x.md"]);
    assert!(
        repo.plan(&["--base", "HEAD"])
            .err()
            .unwrap()
            .contains("assume-unchanged")
    );
}

#[test]
fn committed_planning_never_reads_another_heads_dependency_graph() {
    let (repo, fork, head) = branched();
    repo.git(&["checkout", "--detach", &fork]);
    let plan = compute(repo.path(), &parse(&["--head", &head]).unwrap(), || {
        panic!("must not read dependency facts from another revision")
    })
    .unwrap();
    assert!(commands(&plan).contains(&"verify quick"));
    assert!(
        plan.graph
            .unwrap()
            .contains("committed dependency graph unavailable")
    );
}

#[test]
fn worktree_rejects_untracked_nested_repository_without_a_content_fingerprint() {
    let repo = Repo::new("nested");
    repo.commit("base");
    let child = repo.path().join("child");
    fs::create_dir(&child).unwrap();
    git::git(&child, &["init", "--quiet"]).unwrap();
    git::git(
        &child,
        &[
            "-c",
            "user.name=fixture",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "child",
        ],
    )
    .unwrap();
    let error = repo.plan(&["--base", "HEAD"]).err().unwrap();
    assert!(
        error.contains("cannot fingerprint nested directory"),
        "{error}"
    );
}

#[test]
fn only_selection_rejects_typos_unselected_and_subsumed_checks() {
    let (repo, _, _) = branched();
    let plan = repo.plan(&["--head", "HEAD"]).unwrap();
    assert!(
        select_only(&plan, &["verify unit apx".into()])
            .unwrap_err()
            .contains("unknown check")
    );
    assert!(
        select_only(&plan, &["verify unit store".into()])
            .unwrap_err()
            .contains("does not select it")
    );
    let only = select_only(&plan, &["verify discussion".into()])
        .unwrap()
        .unwrap();
    assert_eq!(only.len(), 1);
    let (broad, _, _) = branched();
    broad.write("Cargo.lock", "lock\n");
    let lock = broad.plan(&[]).unwrap();
    assert!(
        select_only(&lock, &["verify unit app".into()])
            .unwrap_err()
            .contains("runs inside `verify quick`")
    );
}

#[test]
fn executor_statuses_follow_the_plan_and_stop_on_failure_or_drift() {
    let (repo, _, _) = branched();
    let plan = repo.plan(&["--head", "HEAD"]).unwrap();
    let n = plan.checks.len();
    assert!(n >= 4, "{:?}", commands(&plan));
    let clean = &mut || Ok(None);
    let supported = &|_: &Check| None;

    let mut ran = Vec::new();
    let (outcomes, overall) = exec::run_checks(
        &plan,
        None,
        clean,
        &mut |c| {
            ran.push(c.command.clone());
            Ok(())
        },
        supported,
    );
    assert_eq!(overall, Overall::Pass);
    assert_eq!(
        ran,
        commands(&plan),
        "executor runs exactly the planned commands, in order"
    );
    assert!(outcomes.iter().all(|o| o.status == Status::Pass));

    let failing = plan.checks[1].command.clone();
    let (outcomes, overall) = exec::run_checks(
        &plan,
        None,
        &mut || Ok(None),
        &mut |c| {
            if c.command == failing {
                Err("boom".into())
            } else {
                Ok(())
            }
        },
        supported,
    );
    assert_eq!(overall, Overall::Fail);
    assert_eq!(outcomes[1].status, Status::Fail);
    assert!(outcomes[2..].iter().all(|o| o.status == Status::NotRun));

    let mut calls = 0;
    let (outcomes, overall) = exec::run_checks(
        &plan,
        None,
        &mut || {
            calls += 1;
            Ok((calls == 2).then(|| "edited".to_string()))
        },
        &mut |_| Ok(()),
        supported,
    );
    assert_eq!(overall, Overall::Fail);
    assert_eq!(outcomes[0].status, Status::Pass);
    assert!(outcomes[1..].iter().all(|o| o.status == Status::NotRun));
    assert!(outcomes[1].detail.contains("drifted"));

    let mut calls = 0;
    let (outcomes, overall) = exec::run_checks(
        &plan,
        None,
        &mut || {
            calls += 1;
            Ok((calls > n).then(|| "edited during the last check".to_string()))
        },
        &mut |_| Ok(()),
        supported,
    );
    assert_eq!(overall, Overall::Fail);
    assert_eq!(outcomes.last().unwrap().status, Status::Fail);

    // Browser lanes inside tools/local-budget are UNSUPPORTED, never silently run or passed.
    repo.write(CSS, "dirty\n");
    let web = repo.plan(&[]).unwrap();
    let browser = &|c: &Check| exec::unsupported(c, true);
    let (outcomes, overall) =
        exec::run_checks(&web, None, &mut || Ok(None), &mut |_| Ok(()), browser);
    assert_eq!(overall, Overall::Partial);
    assert!(outcomes.iter().any(|o| o.status == Status::Unsupported));
    assert!(outcomes.iter().any(|o| o.status == Status::Pass));

    let only: std::collections::BTreeSet<_> = [plan.checks[0].command.clone()].into();
    let (outcomes, overall) = exec::run_checks(
        &plan,
        Some(&only),
        &mut || Ok(None),
        &mut |_| Ok(()),
        supported,
    );
    assert_eq!(overall, Overall::Partial);
    assert_eq!(
        outcomes.iter().filter(|o| o.status == Status::Pass).count(),
        1
    );
    let summary = exec::summary(&plan, &outcomes, overall);
    assert!(summary.contains("excluded by --only") && summary.contains("PARTIAL"));
}

#[test]
fn a_source_edit_during_execution_invalidates_the_rest_of_the_plan() {
    let (repo, _, _) = branched();
    repo.write(CSS, "dirty\n");
    let plan = repo.plan(&[]).unwrap();
    let args = parse(&[]).unwrap();
    let mut first = true;
    let (outcomes, overall) = exec::run_checks(
        &plan,
        None,
        &mut || exec::drift_of(repo.path(), &args, &plan),
        &mut |_| {
            if first {
                first = false;
                repo.write(DOMAIN, "edited while the first check ran\n");
            }
            Ok(())
        },
        &|_| None,
    );
    assert_eq!(overall, Overall::Fail);
    assert_eq!(outcomes[0].status, Status::Pass);
    assert!(outcomes[1].detail.contains("drifted"), "{outcomes:#?}");
    assert!(outcomes[2..].iter().all(|o| o.status == Status::NotRun));
}
