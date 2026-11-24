use super::*;
use std::fs;

#[test]
fn rules_keep_required_owner_and_transitive_commands() {
    let cases: &[(&str, &[&str])] = &[
        ("AGENTS.md", &["verify static docs"]),
        (
            "packages/store-postgres/src/pool.rs",
            &[
                "verify unit xtask",
                "verify unit identity",
                "verify postgres",
                "verify web",
            ],
        ),
        (
            "packages/expressions/src/lib.rs",
            &["verify expressions", "verify kani", "verify preview"],
        ),
        (
            "services/vink-notifications/src/store.rs",
            &["verify postgres", "verify unit app"],
        ),
        (
            "apps/vink-web/style/vink.css",
            &[
                "verify static tokens",
                "verify web",
                "verify web --scenario standalone",
            ],
        ),
        (
            "apps/vink-web/editor/src/editor.ts",
            &[
                "verify static repo",
                "verify web",
                "verify web --scenario standalone",
            ],
        ),
        (
            "packages/ui-system/shared/tokens.json",
            &[
                "verify static tokens",
                "verify preview",
                "verify web",
                "verify quick",
            ],
        ),
        (
            "services/vink-app/migrations/drop.sql",
            &["verify postgres", "verify web"],
        ),
        (
            "services/vink-identity/contracts/v1.json",
            &["verify web", "verify postgres", "verify quick"],
        ),
        (
            "Cargo.lock",
            &[
                "verify quick",
                "verify postgres",
                "verify web",
                "verify kani",
            ],
        ),
        (
            "rust-toolchain.toml",
            &["verify quick", "verify web", "verify kani"],
        ),
        (
            "apps/vink-web/Cargo.toml",
            &["verify quick", "verify postgres", "verify web"],
        ),
        (
            "new/owner/oracle.md",
            &["verify quick", "verify postgres", "verify web"],
        ),
        (
            "apps/vink-web/e2e/editor-desk-regressions.mjs",
            &["verify web --scenario editor-format --scenario editor-first-draft"],
        ),
        (
            "services/vink-app/tests/postgres.rs",
            &["verify postgres app"],
        ),
    ];
    for (path, expected) in cases {
        let mut plan = Selection::default();
        plan.path(path);
        for cmd in *expected {
            assert!(plan.commands.contains_key(*cmd), "{path} is missing {cmd}");
        }
        plan.describe(&mut String::new()).unwrap();
    }
    for &(dir, owner) in OWNER_DIRS {
        let mut plan = Selection::default();
        plan.path(&format!("{dir}src/lib.rs"));
        for cmd in [
            format!("verify check {owner} --target all-targets"),
            format!("verify unit {owner}"),
        ] {
            assert!(plan.commands.contains_key(&cmd), "{dir} is missing {cmd}");
        }
        plan.describe(&mut String::new()).unwrap();
    }
    let mut plan = Selection::default();
    plan.path("apps/vink-web/e2e/draft-pagination-smoke.mjs");
    assert_eq!(plan.commands.len(), 1);
    assert!(plan.commands.contains_key("verify web --scenario desk"));
    let mut docs = Selection::default();
    docs.path("docs/development/scoped-workflows.md");
    assert_eq!(docs.commands.len(), 1);
    assert!(!docs.broad);
}

#[test]
fn owner_rules_match_workspace_manifests_and_selector_packages() {
    use crate::plan::UnitOwner;
    let root = crate::root();
    let manifest = |dir: &Path| -> toml::Value {
        toml::from_str(&fs::read_to_string(dir.join("Cargo.toml")).unwrap()).unwrap()
    };
    let ws = manifest(&root);
    let members: BTreeSet<_> = ws["workspace"]["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m.as_str().unwrap())
        .collect();
    let mapped: BTreeSet<_> = OWNER_DIRS
        .iter()
        .map(|(dir, _)| dir.trim_end_matches('/'))
        .collect();
    assert_eq!(
        mapped, members,
        "review diff rules when workspace ownership changes"
    );
    for &(dir, selector) in OWNER_DIRS {
        let package = manifest(&root.join(dir));
        assert_eq!(
            package["package"]["name"].as_str().unwrap(),
            UnitOwner::parse(selector).unwrap().package()
        );
    }
}

#[test]
fn invalid_or_truncated_git_records_fail_closed() {
    for raw in [&b"R100\0old\0"[..], b"M\0", b"X\0bad\0", b"M\0\xff\0"] {
        assert!(changes(raw, "test").is_err());
    }
}

#[test]
fn repository_inputs_are_recomputed_and_both_sides_of_moves_are_selected() {
    let dir = std::env::temp_dir().join(format!("vink-changed-{}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    struct Cleanup<'a>(&'a Path);
    impl Drop for Cleanup<'_> {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(self.0);
        }
    }
    let _cleanup = Cleanup(&dir);
    let g = |args: &[&str]| git(&dir, args).unwrap();
    g(&["init", "--quiet"]);
    g(&["config", "user.name", "fixture"]);
    g(&["config", "user.email", "fixture@invalid.example"]);
    let old = "apps/vink-web/e2e/draft-pagination-smoke.mjs";
    fs::create_dir_all(dir.join("apps/vink-web/e2e")).unwrap();
    fs::write(dir.join(old), "move me\n").unwrap();
    fs::write(dir.join("AGENTS.md"), "policy\n").unwrap();
    g(&["add", "."]);
    g(&["-c", "commit.gpgsign=false", "commit", "-qm", "base"]);
    let local = ["--base", "HEAD", "--worktree", "--plan"];
    assert!(describe(&dir, &local).unwrap().contains("no changed paths"));
    g(&["mv", old, "new selector"]);
    g(&["-c", "commit.gpgsign=false", "commit", "-qm", "move"]);
    fs::remove_file(dir.join("AGENTS.md")).unwrap();
    fs::write(dir.join("Cargo.lock"), "shared").unwrap();
    g(&["add", "Cargo.lock"]);
    fs::write(dir.join("new\nuntracked"), "fixture").unwrap();
    let endpoints = describe(&dir, &["--base", "HEAD~1", "--head", "HEAD", "--plan"]).unwrap();
    assert!(
        endpoints.contains("committed R100")
            && endpoints.contains(old)
            && endpoints.contains("new selector")
    );
    assert!(endpoints.contains("selected: cargo xtask verify web --scenario desk"));
    assert!(endpoints.contains("fallback:") && !endpoints.contains("Cargo.lock"));
    let before = g(&["status", "--porcelain=v1", "-z"]);
    let index_before = fs::read(dir.join(".git/index")).unwrap();
    let result = describe(&dir, &local).unwrap();
    assert!(
        result.contains("index A \"Cargo.lock\"") && result.contains("worktree D \"AGENTS.md\"")
    );
    assert!(result.contains("untracked ? \"new\\nuntracked\""));
    assert_eq!(before, g(&["status", "--porcelain=v1", "-z"]));
    assert_eq!(index_before, fs::read(dir.join(".git/index")).unwrap());
    assert!(!dir.join("target").exists());
    fs::write(dir.join("later.rs"), "").unwrap();
    assert!(describe(&dir, &local).unwrap().contains("later.rs"));
    for invalid in [
        vec!["--base", "missing", "--worktree", "--plan"],
        vec!["--base", "--all", "--worktree", "--plan"],
        vec!["--base", "HEAD", "--head", "HEAD"],
        vec!["--base", "HEAD", "--worktree", "--run"],
    ] {
        assert!(describe(&dir, &invalid).is_err());
    }
    g(&["update-index", "--assume-unchanged", "Cargo.lock"]);
    assert!(
        describe(&dir, &local)
            .unwrap_err()
            .contains("assume-unchanged")
    );
}
