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
    Spa,
    Web,
    Worker,
    Identity,
    Preview,
    Unit(UnitOwner),
    Static(crate::verify::StaticScope),
    Check(UnitOwner, CheckTarget),
    Postgres(crate::verify::PostgresScope),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitOwner {
    App,
    Identity,
    Store,
    Xtask,
    Web,
    Notifications,
    Ui,
    Patterns,
    Expressions,
    Markdown,
    Preview,
    WorkViews,
}
impl UnitOwner {
    pub const ALL: [Self; 12] = [
        Self::App,
        Self::Identity,
        Self::Store,
        Self::Xtask,
        Self::Web,
        Self::Notifications,
        Self::Ui,
        Self::Patterns,
        Self::Expressions,
        Self::Markdown,
        Self::Preview,
        Self::WorkViews,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Identity => "identity",
            Self::Store => "store",
            Self::Xtask => "xtask",
            Self::Web => "web",
            Self::Notifications => "notifications",
            Self::Ui => "ui",
            Self::Patterns => "patterns",
            Self::Expressions => "expressions",
            Self::Markdown => "markdown",
            Self::Preview => "preview",
            Self::WorkViews => "work-views",
        }
    }

    pub fn parse(owner: &str) -> Result<Self, String> {
        if owner == "work-view-policy" {
            return Ok(Self::WorkViews);
        }
        Self::ALL
            .into_iter()
            .find(|o| o.name() == owner)
            .ok_or_else(|| format!("unknown Rust owner `{owner}`"))
    }

    pub fn test_targets(self) -> &'static [&'static str] {
        match self {
            Self::Xtask => &["--bins"],
            Self::App | Self::Identity | Self::Notifications => &["--lib", "--bins"],
            _ => &["--lib"],
        }
    }

    pub fn package(self) -> &'static str {
        match self {
            Self::App => "vink-app",
            Self::Identity => "vink-identity",
            Self::Store => "vink-store-postgres",
            Self::Xtask => "xtask",
            Self::Web => "vink-web",
            Self::Notifications => "vink-notifications",
            Self::Ui => "vink-ui",
            Self::Patterns => "vink-patterns",
            Self::Expressions => "vink-expressions",
            Self::Markdown => "vink-markdown-text",
            Self::Preview => "vink-ui-preview",
            Self::WorkViews => "vink-work-view-policy",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckTarget {
    Default,
    Lib,
    Bins,
    AllTargets,
    Api,
    Ssr,
    Hydrate,
    Csr,
}
impl CheckTarget {
    pub fn parse(owner: UnitOwner, target: &str) -> Result<Self, String> {
        use UnitOwner as U;
        match target {
            "lib" if owner != U::Xtask => Ok(Self::Lib),
            "bins" if matches!(owner, U::App | U::Identity | U::Notifications | U::Xtask) => {
                Ok(Self::Bins)
            }
            "all-targets" => Ok(Self::AllTargets),
            "api" if owner == U::App => Ok(Self::Api),
            "ssr" if matches!(owner, U::App | U::Web) => Ok(Self::Ssr),
            "hydrate" if owner == U::Web => Ok(Self::Hydrate),
            "csr" if matches!(owner, U::Web | U::Preview) => Ok(Self::Csr),
            _ => Err(format!(
                "unsupported compile target `{target}` for {}",
                owner.package()
            )),
        }
    }

    pub fn args(self, owner: UnitOwner) -> Vec<&'static str> {
        let mut args = vec!["check", "--locked", "-p", owner.package()];
        args.extend_from_slice(match self {
            Self::Default => owner.test_targets(),
            Self::Lib => &["--lib"],
            Self::Bins => &["--bins"],
            Self::AllTargets => &["--all-targets"],
            Self::Api => &["--bin", "vink-app", "--no-default-features"],
            Self::Ssr if owner == UnitOwner::App => {
                &["--bin", "vink-app", "--features", "legacy-ssr"]
            }
            Self::Ssr => &["--lib", "--features", "ssr"],
            Self::Hydrate => &[
                "--lib",
                "--target",
                "wasm32-unknown-unknown",
                "--features",
                "hydrate",
            ],
            Self::Csr => &[
                "--lib",
                "--target",
                "wasm32-unknown-unknown",
                "--features",
                "csr",
            ],
        });
        args
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
    Spa,
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
    Psql,
    Rustfmt,
    PgBackup,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Service {
    App,
    Identity,
    Worker,
    Preview,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Capture {
    #[default]
    Full,
    Checks,
}
impl Capture {
    pub fn parse(mode: &str) -> Result<Self, String> {
        match mode {
            "full" => Ok(Self::Full),
            "checks" => Ok(Self::Checks),
            _ => Err(format!(
                "unknown Web capture mode `{mode}`; expected full|checks"
            )),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Checks => "checks",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    Main,
    EditorFirstDraft,
    Reader,
    Discussion,
    Appearance,
    Desk,
    EditorChrome,
    Editor,
    EditorFormat,
    Storage,
    Discovery,
    Restart,
    Endorsement,
    Review,
    Auth,
    Password,
    AccountDeletion,
    AuthorSearch,
    Publication,
    Notifications,
    PerfBaseline,
    ReturnPosition,
    Standalone,
}

impl Scenario {
    /// Canonical execution order. Public SSR begins with just the sample publication.
    pub const ALL: [Self; 18] = [
        Self::EditorFirstDraft,
        Self::Reader,
        Self::Discussion,
        Self::Appearance,
        Self::Desk,
        Self::EditorChrome,
        Self::Editor,
        Self::EditorFormat,
        Self::Storage,
        Self::Discovery,
        Self::Restart,
        Self::Endorsement,
        Self::Review,
        Self::Auth,
        Self::Password,
        Self::AuthorSearch,
        Self::Publication,
        Self::Notifications,
    ];
    const OPT_IN: [Self; 4] = [
        Self::PerfBaseline,
        Self::ReturnPosition,
        Self::Standalone,
        Self::AccountDeletion,
    ];

    pub fn selectable() -> impl Iterator<Item = Self> {
        [Self::Main]
            .into_iter()
            .chain(Self::ALL)
            .chain(Self::OPT_IN)
    }

    pub fn in_main(self) -> bool {
        matches!(
            self,
            Self::EditorFirstDraft
                | Self::Reader
                | Self::Discussion
                | Self::Appearance
                | Self::Desk
                | Self::EditorChrome
                | Self::Editor
                | Self::EditorFormat
                | Self::Storage
        )
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::EditorFirstDraft => "editor-first-draft",
            Self::Reader => "reader",
            Self::Discussion => "discussion",
            Self::Appearance => "appearance",
            Self::Desk => "desk",
            Self::EditorChrome => "editor-chrome",
            Self::Editor => "editor",
            Self::EditorFormat => "editor-format",
            Self::Standalone => "standalone",
            Self::Storage => "storage",
            Self::Discovery => "discovery",
            Self::Restart => "restart",
            Self::Endorsement => "endorsement",
            Self::Review => "review",
            Self::Auth => "auth",
            Self::Password => "password",
            Self::AccountDeletion => "account-deletion",
            Self::AuthorSearch => "author-search",
            Self::Publication => "publication",
            Self::Notifications => "notifications",
            Self::PerfBaseline => "perf-baseline",
            Self::ReturnPosition => "return-position",
        }
    }

    pub fn parse(name: &str) -> Result<Self, String> {
        Self::selectable()
            .find(|s| s.name() == name)
            .ok_or_else(|| format!("unknown/empty Web scenario `{name}`"))
    }

    pub fn identity(self) -> bool {
        matches!(
            self,
            Self::Auth
                | Self::Password
                | Self::AccountDeletion
                | Self::AuthorSearch
                | Self::Publication
                | Self::Standalone
        )
    }

    pub fn tag_catalog(self) -> bool {
        matches!(
            self,
            Self::Reader
                | Self::Appearance
                | Self::Editor
                | Self::EditorChrome
                | Self::EditorFormat
                | Self::Discovery
                | Self::PerfBaseline
                | Self::ReturnPosition
        )
    }

    /// A single browser phase against the running sample App; other scenarios coordinate
    /// more than one process/phase or have additional setup in the Web adapter.
    pub fn primary_phase(self) -> Option<&'static str> {
        match self {
            Self::Reader => Some("reader"),
            Self::Discussion => Some("discussion"),
            Self::Appearance => Some("appearance-shell"),
            Self::Desk => Some("desk"),
            Self::EditorChrome => Some("editor-chrome"),
            Self::Editor => Some("editor"),
            Self::EditorFormat => Some("editor-format"),
            Self::Storage => Some("public-storage"),
            Self::Discovery => Some("discovery-corpus"),
            Self::Notifications => Some("notifications"),
            _ => None,
        }
    }

    fn fixture(self) -> &'static str {
        match self {
            Self::Main => "aggregate alias; expands to named suites, including storage",
            Self::EditorFirstDraft => {
                "migrated App scratch; two fresh fixture authors, default and Markdown"
            }
            Self::Reader => {
                "migrated App scratch, sample public work/private draft and catalog; loopback author"
            }
            Self::Discussion => {
                "sample Reader/author; App scratch writes and labelled intercepted projections"
            }
            Self::Appearance => {
                "sample App scratch and operator tag catalog; loopback fixture, appearance and shell/nav matrices"
            }
            Self::Desk => {
                "sample App scratch with loopback author; labelled Desk responses, isolated empty browser context, sign-in-disabled guest App"
            }
            Self::EditorChrome | Self::Editor | Self::EditorFormat => {
                "sample App scratch and operator tag catalog; loopback author, new drafts and CDP input"
            }
            Self::Storage => {
                "App scratch and Chrome profile reuse; public IndexedDB, tabs, migration/failure/reopen"
            }
            Self::Discovery => {
                "App scratch with tag catalog and SQL publication corpus; loopback fixture"
            }
            Self::Restart => {
                "App scratch; loopback author and two operator tags; browser restart-prepare -> graceful App stop -> retire tag -> App start -> after-restart"
            }
            Self::Endorsement => {
                "App scratch; loopback author + second principal with moderator grant"
            }
            Self::Review => "App scratch; loopback author, moderator and reviewer/admin seat",
            Self::Auth => {
                "sample App scratch + separate real Identity scratch/local outbox; also sign-in-disabled guest"
            }
            Self::Password | Self::AccountDeletion | Self::AuthorSearch => {
                "separate fresh App and real Identity scratch; native credentials and local mail outbox"
            }
            Self::Publication => {
                "dedicated seeded App scratch/tag catalog and real Identity scratch/local outbox"
            }
            Self::Notifications => {
                "sample App scratch with loopback author; Notifications R4 browser contracts and responsive states"
            }
            Self::PerfBaseline | Self::ReturnPosition => {
                "guest App scratch with discovery/perf corpus; lab/perf artifact"
            }
            Self::Standalone => {
                "standalone CSR bundle, App API, separate App and real Identity scratch/local outbox"
            }
        }
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
    pub capture: Capture,
    pub filter: Option<String>,
    /// `verify unit OWNER --test NAME`: one integration test target instead of lib/bins.
    pub test: Option<String>,
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
        match r.target {
            T::Static(scope) => {
                plan.tools.extend(scope.tools());
                return plan;
            }
            T::Postgres(scope) => {
                plan.databases.push(crate::TEST_DATABASE_URL);
                if scope.lifecycle() {
                    plan.tools.extend([P::Psql, P::PgBackup]);
                }
                return plan;
            }
            T::Check(_, target) => {
                if matches!(target, CheckTarget::Hydrate | CheckTarget::Csr) {
                    plan.tools.push(P::WasmTarget);
                }
                return plan;
            }
            T::Unit(_) => return plan,
            _ => {}
        }
        plan.artifacts = match r.target {
            T::Api => vec![A::App],
            T::Spa => vec![A::Spa],
            T::Web => vec![A::Bundle, A::App],
            T::Worker => vec![A::Worker],
            T::Identity => vec![A::Identity],
            T::Preview => vec![A::Preview],
            T::Unit(_) | T::Static(_) | T::Check(_, _) | T::Postgres(_) => unreachable!(),
        };
        if matches!(r.target, T::Web | T::Spa | T::Preview) {
            plan.tools
                .extend([P::WasmTarget, P::Bindgen, P::Node, P::Npm]);
        }
        if r.operation == O::Verify && r.target == T::Web {
            plan.scenarios = Scenario::selectable()
                .filter(|s| {
                    *s != Scenario::Main
                        && (r.scenarios.contains(s)
                            || (s.in_main() && r.scenarios.contains(&Scenario::Main)))
                })
                .collect();
            if plan.scenarios.contains(&Scenario::Standalone) {
                if plan.scenarios.len() == 1 {
                    plan.artifacts = vec![A::Spa, A::App];
                } else {
                    plan.artifacts.push(A::Spa);
                }
            }
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
            plan.tools.push(P::Chrome);
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
                _ => unreachable!(),
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
        let rust_scope = match self.request.target {
            Target::Static(scope) => Some(scope.describe()),
            Target::Check(owner, target) => Some(format!(
                "coverage: compile/type check only (no test execution or package binary link)\nselected: cargo {}\ndependencies: selected Cargo target dependency graph\nnot selected: other workspace owners as roots, runtime, tests, App bundle, database, browser\n",
                target.args(owner).join(" "),
            )),
            Target::Unit(owner) => {
                let args = crate::verify::unit_args(
                    owner,
                    self.request.test.as_deref(),
                    self.request.filter.as_deref(),
                );
                let (coverage, excluded) = match &self.request.test {
                    Some(name) => (
                        format!("integration test target `{name}`"),
                        "lib/bins unit tests, other integration targets",
                    ),
                    None => (
                        "selected unit/contract tests".to_string(),
                        "integration targets",
                    ),
                };
                Some(format!(
                    "coverage: {coverage}\nselected: cargo {args} --list; cargo {args} --list --ignored; cargo {args}\ndependencies: compile/link {} test targets; filter selects runnable tests, Cargo can still compile/link the requested targets\nnot selected: other workspace owners as roots, {excluded}, doctests, database, browser\n",
                    owner.package(),
                    args = crate::verify::display_args(&args),
                ))
            }
            Target::Postgres(scope) => Some(scope.describe(self.request.filter.as_deref())),
            _ => None,
        };
        if let Some(scope) = rust_scope {
            return format!(
                "bootstrap: cargo xtask compiles/links its own tool before dispatch; --plan does not run selected commands\nrequired tools: {:?}\nattached database configuration (values hidden): {:?}\n{scope}",
                self.tools, self.databases,
            );
        }
        let web = self.request.operation == Operation::Verify && self.request.target == Target::Web;
        let coverage = if web {
            if Scenario::ALL.iter().all(|s| self.scenarios.contains(s))
                && self.request.capture == Capture::Full
            {
                "FULL Web regression"
            } else {
                "SELECTED Web checks"
            }
        } else {
            "requested target only"
        };
        let auth = if web {
            "specified per scenario below".to_string()
        } else {
            format!("{:?}", self.request.auth)
        };
        let selected: Vec<_> = self.request.scenarios.iter().map(|s| s.name()).collect();
        let excluded: Vec<_> = Scenario::selectable()
            .filter(|s| *s != Scenario::Main && !self.scenarios.contains(s))
            .map(Scenario::name)
            .collect();
        let dependencies: Vec<_> = self
            .scenarios
            .iter()
            .filter(|s| !self.request.scenarios.contains(s))
            .map(|s| s.name())
            .collect();
        let mut fixtures = String::new();
        if web {
            fixtures.push_str(&format!(
                "capture: {} ({})\n",
                self.request.capture.name(),
                if self.request.capture == Capture::Full {
                    "PNG + validated context per scenario; functional and a11y checks"
                } else {
                    "functional, a11y and DOM geometry checks; PNG/visual evidence NOT_RUN"
                },
            ));
            for scenario in &self.scenarios {
                fixtures.push_str(&format!("  {}: {}\n", scenario.name(), scenario.fixture()));
            }
        }
        let mut dag = String::new();
        let profile = if self.request.release {
            "wasm-release"
        } else {
            "dev"
        };
        for artifact in &self.artifacts {
            let feature = match artifact {
                Artifact::Spa => "csr",
                Artifact::Bundle
                    if self.scenarios.iter().any(|s| {
                        matches!(s, Scenario::PerfBaseline | Scenario::ReturnPosition)
                    }) =>
                {
                    "perf-probe"
                }
                Artifact::Bundle => "hydrate",
                _ => continue,
            };
            dag.push_str(&crate::web_dag::describe_dag(
                *artifact == Artifact::Spa,
                profile,
                feature,
            ));
        }
        format!(
            "coverage: {coverage}\noperation: {:?}; target: {:?}; auth: {auth}\nbuild: {:?}; profile: {}\nrequired tools: {:?}\nowned services: {:?}\nattached database configuration (values hidden): {:?}\nmigrations: {:?}; dev seed: {} (Web fixtures follow the scenario catalog)\nselected checks: {selected:?}\nalias expansion: {dependencies:?}\nnot selected Web checks: {excluded:?}\n{fixtures}{dag}API target isolation: no bundle, no SSR dependencies or static routes.\ndata: dev attaches prepared databases; Web verification owns scratch only. No containers.\nartifacts: build from current source; Web bundles may reuse verified bindgen, CSS, editor JS and static outputs.\n",
            self.request.operation,
            self.request.target,
            self.artifacts,
            if self.request.release {
                "release (wasm-release for bundle)"
            } else {
                "dev"
            },
            self.tools,
            self.services,
            self.databases,
            self.migrations,
            self.request.seed,
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
            capture: Capture::Full,
            filter: None,
            test: None,
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
        for s in Scenario::selectable().filter(|s| *s != Scenario::Main) {
            let mut r = request(Operation::Verify, Target::Web);
            r.scenarios = vec![s];
            let p = Plan::resolve(r);
            assert_eq!(p.scenarios, [s]);
            assert_eq!(p.artifacts.contains(&Artifact::Identity), s.identity());
            assert!(!p.artifacts.contains(&Artifact::Worker));
        }
    }
    #[test]
    fn perf_baseline_is_explicit_only_and_reuses_app_scratch() {
        let mut r = request(Operation::Verify, Target::Web);
        r.scenarios = Scenario::ALL.to_vec();
        let full = Plan::resolve(r);
        assert!(!full.scenarios.contains(&Scenario::PerfBaseline));
        let mut r = request(Operation::Verify, Target::Web);
        r.scenarios = vec![Scenario::PerfBaseline];
        let perf = Plan::resolve(r);
        assert_eq!(perf.scenarios, vec![Scenario::PerfBaseline]);
        assert_eq!(perf.services, vec![Service::App]);
        assert!(!perf.artifacts.contains(&Artifact::Identity));
        assert!(perf.describe().contains("SELECTED Web checks"));
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
    #[test]
    fn preview_requires_node_and_npm_for_tailwind_without_duplicating_tools() {
        let build = Plan::resolve(request(Operation::Build, Target::Preview));
        assert_eq!(
            build.tools,
            vec![
                Tool::Rust,
                Tool::WasmTarget,
                Tool::Bindgen,
                Tool::Node,
                Tool::Npm,
            ]
        );
        let verify = Plan::resolve(request(Operation::Verify, Target::Preview));
        assert_eq!(
            verify.tools,
            vec![
                Tool::Rust,
                Tool::WasmTarget,
                Tool::Bindgen,
                Tool::Node,
                Tool::Npm,
                Tool::Chrome,
            ]
        );
    }
}
