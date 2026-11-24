//! Existing regression lanes; selected checks live with their owning adapter.
use crate::plan::{Tool, UnitOwner};
use crate::process::{Report, cargo};
use crate::{
    TEST_DATABASE_URL, TEST_DATABASE_URL_MISSING, checks, discussion, doctor, env, expressions,
    kani, lifecycle, process, provision, root, ui_tokens, web,
};
use std::process::ExitCode;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaticScope {
    Docs,
    Tokens,
    Assets,
    Visual,
    Perf,
    Fmt,
    Repo,
    All,
}
impl StaticScope {
    const STATIC: [Self; 6] = [
        Self::Tokens,
        Self::Assets,
        Self::Visual,
        Self::Perf,
        Self::Fmt,
        Self::Repo,
    ];

    pub fn parse(scope: &str) -> Result<Self, String> {
        match scope {
            "docs" => Ok(Self::Docs),
            "tokens" => Ok(Self::Tokens),
            "assets" => Ok(Self::Assets),
            "visual" => Ok(Self::Visual),
            "perf" => Ok(Self::Perf),
            "fmt" => Ok(Self::Fmt),
            "repo" => Ok(Self::Repo),
            "all" => Ok(Self::All),
            _ => Err(format!("unknown static scope `{scope}`")),
        }
    }

    pub fn tools(self) -> &'static [Tool] {
        match self {
            Self::All => &[Tool::Node, Tool::Rustfmt],
            Self::Visual | Self::Perf => &[Tool::Node],
            Self::Fmt => &[Tool::Rustfmt],
            _ => &[],
        }
    }

    fn steps(self) -> Vec<Self> {
        if self == Self::All {
            Self::STATIC.to_vec()
        } else {
            vec![self]
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Docs => "documentation checks (skills, decisions, links)",
            Self::Tokens => "ui tokens: source valid, generated CSS/Rust/Kotlin match",
            Self::Assets => {
                "assets: empty artwork derivatives and expression manifest/hash/Unicode/drift"
            }
            Self::Visual => {
                "visual evidence oracles: manifest, PNG, overflow, AX, modal unit tests"
            }
            Self::Perf => {
                "web perf analyzer: lab distribution, transport ledger and negative duplicate control"
            }
            Self::Fmt => "fmt",
            Self::Repo => {
                "repository checks (decisions, skills, links, boundaries, fonts, editor packages)"
            }
            Self::All => unreachable!("all resolves to concrete steps"),
        }
    }

    fn command(self) -> &'static str {
        match self {
            Self::Docs => "xtask checks::docs",
            Self::Tokens => "xtask ui_tokens::run(check=true)",
            Self::Assets => "xtask build::check_empty_artwork and expressions::check",
            Self::Visual => "node --test apps/vink-web/e2e/visual-evidence.test.mjs",
            Self::Perf => "node --test apps/vink-web/e2e/web-perf-baseline.test.mjs",
            Self::Fmt => "cargo fmt --all -- --check",
            Self::Repo => "xtask checks::run (includes docs)",
            Self::All => unreachable!("all resolves to concrete steps"),
        }
    }

    fn step(self, report: &mut Report) -> bool {
        let root = root();
        report.step(self.label(), || match self {
            Self::Docs => checks::docs(&root),
            Self::Tokens => ui_tokens::run(&root, true),
            Self::Assets => {
                crate::build::check_empty_artwork(&root).and_then(|()| expressions::check(&root))
            }
            Self::Visual => web::visual_evidence_unit_tests(&root),
            Self::Perf => web::web_perf_analyzer_unit_tests(&root),
            Self::Fmt => process::run(cargo(&root).args(["fmt", "--all", "--", "--check"]), "fmt"),
            Self::Repo => checks::run(&root),
            Self::All => unreachable!("all resolves to concrete steps"),
        })
    }

    pub fn describe(self) -> String {
        let steps = self.steps();
        let selected: Vec<_> = steps.iter().map(|step| step.command()).collect();
        let excluded: Vec<_> = Self::STATIC
            .into_iter()
            .chain([Self::Docs])
            .filter(|step| {
                !steps.contains(step) && !(*step == Self::Docs && steps.contains(&Self::Repo))
            })
            .map(Self::label)
            .collect();
        format!(
            "coverage: static only\nselected: {selected:?}\nnot selected static checks: {excluded:?}\nnot selected: Rust app compile/link, App bundle, database, browser\n"
        )
    }
}

pub fn static_checks(scope: StaticScope) -> Result<(), String> {
    let mut report = Report::default();
    let ok = scope.steps().into_iter().all(|step| step.step(&mut report));
    let _ = report.finish();
    ok.then_some(())
        .ok_or_else(|| "selected static checks failed".into())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostgresScope {
    Full,
    Tests(UnitOwner),
    Lifecycle,
}
const POSTGRES_OWNERS: [(UnitOwner, &str); 4] = [
    (
        UnitOwner::Store,
        "shared store: migration lifecycle and error classes",
    ),
    (
        UnitOwner::App,
        "migrations + persistence/concurrency/HTTP tests",
    ),
    (
        UnitOwner::Identity,
        "identity: migrations and readiness on PostgreSQL",
    ),
    (
        UnitOwner::Notifications,
        "notifications: receipts, rollup, Seen/Read and retention on PostgreSQL",
    ),
];
const POSTGRES_BINS: &[&str] = &[
    "build",
    "--locked",
    "-p",
    "vink-app",
    "-p",
    "vink-identity",
    "-p",
    "vink-notifications",
    "--bins",
];

impl PostgresScope {
    pub fn parse(scope: &str) -> Result<Self, String> {
        if scope == "lifecycle" {
            return Ok(Self::Lifecycle);
        }
        let owner =
            UnitOwner::parse(scope).map_err(|_| format!("unknown PostgreSQL scope `{scope}`"))?;
        if POSTGRES_OWNERS.iter().any(|(o, _)| *o == owner) {
            Ok(Self::Tests(owner))
        } else {
            Err(format!(
                "{} does not own a PostgreSQL test target",
                owner.package()
            ))
        }
    }

    pub fn lifecycle(self) -> bool {
        matches!(self, Self::Full | Self::Lifecycle)
    }

    pub fn describe(self, filter: Option<&str>) -> String {
        let selected = POSTGRES_OWNERS
            .iter()
            .filter(|(owner, _)| self == Self::Full || self == Self::Tests(*owner));
        let excluded: Vec<_> = POSTGRES_OWNERS
            .iter()
            .filter(|(owner, _)| self != Self::Full && self != Self::Tests(*owner))
            .map(|(owner, _)| owner.package())
            .collect();
        let mut text = format!(
            "coverage: {} PostgreSQL\nfixture: real test cluster, VINK_TEST_DATABASE_URL, ScratchLock; SQLx tests own their migration/role fixtures\n",
            if self == Self::Full {
                "FULL"
            } else {
                "SELECTED"
            },
        );
        for (owner, _) in selected {
            let args = postgres_args(*owner, filter);
            let args = display_args(&args);
            if self != Self::Full {
                text.push_str(&format!("selection/compile+link: cargo {args} --list; cargo {args} --list --ignored (no DB access)\n"));
            }
            text.push_str(&format!(
                "selected: cargo {args} --test-threads=<RUST_TEST_THREADS or 2>\n"
            ));
        }
        if self.lifecycle() {
            text.push_str(&format!(
                "dependency: cargo {}\nselected: lifecycle::config_rejection and provision::verify (migrations, role fences, backup/restore/recovery)\n",
                POSTGRES_BINS.join(" "),
            ));
        } else {
            text.push_str(
                "not selected: lifecycle binaries, config rejection, provision/backup/recovery\n",
            );
        }
        text.push_str(&format!(
            "not selected test owners: {excluded:?}; App bundle and browser are excluded\n"
        ));
        text
    }
}

fn test_args<'a>(owner: UnitOwner, targets: &[&'a str], filter: Option<&'a str>) -> Vec<&'a str> {
    let mut args = vec!["test", "--locked", "-p", owner.package()];
    args.extend_from_slice(targets);
    args.push("--");
    args.extend(filter);
    args
}

pub fn unit_args(owner: UnitOwner, filter: Option<&str>) -> Vec<&str> {
    test_args(owner, owner.test_targets(), filter)
}

fn postgres_args(owner: UnitOwner, filter: Option<&str>) -> Vec<&str> {
    test_args(owner, &["--test", "postgres"], filter)
}

pub fn display_args(args: &[&str]) -> String {
    args.iter()
        .map(|arg| {
            if arg
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_:./".contains(&b))
            {
                (*arg).to_string()
            } else {
                format!("'{}'", arg.replace('\'', "'\\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn listed_tests(args: &[&str], ignored: bool) -> Result<usize, String> {
    let mut command = cargo(&root());
    command.args(args).arg("--list");
    if ignored {
        command.arg("--ignored");
    }
    env::clear_capabilities(&mut command);
    let listed = process::output(&mut command, "list selected tests")?;
    if !ignored || !listed.status.success() {
        eprint!("{}", String::from_utf8_lossy(&listed.stderr));
    }
    if !listed.status.success() {
        return Err(format!("test discovery failed: {}", listed.status));
    }
    Ok(String::from_utf8_lossy(&listed.stdout)
        .lines()
        .filter(|line| line.ends_with(": test"))
        .count())
}

fn require_runnable(args: &[&str]) -> Result<usize, String> {
    println!(
        "==> cargo {} --list (compile/link required)",
        display_args(args)
    );
    let start = std::time::Instant::now();
    let total = listed_tests(args, false)?;
    if total == 0 {
        return Err("selection matched zero tests; refusing empty PASS".into());
    }
    let ignored = listed_tests(args, true)?;
    let runnable = total.saturating_sub(ignored);
    if runnable == 0 {
        return Err(
            "selection matched zero runnable tests (only ignored tests); refusing empty PASS"
                .into(),
        );
    }
    println!(
        "==> test discovery (including Cargo compile/link/cache checks): {:.2}s; selected {runnable} runnable tests, {ignored} ignored",
        start.elapsed().as_secs_f64()
    );
    Ok(runnable)
}

pub fn unit(owner: UnitOwner, filter: Option<&str>) -> Result<(), String> {
    let args = unit_args(owner, filter);
    require_runnable(&args)?;
    let mut command = cargo(&root());
    command.args(&args);
    env::clear_capabilities(&mut command);
    process::run(
        &mut command,
        "selected unit/contract test execution (Cargo can validate artifacts again)",
    )
}

#[derive(Clone, Copy)]
pub enum Lane {
    Quick,
    Discussion,
    Expressions,
    /// Not part of `verify all`: it needs a separately installed, pinned verifier.
    Kani,
    Postgres,
    Web,
}

pub fn verify(lanes: &[Lane], env: &env::Env) -> ExitCode {
    let mut report = Report::default();
    for lane in lanes {
        let ok = match lane {
            Lane::Quick => quick(&mut report),
            Lane::Discussion => discussion::verify(&mut report),
            Lane::Expressions => expressions::verify(&mut report),
            Lane::Kani => kani::verify(&mut report),
            Lane::Postgres => postgres(&mut report, env),
            Lane::Web => web::verify(&mut report, env),
        };
        if !ok {
            break;
        }
    }
    report.finish()
}

fn quick(report: &mut Report) -> bool {
    let root = root();
    let steps: [(&str, &[&str]); 7] = [
        (
            "clippy server (workspace, ssr)",
            &[
                "clippy",
                "--locked",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
        ),
        (
            "clippy browser (vink-web, hydrate, wasm32)",
            &[
                "clippy",
                "--locked",
                "-p",
                "vink-web",
                "--lib",
                "--target",
                "wasm32-unknown-unknown",
                "--features",
                "hydrate",
                "--",
                "-D",
                "warnings",
            ],
        ),
        (
            "clippy standalone Web (vink-web, csr, wasm32)",
            &[
                "clippy",
                "--locked",
                "-p",
                "vink-web",
                "--lib",
                "--target",
                "wasm32-unknown-unknown",
                "--features",
                "csr",
                "--",
                "-D",
                "warnings",
            ],
        ),
        (
            "clippy UI preview (vink-ui-preview, csr, wasm32)",
            &[
                "clippy",
                "--locked",
                "-p",
                "vink-ui-preview",
                "--lib",
                "--target",
                "wasm32-unknown-unknown",
                "--features",
                "csr",
                "--",
                "-D",
                "warnings",
            ],
        ),
        (
            "literary core type-boundary doctests",
            &["test", "--locked", "-p", "vink-app", "--doc"],
        ),
        (
            "Reader SSR boundary (isolated public-read fixture, no database)",
            &[
                "test",
                "--locked",
                "-p",
                "vink-app",
                "--test",
                "ssr_boundary",
            ],
        ),
        (
            "unit + contract tests (no database)",
            &["test", "--locked", "--workspace", "--lib", "--bins"],
        ),
    ];
    if !StaticScope::STATIC
        .into_iter()
        .filter(|step| *step != StaticScope::Repo)
        .all(|step| step.step(report))
    {
        return false;
    }
    for (name, args) in steps {
        let mut command = cargo(&root);
        command.args(args);
        if !report.step(name, || process::run(&mut command, name)) {
            return false;
        }
    }
    StaticScope::Repo.step(report)
}

fn postgres(report: &mut Report, env: &env::Env) -> bool {
    let Some(url) = env.get(TEST_DATABASE_URL) else {
        report.fail(
            "PostgreSQL test harness credential",
            TEST_DATABASE_URL_MISSING,
        );
        return false;
    };
    postgres_run(report, env, &url, PostgresScope::Full, None, None)
}

pub fn selected_postgres(
    scope: PostgresScope,
    env: &env::Env,
    filter: Option<&str>,
) -> Result<(), String> {
    let url = env
        .get(TEST_DATABASE_URL)
        .ok_or(TEST_DATABASE_URL_MISSING)?;
    let threads = env.get("RUST_TEST_THREADS").unwrap_or_else(|| "2".into());
    if !threads.parse::<usize>().is_ok_and(|threads| threads > 0) {
        return Err("RUST_TEST_THREADS must be a positive integer".into());
    }
    let mut report = Report::default();
    let ok = postgres_run(&mut report, env, &url, scope, filter, Some(&threads));
    let _ = report.finish();
    ok.then_some(())
        .ok_or_else(|| "selected PostgreSQL checks failed".into())
}

fn postgres_run(
    report: &mut Report,
    env: &env::Env,
    url: &str,
    scope: PostgresScope,
    filter: Option<&str>,
    selected_threads: Option<&str>,
) -> bool {
    if let PostgresScope::Tests(owner) = scope
        && !report.step(
            "selected PostgreSQL binary/test discovery without database",
            || require_runnable(&postgres_args(owner, filter)).map(|_| ()),
        )
    {
        return false;
    }
    if scope == PostgresScope::Lifecycle
        && !report.step("selected lifecycle client tools", || {
            provision::psql_version()?;
            println!("==> {}", provision::backup_clients()?);
            Ok(())
        })
    {
        return false;
    }
    if !report.step("PostgreSQL available (test harness)", || {
        doctor::probe_database(url)
    }) {
        return false;
    }
    let _scratch_lock = match lifecycle::ScratchLock::acquire(url) {
        Ok(lock) => lock,
        Err(e) => {
            report.fail("scratch ownership", &e);
            return false;
        }
    };
    // Bound SQLx database scheduling without changing concurrency inside a test.
    let threads = selected_threads
        .map(str::to_string)
        .or_else(|| env.get("RUST_TEST_THREADS"))
        .unwrap_or_else(|| "2".into());
    let threads_arg = format!("--test-threads={threads}");
    for (owner, name) in POSTGRES_OWNERS {
        if scope != PostgresScope::Full && scope != PostgresScope::Tests(owner) {
            continue;
        }
        let mut tests = cargo(&root());
        tests.args(postgres_args(owner, filter)).arg(&threads_arg);
        if scope == PostgresScope::Full {
            env.apply(&mut tests);
        } else {
            env::clear_capabilities(&mut tests);
        }
        tests.env("DATABASE_URL", url);
        if !report.step(name, || process::run(&mut tests, name)) {
            return false;
        }
    }
    if !scope.lifecycle() {
        return true;
    }
    let root = root();
    let mut bins = cargo(&root);
    bins.args(POSTGRES_BINS);
    if !report.step("build binaries", || {
        process::run(&mut bins, "build binaries")
    }) {
        return false;
    }
    report.step("invalid configuration exits 2", || {
        lifecycle::config_rejection(&root)
    }) && provision::verify(report, &root, url)
}
