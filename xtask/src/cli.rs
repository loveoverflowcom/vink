//! Parse and reject incompatible selections before any executor is entered.
use crate::plan::*;
use crate::verify::{PostgresScope, StaticScope};
pub const HELP: &str = "Scoped workflows (PostgreSQL is attached, never started):
  build api|spa|web|worker|identity|preview [--release] [--plan]
  dev api|web [--auth none|identity|fixture] [--migrate] [--seed] [--plan]
  dev worker|identity [--migrate] [--plan]
  dev preview [--plan]                 Leptos UI preview, no business services
  verify static [docs|tokens|assets|visual|perf|fmt|repo|all] [--plan]
  verify check OWNER [--target lib|bins|all-targets|api|ssr|hydrate|csr] [--plan]
  verify unit OWNER [--filter NAME] [--plan]
  verify postgres [store|app|identity|notifications|lifecycle] [--filter NAME] [--plan]
  verify web [--scenario NAME]... [--capture full|checks] [--release] [--plan]
  verify preview [--plan]               Leptos preview browser regression
  doctor <scoped command above>         check only that command's prerequisites

Rust owners: app, identity, store, xtask, web, notifications, ui, patterns, expressions, markdown, preview.
PostgreSQL without a selector runs the full lane. --filter applies to owner tests only.
Web scenarios: main (alias), editor-first-draft, editor-chrome, editor, editor-format,
  desk, appearance, reader, discussion, storage, discovery, restart, endorsement, review,
  auth, password, account-deletion, author-search, publication, notifications,\n  standalone (CSR + API), perf-baseline, return-position.
No selector runs full Web regression, excluding standalone and the two performance opt-ins.
--capture checks requires explicit scenarios; it retains functional/a11y/DOM checks.
--plan performs no selected task I/O. Read-only Git advisory (requires --plan):
  verify changed --base REV (--head REV | --worktree) --plan
  Append --from-diff BASE_SHA HEAD_SHA committed|worktree to execute one selected verify command.
Auth defaults to none, independent of .env. fixture requires explicit VINK_DEV_PRINCIPAL.
identity uses native credentials and local mail outbox; no fixture fallback or external mail.
Dev defaults to prepared databases: --migrate explicitly migrates selected owners;
--seed explicitly loads App sample data. Ctrl-C stops owned children and keeps logs.
Legacy build/dev/doctor and db commands remain available with their original scope.
";
pub struct Invocation {
    pub plan: Plan,
    pub inspect: bool,
    pub doctor: bool,
}
/// None delegates only known legacy forms to their original dispatcher.
pub fn parse(args: &[&str]) -> Result<Option<Invocation>, String> {
    let (doctor, args) = match args {
        ["doctor", rest @ ..] if !rest.is_empty() => (true, rest),
        _ => (false, args),
    };
    let mut args = args.to_vec();
    let inspect = args.last() == Some(&"--plan");
    if inspect {
        args.pop();
    }
    if doctor && inspect {
        return Err("doctor and --plan are separate operations".into());
    }
    if !doctor
        && !inspect
        && matches!(
            args.as_slice(),
            ["build"] | ["build", "--release"] | ["dev"] | ["doctor"]
        )
    {
        return Ok(None);
    }
    let mut r = Request {
        operation: Operation::Build,
        target: Target::Api,
        auth: Auth::None,
        release: false,
        migrate: false,
        seed: false,
        scenarios: vec![],
        capture: Capture::Full,
        filter: None,
    };
    let mut i = 2;
    match args.as_slice() {
        [verb @ ("build" | "dev"), target, ..] => {
            r.operation = if *verb == "build" {
                Operation::Build
            } else {
                Operation::Dev
            };
            r.target = match *target {
                "api" => Target::Api,
                "web" => Target::Web,
                "spa" if r.operation == Operation::Build => Target::Spa,
                "worker" => Target::Worker,
                "identity" => Target::Identity,
                "preview" => Target::Preview,
                _ => return Err(format!("unknown target `{target}`")),
            };
        }
        ["verify", "static", ..] => {
            r.operation = Operation::Verify;
            r.target = Target::Static(StaticScope::parse(args.get(2).copied().unwrap_or("all"))?);
            i = args.len().min(3);
        }
        ["verify", "check", owner, ..] => {
            r.operation = Operation::Verify;
            r.target = Target::Check(UnitOwner::parse(owner)?, CheckTarget::Default);
            i = 3;
        }
        ["verify", "unit", owner, ..] => {
            r.operation = Operation::Verify;
            r.target = Target::Unit(UnitOwner::parse(owner)?);
            i = 3;
        }
        ["verify", "postgres"] if !doctor && !inspect => return Ok(None),
        ["verify", "postgres", ..] => {
            r.operation = Operation::Verify;
            r.target = Target::Postgres(match args.get(2) {
                Some(owner) => PostgresScope::parse(owner)?,
                None => PostgresScope::Full,
            });
            i = args.len().min(3);
        }
        ["verify", "preview", ..] => {
            r.operation = Operation::Verify;
            r.target = Target::Preview;
        }
        ["verify", "web", ..] => {
            r.operation = Operation::Verify;
            r.target = Target::Web;
        }
        _ if doctor || inspect => {
            return Err("expected an explicit scoped command; see --help".into());
        }
        ["build" | "dev", ..] | ["verify", "unit" | "check", ..] => {
            return Err("missing target/owner; see --help".into());
        }
        _ => return Ok(None),
    }
    let mut seen = std::collections::BTreeSet::new();
    while i < args.len() {
        let flag = args[i];
        if flag != "--scenario" && !seen.insert(flag) {
            return Err(format!("duplicate option {flag}"));
        }
        match flag {
            "--release"
                if (r.operation == Operation::Build && r.target != Target::Preview)
                    || (r.operation == Operation::Verify && r.target == Target::Web) =>
            {
                r.release = true
            }
            "--migrate" if r.operation == Operation::Dev && r.target != Target::Preview => {
                r.migrate = true
            }
            "--seed"
                if r.operation == Operation::Dev
                    && matches!(r.target, Target::Api | Target::Web) =>
            {
                r.seed = true
            }
            "--auth"
                if r.operation == Operation::Dev
                    && matches!(r.target, Target::Api | Target::Web) =>
            {
                i += 1;
                r.auth = match args.get(i) {
                    Some(&"none") => Auth::None,
                    Some(&"identity") => Auth::Identity,
                    Some(&"fixture") => Auth::Fixture,
                    _ => return Err("--auth needs none|identity|fixture".into()),
                };
            }
            "--scenario" if r.operation == Operation::Verify && r.target == Target::Web => {
                i += 1;
                let s = Scenario::parse(args.get(i).copied().unwrap_or(""))?;
                if r.scenarios.contains(&s) {
                    return Err(format!("duplicate scenario {}", s.name()));
                }
                r.scenarios.push(s);
            }
            "--capture" if r.operation == Operation::Verify && r.target == Target::Web => {
                i += 1;
                r.capture = Capture::parse(args.get(i).copied().unwrap_or(""))?;
            }
            "--target" => {
                let Target::Check(owner, _) = r.target else {
                    return Err("--target is only supported by verify check".into());
                };
                i += 1;
                let target = args.get(i).ok_or("--target needs a compile target")?;
                r.target = Target::Check(owner, CheckTarget::parse(owner, target)?);
            }
            "--filter"
                if matches!(
                    r.target,
                    Target::Unit(_) | Target::Postgres(PostgresScope::Tests(_))
                ) =>
            {
                i += 1;
                let f = args
                    .get(i)
                    .filter(|s| !s.trim().is_empty() && !s.starts_with('-'))
                    .ok_or("--filter needs a nonempty test name")?;
                r.filter = Some((*f).into());
            }
            _ => return Err(format!("unsupported option `{flag}` for this command")),
        }
        i += 1;
    }
    if r.target == Target::Identity {
        r.auth = Auth::Identity;
    }
    if r.operation == Operation::Verify && r.target == Target::Web && r.scenarios.is_empty() {
        if r.capture == Capture::Checks {
            return Err("--capture checks requires explicit --scenario selectors".into());
        }
        r.scenarios = Scenario::ALL.to_vec();
    }
    Ok(Some(Invocation {
        plan: Plan::resolve(r),
        inspect,
        doctor,
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn release_web_plan_keeps_the_selected_checks() {
        let plan = parse(&[
            "verify",
            "web",
            "--scenario",
            "perf-baseline",
            "--release",
            "--plan",
        ])
        .unwrap()
        .unwrap()
        .plan;
        assert!(plan.request.release);
        assert!(
            plan.describe()
                .contains("release (wasm-release for bundle)")
        );
        assert_eq!(plan.scenarios, [Scenario::PerfBaseline]);
        assert!(
            !parse(&["verify", "web"])
                .unwrap()
                .unwrap()
                .plan
                .request
                .release
        );
    }

    #[test]
    fn invalid_combinations_and_empty_selectors_are_errors() {
        for args in [
            vec!["dev", "api", "--release"],
            vec!["build", "api", "--auth", "identity"],
            vec!["dev", "worker", "--seed"],
            vec!["verify", "web", "--scenario", ""],
            vec!["verify", "web", "--capture", "checks"],
            vec!["verify", "web", "--scenario", "desk", "--capture", "typo"],
            vec!["verify", "web", "--scenario", "desk", "--scenario", "desk"],
            vec!["verify", "static", "typo"],
            vec!["verify", "static", "tokens", "--filter", "test"],
            vec!["verify", "check", "store", "--target", "hydrate"],
            vec!["verify", "check", "xtask", "--target", "lib"],
            vec!["verify", "check", "typo"],
            vec!["verify", "postgres", "typo"],
            vec!["verify", "postgres", "ui"],
            vec!["verify", "postgres", "lifecycle", "--filter", "recovery"],
            vec!["verify", "postgres", "store", "--filter", "--ignored"],
            vec!["verify", "postgres", "store", "--filter", " "],
            vec!["verify", "web", "--scenario", "typo"],
            vec!["verify", "unit", "app", "--filter", ""],
            vec!["dev", "api", "--auth", "fixture", "--auth", "none"],
            vec!["build", "preview", "--release"],
            vec!["dev", "preview", "--migrate"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }
    #[test]
    fn rust_selectors_resolve_only_the_requested_scope() {
        type Case<'a> = (&'a [&'a str], &'a [Tool], &'a [&'a str], &'a str);
        let cases: &[Case<'_>] = &[
            (
                &["verify", "static", "docs"],
                &[Tool::Rust],
                &[],
                "checks::docs",
            ),
            (
                &["verify", "check", "web", "--target", "hydrate"],
                &[Tool::Rust, Tool::WasmTarget],
                &[],
                "--target wasm32-unknown-unknown --features hydrate",
            ),
            (
                &["verify", "check", "web", "--target", "csr"],
                &[Tool::Rust, Tool::WasmTarget],
                &[],
                "--target wasm32-unknown-unknown --features csr",
            ),
            (
                &["verify", "unit", "web", "--filter", "endorsement"],
                &[Tool::Rust],
                &[],
                "-p vink-web --lib --features ssr -- endorsement",
            ),
            (
                &["verify", "check", "app", "--target", "api"],
                &[Tool::Rust],
                &[],
                "--bin vink-app --no-default-features",
            ),
            (
                &["verify", "unit", "expressions", "--filter", "candidate"],
                &[Tool::Rust],
                &[],
                "-p vink-expressions --lib -- candidate",
            ),
            (
                &["verify", "postgres", "store", "--filter", "fresh_database"],
                &[Tool::Rust],
                &["VINK_TEST_DATABASE_URL"],
                "-p vink-store-postgres --test postgres -- fresh_database",
            ),
            (
                &["verify", "postgres", "lifecycle"],
                &[Tool::Rust, Tool::Psql, Tool::PgBackup],
                &["VINK_TEST_DATABASE_URL"],
                "provision::verify",
            ),
        ];
        for (args, tools, databases, command) in cases {
            let plan = parse(args).unwrap().unwrap().plan;
            assert_eq!(&plan.tools, tools, "{args:?}");
            assert_eq!(&plan.databases, databases, "{args:?}");
            assert!(
                plan.artifacts.is_empty() && plan.services.is_empty(),
                "{args:?}"
            );
            assert!(plan.describe().contains(command), "{}", plan.describe());
        }
    }

    #[test]
    fn full_postgres_keeps_the_legacy_entrypoint_and_all_owners() {
        assert!(parse(&["verify", "postgres"]).unwrap().is_none());
        let plan = parse(&["verify", "postgres", "--plan"])
            .unwrap()
            .unwrap()
            .plan;
        for package in [
            "vink-app",
            "vink-store-postgres",
            "vink-identity",
            "vink-notifications",
        ] {
            assert!(
                plan.describe()
                    .contains(&format!("-p {package} --test postgres"))
            );
        }
        assert!(plan.describe().contains("provision::verify"));
    }

    #[test]
    fn main_expands_once_and_restart_has_its_own_setup() {
        let plan = parse(&[
            "verify",
            "web",
            "--scenario",
            "restart",
            "--scenario",
            "storage",
            "--scenario",
            "main",
        ])
        .unwrap()
        .unwrap()
        .plan;
        let mut expected: Vec<_> = Scenario::ALL.into_iter().filter(|s| s.in_main()).collect();
        expected.push(Scenario::Restart);
        assert_eq!(plan.scenarios, expected);
        assert_eq!(
            plan.scenarios
                .iter()
                .filter(|s| **s == Scenario::Storage)
                .count(),
            1
        );
        assert!(!plan.artifacts.contains(&Artifact::Identity));
        let restart = parse(&[
            "verify",
            "web",
            "--scenario",
            "restart",
            "--capture",
            "checks",
        ])
        .unwrap()
        .unwrap()
        .plan;
        assert_eq!(restart.scenarios, [Scenario::Restart]);
        assert!(restart.describe().contains("restart-prepare"));
        assert!(restart.describe().contains("PNG/visual evidence NOT_RUN"));
        assert_eq!(
            parse(&["verify", "web"]).unwrap().unwrap().plan.scenarios,
            Scenario::ALL
        );
    }

    #[test]
    fn inspection_and_doctor_share_the_execution_plan() {
        let p = parse(&["dev", "api", "--auth", "identity"])
            .unwrap()
            .unwrap()
            .plan;
        assert_eq!(
            p,
            parse(&["dev", "api", "--auth", "identity", "--plan"])
                .unwrap()
                .unwrap()
                .plan
        );
        assert_eq!(
            p,
            parse(&["doctor", "dev", "api", "--auth", "identity"])
                .unwrap()
                .unwrap()
                .plan
        );
        assert!(parse(&["dev"]).unwrap().is_none());
    }
}
