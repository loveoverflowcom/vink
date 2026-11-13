//! Parse and reject incompatible selections before any executor is entered.
use crate::plan::*;
pub const HELP: &str = "Scoped workflows (PostgreSQL is attached, never started):
  build api|web|worker|identity|preview [--release] [--plan]
  dev api|web [--auth none|identity|fixture] [--migrate] [--seed] [--plan]
  dev worker|identity [--migrate] [--plan]
  dev preview [--plan]                 Leptos UI preview, no business services
  verify unit app|identity|store|xtask|web [--filter NAME] [--plan]
  verify web [--scenario NAME]... [--plan]
  verify preview [--plan]               Leptos preview browser regression
  doctor <scoped command above>         check only that command's prerequisites

Web scenarios: main, discovery, restart, endorsement, review, auth, password, publication.
No selector means full Web regression. restart depends on main. --plan performs no task I/O.
Auth defaults to none, independent of .env. fixture requires explicit VINK_DEV_PRINCIPAL.
identity uses native credentials and local mail outbox; no fixture fallback or SMTP.
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
                "worker" => Target::Worker,
                "identity" => Target::Identity,
                "preview" => Target::Preview,
                _ => return Err(format!("unknown target `{target}`")),
            };
        }
        ["verify", "unit", owner, ..] => {
            r.operation = Operation::Verify;
            r.target = Target::Unit(match *owner {
                "app" => UnitOwner::App,
                "identity" => UnitOwner::Identity,
                "store" => UnitOwner::Store,
                "xtask" => UnitOwner::Xtask,
                "web" => UnitOwner::Web,
                _ => return Err(format!("unknown unit owner `{owner}`")),
            });
            i = 3;
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
        ["build" | "dev", ..] | ["verify", "unit", ..] => {
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
            "--release" if r.operation == Operation::Build && r.target != Target::Preview => {
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
            "--filter" if matches!(r.target, Target::Unit(_)) => {
                i += 1;
                let f = args
                    .get(i)
                    .filter(|s| !s.is_empty() && !s.starts_with('-'))
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
    fn invalid_combinations_and_empty_selectors_are_errors() {
        for args in [
            vec!["dev", "api", "--release"],
            vec!["build", "api", "--auth", "identity"],
            vec!["dev", "worker", "--seed"],
            vec!["verify", "web", "--scenario", ""],
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
