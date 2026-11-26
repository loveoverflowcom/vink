//! Conservative owner/path registry. Facts about crate edges, compile inputs, CSS imports,
//! Kani modules and browser-fixture imports come from `graph`; this file only names which
//! existing xtask selector owns each kind of input. An input no rule claims broadens to every
//! regression lane instead of selecting nothing.
use super::graph::{Graph, Package};
use crate::plan::{CheckTarget, Scenario, UnitOwner};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    pub reasons: BTreeSet<String>,
    pub paths: BTreeSet<String>,
}

impl Entry {
    fn note(&mut self, reason: &str, path: &str) {
        self.reasons.insert(reason.to_string());
        self.paths.insert(path.to_string());
    }
}

/// Every lane CI runs, plus the explicit Web opt-ins. Used for unknown or repository-wide
/// inputs; the merged Web command and the release performance pair are added separately.
pub const BROAD: [&str; 7] = [
    "verify quick",
    "verify discussion",
    "verify expressions",
    "verify kani",
    "verify postgres",
    "verify preview",
    "ui-preview cmp verify",
];

#[derive(Debug, Default)]
pub struct Selection {
    checks: BTreeMap<String, Entry>,
    scenarios: BTreeMap<&'static str, Entry>,
    pub owners: BTreeMap<String, BTreeSet<String>>,
    pub unknown: BTreeSet<String>,
    pub fallback: BTreeMap<String, BTreeSet<String>>,
    pub manual: BTreeMap<String, Entry>,
    pub artifacts: BTreeMap<String, Entry>,
    /// Boundaries deliberately not selected, with the condition that would select them.
    pub consumers: BTreeMap<String, Entry>,
}

impl Selection {
    pub fn add(&mut self, cmd: &str, reason: &str, path: &str) {
        self.checks
            .entry(cmd.into())
            .or_default()
            .note(reason, path);
    }

    pub fn scenario(&mut self, scenario: Scenario, reason: &str, path: &str) {
        let names: Vec<_> = if scenario == Scenario::Main {
            Scenario::ALL
                .into_iter()
                .filter(|s| s.in_main())
                .map(Scenario::name)
                .collect()
        } else {
            vec![scenario.name()]
        };
        for name in names {
            self.scenarios.entry(name).or_default().note(reason, path);
        }
    }

    fn scenarios(&mut self, list: &[Scenario], reason: &str, path: &str) {
        for scenario in list {
            self.scenario(*scenario, reason, path);
        }
    }

    /// Every functional Web scenario; release performance stays explicit.
    fn web_all(&mut self, reason: &str, path: &str) {
        for scenario in Scenario::selectable().filter(|s| {
            !matches!(
                s,
                Scenario::Main | Scenario::PerfBaseline | Scenario::ReturnPosition
            )
        }) {
            self.scenario(scenario, reason, path);
        }
    }

    fn identity_web(&mut self, reason: &str, path: &str) {
        for s in Scenario::selectable().filter(|s| s.identity()) {
            self.scenario(s, reason, path);
        }
    }

    pub fn owner(&mut self, path: &str, owner: &str) {
        self.owners
            .entry(path.into())
            .or_default()
            .insert(owner.into());
    }

    fn manual(&mut self, what: &str, reason: &str, path: &str) {
        self.manual
            .entry(what.into())
            .or_default()
            .note(reason, path);
    }

    fn artifact(&mut self, what: &str, reason: &str, path: &str) {
        self.artifacts
            .entry(what.into())
            .or_default()
            .note(reason, path);
    }

    fn consumer(&mut self, check: &str, reason: &str, path: &str) {
        self.consumers
            .entry(check.into())
            .or_default()
            .note(reason, path);
    }

    /// Broad selection also compiles and tests every workspace owner directly: integration
    /// targets and the `api` variant are outside `verify quick`.
    pub fn expand_broad(&mut self, g: &Graph) {
        if self.fallback.is_empty() {
            return;
        }
        let paths: Vec<String> = self.fallback.values().flatten().cloned().collect();
        for package in &g.packages {
            for path in &paths {
                direct(self, Some(package), path, "fallback: every workspace owner");
            }
        }
    }

    pub fn fallback(&mut self, reason: &str, path: &str) {
        self.fallback
            .entry(reason.into())
            .or_default()
            .insert(path.into());
        let why = format!("fallback: {reason}");
        for cmd in BROAD {
            self.add(cmd, &why, path);
        }
        self.web_all(&why, path);
        self.scenario(Scenario::PerfBaseline, &why, path);
        self.scenario(Scenario::ReturnPosition, &why, path);
        self.manual(
            "review opt-in, native, formal-bound and release obligations in the feature map",
            &why,
            path,
        );
    }

    /// Selected root commands. Web scenarios merge into one dev invocation (or the full
    /// default lane), separate functional opt-ins and one release performance invocation.
    pub fn commands(&self) -> BTreeMap<String, Entry> {
        let mut out = self.checks.clone();
        let mut merge = |cmd: String, names: &[&str]| {
            let entry = out.entry(cmd).or_default();
            for name in names {
                if let Some(e) = self.scenarios.get(name) {
                    for reason in &e.reasons {
                        entry.reasons.insert(format!("{name}: {reason}"));
                    }
                    entry.paths.extend(e.paths.iter().cloned());
                }
            }
        };
        let picked = |filter: &dyn Fn(Scenario) -> bool| -> Vec<&'static str> {
            Scenario::selectable()
                .filter(|s| *s != Scenario::Main && filter(*s))
                .map(Scenario::name)
                .filter(|n| self.scenarios.contains_key(n))
                .collect()
        };
        let dev = picked(&|s| Scenario::ALL.contains(&s));
        if dev.len() == Scenario::ALL.len() {
            merge("verify web".into(), &dev);
        } else if !dev.is_empty() {
            let flags: Vec<_> = dev.iter().map(|n| format!("--scenario {n}")).collect();
            merge(format!("verify web {}", flags.join(" ")), &dev);
        }
        for name in picked(&|s| {
            !Scenario::ALL.contains(&s)
                && !matches!(s, Scenario::PerfBaseline | Scenario::ReturnPosition)
        }) {
            merge(format!("verify web --scenario {name}"), &[name]);
        }
        let perf = picked(&|s| matches!(s, Scenario::PerfBaseline | Scenario::ReturnPosition));
        if !perf.is_empty() {
            let flags: Vec<_> = perf.iter().map(|n| format!("--scenario {n}")).collect();
            merge(format!("verify web {} --release", flags.join(" ")), &perf);
        }
        out
    }
}

/// Commands that a broader selected command already executes with the same or stronger
/// assertions. The broader command keeps the reasons of the subsumed one.
pub fn subsumed(commands: &BTreeMap<String, Entry>) -> BTreeMap<String, &'static str> {
    let mut out = BTreeMap::new();
    for cmd in commands.keys() {
        let words: Vec<_> = cmd.split_ascii_whitespace().collect();
        let by = match words.as_slice() {
            // quick: workspace clippy -D warnings over all targets with default features,
            // the three WASM configurations, workspace lib/bins tests, App doctests, the SSR
            // boundary test and every static step including docs.
            ["verify", "static", _] => Some("verify quick"),
            ["verify", "check", _, "--target", t] if *t != "api" => Some("verify quick"),
            ["verify", "unit", _] => Some("verify quick"),
            ["verify", "unit", "app", "--test", "ssr_boundary"] => Some("verify quick"),
            ["verify", "postgres", _] => Some("verify postgres"),
            _ => None,
        };
        if let Some(by) = by
            && commands.contains_key(by)
        {
            out.insert(cmd.clone(), by);
        }
    }
    out
}

const WORKSPACE_FILES: [&str; 6] = [
    "Cargo.lock",
    "Cargo.toml",
    "rust-toolchain.toml",
    "rust-toolchain",
    ".gitignore",
    ".gitattributes",
];
const DOC_DIRS: [&str; 6] = [
    "docs/",
    "prd/",
    "decisions/",
    ".agents/",
    ".claude/",
    ".codex/",
];
const EDITOR_DIR: &str = "apps/vink-web/editor/";
const EMPTY_ARTWORK_DIR: &str = "apps/vink-web/assets/empty/";
const PREVIEW_DIR: &str = "packages/ui-system/leptos/preview/";
/// Gradle projects of `ui-preview cmp verify` (packages/ui-system/cmp/settings.gradle.kts).
const CMP_DIRS: [&str; 2] = ["packages/ui-system/cmp/", "packages/vink-patterns/cmp/"];
const SHARED_UI_DIR: &str = "packages/ui-system/shared/";

/// Browser fixture modules imported by `browser-smoke.mjs`, mapped to the scenarios whose
/// phases call them (derived from the dispatcher and `web.rs`), plus manual phases with
/// distinct assertions or setup. Helper modules map through their importers. An unregistered
/// module with no registered caller broadens to the full Web lane.
#[rustfmt::skip]
pub const BROWSER_MODULES: &[(&str, &[Scenario], &[&str])] = {
    use Scenario as S;
    &[
        ("apps/vink-web/e2e/auth-password-smoke.mjs", &[S::Auth, S::Password, S::Standalone, S::AccountDeletion], &[]),
        ("apps/vink-web/e2e/auth-smoke.mjs", &[S::Auth], &[]),
        ("apps/vink-web/e2e/author-search-smoke.mjs", &[S::AuthorSearch], &[]),
        ("apps/vink-web/e2e/author-works-smoke.mjs", &[S::Desk], &[]),
        ("apps/vink-web/e2e/desk-empty-smoke.mjs", &[S::Desk], &["desk-empty-context"]),
        ("apps/vink-web/e2e/desk-lifecycle.mjs", &[S::Desk], &["desk-empty-context"]),
        ("apps/vink-web/e2e/discovery-chip-geometry.mjs", &[], &["discovery-chip-geometry"]),
        ("apps/vink-web/e2e/discovery-return-position.mjs", &[S::ReturnPosition], &[]),
        ("apps/vink-web/e2e/discovery-smoke.mjs", &[S::Discovery, S::Reader], &[]),
        ("apps/vink-web/e2e/document-budget.mjs", &[], &["document-budget"]),
        ("apps/vink-web/e2e/document.mjs", &[S::Reader, S::Editor, S::EditorChrome, S::EditorFormat], &[]),
        ("apps/vink-web/e2e/draft-pagination-smoke.mjs", &[S::Desk], &[]),
        ("apps/vink-web/e2e/editor-autosave.mjs", &[S::Editor, S::EditorChrome, S::Restart], &[]),
        ("apps/vink-web/e2e/editor-desk-regressions.mjs", &[S::EditorFirstDraft, S::EditorFormat], &[]),
        ("apps/vink-web/e2e/editor-markdown-budget.mjs", &[S::EditorFormat], &[]),
        ("apps/vink-web/e2e/editor-paper-first-smoke.mjs", &[S::EditorChrome, S::EditorFormat], &[]),
        ("apps/vink-web/e2e/editor-writing-smoke.mjs", &[S::EditorChrome], &[]),
        ("apps/vink-web/e2e/endorsement-smoke.mjs", &[S::Discussion, S::Endorsement], &[]),
        ("apps/vink-web/e2e/markdown-reader-smoke.mjs", &[S::Review], &[]),
        ("apps/vink-web/e2e/moderation-queue-smoke.mjs", &[S::Review], &[]),
        ("apps/vink-web/e2e/notifications-smoke.mjs", &[S::Notifications], &[]),
        ("apps/vink-web/e2e/public-storage-smoke.mjs", &[S::Storage], &[]),
        ("apps/vink-web/e2e/publication-author-smoke.mjs", &[S::Publication], &[]),
        ("apps/vink-web/e2e/reader-auth-smoke.mjs", &[S::Auth], &[]),
        ("apps/vink-web/e2e/reader-containment.mjs", &[], &["reader-containment"]),
        ("apps/vink-web/e2e/reader-intent-cost.mjs", &[], &["reader-intent-cost"]),
        ("apps/vink-web/e2e/reader-ssr-boundary.mjs", &[S::Reader], &["reader-guest-history"]),
        ("apps/vink-web/e2e/reader-startup.mjs", &[], &["reader-startup"]),
        ("apps/vink-web/e2e/review-withdrawn-smoke.mjs", &[S::Review], &[]),
        ("apps/vink-web/e2e/visual-evidence.mjs",
            &[S::Reader, S::Discussion, S::Appearance, S::Desk, S::Editor, S::EditorChrome, S::EditorFormat],
            &[]),
        ("apps/vink-web/e2e/web-perf-baseline.mjs", &[S::PerfBaseline], &[]),
        ("apps/vink-web/e2e/web-request-ledger.mjs", &[S::Reader], &[]),
        ("apps/vink-web/e2e/work-views-smoke.mjs", &[S::Reader], &[]),
        ("packages/ui-system/leptos/preview/e2e/select-geometry.mjs", &[S::Desk], &[]),
    ]
};
pub const BROWSER_DISPATCHER: &str = "apps/vink-web/e2e/browser-smoke.mjs";
/// Node tests and the static selector that runs them (`None`: no xtask selector).
pub const NODE_TESTS: [(&str, Option<&str>); 7] = [
    (
        "apps/vink-web/e2e/visual-evidence.test.mjs",
        Some("verify static visual"),
    ),
    (
        "apps/vink-web/e2e/web-perf-baseline.test.mjs",
        Some("verify static perf"),
    ),
    ("apps/vink-web/e2e/appearance-bootstrap.test.mjs", None),
    ("apps/vink-web/assets/empty/verify.test.mjs", None),
    (
        "packages/expressions/validate.test.mjs",
        Some("verify static assets"),
    ),
    (
        "packages/ui-system/leptos/preview/e2e/focus-capture.test.mjs",
        Some("verify preview"),
    ),
    (
        "packages/ui-system/leptos/preview/e2e/overlay-capture.test.mjs",
        Some("verify preview"),
    ),
];
/// SQL fixtures that `web.rs` loads for specific scenarios.
const WEB_SQL_FIXTURES: [(&str, &[Scenario]); 3] = [
    (
        "apps/vink-web/e2e/fixtures/discovery-publications.sql",
        &[
            Scenario::Discovery,
            Scenario::PerfBaseline,
            Scenario::ReturnPosition,
        ],
    ),
    (
        "apps/vink-web/e2e/fixtures/reader-contextual-auth.sql",
        &[Scenario::Auth],
    ),
    (
        "apps/vink-web/e2e/fixtures/web-perf-journey.sql",
        &[Scenario::PerfBaseline, Scenario::ReturnPosition],
    ),
];
/// Editor runtime consumers: draft creation/editing, restart buffers, the #264 Markdown review
/// editor, publication after autosave and the CSR editor.
const EDITOR_SCENARIOS: [Scenario; 8] = [
    Scenario::EditorFirstDraft,
    Scenario::EditorChrome,
    Scenario::Editor,
    Scenario::EditorFormat,
    Scenario::Restart,
    Scenario::Review,
    Scenario::Publication,
    Scenario::Standalone,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Pure parsing/planning/doctor code: the owner's tests are the boundary.
    Unit,
    Docs,
    Repo,
    Tokens,
    Discussion,
    Expressions,
    Kani,
    Web,
    /// Hosts the release performance lab as well as the default Web scenarios.
    WebPerf,
    Preview,
    Cmp,
    Postgres,
    Infra,
    /// Runner code shared by every lane: any lane could observe the change.
    Shared,
}

/// Every xtask module and the lanes whose behavior it implements.
#[rustfmt::skip]
pub const XTASK_MODULES: &[(&str, &[Role])] = {
    use Role as R;
    &[
        ("build", &[R::WebPerf, R::Preview]),
        ("changed", &[R::Unit]),
        ("checks", &[R::Repo, R::Docs]),
        ("cli", &[R::Unit]),
        ("decision", &[R::Docs]),
        ("discussion", &[R::Discussion]),
        ("doctor", &[R::Unit]),
        ("env", &[R::Shared]),
        ("expressions", &[R::Expressions]),
        ("identity", &[R::Postgres]),
        ("infra", &[R::Infra]),
        ("kani", &[R::Kani]),
        ("legacy", &[R::Shared]),
        ("lifecycle", &[R::Shared]),
        ("main", &[R::Shared]),
        ("notifications", &[R::Postgres]),
        ("pgurl", &[R::Postgres, R::Web]),
        ("plan", &[R::Unit]),
        ("process", &[R::Shared]),
        ("provision", &[R::Postgres]),
        ("tailwind", &[R::Web, R::Preview]),
        ("ui_navigation", &[R::Preview]),
        ("ui_preview", &[R::Preview, R::Cmp]),
        ("ui_qualification", &[R::Preview, R::Cmp]),
        ("ui_states", &[R::Preview]),
        ("ui_toggle", &[R::Preview]),
        ("ui_tokens", &[R::Tokens, R::Cmp]),
        ("verify", &[R::Shared]),
        ("web", &[R::WebPerf]),
        ("web_assets", &[R::Web]),
        ("web_boundary", &[R::Web]),
        ("workflow", &[R::Shared]),
    ]
};

/// Directory prefixes of the production modules each `verify discussion` replay filter selects.
pub fn discussion_sources() -> Vec<String> {
    let mut out: Vec<String> = crate::discussion::FILTERS
        .iter()
        .map(|filter| {
            let module: Vec<_> = filter
                .split("::")
                .filter(|p| !p.is_empty() && !p.ends_with("tests") && !p.ends_with("_replay"))
                .collect();
            format!("services/vink-app/src/{}", module.join("/"))
        })
        .collect();
    let web = crate::discussion::WEB_REPLAY_FILTER.trim_end_matches("::");
    out.push(format!("apps/vink-web/src/{web}"));
    out.sort();
    out.dedup();
    out
}

pub fn classify(sel: &mut Selection, g: &Graph, path: &str, includers: &[String]) {
    if WORKSPACE_FILES.contains(&path) || path.starts_with(".cargo/") {
        sel.owner(path, "workspace");
        sel.fallback(
            "workspace-wide build, toolchain, lock or Git input changes every owner",
            path,
        );
        return;
    }
    let mut claimed = false;
    if is_docs(path) {
        claimed = true;
        sel.owner(path, "docs");
        sel.add(
            "verify static docs",
            "docs/skills/decisions: skill metadata, decision registry and relative links",
            path,
        );
    }
    if path.starts_with(".github/") {
        claimed = true;
        sel.owner(path, "ci");
        sel.add(
            "verify static repo",
            "CI definition: repository guards that read tool pins",
            path,
        );
        sel.manual(
            "GitHub Actions run for the changed workflow",
            "local lanes never execute CI definitions (GITHUB_CI evidence is separate)",
            path,
        );
    }
    claimed |= formal(sel, g, path);
    claimed |= tokens(sel, path);
    claimed |= stylesheet(sel, g, path);
    claimed |= web_assets(sel, path);
    claimed |= browser(sel, g, path);
    claimed |= contracts(sel, path);
    let migration = sql(sel, path);
    claimed |= migration;
    claimed |= cargo(sel, g, path);
    claimed |= other(sel, path);
    for includer in includers {
        claimed = true;
        let reason = format!("compile input of {includer} (include_*/module/migrate!)");
        if includer.starts_with("xtask/") || migration {
            // Data embedded by the harness, or migrations whose lanes the SQL rule owns:
            // rebuild and test the embedding package without its source-area lanes.
            let package = g.package_of(includer);
            if let Some(package) = package {
                sel.owner(path, &format!("{} (compile input)", package.label()));
            }
            direct(sel, package, path, &reason);
        } else {
            rust_input(sel, g, includer, path, &reason);
        }
    }
    if !claimed {
        sel.owner(path, "unknown");
        sel.unknown.insert(path.into());
        sel.fallback("no owner rule claims this input", path);
    }
}

/// Markdown anywhere is checked for links; fixture Markdown is test data, not docs.
fn is_docs(path: &str) -> bool {
    DOC_DIRS.iter().any(|d| path.starts_with(d))
        || (path.ends_with(".md") && !path.contains("/fixtures/"))
        || path == ".env.example"
}

fn formal(sel: &mut Selection, g: &Graph, path: &str) -> bool {
    let mut hit = false;
    for (dir, lane, what) in [
        (
            "services/vink-app/behavior/",
            "verify discussion",
            "Quint discussion/publication/target models, vectors and Rust replay",
        ),
        (
            "apps/vink-web/behavior/",
            "verify discussion",
            "Web read-lifecycle model and ReadOwner/PageChain replay",
        ),
        (
            "packages/expressions/behavior/",
            "verify expressions",
            "expression lifecycle model, trace drift and replay",
        ),
    ] {
        if path.starts_with(dir) {
            hit = true;
            sel.owner(path, "formal model");
            sel.add(lane, &format!("formal: {what}"), path);
        }
    }
    if discussion_sources().iter().any(|p| path.starts_with(p)) {
        sel.add(
            "verify discussion",
            "formal: production seam replayed by the discussion lane",
            path,
        );
    }
    if path.starts_with("packages/expressions/src/") {
        sel.add(
            "verify expressions",
            "formal: shared expression policy replay",
            path,
        );
    }
    if g.kani_inputs.iter().any(|p| path.starts_with(p.as_str())) {
        hit |= path.contains("/kani/");
        sel.add(
            "verify kani",
            "formal: compiled by an owner-local Kani manifest (all harnesses must verify)",
            path,
        );
    }
    hit
}

fn tokens(sel: &mut Selection, path: &str) -> bool {
    use crate::ui_tokens as t;
    let outputs = [
        t::CSS_OUT,
        t::TAILWIND_THEME_OUT,
        t::RUST_TOKENS_OUT,
        t::RUST_CATALOG_OUT,
        t::RUST_ICONS_OUT,
        t::RUST_IDENTITY_OUT,
        crate::ui_states::RUST_STATES_OUT,
        crate::ui_navigation::RUST_NAVIGATION_OUT,
        crate::ui_toggle::RUST_TOGGLE_OUT,
        t::KOTLIN_TOKENS_OUT,
        t::KOTLIN_CATALOG_OUT,
        t::KOTLIN_ICONS_OUT,
        t::KOTLIN_IDENTITY_OUT,
        crate::ui_states::KOTLIN_STATES_OUT,
        crate::ui_navigation::KOTLIN_NAVIGATION_OUT,
        crate::ui_toggle::KOTLIN_TOGGLE_OUT,
    ];
    let input = path.starts_with(SHARED_UI_DIR);
    if !input && !outputs.contains(&path) {
        return false;
    }
    sel.owner(path, "ui tokens");
    sel.add(
        "verify static tokens",
        "generated UI tokens: source valid and CSS/Rust/Kotlin outputs match",
        path,
    );
    if input {
        let why = "shared UI source regenerates Web/Preview CSS, vink-ui Rust and CMP Kotlin";
        sel.artifact("generated UI token outputs", why, path);
        sel.web_all(why, path);
        sel.add("verify preview", why, path);
        sel.add("ui-preview cmp verify", why, path);
        sel.manual(
            "Android/iOS diagnostic hosts (opt-in, VINK_UI_NATIVE_TARGETS / ui-preview android|ios)",
            why,
            path,
        );
    } else if path.ends_with(".kt") {
        sel.add(
            "ui-preview cmp verify",
            "generated Kotlin feeds Compose Multiplatform",
            path,
        );
    }
    true
}

fn stylesheet(sel: &mut Selection, g: &Graph, path: &str) -> bool {
    let mut hit = false;
    for sheet in &g.stylesheets {
        let web = sheet.entry == crate::tailwind::WEB_ENTRY;
        let artifact = if web {
            "Web stylesheet /pkg/vink.css (Tailwind)"
        } else {
            "Preview stylesheet vink-preview.css (Tailwind)"
        };
        let imported = sheet.files.contains(path);
        let package = path.starts_with(crate::tailwind::DIR)
            && path
                .rsplit('/')
                .next()
                .is_some_and(|name| name.starts_with("package"));
        if imported || package {
            hit = true;
            sel.owner(path, "stylesheet");
            let why = format!("CSS import closure of {}", sheet.entry);
            sel.artifact(artifact, &why, path);
            sel.add(
                "verify static repo",
                "Tailwind input guard and package boundaries",
                path,
            );
            sel.add(
                "verify static tokens",
                "stylesheet build runs the token check first",
                path,
            );
            if web {
                sel.web_all(&format!("{why}: global to every SSR/CSR route"), path);
                sel.manual(
                    "open same-scenario captures at the affected themes, sizes and text scales",
                    "CSS changes need inspected visual evidence, not only DOM checks",
                    path,
                );
            } else {
                sel.add("verify preview", &why, path);
            }
        } else if sheet.sources.iter().any(|s| path.starts_with(s.as_str())) {
            sel.artifact(
                artifact,
                &format!("Tailwind scans class names in {}", sheet.entry),
                path,
            );
        }
    }
    hit
}

fn web_assets(sel: &mut Selection, path: &str) -> bool {
    if path.starts_with(EDITOR_DIR) {
        let why = "Editor runtime: typecheck, Markdown corpus and bundle run in every Web build";
        sel.owner(path, "editor");
        sel.artifact("Editor runtime /pkg/vink-editor.js", why, path);
        sel.add(
            "verify static repo",
            "Editor packages are pinned and locked",
            path,
        );
        sel.scenarios(&EDITOR_SCENARIOS, why, path);
        return true;
    }
    if path.starts_with(&format!("{}/", crate::checks::FONTS_DIR))
        || path.starts_with(&format!("{}/", crate::checks::BRAND_DIR))
    {
        let why = "static Web asset (#500 receipt: file set and SHA-256 must match)";
        sel.owner(path, "web assets");
        sel.artifact("Web static fonts/brand", why, path);
        sel.add(
            "verify static repo",
            "font and brand provenance guards",
            path,
        );
        sel.web_all(why, path);
        return true;
    }
    if path.starts_with(EMPTY_ARTWORK_DIR) {
        let why = "empty artwork derivatives copied into /pkg";
        sel.owner(path, "web assets");
        sel.artifact("Web empty artwork", why, path);
        sel.add(
            "verify static assets",
            "WebP container, dimensions, hash and byte budget",
            path,
        );
        sel.web_all(why, path);
        return true;
    }
    if matches!(
        path,
        "apps/vink-web/index.html"
            | "apps/vink-web/dev-server.mjs"
            | "infra/cloudflare/production/local-edge.mjs"
    ) {
        sel.owner(path, "standalone shell");
        sel.scenario(
            Scenario::Standalone,
            "CSR index, local edge or dev server used by the standalone fixture",
            path,
        );
        return true;
    }
    false
}

fn browser(sel: &mut Selection, g: &Graph, path: &str) -> bool {
    if path == BROWSER_DISPATCHER {
        let why = "browser dispatcher hosts every phase and inline assertion";
        sel.owner(path, "browser fixture");
        sel.web_all(why, path);
        sel.scenario(Scenario::PerfBaseline, why, path);
        sel.scenario(Scenario::ReturnPosition, why, path);
        return true;
    }
    if let Some((_, selector)) = NODE_TESTS.iter().find(|(t, _)| *t == path) {
        sel.owner(path, "node test");
        node_test(sel, path, *selector, path);
        return true;
    }
    if let Some((_, scenarios)) = WEB_SQL_FIXTURES.iter().find(|(f, _)| *f == path) {
        sel.owner(path, "browser fixture");
        sel.scenarios(
            scenarios,
            "SQL fixture loaded by web.rs for this scenario",
            path,
        );
        return true;
    }
    if !super::graph::BROWSER_DIRS
        .iter()
        .any(|d| path.starts_with(d))
    {
        return false;
    }
    if path.starts_with(PREVIEW_DIR) {
        sel.add(
            "verify preview",
            "Preview browser fixture or capture oracle",
            path,
        );
    }
    sel.owner(path, "browser fixture");
    let mut roots = vec![path.to_string()];
    roots.extend(g.importer_closure(path));
    let mut mapped = false;
    for root in &roots {
        let via = if root == path {
            "browser fixture called by this scenario".to_string()
        } else {
            format!("imported (transitively) by {root}")
        };
        if root == BROWSER_DISPATCHER {
            // Reaching the dispatcher through a registered module is mapped by that module.
            let direct = g
                .importers
                .get(path)
                .is_some_and(|i| i.contains(BROWSER_DISPATCHER));
            if direct && !BROWSER_MODULES.iter().any(|(m, _, _)| *m == path) {
                sel.fallback(
                    "browser module imported by the dispatcher is missing from BROWSER_MODULES",
                    path,
                );
                mapped = true;
            }
        } else if let Some((_, scenarios, phases)) =
            BROWSER_MODULES.iter().find(|(m, _, _)| m == root)
        {
            sel.scenarios(scenarios, &via, path);
            manual_phases(sel, phases, path);
            mapped = true;
        } else if let Some((_, selector)) = NODE_TESTS.iter().find(|(t, _)| t == root) {
            node_test(sel, root, *selector, path);
            mapped = true;
        }
    }
    if !mapped && !path.starts_with(PREVIEW_DIR) {
        sel.fallback("browser module with no registered caller", path);
    }
    true
}

fn manual_phases(sel: &mut Selection, phases: &[&str], path: &str) {
    for phase in phases {
        sel.manual(
            &format!("browser phase `{phase}`: no xtask scenario runs it (node apps/vink-web/e2e/browser-smoke.mjs --phase {phase} against a prepared App)"),
            "browser assertions or setup specific to this manual phase",
            path,
        );
    }
}

fn node_test(sel: &mut Selection, test: &str, selector: Option<&str>, path: &str) {
    match selector {
        Some(cmd) => sel.add(cmd, &format!("Node test {test}"), path),
        None => sel.manual(
            &format!("node --test {test} (no xtask selector runs it)"),
            "Node test outside every xtask lane",
            path,
        ),
    }
}

fn contracts(sel: &mut Selection, path: &str) -> bool {
    let (owner, checks): (&str, &[&str]) = if path.starts_with("services/vink-app/contracts/") {
        (
            "app HTTP contract",
            &[
                "verify unit app",
                "verify postgres app",
                "verify unit web",
                "verify unit notifications",
            ],
        )
    } else if path.starts_with("services/vink-identity/contracts/") {
        (
            "identity RPC contract",
            &[
                "verify unit identity",
                "verify postgres identity",
                "verify unit app",
            ],
        )
    } else if path.starts_with("services/vink-notifications/contracts/") {
        (
            "notifications RPC contract",
            &[
                "verify unit notifications",
                "verify postgres",
                "verify unit app",
            ],
        )
    } else {
        return false;
    };
    sel.owner(path, owner);
    for cmd in checks {
        sel.add(
            cmd,
            "contract: producer and consumer fixture/shape tests",
            path,
        );
    }
    if owner.starts_with("identity") {
        sel.identity_web("Identity contract consumed across real processes", path);
    } else {
        sel.consumer(
            "verify web --scenario <consumers>",
            "browser consumers of a contract: add the routes' scenarios when the response shape or status changes",
            path,
        );
    }
    true
}

fn sql(sel: &mut Selection, path: &str) -> bool {
    if path.starts_with("services/postgres/") {
        sel.owner(path, "PostgreSQL provisioning");
        sel.add(
            "verify postgres",
            "provisioning, roles and recovery affect every database owner",
            path,
        );
        return true;
    }
    if path.starts_with("packages/store-postgres/tests/migrations/") {
        sel.owner(path, "store test migrations");
        sel.add(
            "verify postgres store",
            "shared migrator fixtures on a real cluster",
            path,
        );
        return true;
    }
    for (dir, owner) in [
        ("services/vink-app/", "app"),
        ("services/vink-identity/", "identity"),
        ("services/vink-notifications/", "notifications"),
    ] {
        let Some(local) = path.strip_prefix(dir) else {
            continue;
        };
        if !(local.starts_with("migrations/") || local.starts_with("migrations-rollback/")) {
            continue;
        }
        sel.owner(path, &format!("{owner} migrations"));
        sel.add(
            &format!("verify postgres {owner}"),
            "migration: owner persistence tests on a real cluster",
            path,
        );
        sel.add(
            "verify postgres lifecycle",
            "migration: chain, role fences, provisioning, backup/restore and rollback",
            path,
        );
        if owner == "app" {
            sel.consumer(
                "verify web",
                "Web scenarios use App migrations only through vink-app persistence (selected when that source changes)",
                path,
            );
        }
        return true;
    }
    false
}

fn other(sel: &mut Selection, path: &str) -> bool {
    if CMP_DIRS.iter().any(|d| path.starts_with(d)) {
        sel.owner(path, "cmp");
        sel.add(
            "ui-preview cmp verify",
            "Compose Multiplatform UI and desktop preview",
            path,
        );
        sel.manual(
            "Android/iOS diagnostic hosts (opt-in, ui-preview android|ios)",
            "native targets are outside the default lanes",
            path,
        );
        return true;
    }
    if path.starts_with(PREVIEW_DIR) && !path.ends_with(".rs") {
        sel.owner(path, "preview");
        sel.add(
            "verify preview",
            "Leptos preview assets and browser checks",
            path,
        );
        return true;
    }
    if path.starts_with("packages/expressions/")
        && !path.ends_with(".rs")
        && !path.starts_with("packages/expressions/behavior/")
        && !path.starts_with("packages/expressions/kani/")
    {
        sel.owner(path, "expressions assets");
        sel.add(
            "verify static assets",
            "expression manifest, Unicode, hash and drift",
            path,
        );
        sel.add("verify expressions", "expression catalog and model", path);
        sel.add("verify preview", "expression renderer in Preview", path);
        return true;
    }
    // Markdown under infra/ is documentation only (claimed by the docs rule).
    if (path.starts_with("infra/") && !path.ends_with(".md")) || path == ".dockerignore" {
        sel.owner(path, "infra");
        sel.add(
            "verify unit xtask",
            "infra inventory and DNS plan tests live in xtask",
            path,
        );
        sel.manual(
            "operator/OCI verification of infra assets (infra/production/README.md)",
            "deployment assets are not executed by local lanes",
            path,
        );
        return true;
    }
    if path.starts_with("tools/") {
        sel.owner(path, "local budget");
        sel.manual(
            "python3.13 tools/test_local_budget.py (no xtask selector)",
            "local resource launcher and its tests",
            path,
        );
        return true;
    }
    false
}

fn cargo(sel: &mut Selection, g: &Graph, path: &str) -> bool {
    let Some(package) = g.package_of(path) else {
        return false;
    };
    let local = &path[package.dir.len()..];
    let label = package.label();
    if local == "Cargo.toml" || local == "build.rs" {
        sel.owner(path, &label);
        let why = "manifest/build script: dependencies, features and targets of this package";
        // The manifest affects the whole package: its composition root stands for every area.
        let root = if package.owner == Some(UnitOwner::Xtask) {
            "src/main.rs"
        } else {
            "src/lib.rs"
        };
        rust_source(sel, g, package, root, path, why);
        sel.manual(
            "confirm Cargo.lock is unchanged or review the lock diff (selected separately when it changes)",
            why,
            path,
        );
        return true;
    }
    if local.starts_with("src/") {
        sel.owner(path, &label);
        rust_source(sel, g, package, local, path, "source of this package");
        return true;
    }
    if let Some(rest) = local.strip_prefix("tests/") {
        sel.owner(path, &label);
        direct_compile(sel, package, path, "test source of this package");
        let module_dir = rest.split_once('/').map(|(dir, _)| dir);
        let targets: Vec<_> = package
            .tests()
            .filter(|t| {
                t.src == path
                    || module_dir.is_some_and(|d| t.src.ends_with(&format!("tests/{d}.rs")))
            })
            .collect();
        let targets = if targets.is_empty() {
            package.tests().collect() // shared support or fixtures: every test target
        } else {
            targets
        };
        for target in targets {
            test_target(sel, package, &target.name, path);
        }
        return true;
    }
    if local.starts_with("examples/") || local.starts_with("benches/") {
        sel.owner(path, &label);
        direct_compile(sel, package, path, "example/bench target of this package");
        return true;
    }
    false
}

/// A source file of `includer` changed (or one of its compile inputs did).
fn rust_input(sel: &mut Selection, g: &Graph, includer: &str, path: &str, reason: &str) {
    let Some(package) = g.package_of(includer) else {
        sel.fallback("compile input of a file outside the workspace", path);
        return;
    };
    sel.owner(path, &format!("{} (compile input)", package.label()));
    let local = &includer[package.dir.len()..];
    if let Some(rest) = local.strip_prefix("tests/") {
        direct_compile(sel, package, path, reason);
        let stem = rest.split('/').next().unwrap_or("").trim_end_matches(".rs");
        for target in package.tests().filter(|t| t.name == stem) {
            test_target(sel, package, &target.name, path);
        }
    } else {
        rust_source(sel, g, package, local, path, reason);
    }
}

fn rust_source(
    sel: &mut Selection,
    g: &Graph,
    package: &Package,
    local: &str,
    path: &str,
    why: &str,
) {
    direct(sel, Some(package), path, why);
    package_lanes(sel, package, local, path);
    for consumer in g.dependents(&package.name) {
        let reason = format!(
            "dependency closure: {} depends on {}",
            consumer.name, package.name
        );
        direct(sel, Some(consumer), path, &reason);
        boundary(sel, consumer, path, &reason);
    }
}

fn owner_or_fallback(sel: &mut Selection, package: &Package, path: &str) -> Option<&'static str> {
    match package.owner {
        Some(owner) => Some(owner.name()),
        None => {
            sel.fallback(
                "workspace package has no xtask owner selector (extend UnitOwner)",
                path,
            );
            None
        }
    }
}

fn direct_compile(sel: &mut Selection, package: &Package, path: &str, why: &str) {
    let Some(owner) = owner_or_fallback(sel, package, path) else {
        return;
    };
    sel.add(
        &format!("verify check {owner} --target all-targets"),
        &format!("{why}: compile every target"),
        path,
    );
}

/// Compile all targets and feature variants, run lib/bins unit tests and every integration
/// target except `postgres`, which needs its database selector.
fn direct(sel: &mut Selection, package: Option<&Package>, path: &str, why: &str) {
    let Some(package) = package else {
        sel.fallback("xtask package missing from cargo metadata", path);
        return;
    };
    direct_compile(sel, package, path, why);
    let Some(owner) = package.owner else {
        return;
    };
    for variant in ["api", "ssr", "hydrate", "csr"] {
        if CheckTarget::parse(owner, variant).is_ok() && declares(package, variant) {
            sel.add(
                &format!("verify check {} --target {variant}", owner.name()),
                &format!("{why}: feature/target variant `{variant}`"),
                path,
            );
        }
    }
    if package
        .targets
        .iter()
        .any(|t| matches!(t.kind.as_str(), "lib" | "bin" | "cdylib" | "rlib"))
    {
        sel.add(
            &format!("verify unit {}", owner.name()),
            &format!("{why}: lib/bins unit and contract tests"),
            path,
        );
    }
    for target in package.tests().filter(|t| t.name != "postgres") {
        test_target(sel, package, &target.name, path);
    }
}

/// A compile variant exists only when Cargo declares the feature it selects.
pub fn declares(package: &Package, variant: &str) -> bool {
    let feature = match variant {
        "api" => "default",
        "ssr" if package.features.contains("legacy-ssr") => "legacy-ssr",
        other => other,
    };
    package.features.contains(feature)
}

fn test_target(sel: &mut Selection, package: &Package, name: &str, path: &str) {
    let Some(owner) = package.owner else {
        return;
    };
    if name == "postgres" {
        sel.add(
            &format!("verify postgres {}", owner.name()),
            "integration target `postgres` on a real cluster",
            path,
        );
    } else {
        sel.add(
            &format!("verify unit {} --test {name}", owner.name()),
            &format!("integration target `{name}` of {}", package.name),
            path,
        );
    }
}

/// Lanes for a direct change to `package`, refined by source area where one exists.
fn package_lanes(sel: &mut Selection, package: &Package, local: &str, path: &str) {
    use UnitOwner as U;
    let Some(owner) = package.owner else { return };
    match owner {
        U::App => app_lanes(sel, local, path),
        U::Web => sel.web_all("Web crate renders every SSR/hydrated/CSR route", path),
        U::Identity => {
            sel.add(
                "verify postgres identity",
                "Identity service persistence and readiness",
                path,
            );
            sel.identity_web("real Identity process behind the auth scenarios", path);
        }
        U::Notifications => {
            sel.add(
                "verify postgres",
                "service plus App Gateway/worker relay across processes, roles and recovery",
                path,
            );
            sel.add("verify unit app", "App Gateway and relay consumers", path);
        }
        U::Store => {
            sel.add(
                "verify postgres",
                "shared pool/migrator used by every database owner and the lifecycle",
                path,
            );
            sel.web_all(
                "xtask Web verification creates its scratch databases with the shared migrator",
                path,
            );
        }
        U::Expressions => {
            sel.add(
                "verify static assets",
                "expression catalog and manifest",
                path,
            );
        }
        U::Preview => sel.add("verify preview", "Leptos preview browser checks", path),
        U::Xtask => xtask_lanes(sel, local, path),
        U::Ui | U::Patterns | U::Markdown | U::WorkViews => {}
    }
}

fn app_lanes(sel: &mut Selection, local: &str, path: &str) {
    let pg = "verify postgres app";
    if local.starts_with("src/domain/") || local == "src/domain.rs" {
        sel.consumer(
            pg,
            "pure domain: add it when the rule changes persisted or HTTP-visible behavior",
            path,
        );
        return;
    }
    match local {
        "src/lib.rs" | "src/main.rs" | "src/runtime.rs" | "src/config.rs" | "src/http/ssr.rs"
        | "src/http/mod.rs" => {
            sel.add(pg, "App composition, router or SSR serving", path);
            sel.add(
                "verify postgres lifecycle",
                "binaries, configuration rejection and provisioning",
                path,
            );
            sel.web_all("App serves every Web route", path);
        }
        "src/dev_seed.rs" => {
            sel.add(pg, "sample fixture seed", path);
            sel.web_all("Web scenarios start from the sample seed", path);
        }
        l if l.starts_with("src/notification_")
            || l.starts_with("src/bin/")
            || l.ends_with("_worker.rs") =>
        {
            sel.add(
                "verify postgres",
                "worker/relay processes across App, Notifications and recovery",
                path,
            );
        }
        l if l.starts_with("src/identity/") => {
            sel.add(pg, "App identity client", path);
            sel.identity_web("App talks to the real Identity process", path);
        }
        l if l.starts_with("src/http/")
            || l.starts_with("src/application/")
            || l.starts_with("src/persistence/") =>
        {
            sel.add(pg, "HTTP/application/persistence on a real cluster", path);
            sel.consumer(
                "verify web --scenario <consumers>",
                "add the scenarios of affected routes when a Web-visible response changes",
                path,
            );
        }
        _ => {
            sel.add(pg, "unclassified App source area", path);
            sel.web_all("unclassified App source area", path);
        }
    }
}

fn xtask_lanes(sel: &mut Selection, local: &str, path: &str) {
    let stem = local
        .trim_start_matches("src/")
        .split(['/', '.'])
        .next()
        .unwrap_or_default();
    let Some((_, roles)) = XTASK_MODULES.iter().find(|(m, _)| *m == stem) else {
        sel.fallback("xtask module missing from XTASK_MODULES", path);
        return;
    };
    for role in *roles {
        let why = format!("xtask `{stem}` implements this lane");
        match role {
            Role::Unit => {}
            Role::Docs => sel.add("verify static docs", &why, path),
            Role::Repo => sel.add("verify static repo", &why, path),
            Role::Tokens => sel.add("verify static tokens", &why, path),
            Role::Discussion => sel.add("verify discussion", &why, path),
            Role::Expressions => sel.add("verify expressions", &why, path),
            Role::Kani => sel.add("verify kani", &why, path),
            Role::Web => sel.web_all(&why, path),
            Role::WebPerf => {
                sel.web_all(&why, path);
                sel.scenario(Scenario::PerfBaseline, &why, path);
                sel.scenario(Scenario::ReturnPosition, &why, path);
            }
            Role::Preview => sel.add("verify preview", &why, path),
            Role::Cmp => sel.add("ui-preview cmp verify", &why, path),
            Role::Postgres => sel.add("verify postgres", &why, path),
            Role::Infra => sel.manual(
                "operator/OCI verification of infra assets (infra/production/README.md)",
                &why,
                path,
            ),
            Role::Shared => sel.fallback("shared xtask runner code", path),
        }
    }
}

/// Lanes owned by a package that only depends on the changed one.
fn boundary(sel: &mut Selection, consumer: &Package, path: &str, why: &str) {
    use UnitOwner as U;
    match consumer.owner {
        Some(U::App) => sel.add("verify postgres app", why, path),
        Some(U::Web) => sel.web_all(why, path),
        Some(U::Identity) => {
            sel.add("verify postgres identity", why, path);
            sel.identity_web(why, path);
        }
        Some(U::Notifications) | Some(U::Store) => sel.add("verify postgres", why, path),
        Some(U::Preview) => sel.add("verify preview", why, path),
        Some(U::Expressions) => sel.add("verify expressions", why, path),
        _ => {}
    }
}
