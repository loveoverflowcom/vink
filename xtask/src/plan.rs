//! Pure dependency selection. The executor consumes these exact artifacts and scenarios.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Build,
    Dev,
    Verify,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Api,
    Web,
    Worker,
    Identity,
    Preview,
    Unit(UnitOwner),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitOwner {
    App,
    Identity,
    Store,
    Xtask,
    Web,
}
impl UnitOwner {
    pub fn test_targets(self) -> &'static [&'static str] {
        match self {
            Self::Xtask => &["--bins"],
            Self::Store | Self::Web => &["--lib"],
            Self::App | Self::Identity => &["--lib", "--bins"],
        }
    }
    pub fn package(self) -> &'static str {
        match self {
            Self::App => "vink-app",
            Self::Identity => "vink-identity",
            Self::Store => "vink-store-postgres",
            Self::Xtask => "xtask",
            Self::Web => "vink-web",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Auth {
    #[default]
    None,
    Identity,
    Fixture,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Artifact {
    App,
    Worker,
    Identity,
    Bundle,
    Preview,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Rust,
    WasmTarget,
    Bindgen,
    Node,
    Npm,
    Chrome,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Service {
    App,
    Identity,
    Worker,
    Preview,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    Main,
    Discovery,
    Restart,
    Endorsement,
    Review,
    Auth,
    Password,
    Publication,
}
impl Scenario {
    pub const ALL: [Self; 8] = [
        Self::Main,
        Self::Discovery,
        Self::Restart,
        Self::Endorsement,
        Self::Review,
        Self::Auth,
        Self::Password,
        Self::Publication,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Discovery => "discovery",
            Self::Restart => "restart",
            Self::Endorsement => "endorsement",
            Self::Review => "review",
            Self::Auth => "auth",
            Self::Password => "password",
            Self::Publication => "publication",
        }
    }
    pub fn parse(s: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|x| x.name() == s)
            .ok_or_else(|| format!("unknown/empty Web scenario `{s}`"))
    }
    pub fn identity(self) -> bool {
        matches!(self, Self::Auth | Self::Password | Self::Publication)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub operation: Operation,
    pub target: Target,
    pub auth: Auth,
    pub release: bool,
    pub migrate: bool,
    pub seed: bool,
    pub scenarios: Vec<Scenario>,
    pub filter: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub request: Request,
    pub artifacts: Vec<Artifact>,
    pub tools: Vec<Tool>,
    pub services: Vec<Service>,
    pub databases: Vec<&'static str>,
    pub migrations: Vec<&'static str>,
    pub scenarios: Vec<Scenario>,
}
impl Plan {
    pub fn resolve(request: Request) -> Self {
        use {Artifact as A, Operation as O, Service as S, Target as T, Tool as P};
        let mut plan = Self {
            request,
            artifacts: vec![],
            tools: vec![P::Rust],
            services: vec![],
            databases: vec![],
            migrations: vec![],
            scenarios: vec![],
        };
        let r = &plan.request;
        if let T::Unit(_) = r.target {
            return plan;
        }
        plan.artifacts = match r.target {
            T::Api => vec![A::App],
            T::Web => vec![A::Bundle, A::App],
            T::Worker => vec![A::Worker],
            T::Identity => vec![A::Identity],
            T::Preview => vec![A::Preview],
            T::Unit(_) => unreachable!(),
        };
        if matches!(r.target, T::Web | T::Preview) {
            plan.tools.extend([P::WasmTarget, P::Bindgen]);
        }
        if r.target == T::Web {
            plan.tools.extend([P::Node, P::Npm]);
        }
        if r.operation == O::Verify && r.target == T::Web {
            plan.scenarios = Scenario::ALL
                .into_iter()
                .filter(|s| {
                    r.scenarios.contains(s)
                        || (*s == Scenario::Main && r.scenarios.contains(&Scenario::Restart))
                })
                .collect();
            plan.tools.push(P::Chrome);
            plan.databases.push("VINK_TEST_DATABASE_URL");
            plan.migrations.push("App scratch");
            plan.services.push(S::App);
            if plan.scenarios.iter().any(|s| s.identity()) {
                plan.artifacts.push(A::Identity);
                plan.services.push(S::Identity);
                plan.migrations.push("Identity scratch");
            }
        } else if r.operation == O::Verify && r.target == T::Preview {
            plan.tools.extend([P::Node, P::Chrome]);
            plan.services.push(S::Preview);
        } else if r.operation == O::Dev {
            match r.target {
                T::Api | T::Web => {
                    plan.services.push(S::App);
                    plan.databases.push("DATABASE_URL");
                }
                T::Worker => {
                    plan.services.push(S::Worker);
                    plan.databases.push("DATABASE_URL");
                }
                T::Identity => {}
                T::Preview => plan.services.push(S::Preview),
                T::Unit(_) => unreachable!(),
            }
            if r.auth == Auth::Identity || r.target == T::Identity {
                if !plan.artifacts.contains(&A::Identity) {
                    plan.artifacts.push(A::Identity);
                }
                plan.services.insert(0, S::Identity);
                plan.databases.push("VINK_IDENTITY_DATABASE_URL");
            }
            if r.migrate {
                if r.target != T::Identity {
                    plan.migrations.push("App");
                    plan.databases.push("VINK_MIGRATE_DATABASE_URL");
                    if !plan.artifacts.contains(&A::App) {
                        plan.artifacts.push(A::App);
                    }
                }
                if plan.services.contains(&S::Identity) {
                    plan.migrations.push("Identity");
                    plan.databases.push("VINK_IDENTITY_MIGRATE_DATABASE_URL");
                }
            }
        }
        plan
    }
    pub fn describe(&self) -> String {
        let coverage =
            if self.request.operation == Operation::Verify && self.request.target == Target::Web {
                if self.request.scenarios.len() == Scenario::ALL.len() {
                    "FULL Web regression"
                } else {
                    "SELECTED Web checks (excluded checks NOT_SELECTED)"
                }
            } else {
                "requested target only"
            };
        let auth = if self.request.operation == Operation::Verify
            && self.request.target == Target::Web
        {
            "explicit fixture for main/discovery/restart/endorsement/review; real Identity for auth/password/publication".to_string()
        } else {
            format!("{:?}", self.request.auth)
        };
        let excluded: Vec<_> = Scenario::ALL
            .into_iter()
            .filter(|s| !self.scenarios.contains(s))
            .map(Scenario::name)
            .collect();
        let dependencies: Vec<_> = self
            .scenarios
            .iter()
            .filter(|s| !self.request.scenarios.contains(s))
            .map(|s| s.name())
            .collect();
        format!(
            "coverage: {coverage}\noperation: {:?}; target: {:?}; auth: {auth}\nbuild: {:?}\nrequired tools: {:?}\nowned services: {:?}\nattached database configuration (values hidden): {:?}\nmigrations: {:?}; dev seed: {} (Web fixtures follow the scenario catalog)\nselected checks: {:?}; filter: {:?}\ndependency checks: {:?} (restart requires main's saved draft and browser state)\nnot selected Web checks: {:?}\nAPI target isolation: no bundle, no SSR/static routes; Rust SSR dependencies remain compiled.\ndata: dev attaches prepared databases; Web verification owns scratch only. No containers.\nartifacts: build from current source every invocation; no no-build/reuse mode.\n",
            self.request.operation,
            self.request.target,
            self.artifacts,
            self.tools,
            self.services,
            self.databases,
            self.migrations,
            self.request.seed,
            self.request.scenarios,
            self.request.filter,
            dependencies,
            excluded
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn request(operation: Operation, target: Target) -> Request {
        Request {
            operation,
            target,
            auth: Auth::None,
            release: false,
            migrate: false,
            seed: false,
            scenarios: vec![],
            filter: None,
        }
    }
    #[test]
    fn public_api_has_only_its_real_dependencies() {
        let p = Plan::resolve(request(Operation::Dev, Target::Api));
        assert_eq!(p.artifacts, vec![Artifact::App]);
        assert_eq!(p.tools, vec![Tool::Rust]);
        assert_eq!(p.services, vec![Service::App]);
        assert_eq!(p.databases, vec!["DATABASE_URL"]);
        assert!(p.migrations.is_empty());
    }
    #[test]
    fn scenarios_declare_setup_without_expanding_to_full_suite() {
        for s in Scenario::ALL {
            let mut r = request(Operation::Verify, Target::Web);
            r.scenarios = vec![s];
            let p = Plan::resolve(r);
            assert_eq!(
                p.scenarios,
                if s == Scenario::Restart {
                    vec![Scenario::Main, s]
                } else {
                    vec![s]
                }
            );
            assert_eq!(p.artifacts.contains(&Artifact::Identity), s.identity());
            assert!(!p.artifacts.contains(&Artifact::Worker));
        }
    }
    #[test]
    fn builds_and_unit_tests_never_require_services_or_databases() {
        for t in [
            Target::Api,
            Target::Web,
            Target::Worker,
            Target::Identity,
            Target::Preview,
            Target::Unit(UnitOwner::App),
        ] {
            let p = Plan::resolve(request(Operation::Build, t));
            assert!(p.services.is_empty());
            assert!(p.databases.is_empty());
            assert!(p.migrations.is_empty());
        }
    }
    #[test]
    fn auth_and_worker_migrations_select_only_their_owners() {
        let mut r = request(Operation::Dev, Target::Api);
        r.auth = Auth::Identity;
        r.migrate = true;
        let p = Plan::resolve(r);
        assert_eq!(p.artifacts, vec![Artifact::App, Artifact::Identity]);
        assert_eq!(p.migrations, vec!["App", "Identity"]);
        let mut r = request(Operation::Dev, Target::Worker);
        r.migrate = true;
        let p = Plan::resolve(r);
        assert_eq!(p.artifacts, vec![Artifact::Worker, Artifact::App]);
        assert_eq!(p.services, vec![Service::Worker]);
    }
}
