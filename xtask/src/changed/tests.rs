use super::rules::{self, BROWSER_DISPATCHER, BROWSER_MODULES, NODE_TESTS, XTASK_MODULES};
use super::*;
use crate::plan::{CheckTarget, Scenario, UnitOwner};
use std::sync::OnceLock;

mod repo;

fn graph() -> &'static graph::Graph {
    static GRAPH: OnceLock<graph::Graph> = OnceLock::new();
    GRAPH.get_or_init(|| graph::Graph::load(&crate::root()).unwrap())
}

fn select_with(g: &graph::Graph, paths: &[&str]) -> (rules::Selection, Vec<String>) {
    let mut selection = rules::Selection::default();
    for path in paths {
        rules::classify(&mut selection, g, path, &g.includers(path));
    }
    selection.expand_broad(g);
    let (checks, _) = checks(&selection).expect("every emitted selector parses");
    let commands = checks.into_iter().map(|c| c.command).collect();
    (selection, commands)
}

fn select(paths: &[&str]) -> (rules::Selection, Vec<String>) {
    select_with(graph(), paths)
}

/// Required checks absent from `selected`: the planner regression oracle.
fn missing<'a>(required: &[&'a str], selected: &[String]) -> Vec<&'a str> {
    required
        .iter()
        .copied()
        .filter(|r| !selected.iter().any(|s| s == r))
        .collect()
}

const FULL_WEB: [&str; 2] = ["verify web", "verify web --scenario standalone"];
const EDITOR_WEB: &str = "verify web --scenario editor-first-draft --scenario editor-chrome --scenario editor --scenario editor-format --scenario restart --scenario review --scenario publication";
const PERF: &str = "verify web --scenario perf-baseline --scenario return-position --release";
const APP_DIRECT: [&str; 6] = [
    "verify check app --target all-targets",
    "verify check app --target api",
    "verify check app --target ssr",
    "verify unit app",
    "verify unit app --test document_budget",
    "verify unit app --test ssr_boundary",
];

struct Case {
    paths: &'static [&'static str],
    required: &'static [&'static str],
    forbidden: &'static [&'static str],
    broad: bool,
}

/// Independent expectations per kind of change (AC-02). `forbidden` keeps cheap changes cheap.
fn matrix() -> Vec<Case> {
    let c = |paths, required, forbidden, broad| Case {
        paths,
        required,
        forbidden,
        broad,
    };
    vec![
        // Docs-only: static docs, nothing compiled.
        c(
            &[
                "docs/development/scoped-workflows.md",
                ".agents/skills/vink-engineering/references/verification.md",
                "packages/ui-system/EXTENDING.md",
            ],
            &["verify static docs"],
            &[
                "verify web",
                "verify unit app",
                "verify postgres",
                "verify quick",
                "verify check xtask --target all-targets",
            ],
            false,
        ),
        // CSS-only: Tailwind/tokens guards and every Web route; no database lane.
        c(
            &["apps/vink-web/style/vink.css"],
            &[
                "verify static tokens",
                "verify static repo",
                FULL_WEB[0],
                FULL_WEB[1],
            ],
            &[
                "verify postgres",
                "verify quick",
                "verify preview",
                "verify unit app",
            ],
            false,
        ),
        c(
            &["apps/vink-preview/leptos/style/preview.css"],
            &["verify static repo", "verify preview"],
            &[
                "verify web",
                "verify web --scenario standalone",
                "verify postgres",
            ],
            false,
        ),
        c(
            &["packages/vink-patterns/leptos/patterns.css"],
            &["verify preview", FULL_WEB[0], FULL_WEB[1]],
            &["verify postgres"],
            false,
        ),
        // Editor JS-only: Editor package guard and the editor-loading scenarios.
        c(
            &["apps/vink-web/editor/src/markdown.ts"],
            &["verify static repo", EDITOR_WEB, FULL_WEB[1]],
            &["verify web", "verify postgres", "verify unit app"],
            false,
        ),
        // Rust domain: focused owner tests plus the formal lanes that compile or replay it.
        c(
            &["services/vink-app/src/domain/discussion/reaction.rs"],
            &[
                APP_DIRECT[0],
                APP_DIRECT[1],
                APP_DIRECT[3],
                APP_DIRECT[4],
                APP_DIRECT[5],
                "verify discussion",
                "verify kani",
            ],
            &[
                "verify postgres app",
                "verify postgres",
                "verify web",
                "verify quick",
                "verify expressions",
            ],
            false,
        ),
        c(
            &["services/vink-app/src/domain/draft.rs"],
            &[APP_DIRECT[3], "verify kani"],
            &["verify discussion", "verify postgres app", "verify web"],
            false,
        ),
        // HTTP API: API compile variant, contract/unit tests and the HTTP integration target.
        c(
            &["services/vink-app/src/http/drafts.rs"],
            &[APP_DIRECT[1], APP_DIRECT[3], "verify postgres app"],
            &["verify web", "verify quick", "verify kani"],
            false,
        ),
        c(
            &["services/vink-app/src/main.rs"],
            &[
                "verify postgres app",
                "verify postgres lifecycle",
                FULL_WEB[0],
                FULL_WEB[1],
            ],
            &["verify quick"],
            false,
        ),
        // SQL/migration: owner PostgreSQL plus lifecycle; Web only as a recorded consumer.
        c(
            &["services/vink-app/migrations/20261204000000_saved_works.sql"],
            &[
                "verify postgres app",
                "verify postgres lifecycle",
                APP_DIRECT[3],
            ],
            &["verify web", "verify postgres identity", "verify quick"],
            false,
        ),
        c(
            &["services/vink-identity/migrations/20261120000000_resend_payload_fence.sql"],
            &["verify postgres identity", "verify postgres lifecycle"],
            &["verify postgres app", "verify web"],
            false,
        ),
        c(
            &["services/postgres/provision.sh"],
            &["verify postgres"],
            &["verify web", "verify quick"],
            false,
        ),
        c(
            &["services/vink-app/tests/postgres/abuse.rs"],
            &["verify postgres app", APP_DIRECT[0]],
            &["verify unit app", "verify web"],
            false,
        ),
        // Contracts: producer and consumer tests across processes.
        c(
            &["services/vink-app/contracts/http/v1/README.md"],
            &[
                "verify static docs",
                "verify unit app",
                "verify postgres app",
                "verify unit web",
                "verify unit notifications",
            ],
            &[],
            false,
        ),
        c(
            &["services/vink-identity/contracts/rpc/v1/fixtures/capabilities.native.json"],
            &[
                "verify unit identity",
                "verify postgres identity",
                "verify unit app",
                "verify web --scenario auth --scenario password --scenario author-search --scenario publication",
                FULL_WEB[1],
            ],
            &["verify quick"],
            false,
        ),
        c(
            &["services/vink-notifications/contracts/rpc/v1/README.md"],
            &[
                "verify unit notifications",
                "verify postgres",
                "verify unit app",
            ],
            &[],
            false,
        ),
        // Shared packages expand through the actual Cargo dependency closure.
        c(
            &["packages/store-postgres/src/lib.rs"],
            &[
                "verify unit store",
                "verify postgres",
                "verify unit app",
                "verify unit identity",
                "verify unit notifications",
                "verify unit xtask",
                "verify unit xtask --test scoped_cli",
                FULL_WEB[0],
            ],
            &["verify quick"],
            false,
        ),
        c(
            &["packages/markdown-text/src/lib.rs"],
            &[
                "verify unit markdown",
                "verify unit app",
                "verify unit web",
                "verify check web --target hydrate",
                "verify check web --target csr",
                "verify kani",
                "verify postgres app",
                FULL_WEB[0],
                FULL_WEB[1],
            ],
            &["verify quick"],
            false,
        ),
        c(
            &["packages/work-view-policy/src/lib.rs"],
            &[
                "verify unit work-views",
                "verify unit app",
                "verify unit web",
                FULL_WEB[0],
            ],
            &["verify quick"],
            false,
        ),
        c(
            &["packages/ui-system/leptos/src/lib.rs"],
            &[
                "verify unit ui",
                "verify unit patterns",
                "verify unit web",
                "verify unit preview",
                "verify preview",
                "verify unit patterns --test editor_tools",
                FULL_WEB[0],
            ],
            &["verify quick"],
            false,
        ),
        c(
            &["packages/expressions/src/lib.rs"],
            &[
                "verify unit expressions",
                "verify expressions",
                "verify kani",
                "verify static assets",
                "verify unit preview",
                "verify preview",
            ],
            &["verify web", "verify quick"],
            false,
        ),
        c(
            &["apps/vink-web/Cargo.toml"],
            &[
                "verify check web --target hydrate",
                "verify check web --target csr",
                "verify unit web",
                "verify unit app",
                "verify postgres app",
                FULL_WEB[0],
                FULL_WEB[1],
            ],
            &["verify quick"],
            false,
        ),
        c(
            &["apps/vink-web/src/read_lifecycle.rs"],
            &[
                "verify discussion",
                "verify unit web",
                "verify unit app --test ssr_boundary",
                FULL_WEB[0],
            ],
            &["verify kani"],
            false,
        ),
        // Embedded inputs map through the Rust source that embeds them.
        c(
            &["apps/vink-web/appearance.js"],
            &[
                "verify unit web",
                FULL_WEB[0],
                FULL_WEB[1],
                "verify unit xtask",
            ],
            &[],
            false,
        ),
        c(
            &["apps/vink-web/public-cache/indexeddb.js"],
            &["verify unit web", FULL_WEB[0]],
            &[],
            false,
        ),
        c(
            &["apps/vink-web/index.html"],
            &[FULL_WEB[1], "verify unit xtask"],
            &["verify web"],
            false,
        ),
        // Browser fixtures: registered callers plus ES import closure.
        c(
            &["apps/vink-web/e2e/draft-pagination-smoke.mjs"],
            &["verify web --scenario desk"],
            &["verify web", "verify postgres"],
            false,
        ),
        c(
            &["apps/vink-web/e2e/desk-empty-smoke.mjs"],
            &[
                "verify web --scenario desk --scenario auth --scenario password",
                FULL_WEB[1],
            ],
            &["verify web"],
            false,
        ),
        c(
            &["apps/vink-web/e2e/web-request-ledger.mjs"],
            &[
                "verify static perf",
                "verify web --scenario reader",
                "verify web --scenario perf-baseline --release",
            ],
            &["verify web", PERF],
            false,
        ),
        c(
            &["apps/vink-web/e2e/fixtures/discovery-publications.sql"],
            &["verify web --scenario discovery", PERF, "verify unit xtask"],
            &["verify web"],
            false,
        ),
        c(
            &[BROWSER_DISPATCHER],
            &[FULL_WEB[0], FULL_WEB[1], PERF],
            &["verify postgres"],
            false,
        ),
        c(
            &["apps/vink-web/e2e/visual-evidence.test.mjs"],
            &["verify static visual"],
            &["verify web"],
            false,
        ),
        c(
            &["packages/ui-system/leptos/e2e/select-geometry.mjs"],
            &[
                "verify preview",
                "verify web --scenario editor-first-draft --scenario desk --scenario editor-format",
            ],
            &["verify web"],
            false,
        ),
        // Formal model inputs.
        c(
            &["services/vink-app/behavior/discussion/verify.mjs"],
            &["verify discussion"],
            &["verify unit app", "verify web"],
            false,
        ),
        c(
            &["services/vink-app/kani/lib.rs"],
            &["verify kani"],
            &["verify unit app"],
            false,
        ),
        c(
            &["packages/expressions/behavior/lifecycle.qnt"],
            &["verify expressions"],
            &["verify web", "verify preview"],
            false,
        ),
        // Generated tokens and CMP.
        c(
            &["packages/ui-system/shared/tokens.json"],
            &[
                "verify static tokens",
                "verify preview",
                "ui-preview cmp verify",
                FULL_WEB[0],
            ],
            &["verify postgres"],
            false,
        ),
        c(
            &["packages/ui-system/cmp/ui/src/commonMain/kotlin/vink/ui/VinkTokens.kt"],
            &["verify static tokens", "ui-preview cmp verify"],
            &["verify web"],
            false,
        ),
        c(
            &["packages/vink-patterns/cmp/build.gradle.kts"],
            &["ui-preview cmp verify"],
            &["verify web"],
            false,
        ),
        // xtask: owner tests plus the lanes each module implements.
        c(
            &["xtask/src/changed/rules.rs", "xtask/tests/scoped_cli.rs"],
            &[
                "verify check xtask --target all-targets",
                "verify unit xtask",
                "verify unit xtask --test scoped_cli",
            ],
            &["verify web", "verify postgres", "verify quick"],
            false,
        ),
        c(
            &["xtask/src/web_dag.rs", "xtask/src/tailwind_inputs.mjs"],
            &["verify unit xtask", FULL_WEB[0], FULL_WEB[1]],
            &["verify quick", "verify postgres"],
            false,
        ),
        c(
            &["xtask/src/web.rs"],
            &["verify unit xtask", FULL_WEB[0], FULL_WEB[1], PERF],
            &[],
            false,
        ),
        c(
            &["xtask/src/discussion.rs"],
            &["verify discussion", "verify unit xtask"],
            &["verify web"],
            false,
        ),
        c(
            &["packages/vink-patterns/leptos/tests/editor_tools.rs"],
            &[
                "verify unit patterns --test editor_tools",
                "verify check patterns --target all-targets",
            ],
            &["verify unit patterns", "verify web"],
            false,
        ),
        c(
            &["infra/production/compose.yaml"],
            &["verify unit xtask"],
            &["verify web"],
            false,
        ),
        c(
            &["infra/production/production-handoff.md"],
            &["verify static docs"],
            &["verify unit xtask", "verify web"],
            false,
        ),
        c(
            &[".github/workflows/ci.yml"],
            &["verify static repo"],
            &["verify quick"],
            false,
        ),
        // Repository-wide or unknown inputs broaden to every CI lane plus the Web opt-ins.
        c(&["Cargo.lock"], &rules::BROAD, &[], true),
        c(
            &["rust-toolchain.toml"],
            &[FULL_WEB[0], FULL_WEB[1], PERF, "verify quick"],
            &[],
            true,
        ),
        c(
            &["xtask/src/verify.rs"],
            &["verify quick", "verify postgres", FULL_WEB[0]],
            &[],
            true,
        ),
        c(
            &["new/owner/oracle.txt"],
            &[
                "verify quick",
                "verify kani",
                "ui-preview cmp verify",
                FULL_WEB[0],
                PERF,
            ],
            &[],
            true,
        ),
    ]
}

#[test]
fn matrix_selects_required_checks_and_keeps_narrow_changes_narrow() {
    for case in matrix() {
        let (selection, commands) = select(case.paths);
        let absent = missing(case.required, &commands);
        assert!(
            absent.is_empty(),
            "{:?} misses {absent:?}; selected {commands:#?}",
            case.paths
        );
        for forbidden in case.forbidden {
            assert!(
                !commands.iter().any(|c| c == forbidden),
                "{:?} unexpectedly selects {forbidden}; selected {commands:#?}",
                case.paths
            );
        }
        assert_eq!(
            !selection.fallback.is_empty(),
            case.broad,
            "{:?} fallback: {:?}",
            case.paths,
            selection.fallback
        );
        assert_eq!(
            selection.unknown.contains("new/owner/oracle.txt"),
            case.paths == ["new/owner/oracle.txt"]
        );
    }
}

#[test]
fn narrow_inputs_select_exactly_their_owner() {
    for (path, expected) in [
        (
            "docs/development/feature-map.md",
            &["verify static docs"][..],
        ),
        (
            "apps/vink-web/e2e/draft-pagination-smoke.mjs",
            &["verify web --scenario desk"],
        ),
        (
            "apps/vink-web/e2e/author-works-smoke.mjs",
            &["verify web --scenario desk"],
        ),
        (
            "apps/vink-web/e2e/visual-evidence.test.mjs",
            &["verify static visual"],
        ),
        (
            "services/vink-app/behavior/discussion/verify.mjs",
            &["verify discussion"],
        ),
    ] {
        assert_eq!(select(&[path]).1, expected, "{path}");
    }
    // Manual-only inputs are reported, never silently dropped or broadened.
    let (selection, commands) = select(&[
        "apps/vink-web/e2e/appearance-bootstrap.test.mjs",
        "tools/local-budget",
    ]);
    assert!(commands.is_empty());
    assert!(selection.fallback.is_empty());
    assert_eq!(selection.manual.len(), 2, "{:?}", selection.manual.keys());
}

#[test]
fn generated_vector_outputs_select_the_generator_check() {
    for path in [
        crate::ui_states::RUST_STATES_OUT,
        crate::ui_states::KOTLIN_STATES_OUT,
        crate::ui_navigation::RUST_NAVIGATION_OUT,
        crate::ui_navigation::KOTLIN_NAVIGATION_OUT,
        crate::ui_toggle::RUST_TOGGLE_OUT,
        crate::ui_toggle::KOTLIN_TOGGLE_OUT,
    ] {
        let (_, commands) = select(&[path]);
        assert!(
            commands.iter().any(|c| c == "verify static tokens"),
            "{path}"
        );
    }
}

#[test]
fn browser_reports_distinct_manual_coverage_alongside_its_scenario() {
    let (selection, commands) = select(&["apps/vink-web/e2e/reader-ssr-boundary.mjs"]);
    assert!(commands.iter().any(|c| c == "verify web --scenario reader"));
    assert!(
        selection
            .manual
            .keys()
            .any(|m| m.contains("`reader-guest-history`"))
    );
    let (selection, _) = select(&["apps/vink-web/e2e/author-works-smoke.mjs"]);
    assert!(
        !selection
            .manual
            .keys()
            .any(|m| m.contains("`author-works`"))
    );
}

#[test]
fn deleting_an_embedded_fixture_keeps_its_cross_package_test() {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    let source = "apps/vink-web/src/document.rs";
    let fixture = "services/vink-app/tests/fixtures/title-authority/render.json";
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("vink-deleted-embed-{}-{stamp}", std::process::id()));
    let dst = root.join(source);
    fs::create_dir_all(dst.parent().unwrap()).unwrap();
    fs::copy(crate::root().join(source), &dst).unwrap();
    assert!(!root.join(fixture).exists());

    let mut g = graph().clone();
    let inputs = graph::compile_inputs(&root, &g.packages);
    fs::remove_dir_all(&root).unwrap();
    g.includes = inputs.unwrap();
    assert_eq!(g.includers(fixture), [source]);
    let (selection, commands) = select_with(&g, &[fixture]);
    assert!(selection.fallback.is_empty());
    assert!(commands.iter().any(|c| c == "verify unit web"));
}

#[test]
fn negative_controls_show_the_oracle_detects_dropped_dependency_facts() {
    // Each mutation removes one real dependency fact; the matrix oracle must notice.
    let web_src = "apps/vink-web/src/read_lifecycle.rs";
    let mut no_app_edge = graph().clone();
    for package in &mut no_app_edge.packages {
        package.deps.remove("vink-web");
    }
    assert_eq!(
        missing(
            &["verify unit app", "verify postgres app"],
            &select_with(&no_app_edge, &[web_src]).1
        ),
        ["verify unit app", "verify postgres app"]
    );
    let mut no_imports = graph().clone();
    no_imports.importers.clear();
    let desk = "apps/vink-web/e2e/desk-empty-smoke.mjs";
    assert!(
        !missing(
            &["verify web --scenario desk --scenario auth --scenario password"],
            &select_with(&no_imports, &[desk]).1
        )
        .is_empty()
    );
    let mut no_kani = graph().clone();
    no_kani.kani_inputs.clear();
    let domain = "services/vink-app/src/domain/draft.rs";
    assert_eq!(
        missing(&["verify kani"], &select_with(&no_kani, &[domain]).1),
        ["verify kani"]
    );
    // Losing the embedded-input fact for a crate-only file fails safe: it becomes unknown and
    // broadens, so the required Web checks are still selected.
    let mut no_includes = graph().clone();
    no_includes.includes.clear();
    let crate_input = "apps/vink-web/public-cache/indexeddb.js";
    let (selection, commands) = select_with(&no_includes, &[crate_input]);
    assert!(selection.unknown.contains(crate_input));
    assert!(missing(&FULL_WEB, &commands).is_empty());
    // Dropping a selected command is reported by the same oracle the matrix uses.
    for case in matrix() {
        let (_, commands) = select(case.paths);
        for required in case.required {
            let degraded: Vec<_> = commands.iter().filter(|c| c != required).cloned().collect();
            assert_eq!(missing(case.required, &degraded), [*required]);
        }
    }
}

#[test]
fn quick_only_absorbs_what_it_executes() {
    let (_, commands) = select(&["Cargo.lock"]);
    for kept in [
        "verify check app --target api",
        "verify unit app --test document_budget",
        "verify unit xtask --test scoped_cli",
    ] {
        assert!(
            commands.iter().any(|c| c == kept),
            "{kept} is not part of quick"
        );
    }
    for absorbed in [
        "verify static docs",
        "verify unit app",
        "verify unit app --test ssr_boundary",
        "verify check web --target hydrate",
        "verify postgres app",
    ] {
        assert!(
            !commands.iter().any(|c| c == absorbed),
            "{absorbed} runs inside a broader lane"
        );
    }
}

#[test]
fn every_workspace_package_feature_and_owner_maps_to_a_real_selector() {
    let g = graph();
    for owner in UnitOwner::ALL {
        assert_eq!(UnitOwner::parse(owner.name()), Ok(owner));
        assert!(
            g.package(owner.package()).is_some(),
            "{} has no package",
            owner.name()
        );
    }
    for package in &g.packages {
        let owner = package.owner.unwrap_or_else(|| {
            panic!(
                "{} has no UnitOwner: extend plan.rs and the planner",
                package.name
            )
        });
        for feature in ["hydrate", "csr", "ssr", "legacy-ssr"] {
            if package.features.contains(feature) {
                let variant = if feature == "legacy-ssr" {
                    "ssr"
                } else {
                    feature
                };
                assert!(
                    CheckTarget::parse(owner, variant).is_ok() && rules::declares(package, variant),
                    "{} declares `{feature}` without a verify check variant",
                    package.name
                );
            }
        }
        for target in package.tests() {
            assert!(
                target.src.starts_with(&format!("{}tests/", package.dir)),
                "{}",
                target.src
            );
            if target.name == "postgres" {
                crate::verify::PostgresScope::parse(owner.name()).unwrap();
            }
        }
    }
}

#[test]
fn browser_and_xtask_registries_match_the_checkout() {
    let root = crate::root();
    let g = graph();
    for (module, _, _) in BROWSER_MODULES {
        assert!(
            root.join(module).is_file(),
            "stale BROWSER_MODULES entry {module}"
        );
    }
    for (test, _) in NODE_TESTS {
        assert!(root.join(test).is_file(), "stale NODE_TESTS entry {test}");
    }
    let direct: Vec<_> = g
        .importers
        .iter()
        .filter(|(_, importers)| importers.contains(BROWSER_DISPATCHER))
        .map(|(module, _)| module.as_str())
        .collect();
    assert!(
        direct.len() > 20,
        "dispatcher imports were not discovered: {direct:?}"
    );
    for module in direct {
        assert!(
            BROWSER_MODULES.iter().any(|(m, _, _)| *m == module),
            "{module} is imported by the dispatcher but missing from BROWSER_MODULES"
        );
    }
    for dir in graph::BROWSER_DIRS {
        for entry in std::fs::read_dir(root.join(dir)).unwrap() {
            let name = entry.unwrap().file_name().to_string_lossy().into_owned();
            if !name.ends_with(".mjs") {
                continue;
            }
            let module = format!("{dir}{name}");
            let mut sel = rules::Selection::default();
            rules::classify(&mut sel, g, &module, &g.includers(&module));
            assert!(sel.fallback.is_empty(), "{module}: {:?}", sel.fallback);
        }
    }
    let mut stems = std::collections::BTreeSet::new();
    for entry in std::fs::read_dir(root.join("xtask/src")).unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().into_owned();
        stems.insert(name.trim_end_matches(".rs").to_string());
    }
    let registered: std::collections::BTreeSet<_> =
        XTASK_MODULES.iter().map(|(m, _)| m.to_string()).collect();
    assert_eq!(
        registered, stems,
        "XTASK_MODULES must name every xtask/src module"
    );
    for prefix in rules::discussion_sources() {
        assert!(
            root.join(format!("{prefix}.rs")).is_file() || root.join(&prefix).is_dir(),
            "discussion replay filter maps to missing source {prefix}"
        );
    }
    assert!(g.kani_inputs.contains("services/vink-app/src/domain/"));
    assert!(g.kani_inputs.contains("packages/markdown-text/"));
    let web = g
        .stylesheets
        .iter()
        .find(|s| s.entry == crate::tailwind::WEB_ENTRY)
        .unwrap();
    assert!(web.files.contains("apps/vink-web/style/vink.css"));
    assert!(web.sources.contains("apps/vink-web/src/"));
}

#[test]
fn every_tracked_file_has_an_owner_rule() {
    // Mapping drift: a new kind of input must get a rule (or join the intended broad set)
    // instead of silently relying on the runtime fallback.
    let root = crate::root();
    let g = graph();
    let files = git::git(&root, &["ls-files", "-z"]).unwrap();
    let mut unexpected = Vec::new();
    for path in std::str::from_utf8(&files).unwrap().split_terminator('\0') {
        let mut sel = rules::Selection::default();
        rules::classify(&mut sel, g, path, &g.includers(path));
        let intended_broad = path.starts_with(".cargo/")
            || matches!(
                path,
                "Cargo.lock"
                    | "Cargo.toml"
                    | "rust-toolchain.toml"
                    | ".gitignore"
                    | ".gitattributes"
                    | "xtask/Cargo.toml"
            )
            || XTASK_MODULES.iter().any(|(m, roles)| {
                roles.contains(&rules::Role::Shared) && path == format!("xtask/src/{m}.rs")
            });
        if !sel.unknown.is_empty() || (!sel.fallback.is_empty() && !intended_broad) {
            unexpected.push(format!(
                "{path}: {:?}",
                sel.fallback.keys().collect::<Vec<_>>()
            ));
        }
    }
    assert!(
        unexpected.is_empty(),
        "unclaimed inputs (add an owner rule in xtask/src/changed/rules.rs):\n{}",
        unexpected.join("\n")
    );
}

#[test]
fn web_merge_uses_canonical_order_and_full_lane_names() {
    let mut sel = rules::Selection::default();
    sel.scenario(Scenario::Publication, "x", "p");
    sel.scenario(Scenario::Desk, "x", "p");
    sel.scenario(Scenario::PerfBaseline, "x", "p");
    let commands: Vec<_> = sel.commands().into_keys().collect();
    assert_eq!(
        commands,
        [
            "verify web --scenario desk --scenario publication",
            "verify web --scenario perf-baseline --release",
        ]
    );
    let mut all = rules::Selection::default();
    all.scenario(Scenario::Main, "x", "p");
    for s in Scenario::ALL {
        all.scenario(s, "x", "p");
    }
    assert!(all.commands().contains_key("verify web"));
    for command in all.commands().keys().chain(commands.iter()) {
        describe_check(command).unwrap();
    }
}

#[test]
fn raw_records_fail_closed() {
    let ok = b":100644 100644 0000000000000000000000000000000000000000 1111111111111111111111111111111111111111 R087\0old\0new\0";
    let parsed = git::raw_changes(ok, git::Layer::Committed).unwrap();
    assert_eq!(parsed[0].paths, ["old", "new"]);
    assert_eq!(parsed[0].status, "R087");
    let gitlink = b":160000 160000 0000000000000000000000000000000000000000 1111111111111111111111111111111111111111 M\0sub\0";
    assert!(git::raw_changes(gitlink, git::Layer::Committed).unwrap()[0].gitlink);
    for raw in [
        &b":100644 100644 a b R100\0old\0"[..],
        b":100644 100644 a b M\0",
        b":100644 100644 a b X\0bad\0",
        b"M\0path\0",
        b":100644 100644 a b M\0\xff\0",
    ] {
        assert!(
            git::raw_changes(raw, git::Layer::Committed).is_err(),
            "{raw:?}"
        );
    }
}

#[test]
fn cli_arguments_are_strict() {
    assert_eq!(parse(&[]).unwrap(), Args::default());
    let args = parse(&[
        "--base",
        "x",
        "--head",
        "y",
        "--only",
        "cargo xtask verify  unit app",
        "--plan",
    ])
    .unwrap();
    assert_eq!(args.only, ["verify unit app"]);
    assert!(args.plan);
    for bad in [
        &["--base"][..],
        &["--base", "--all"],
        &["--head", "x", "--worktree"],
        &["--worktree", "--head", "x"],
        &["--plan", "--plan"],
        &["--only", "verify unit app", "--only", "verify unit app"],
        &["--run"],
        &["--expect", ""],
    ] {
        assert!(parse(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn account_deletion_opt_in_is_selected_for_its_fixture_and_identity_contract() {
    for input in [
        "apps/vink-web/e2e/auth-password-smoke.mjs",
        "services/vink-identity/contracts/rpc/v1/README.md",
        rules::BROWSER_DISPATCHER,
    ] {
        assert!(
            select(&[input])
                .1
                .iter()
                .any(|cmd| cmd == "verify web --scenario account-deletion"),
            "{input}"
        );
    }
}

#[test]
fn dispatcher_records_bundle_smoke_and_owner_alias_uses_the_same_unit_target() {
    let (selection, _) = select(&[rules::BROWSER_DISPATCHER]);
    assert!(
        selection
            .manual
            .keys()
            .any(|name| name.contains("reader-bundle"))
    );
    assert_eq!(
        crate::plan::UnitOwner::parse("work-view-policy").unwrap(),
        crate::plan::UnitOwner::WorkViews
    );
    assert_eq!(
        normalize("cargo xtask verify unit work-view-policy"),
        "verify unit work-views"
    );
}
