//! Process-level checks: the web lane (real server + headless Chrome) and binary lifecycle.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::doctor;
use crate::env::Env;
use crate::plan::{Plan, Scenario};
use crate::process::Report;
use crate::root;
const APP_ROLE: &str = "vink_verify_web_runtime";

/// Scratch database recreated by every `verify web` run; never a developer database.
const SCRATCH_DATABASE: &str = "vink_verify_web";
const DROP_SCRATCH: &str = "DROP DATABASE IF EXISTS vink_verify_web WITH (FORCE)";
const CREATE_SCRATCH: &str = "CREATE DATABASE vink_verify_web";
/// `dev_seed::SAMPLE_AUTHOR`: the browser acts as the owner of the sample work.
const SAMPLE_AUTHOR: &str = "11111111-1111-4111-8111-111111111111";
/// `dev_seed::OTHER_PRINCIPAL`: a reader of that work for the endorsement phase (LVE #73), with
/// a moderator grant so it can approve the author's comments it then endorses (neither the
/// comments' writer nor the work's owner, so not recused).
const OTHER_PRINCIPAL: &str = "22222222-2222-4222-8222-222222222222";
/// The publication-author scenario gets its own App database: the Auth Web phase deliberately
/// exhausts the App's per-network sign-in limiter (stored in the database), and a scenario that
/// signs in three people must not depend on running before or after it.
const PUBLICATION_DATABASE: &str = "vink_verify_web_publication";
const PASSWORD_AUTH_DATABASE: &str = "vink_verify_web_password_auth";
/// Auth Web phase: Identity's own scratch database and a runtime role that owns nothing in it.
const IDENTITY_DATABASE: &str = "vink_verify_web_identity";
const IDENTITY_ROLE: &str = "vink_verify_web_identity_runtime";

use crate::lifecycle::{Guard, binary, free_port, http_status};

/// Replaces the database name in a PostgreSQL URL.
fn with_database(url: &str, database: &str) -> Result<String, String> {
    let scheme_end = url.find("://").ok_or("DATABASE_URL has no scheme")? + 3;
    let (base, query) = match url.find('?') {
        Some(i) => (&url[..i], &url[i..]),
        None => (url, ""),
    };
    let path_start = base[scheme_end..]
        .find('/')
        .map(|i| scheme_end + i)
        .unwrap_or(base.len());
    Ok(format!("{}/{database}{query}", &base[..path_start]))
}

fn recreate_scratch_database(url: &str) -> Result<String, String> {
    let admin = url.to_string();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    runtime.block_on(async move {
        use sqlx::{Connection, Executor};
        let mut connection = sqlx::PgConnection::connect(&admin)
            .await
            .map_err(|e| format!("connect: {e}"))?;
        connection
            .execute(DROP_SCRATCH)
            .await
            .map_err(|e| format!("drop scratch database: {e}"))?;
        connection
            .execute(CREATE_SCRATCH)
            .await
            .map_err(|e| format!("create scratch database (needs CREATEDB): {e}"))?;
        Ok::<_, String>(())
    })?;
    with_database(url, SCRATCH_DATABASE)
}

fn drop_scratch_database(url: &str) {
    let admin = url.to_string();
    if let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        runtime.block_on(async move {
            use sqlx::{Connection, Executor};
            if let Ok(mut connection) = sqlx::PgConnection::connect(&admin).await {
                let _ = connection.execute(DROP_SCRATCH).await;
            }
        });
    }
}

/// Everything served under `/pkg/` is public. Server settings and throwaway passwords must not
/// appear in it.
fn bundle_secrets(dir: &Path) -> Result<(), String> {
    const NEEDLES: [&str; 6] = [
        "postgres://",
        "postgresql://",
        "DATABASE_URL",
        "VINK_IDENTITY",
        "_PASSWORD",
        "-local-only",
    ];
    let mut stack = vec![dir.to_path_buf()];
    let mut files = 0;
    while let Some(path) = stack.pop() {
        for entry in std::fs::read_dir(&path).map_err(|e| format!("{}: {e}", path.display()))? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            files += 1;
            let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
            let text = String::from_utf8_lossy(&bytes);
            if let Some(needle) = NEEDLES.iter().find(|n| text.contains(**n)) {
                return Err(format!("{} contains `{needle}`", path.display()));
            }
        }
    }
    println!("==> {files} bundle file(s) scanned");
    Ok(())
}

struct Server {
    port: u16,
    env: Vec<(String, String)>,
    log: PathBuf,
    release: bool,
}

impl Server {
    fn profile_name(&self) -> &'static str {
        if self.release { "release" } else { "dev" }
    }

    fn binary(&self, root: &Path, name: &str) -> PathBuf {
        if self.release {
            root.join("target/release").join(name)
        } else {
            binary(root, name)
        }
    }

    fn as_guest(&self) -> Self {
        Self {
            port: self.port,
            env: self
                .env
                .iter()
                .filter(|(key, _)| key != "VINK_DEV_PRINCIPAL")
                .cloned()
                .collect(),
            log: self.log.with_file_name("server-guest.log"),
            release: self.release,
        }
    }

    fn as_guest_on(&self, port: u16) -> Self {
        let mut guest = self.as_guest();
        guest.port = port;
        for (key, value) in &mut guest.env {
            match key.as_str() {
                "VINK_BIND" => *value = format!("127.0.0.1:{port}"),
                "VINK_PUBLIC_ORIGIN" => *value = format!("http://127.0.0.1:{port}"),
                _ => {}
            }
        }
        guest.log = self.log.with_file_name("server-reader-guest.log");
        guest
    }

    /// The same database served to another loopback fixture principal on its own port.
    fn with_principal(&self, principal: &str, port: u16) -> Self {
        let mut env = self.env.clone();
        for (key, value) in &mut env {
            match key.as_str() {
                "VINK_DEV_PRINCIPAL" => *value = principal.into(),
                "VINK_BIND" => *value = format!("127.0.0.1:{port}"),
                "VINK_PUBLIC_ORIGIN" => *value = format!("http://127.0.0.1:{port}"),
                _ => {}
            }
        }
        Self {
            port,
            env,
            log: self.log.with_file_name("server-reader.log"),
            release: self.release,
        }
    }

    /// The same database served with sign-in open against a real Identity process and without
    /// any fixture principal: every private answer depends on a session Identity verifies.
    fn with_identity(&self, port: u16, socket: &Path, uid: u32) -> Self {
        let mut env: Vec<(String, String)> = self
            .env
            .iter()
            .filter(|(key, _)| key != "VINK_DEV_PRINCIPAL")
            .cloned()
            .collect();
        for (key, value) in &mut env {
            match key.as_str() {
                "VINK_BIND" => *value = format!("127.0.0.1:{port}"),
                "VINK_PUBLIC_ORIGIN" => *value = format!("http://127.0.0.1:{port}"),
                _ => {}
            }
        }
        env.extend([
            (
                "VINK_IDENTITY_SOCKET".to_string(),
                socket.display().to_string(),
            ),
            ("VINK_IDENTITY_SERVER_UID".to_string(), uid.to_string()),
            ("VINK_AUTH_SIGN_IN".to_string(), "enabled".to_string()),
        ]);
        Self {
            port,
            env,
            log: self.log.with_file_name("server-auth.log"),
            release: self.release,
        }
    }

    /// The same server settings on another App database (its own rows, including rate limits).
    fn with_database(&self, database: &str) -> Result<Self, String> {
        let mut env = self.env.clone();
        for (key, value) in &mut env {
            if key == "DATABASE_URL" || key == "VINK_MIGRATE_DATABASE_URL" {
                *value = with_database(value, database)?;
            }
        }
        Ok(Self {
            port: self.port,
            env,
            log: self.log.with_file_name("server-publication.log"),
            release: self.release,
        })
    }

    fn command(&self, root: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(self.binary(root, "vink-app"));
        crate::env::clear_capabilities(&mut command);
        command.current_dir(root).args(args);
        let migrate = args.first() == Some(&"migrate");
        command.envs(
            self.env
                .iter()
                .filter(|(k, _)| {
                    if migrate {
                        k == "VINK_MIGRATE_DATABASE_URL" || k == "VINK_LOG_FORMAT"
                    } else {
                        k != "VINK_MIGRATE_DATABASE_URL"
                    }
                })
                .map(|(k, v)| (k, v)),
        );
        command
    }

    fn start(&self, root: &Path) -> Result<Guard, String> {
        self.start_command(root, "serve")
    }

    fn start_command(&self, root: &Path, verb: &str) -> Result<Guard, String> {
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log)
            .map_err(|e| format!("{}: {e}", self.log.display()))?;
        let child = self
            .command(root, &[verb])
            .stdout(Stdio::null())
            .stderr(log)
            .spawn()
            .map_err(|e| format!("spawn vink-app: {e}"))?;
        let mut guard = Guard(Some(child));
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            crate::lifecycle::check_cancelled()?;
            if let Some(Ok(Some(status))) = guard.0.as_mut().map(Child::try_wait) {
                return Err(format!(
                    "vink-app exited early with {status}; see {}",
                    self.log.display()
                ));
            }
            if http_status(self.port, "/health/ready") == Some(200) {
                println!(
                    "==> vink-app ready on port {} (pid {})",
                    self.port,
                    guard.pid()
                );
                return Ok(guard);
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        Err(format!("not ready within 30s; see {}", self.log.display()))
    }
}

struct Browser {
    chrome: String,
    capture: crate::plan::Capture,
}

fn scenario_state(state: &Path, scenario: &str) -> std::path::PathBuf {
    state.with_file_name(scenario).join("smoke-state.json")
}

fn smoke(
    root: &Path,
    node_args: &[&str],
    chrome: &Browser,
    base: &str,
    state: &Path,
) -> Result<(), String> {
    smoke_with_env(root, node_args, chrome, base, state, &[])
}

/// `envs` reach only this browser phase. The publication-author scenario uses them to run the
/// operator CLI (`vink-app admin add`) that seats its admin: setup, never a check.
fn smoke_with_env(
    root: &Path,
    node_args: &[&str],
    chrome: &Browser,
    base: &str,
    state: &Path,
    envs: &[(String, String)],
) -> Result<(), String> {
    let mut command = Command::new("node");
    crate::env::clear_capabilities(&mut command);
    command
        .current_dir(root)
        .arg(root.join("apps/vink-web/e2e/browser-smoke.mjs"))
        .args([
            "--base",
            base,
            "--chrome",
            &chrome.chrome,
            "--capture",
            chrome.capture.name(),
            "--state",
        ])
        .arg(state)
        .args(node_args)
        .envs(
            envs.iter()
                .filter(|(k, _)| k != "VINK_MIGRATE_DATABASE_URL")
                .map(|(k, v)| (k, v)),
        );
    crate::process::run(
        &mut command,
        &format!("browser smoke {}", node_args.join(" ")),
    )
}

/// Browser phases that run against the real Identity process (no fixture principal).
#[derive(Clone, Copy)]
enum IdentityPhase {
    /// Email code, pen name, draft, sign-out, account switch.
    Auth,
    /// AUTH-PASSWORD-WEB-01: the same App against native Identity with both methods.
    PasswordAuth,
    AccountDeletion,
    Standalone,
    /// Author discovery with separate public and session-scoped projections.
    AuthorSearch,
    /// PUB-AUTHOR-WEB-01: the author's whole publication path with a second, granted moderator.
    PublicationAuthor,
}

/// Starts the real `vink-identity` (email-code or native authority) on its own scratch database and runs
/// the Auth Web browser phase against an App that has no fixture principal. The local outbox is
/// the only mail transport, so the browser check reads codes from that directory; no production
/// transport is exercised. The runtime role is created here with a throwaway password and owns
/// nothing (`verify postgres` is the evidence for `services/postgres` provisioning itself).
fn auth_web(
    root: &Path,
    server: &Server,
    admin_url: &str,
    chrome: &Browser,
    state: &Path,
    phase: IdentityPhase,
) -> Result<(), String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let dir = state.parent().ok_or("state file has no directory")?;
    let socket_owner = crate::lifecycle::SocketDir::new()?;
    let (sock_dir, outbox) = (socket_owner.0.clone(), dir.join("outbox"));
    for private in [&sock_dir, &outbox] {
        std::fs::create_dir_all(private).map_err(|e| format!("{}: {e}", private.display()))?;
        std::fs::set_permissions(private, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
    }
    let socket = sock_dir.join("identity.sock");
    let uid = std::fs::metadata(&sock_dir)
        .map(|m| m.uid())
        .map_err(|e| e.to_string())?;
    let admin = crate::pgurl::PgUrl::parse(admin_url)?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or_default();
    let password = format!("verify-{:x}-{nonce:x}", std::process::id());
    drop_identity_scratch(admin_url);
    for statement in [
        format!("CREATE DATABASE {IDENTITY_DATABASE}"),
        format!("CREATE ROLE {IDENTITY_ROLE} LOGIN PASSWORD '{password}'"),
    ] {
        crate::identity::sql(admin_url, &statement)
            .map_err(|e| format!("Identity scratch database/role: {e}"))?;
    }
    let admin_identity = admin.with_database(IDENTITY_DATABASE);
    let mut migrate = Command::new(server.binary(root, "vink-identity"));
    crate::env::clear_capabilities(&mut migrate);
    migrate
        .current_dir(root)
        .arg("migrate")
        .env("VINK_IDENTITY_MIGRATE_DATABASE_URL", &admin_identity)
        .env("VINK_LOG_FORMAT", "text");
    crate::process::run(&mut migrate, "vink-identity migrate (scratch)")?;
    for statement in [
        "REVOKE CREATE ON SCHEMA public FROM PUBLIC".to_string(),
        format!("GRANT USAGE ON SCHEMA public TO {IDENTITY_ROLE}"),
        format!(
            "GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO {IDENTITY_ROLE}"
        ),
        format!("GRANT USAGE ON ALL SEQUENCES IN SCHEMA public TO {IDENTITY_ROLE}"),
    ] {
        crate::identity::sql(&admin_identity, &statement)
            .map_err(|e| format!("Identity runtime grants: {e}"))?;
    }
    crate::identity::sql(
        &admin_identity,
        &format!("REVOKE INSERT, UPDATE, DELETE ON _sqlx_migrations FROM {IDENTITY_ROLE}"),
    )?;
    let log = std::fs::File::create(dir.join("identity.log")).map_err(|e| e.to_string())?;
    let mut identity_command = Command::new(server.binary(root, "vink-identity"));
    crate::env::clear_capabilities(&mut identity_command);
    let child = identity_command
        .current_dir(root)
        .arg("serve")
        .env(
            "VINK_IDENTITY_DATABASE_URL",
            admin.with(IDENTITY_ROLE, &password, IDENTITY_DATABASE),
        )
        .env("VINK_IDENTITY_SOCKET", &socket)
        .env("VINK_IDENTITY_CALLER_UID", uid.to_string())
        .env(
            "VINK_IDENTITY_AUTHORITY",
            match phase {
                IdentityPhase::PasswordAuth
                | IdentityPhase::AccountDeletion
                | IdentityPhase::AuthorSearch
                | IdentityPhase::Standalone => "native",
                _ => "email-code",
            },
        )
        .env("VINK_IDENTITY_MAIL_TRANSPORT", "local-outbox")
        .env("VINK_IDENTITY_LOCAL_OUTBOX", &outbox)
        .env("VINK_LOG_FORMAT", "text")
        .stdout(Stdio::null())
        .stderr(log)
        .spawn()
        .map_err(|e| format!("spawn vink-identity: {e}"))?;
    let mut identity = Guard(Some(child));
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        crate::lifecycle::check_cancelled()?;
        if let Some(Ok(Some(status))) = identity.0.as_mut().map(Child::try_wait) {
            return Err(format!(
                "vink-identity exited early with {status}; see {}",
                dir.join("identity.log").display()
            ));
        }
        if crate::identity::request(&socket, "GET", "/health/ready", &[], b"")
            .is_ok_and(|(status, _)| status == 200)
        {
            break;
        }
        if Instant::now() > deadline {
            return Err("vink-identity not ready on its socket within 20s".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    if matches!(phase, IdentityPhase::Auth) {
        let url = server
            .env
            .iter()
            .find(|(key, _)| key == "DATABASE_URL")
            .map(|(_, value)| value)
            .ok_or("Auth App scratch URL missing")?;
        crate::identity::sql(
            url,
            include_str!("../../apps/vink-web/e2e/fixtures/reader-contextual-auth.sql"),
        )?;
    }
    let mut app = server.with_identity(free_port()?, &socket, uid);
    let mut frontend = None;
    let origin = if matches!(phase, IdentityPhase::Standalone) {
        let port = free_port()?;
        let origin = format!("http://127.0.0.1:{port}");
        for (key, value) in &mut app.env {
            if key == "VINK_PUBLIC_ORIGIN" {
                value.clone_from(&origin);
            }
        }
        let mut command = Command::new("node");
        crate::env::clear_capabilities(&mut command);
        command.current_dir(root);
        if std::env::var("VINK_STANDALONE_WORKERD").as_deref() == Ok("1") {
            let staged = dir.join("pages");
            let mut pack = Command::new("python3");
            pack.current_dir(root)
                .args([
                    "-B",
                    "infra/cloudflare/production/artifacts.py",
                    "--spa",
                    crate::build::SPA_SCRATCH_SITE,
                    "--out",
                ])
                .arg(&staged);
            crate::process::run(&mut pack, "stage standalone Pages deployment")?;
            command
                .arg("infra/cloudflare/production/local-edge.mjs")
                .arg(staged)
                .arg(format!("http://127.0.0.1:{}", app.port))
                .arg(port.to_string());
        } else {
            command
                .args([
                    "apps/vink-web/dev-server.mjs",
                    crate::build::SPA_SCRATCH_SITE,
                ])
                .arg(port.to_string())
                .arg(app.port.to_string());
        }
        let mut host = Guard(Some(command.spawn().map_err(|e| e.to_string())?));
        crate::workflow::ready(&mut host, || {
            std::net::TcpStream::connect(("127.0.0.1", port)).is_ok()
        })?;
        frontend = Some(host);
        origin
    } else {
        format!("http://127.0.0.1:{}", app.port)
    };
    let mut guard = app.start_command(
        root,
        if frontend.is_some() {
            "serve-api"
        } else {
            "serve"
        },
    )?;
    let outbox = outbox.display().to_string();
    let result = match phase {
        IdentityPhase::Standalone => {
            smoke(root, &["--phase", "standalone"], chrome, &origin, state)
        }
        IdentityPhase::Auth => smoke(
            root,
            &["--phase", "auth", "--outbox", &outbox],
            chrome,
            &origin,
            state,
        ),
        IdentityPhase::PasswordAuth
        | IdentityPhase::AccountDeletion
        | IdentityPhase::AuthorSearch => smoke_with_env(
            root,
            &[
                "--phase",
                match phase {
                    IdentityPhase::AuthorSearch => "author-search",
                    IdentityPhase::AccountDeletion => "account-deletion",
                    _ => "auth-password",
                },
                "--outbox",
                &outbox,
            ],
            chrome,
            &origin,
            state,
            &[(
                "VINK_HANDLE_SCRATCH".into(),
                app.env
                    .iter()
                    .find(|(key, _)| {
                        key == if matches!(phase, IdentityPhase::AccountDeletion) {
                            "VINK_MIGRATE_DATABASE_URL"
                        } else {
                            "DATABASE_URL"
                        }
                    })
                    .ok_or("missing password scratch database URL")?
                    .1
                    .clone(),
            )],
        ),
        IdentityPhase::PublicationAuthor => {
            let mut envs = app.env.clone();
            envs.push((
                "VINK_E2E_APP_BIN".into(),
                app.binary(root, "vink-app").display().to_string(),
            ));
            smoke_with_env(
                root,
                &["--phase", "publication-author", "--outbox", &outbox],
                chrome,
                &origin,
                state,
                &envs,
            )
        }
    };
    let _ = guard.terminate(Duration::from_secs(15));
    let _ = identity.terminate(Duration::from_secs(10));
    result
}

/// PUB-AUTHOR-WEB-01 on its own App database: created, migrated, seeded with the operator CLI
/// (including the tag catalog the scenario picks from), run against the real Identity, dropped.
fn publication_author(
    root: &Path,
    server: &Server,
    admin_url: &str,
    chrome: &Browser,
    state: &Path,
) -> Result<(), String> {
    let drop = format!("DROP DATABASE IF EXISTS {PUBLICATION_DATABASE} WITH (FORCE)");
    let _ = crate::identity::sql(admin_url, &drop);
    crate::identity::sql(
        admin_url,
        &format!("CREATE DATABASE {PUBLICATION_DATABASE}"),
    )
    .map_err(|e| format!("publication scratch database: {e}"))?;
    let result = (|| {
        let own = server.with_database(PUBLICATION_DATABASE)?;
        prepare_app(root, &own, true)?;
        seed_tag_catalog(root, &own)?;
        auth_web(
            root,
            &own,
            admin_url,
            chrome,
            state,
            IdentityPhase::PublicationAuthor,
        )
    })();
    let _ = crate::identity::sql(admin_url, &drop);
    result
}

/// The email-code phase exhausts the App's network limiter. Password auth also uses email
/// (including enrollment), so give it independent App state and fresh Identity state.
fn native_auth_scratch(
    root: &Path,
    server: &Server,
    admin_url: &str,
    chrome: &Browser,
    state: &Path,
    phase: IdentityPhase,
) -> Result<(), String> {
    let drop = format!("DROP DATABASE IF EXISTS {PASSWORD_AUTH_DATABASE} WITH (FORCE)");
    let _ = crate::identity::sql(admin_url, &drop);
    crate::identity::sql(
        admin_url,
        &format!("CREATE DATABASE {PASSWORD_AUTH_DATABASE}"),
    )
    .map_err(|e| format!("password auth scratch database: {e}"))?;
    let result = (|| {
        let app = server.with_database(PASSWORD_AUTH_DATABASE)?;
        prepare_app(
            root,
            &app,
            matches!(
                phase,
                IdentityPhase::Standalone | IdentityPhase::AccountDeletion
            ),
        )?;
        if matches!(phase, IdentityPhase::AccountDeletion) {
            crate::process::run(
                &mut app.command(root, &["principal", "backfill-handles"]),
                "provision public handles in deletion scratch",
            )?;
        }
        if matches!(phase, IdentityPhase::Standalone) {
            // The standalone writer checks a pending human review. Pin its isolated
            // fixture independently of the default evaluator and deployment settings.
            crate::identity::sql(
                &with_database(admin_url, PASSWORD_AUTH_DATABASE)?,
                "UPDATE moderation_automation_state SET mode = 'paused' WHERE id = 1",
            )?;
            seed_tag_catalog(root, &app)?;
            crate::process::run(
                &mut app.command(root, &["principal", "backfill-handles"]),
                "provision public handles in standalone scratch",
            )?;
        }
        auth_web(root, &app, admin_url, chrome, state, phase)
    })();
    let _ = crate::identity::sql(admin_url, &drop);
    result
}

fn drop_identity_scratch(admin_url: &str) {
    for statement in [
        format!("DROP DATABASE IF EXISTS {IDENTITY_DATABASE} WITH (FORCE)"),
        format!("DROP ROLE IF EXISTS {IDENTITY_ROLE}"),
    ] {
        let _ = crate::identity::sql(admin_url, &statement);
    }
}

pub fn visual_evidence_unit_tests(root: &Path) -> Result<(), String> {
    crate::doctor::node_version()?;
    let mut node = Command::new("node");
    node.current_dir(root)
        .args(["--test", "apps/vink-web/e2e/visual-evidence.test.mjs"]);
    crate::process::run(&mut node, "visual evidence oracle unit tests")
}

/// #234 analyzer invariants (lab statistics, transport classification and the negative
/// duplicate control). Pure Node: no server, database or browser, so it belongs to `quick`.
pub fn web_perf_analyzer_unit_tests(root: &Path) -> Result<(), String> {
    crate::doctor::node_version()?;
    let mut node = Command::new("node");
    node.current_dir(root)
        .args(["--test", "apps/vink-web/e2e/web-perf-baseline.test.mjs"]);
    crate::process::run(&mut node, "web perf analyzer unit tests")
}

pub fn verify(report: &mut Report, env: &Env) -> bool {
    let plan = crate::cli::parse(&["verify", "web"])
        .expect("valid command")
        .expect("scoped")
        .plan;
    println!("{}", plan.describe());
    verify_selected(report, env, &plan)
}

/// One catalog/plan for both full and selected verification. Setup belongs to the scenario,
/// so choosing auth alone never relies on main having migrated/seeded a different run.
pub fn verify_selected(report: &mut Report, env: &Env, plan: &Plan) -> bool {
    let root = root();
    if !report.step("selected prerequisites", || doctor::scoped(plan, env)) {
        return false;
    }
    if !report.step(
        "visual evidence oracles: manifest, PNG, overflow, AX, modal unit tests",
        || visual_evidence_unit_tests(&root),
    ) {
        return false;
    }
    let url = env
        .get(crate::TEST_DATABASE_URL)
        .expect("preflight checked");
    let chrome = Browser {
        chrome: doctor::chrome_binary(env).expect("preflight checked"),
        capture: plan.request.capture,
    };
    let _scratch_lock = match crate::lifecycle::ScratchLock::acquire(&url) {
        Ok(lock) => lock,
        Err(e) => {
            report.fail("scratch ownership", &e);
            return false;
        }
    };
    if !report.step("selected artifacts", || crate::workflow::artifacts(plan)) {
        return false;
    }
    if !report.step("browser bundle contains no credentials", || {
        if plan.artifacts.contains(&crate::plan::Artifact::Spa) {
            bundle_secrets(&root.join(crate::build::SPA_SCRATCH_SITE))?;
        }
        if plan.artifacts.contains(&crate::plan::Artifact::Bundle) {
            bundle_secrets(&root.join("target/site/pkg"))?;
        }
        Ok(())
    }) {
        return false;
    }
    let out = match crate::lifecycle::run_directory(&root, "verify-web") {
        Ok(out) => out,
        Err(e) => {
            report.fail("evidence directory", &e);
            return false;
        }
    };
    println!("==> Web run evidence: {}", out.display());
    let primary = plan
        .scenarios
        .iter()
        .any(|s| !s.identity() || *s == Scenario::Auth);
    let result = (|| -> Result<bool, String> {
        let scratch_url = if primary {
            recreate_scratch_database(&url)?
        } else {
            with_database(&url, SCRATCH_DATABASE)?
        };
        crate::identity::sql(&url, &format!("DROP ROLE IF EXISTS {APP_ROLE}"))?;
        let password = format!(
            "web-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_nanos()
        );
        crate::identity::sql(
            &url,
            &format!("CREATE ROLE {APP_ROLE} LOGIN PASSWORD '{password}'"),
        )?;
        let runtime_url =
            crate::pgurl::PgUrl::parse(&url)?.with(APP_ROLE, &password, SCRATCH_DATABASE);
        let port = free_port()?;
        let origin = format!("http://127.0.0.1:{port}");
        let server = Server {
            port,
            env: vec![
                ("DATABASE_URL".into(), runtime_url),
                ("VINK_MIGRATE_DATABASE_URL".into(), scratch_url),
                ("VINK_PROFILE".into(), "local".into()),
                ("VINK_BIND".into(), format!("127.0.0.1:{port}")),
                ("VINK_PUBLIC_ORIGIN".into(), origin.clone()),
                (
                    "VINK_SITE_ROOT".into(),
                    root.join("target/site").display().to_string(),
                ),
                ("VINK_DEV_PRINCIPAL".into(), SAMPLE_AUTHOR.into()),
                ("VINK_AUTH_SIGN_IN".into(), "disabled".into()),
                ("VINK_LOG_FORMAT".into(), "text".into()),
            ],
            log: out.join("server.log"),
            release: plan.request.release,
        };
        let state = out.join("smoke-state.json");
        if primary {
            prepare_app(&root, &server, true)?;
        }
        let fixture: Vec<_> = plan
            .scenarios
            .iter()
            .copied()
            .filter(|s| !s.identity())
            .collect();
        let mut ok = fixture.is_empty()
            || run_web_checks(report, &root, &server, &chrome, &origin, &state, &fixture);
        for scenario in plan.scenarios.iter().filter(|s| s.identity()) {
            if !ok {
                break;
            }
            let state = scenario_state(&state, scenario.name());
            ok = report.step(
                &format!("scenario {} (real Identity/local outbox)", scenario.name()),
                || match scenario {
                    Scenario::Auth => {
                        auth_web(&root, &server, &url, &chrome, &state, IdentityPhase::Auth)?;
                        // A real deployment with the sign-in gate off, not a fabricated refusal.
                        let guest = server.as_guest();
                        let mut guard = guest.start(&root)?;
                        let result = smoke(
                            &root,
                            &["--phase", "reader-auth-closed"],
                            &chrome,
                            &origin,
                            &scenario_state(&state, "reader-auth-closed"),
                        );
                        let _ = guard.terminate(Duration::from_secs(15));
                        result
                    }
                    Scenario::Standalone => native_auth_scratch(
                        &root,
                        &server,
                        &url,
                        &chrome,
                        &state,
                        IdentityPhase::Standalone,
                    ),
                    Scenario::Password | Scenario::AccountDeletion | Scenario::AuthorSearch => {
                        native_auth_scratch(
                            &root,
                            &server,
                            &url,
                            &chrome,
                            &state,
                            match scenario {
                                Scenario::AuthorSearch => IdentityPhase::AuthorSearch,
                                Scenario::AccountDeletion => IdentityPhase::AccountDeletion,
                                _ => IdentityPhase::PasswordAuth,
                            },
                        )
                    }
                    Scenario::Publication => {
                        publication_author(&root, &server, &url, &chrome, &state)
                    }
                    _ => unreachable!(),
                },
            );
        }
        Ok(ok)
    })();
    // Only fixed, reserved scratch names, while holding the shared DB lock. Never DATABASE_URL.
    if plan.scenarios.iter().any(|scenario| scenario.identity()) {
        drop_identity_scratch(&url);
    }
    if plan.scenarios.contains(&Scenario::Publication) {
        let _ = crate::identity::sql(
            &url,
            &format!("DROP DATABASE IF EXISTS {PUBLICATION_DATABASE} WITH (FORCE)"),
        );
    }
    if plan.scenarios.iter().any(|scenario| {
        matches!(
            scenario,
            Scenario::Password
                | Scenario::AccountDeletion
                | Scenario::AuthorSearch
                | Scenario::Standalone
        )
    }) {
        let _ = crate::identity::sql(
            &url,
            &format!("DROP DATABASE IF EXISTS {PASSWORD_AUTH_DATABASE} WITH (FORCE)"),
        );
    }
    if primary {
        drop_scratch_database(&url);
    }
    let _ = crate::identity::sql(&url, &format!("DROP ROLE IF EXISTS {APP_ROLE}"));
    match result {
        Ok(ok) => ok,
        Err(e) => {
            report.fail("scenario setup/execution", &e);
            false
        }
    }
}

fn prepare_app(root: &Path, server: &Server, seed: bool) -> Result<(), String> {
    crate::process::run(
        &mut server.command(root, &["migrate"]),
        "migrate App scratch",
    )?;
    let url = &server
        .env
        .iter()
        .find(|(k, _)| k == "VINK_MIGRATE_DATABASE_URL")
        .ok_or("missing scratch migration URL")?
        .1;
    for sql in [
        "REVOKE CREATE ON SCHEMA public FROM PUBLIC".to_string(),
        format!("GRANT USAGE ON SCHEMA public TO {APP_ROLE}"),
        format!(
            "GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO {APP_ROLE}"
        ),
        format!("GRANT USAGE ON ALL SEQUENCES IN SCHEMA public TO {APP_ROLE}"),
    ] {
        crate::identity::sql(url, &sql)?;
    }
    crate::identity::sql(
        url,
        &format!("REVOKE INSERT, UPDATE, DELETE ON _sqlx_migrations FROM {APP_ROLE}"),
    )?;
    if seed {
        crate::process::run(
            &mut server.command(root, &["dev-seed"]),
            "seed App scratch (runtime role)",
        )?;
    }
    Ok(())
}

/// Tag IDs the browser phases need from the seeded catalog.
struct TagSeed {
    /// Đô thị: saved on the restart probe draft, then retired.
    city: String,
    /// Mùa: the curator's successor for Đô thị.
    season: String,
    /// The catalog version after seeding; the retire step names it.
    version: u32,
}

/// One `vink-app tag …` operator command against the scratch database; returns its stdout.
fn tag_command(root: &Path, server: &Server, args: &[&str]) -> Result<String, String> {
    let mut all = vec!["tag"];
    all.extend_from_slice(args);
    let out = server
        .command(root, &all)
        .output()
        .map_err(|e| format!("vink-app tag: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    if !out.status.success() {
        return Err(format!(
            "vink-app tag {}: {} / {}",
            args.join(" "),
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(stdout)
}

/// The ID in the `tag\t<id>\t…` line the operator command prints for the changed tag.
fn changed_tag_id(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("tag\t"))
        .and_then(|rest| rest.split('\t').next())
        .map(str::to_string)
}

/// TAG-PR-05: a small curated catalog for the editor checks, written through the operator command
/// (`CuratorAuthority::operator`, audited), never by SQL.
fn seed_tag_catalog(root: &Path, server: &Server) -> Result<TagSeed, String> {
    let mut version = 1_u32;
    let mut ids = Vec::new();
    for (facet, label) in [
        ("form", "Tản văn"),
        ("form", "Thơ"),
        ("theme", "Ký ức"),
        ("theme", "Gia đình"),
        ("theme", "Mùa"),
        ("theme", "Đô thị"),
    ] {
        let stdout = tag_command(
            root,
            server,
            &["create", &version.to_string(), facet, label],
        )?;
        ids.push(changed_tag_id(&stdout).ok_or(format!("no tag id in: {stdout}"))?);
        version += 1;
    }
    tag_command(
        root,
        server,
        &["alias", &version.to_string(), &ids[2], "Hồi ức"],
    )?;
    version += 1;
    tag_command(
        root,
        server,
        &[
            "describe",
            &version.to_string(),
            &ids[2],
            "Những điều đã qua còn ở lại trong người viết.",
        ],
    )?;
    version += 1;
    println!("==> scratch tag catalog seeded at version {version}");
    Ok(TagSeed {
        city: ids[5].clone(),
        season: ids[4].clone(),
        version,
    })
}

fn seed_restart_tags(root: &Path, server: &Server) -> Result<TagSeed, String> {
    let season = tag_command(root, server, &["create", "1", "theme", "Mùa"])?;
    let city = tag_command(root, server, &["create", "2", "theme", "Đô thị"])?;
    Ok(TagSeed {
        season: changed_tag_id(&season).ok_or("restart successor tag ID missing")?,
        city: changed_tag_id(&city).ok_or("restart tag ID missing")?,
        version: 3,
    })
}

/// Review principal belongs to Review setup, including a standalone scenario run.
fn seed_review_fixture(server: &Server) -> Result<(), String> {
    use sqlx::Connection;
    let url = server
        .env
        .iter()
        .find(|(key, _)| key == "DATABASE_URL")
        .map(|(_, value)| value)
        .ok_or("scratch URL missing")?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    runtime.block_on(async {
        let mut connection=sqlx::PgConnection::connect(url).await.map_err(|e|e.to_string())?;
        sqlx::query("INSERT INTO principals (id, display_name) VALUES ('77777777-7777-4777-8777-777777777777', '[Mẫu] Người duyệt tác phẩm') ON CONFLICT (id) DO NOTHING")
            .execute(&mut connection).await.map_err(|e| e.to_string())?;
        Ok(())
    })
}

/// Labelled publication fixtures, solely in this lane's disposable database.
fn seed_discovery_fixture(server: &Server) -> Result<(), String> {
    use sqlx::Connection;
    let url = server
        .env
        .iter()
        .find(|(key, _)| key == "DATABASE_URL")
        .map(|(_, value)| value)
        .ok_or("scratch URL missing")?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    runtime.block_on(async {
        let mut connection = sqlx::PgConnection::connect(url)
            .await
            .map_err(|e| e.to_string())?;
        sqlx::raw_sql(include_str!(
            "../../apps/vink-web/e2e/fixtures/discovery-publications.sql"
        ))
        .execute(&mut connection)
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

/// Real App/PostgreSQL synthetic journey fixture. #100 contributes works 1..14;
/// this opt-in delta contributes 15..200 and one hot thread of 1,000 comments.
fn seed_perf_journey(server: &Server) -> Result<(), String> {
    use sqlx::Connection;
    let url = server
        .env
        .iter()
        .find(|(key, _)| key == "DATABASE_URL")
        .map(|(_, value)| value)
        .ok_or("scratch URL missing")?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    runtime.block_on(async {
        let mut connection = sqlx::PgConnection::connect(url)
            .await
            .map_err(|e| e.to_string())?;
        sqlx::raw_sql(include_str!(
            "../../apps/vink-web/e2e/fixtures/web-perf-journey.sql"
        ))
        .execute(&mut connection)
        .await
        .map_err(|e| format!("seed #234 journey: {e}"))?;
        let (works, comments): (i64, i64) = sqlx::query_as(
            "SELECT
                (SELECT count(*) FROM revisions WHERE title LIKE '[Mẫu #100] Kiểm thử %'),
                (SELECT count(*) FROM comments WHERE body LIKE '[Mẫu #234]%')",
        )
        .fetch_one(&mut connection)
        .await
        .map_err(|e| format!("read #234 seed counts: {e}"))?;
        if (works, comments) != (200, 1000) {
            return Err(format!(
                "#234 corpus is not exact: {works} labelled public works, {comments} comments"
            ));
        }
        println!("==> #234 scratch journey: {works} works / {comments} comments");
        Ok(())
    })
}

fn editor_first_drafts(
    root: &Path,
    server: &Server,
    chrome: &Browser,
    state: &Path,
) -> Result<(), String> {
    let (_, url) = server
        .env
        .iter()
        .find(|(key, _)| key == "VINK_MIGRATE_DATABASE_URL")
        .ok_or("scratch migrator URL missing")?;
    for (index, format) in ["default", "markdown"].into_iter().enumerate() {
        let principal = format!("88888888-8888-4888-8888-{:012}", index + 256);
        crate::identity::sql(
            url,
            &format!(
                "INSERT INTO principals (id, display_name) VALUES ('{principal}', '[Mẫu] Bản thảo đầu tiên')"
            ),
        )?;
        let mut fixture = server.with_principal(&principal, free_port()?);
        let state = scenario_state(state, format);
        std::fs::create_dir_all(
            state
                .parent()
                .ok_or("first-draft evidence directory missing")?,
        )
        .map_err(|error| error.to_string())?;
        fixture.log = state.with_file_name("server.log");
        let mut running = fixture.start(root)?;
        smoke(
            root,
            &["--phase", "editor-first-draft", "--first-format", format],
            chrome,
            &format!("http://127.0.0.1:{}", fixture.port),
            &state,
        )?;
        let status = running.terminate(Duration::from_secs(15))?;
        if !status.success() {
            return Err(format!("first-draft fixture {format} exited with {status}"));
        }
    }
    Ok(())
}

fn run_web_checks(
    report: &mut Report,
    root: &Path,
    server: &Server,
    chrome: &Browser,
    origin: &str,
    state: &Path,
    selected: &[Scenario],
) -> bool {
    let restart = selected.contains(&Scenario::Restart);
    let mut seed = None;
    if (restart || selected.iter().any(|s| s.tag_catalog()))
        && !report.step(
            "scratch database: tag catalog through `vink-app tag`",
            || {
                seed = Some(if selected.iter().any(|s| s.tag_catalog()) {
                    seed_tag_catalog(root, server)?
                } else {
                    seed_restart_tags(root, server)?
                });
                Ok(())
            },
        )
    {
        return false;
    }
    if selected.contains(&Scenario::EditorFirstDraft)
        && !report.step(
            "first-use draft formats with fresh scratch principals",
            || {
                editor_first_drafts(
                    root,
                    server,
                    chrome,
                    &scenario_state(state, "editor-first-draft"),
                )
            },
        )
    {
        return false;
    }
    if restart || selected.iter().any(|s| s.primary_phase().is_some()) {
        let mut guard = match server.start(root) {
            Ok(guard) => guard,
            Err(e) => {
                report.fail("server starts and reports ready", &e);
                return false;
            }
        };
        for scenario in selected {
            let Some(phase) = scenario.primary_phase() else {
                continue;
            };
            if !report.step(&format!("scenario {} ({phase})", scenario.name()), || {
                if *scenario == Scenario::Discovery {
                    seed_discovery_fixture(server)?;
                }
                smoke(
                    root,
                    &["--phase", phase, "--build-profile", server.profile_name()],
                    chrome,
                    origin,
                    &scenario_state(state, scenario.name()),
                )
            }) {
                return false;
            }
        }
        if selected.contains(&Scenario::Reader)
            && !report.step(
                "work views intake: threshold, pause/resume, retry, E-37 and author readback (#343)",
                || {
                    let guest = server.as_guest_on(free_port()?);
                    let guest_origin = format!("http://127.0.0.1:{}", guest.port);
                    let mut guest_guard = guest.start(root)?;
                    let result = smoke(
                        root,
                        &["--phase", "reader-views", "--author-base", origin],
                        chrome,
                        &guest_origin,
                        &scenario_state(state, "reader-views"),
                    );
                    let stopped = guest_guard.terminate(Duration::from_secs(15))?;
                    result?;
                    stopped
                        .success()
                        .then_some(())
                        .ok_or_else(|| format!("guest App exited with {stopped}"))
                },
            )
        {
            return false;
        }
        let restart_state = scenario_state(state, "restart");
        if restart
            && !report.step(
                "restart dependency: browser saves and reloads draft text and tag",
                || {
                    smoke(
                        root,
                        &["--phase", "restart-prepare"],
                        chrome,
                        origin,
                        &restart_state,
                    )
                },
            )
        {
            return false;
        }
        if !report.step("server stops gracefully on SIGTERM", || {
            let status = guard.terminate(Duration::from_secs(15))?;
            status
                .success()
                .then_some(())
                .ok_or(format!("exited with {status}"))
        }) {
            return false;
        }
        if selected.contains(&Scenario::Desk)
            && !report.step("guest Desk with sign-in disabled", || {
                let mut guest = server.as_guest().start(root)?;
                let result = smoke(
                    root,
                    &["--phase", "desk-guest"],
                    chrome,
                    origin,
                    &scenario_state(&scenario_state(state, "desk"), "guest"),
                );
                let stopped = guest.terminate(Duration::from_secs(15))?;
                result?;
                stopped
                    .success()
                    .then_some(())
                    .ok_or_else(|| format!("guest App exited with {stopped}"))
            })
        {
            return false;
        }
        if restart
            && !report.step(
                "restart: confirmed draft, retired tag, unsaved buffer and graceful stop",
                || {
                    let seed = seed.as_ref().ok_or("restart catalog was not seeded")?;
                    tag_command(
                        root,
                        server,
                        &[
                            "retire",
                            &seed.version.to_string(),
                            &seed.city,
                            &seed.season,
                        ],
                    )?;
                    let mut restarted = server.start(root)?;
                    let result = smoke(
                        root,
                        &["--phase", "after-restart"],
                        chrome,
                        origin,
                        &restart_state,
                    );
                    let status = restarted.terminate(Duration::from_secs(15))?;
                    result?;
                    status
                        .success()
                        .then_some(())
                        .ok_or_else(|| format!("restarted App exited with {status}"))
                },
            )
        {
            return false;
        }
    }
    if selected
        .iter()
        .any(|s| matches!(s, Scenario::PerfBaseline | Scenario::ReturnPosition))
        && !report.step(
            "guest performance lab (200 works, 1000 comments; selected Reader journey checks)",
            || {
                if !selected.contains(&Scenario::Discovery) {
                    seed_discovery_fixture(server)?;
                }
                seed_perf_journey(server)?;
                // LocalFixture performs an additional actor reconciliation on mount. Don't
                // benchmark it as if it were part of a guest's public journey.
                let guest = server.as_guest();
                let mut guard = guest.start(root)?;
                for scenario in [Scenario::PerfBaseline, Scenario::ReturnPosition] {
                    if selected.contains(&scenario) {
                        smoke(
                            root,
                            &[
                                "--phase",
                                scenario.name(),
                                "--build-profile",
                                guest.profile_name(),
                            ],
                            chrome,
                            origin,
                            &scenario_state(state, scenario.name()),
                        )?;
                    }
                }
                let status = guard.terminate(Duration::from_secs(15))?;
                status
                    .success()
                    .then_some(())
                    .ok_or_else(|| format!("guest App exited with {status}"))
            },
        )
    {
        return false;
    }
    if !selected
        .iter()
        .any(|s| matches!(s, Scenario::Endorsement | Scenario::Review))
    {
        return true;
    }
    // LVE #73: two servers on the same scratch database. The author's (this origin) writes the
    // comments; a second loopback fixture principal, with a moderator grant, approves them and
    // endorses in the browser on its own origin.
    let reader = match free_port() {
        Ok(port) => server.with_principal(OTHER_PRINCIPAL, port),
        Err(e) => {
            report.fail("free port", &e);
            return false;
        }
    };
    let mut grant = reader.command(
        root,
        &["moderator", "grant", OTHER_PRINCIPAL, "verify web LVE"],
    );
    if let Err(e) = crate::process::run(&mut grant, "vink-app moderator grant (scratch)") {
        report.fail("scratch database: moderator grant", &e);
        return false;
    }
    // O-17@r2 (#159): the same second principal holds an admin seat for the review phase.
    if selected.contains(&Scenario::Review) {
        if let Err(e) = seed_review_fixture(server) {
            report.fail("scratch review principal", &e);
            return false;
        }
        let mut seat = reader.command(root, &["admin", "add", OTHER_PRINCIPAL, "verify web O-17"]);
        if let Err(e) = crate::process::run(&mut seat, "vink-app admin add (scratch)") {
            report.fail("scratch database: admin seat", &e);
            return false;
        }
    }
    let mut author = match server.start(root) {
        Ok(guard) => guard,
        Err(e) => {
            report.fail("author server starts again", &e);
            return false;
        }
    };
    let mut endorsing = match reader.start(root) {
        Ok(guard) => guard,
        Err(e) => {
            report.fail("server starts as a second fixture principal", &e);
            return false;
        }
    };
    let mut reviewer = match free_port() {
        Ok(port) => server.with_principal("77777777-7777-4777-8777-777777777777", port),
        Err(e) => {
            report.fail("reviewer port", &e);
            return false;
        }
    };
    reviewer.log = reviewer.log.with_file_name("server-reviewer.log");
    let mut reviewing = if selected.contains(&Scenario::Review) {
        Some(match reviewer.start(root) {
            Ok(guard) => guard,
            Err(e) => {
                report.fail("reviewer fixture starts", &e);
                return false;
            }
        })
    } else {
        None
    };
    let reviewer_origin = format!("http://127.0.0.1:{}", reviewer.port);
    let reader_origin = format!("http://127.0.0.1:{}", reader.port);
    let ok = !selected.contains(&Scenario::Endorsement)
        || report.step(
            "literary endorsement: real writes, double tap, lost ACK + replay, 409, keyboard, 401",
            || {
                smoke(
                    root,
                    &["--phase", "endorsement", "--author-base", origin],
                    chrome,
                    &reader_origin,
                    &scenario_state(state, "endorsement"),
                )
            },
        );
    let ok = ok
        && (!selected.contains(&Scenario::Review) || report.step(
            "review-withdrawn, moderation queues and Markdown (#264): exact snapshots, Reader SSR/hydration and current visibility",
            || {
                smoke(
                    root,
                    &["--phase", "review-withdrawn", "--author-base", origin, "--reviewer-base", &reviewer_origin],
                    chrome,
                    &reader_origin,
                    &scenario_state(state, "review"),
                )
            },
        ));
    if let Some(ref mut reviewing) = reviewing {
        let _ = reviewing.terminate(Duration::from_secs(15));
    }
    let _ = endorsing.terminate(Duration::from_secs(15));
    let _ = author.terminate(Duration::from_secs(15));
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_only_the_database_name() {
        assert_eq!(
            with_database("postgres://u:p@h:5432/vink?sslmode=disable", "x").unwrap(),
            "postgres://u:p@h:5432/x?sslmode=disable"
        );
        assert_eq!(
            with_database("postgres://u@h", "x").unwrap(),
            "postgres://u@h/x"
        );
    }

    #[test]
    fn reads_the_changed_tag_id_from_the_operator_output() {
        let stdout = "catalog version 2: create 0192\ntag\t0192-a\ttheme\tactive\tký-ức\tKý ức\t-\t-\nname\t0192-a\tcanonical\tký-ức\tKý ức\n";
        assert_eq!(changed_tag_id(stdout).as_deref(), Some("0192-a"));
        assert_eq!(changed_tag_id("catalog version 3: alias x\n"), None);
    }

    #[test]
    fn strips_terminal_colors() {
        assert_eq!(
            crate::lifecycle::strip_ansi("\u{1b}[32m INFO\u{1b}[0m ready"),
            " INFO ready"
        );
    }
}
