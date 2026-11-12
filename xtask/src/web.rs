//! Process-level checks: the web lane (real server + headless Chrome) and binary lifecycle.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::doctor;
use crate::env::Env;
use crate::process::Report;
use crate::root;

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
/// Auth Web phase: Identity's own scratch database and a runtime role that owns nothing in it.
const IDENTITY_DATABASE: &str = "vink_verify_web_identity";
const IDENTITY_ROLE: &str = "vink_verify_web_identity_runtime";

/// Child process that is terminated when dropped, so a failed check never leaks a server.
pub struct Guard(pub Option<Child>);

impl Guard {
    fn pid(&self) -> u32 {
        self.0.as_ref().map(Child::id).unwrap_or_default()
    }

    /// SIGTERM, then wait up to `timeout` for a clean exit.
    pub fn terminate(&mut self, timeout: Duration) -> Result<ExitStatus, String> {
        let Some(mut child) = self.0.take() else {
            return Err("process already stopped".into());
        };
        let pid = child.id().to_string();
        let _ = Command::new("kill").args(["-TERM", &pid]).status();
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                return Ok(status);
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("did not exit within {timeout:?} after SIGTERM"));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub fn binary(root: &Path, name: &str) -> PathBuf {
    root.join("target/debug").join(name)
}

/// Without their credential the binaries exit 2 naming it: `serve` and the worker need
/// DATABASE_URL; `migrate` needs VINK_MIGRATE_DATABASE_URL even when DATABASE_URL is set.
pub fn config_rejection(root: &Path) -> Result<(), String> {
    let cases = [
        ("vink-app", None, "DATABASE_URL: missing"),
        ("vink-worker", None, "DATABASE_URL: missing"),
        (
            "vink-app",
            Some("migrate"),
            "VINK_MIGRATE_DATABASE_URL: missing",
        ),
    ];
    for (name, command, needle) in cases {
        let mut process = Command::new(binary(root, name));
        process
            .args(command)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default());
        if command.is_some() {
            // The runtime credential must not stand in for the migration credential.
            process.env("DATABASE_URL", "postgres://runtime:unused@127.0.0.1:1/vink");
        }
        let out = process.output().map_err(|e| format!("{name}: {e}"))?;
        let stderr = String::from_utf8_lossy(&out.stderr);
        if out.status.code() != Some(2) || !stderr.contains(needle) || stderr.contains("unused@") {
            return Err(format!(
                "{name} {}: expected exit 2 naming `{needle}`, got {} / {stderr}",
                command.unwrap_or_default(),
                out.status
            ));
        }
    }
    println!("==> invalid configuration: serve, worker and migrate exit 2");
    Ok(())
}

/// Starts the worker, waits for its start log line, checks it reports `schema` and, when the
/// schema is current, a reachable discussion outbox (`outbox="ready"`, DISC-04) under this role;
/// sends SIGTERM and expects exit code 0.
pub fn worker_lifecycle(root: &Path, url: &str, schema: &str) -> Result<(), String> {
    let mut command = Command::new(binary(root, "vink-worker"));
    // Explicit local configuration, independent of any `.env` (CI has none).
    command
        .current_dir(root)
        .env("DATABASE_URL", url)
        .env("VINK_PROFILE", "local")
        .env("VINK_LOG_FORMAT", "text")
        // Lifecycle assertions consume INFO logs regardless of the invoking shell's filter.
        .env("RUST_LOG", "info,sqlx=warn")
        .env_remove("VINK_DEV_PRINCIPAL")
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|e| format!("spawn vink-worker: {e}"))?;
    let stderr = child.stderr.take().expect("piped stderr");
    let mut guard = Guard(Some(child));
    let lines = stream_lines(stderr);
    let ready = wait_for_line(&lines, "vink-worker started", Duration::from_secs(20))?;
    println!("==> {}", ready.trim());
    if !ready.contains(schema) {
        return Err(format!("worker did not report `{schema}`: {ready}"));
    }
    if schema.contains("current") && !ready.contains("outbox=\"ready\"") {
        return Err(format!("worker did not report a reachable outbox: {ready}"));
    }
    let status = guard.terminate(Duration::from_secs(10))?;
    if !status.success() {
        return Err(format!("worker exited with {status}"));
    }
    wait_for_line(&lines, "vink-worker stopped", Duration::from_secs(2))?;
    Ok(())
}

fn stream_lines(stream: impl Read + Send + 'static) -> mpsc::Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    receiver
}

fn wait_for_line(
    lines: &mpsc::Receiver<String>,
    needle: &str,
    timeout: Duration,
) -> Result<String, String> {
    let deadline = Instant::now() + timeout;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match lines.recv_timeout(left) {
            Ok(line) if strip_ansi(&line).contains(needle) => return Ok(strip_ansi(&line)),
            Ok(_) => {}
            Err(_) => return Err(format!("no `{needle}` log line within {timeout:?}")),
        }
    }
}

pub fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

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

pub fn free_port() -> Result<u16, String> {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .map_err(|e| format!("no free port: {e}"))
}

/// Minimal HTTP/1.1 GET returning the status code; enough for readiness polling.
fn http_status(port: u16, path: &str) -> Option<u16> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    let mut head = [0u8; 16];
    stream.read_exact(&mut head).ok()?;
    std::str::from_utf8(&head[9..12]).ok()?.parse().ok()
}

struct Server {
    port: u16,
    env: Vec<(String, String)>,
    log: PathBuf,
}

impl Server {
    /// The same server and database for another loopback fixture principal.
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
        }
    }

    fn command(&self, root: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(binary(root, "vink-app"));
        command
            .current_dir(root)
            .args(args)
            .envs(self.env.iter().map(|(k, v)| (k, v)));
        command
    }

    fn start(&self, root: &Path) -> Result<Guard, String> {
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log)
            .map_err(|e| format!("{}: {e}", self.log.display()))?;
        let child = self
            .command(root, &["serve"])
            .stdout(Stdio::null())
            .stderr(log)
            .spawn()
            .map_err(|e| format!("spawn vink-app: {e}"))?;
        let mut guard = Guard(Some(child));
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
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

fn smoke(
    root: &Path,
    node_args: &[&str],
    chrome: &str,
    base: &str,
    state: &Path,
) -> Result<(), String> {
    let mut command = Command::new("node");
    command
        .current_dir(root)
        .arg(root.join("apps/vink-web/e2e/browser-smoke.mjs"))
        .args(["--base", base, "--chrome", chrome, "--state"])
        .arg(state)
        .args(node_args);
    crate::process::run(
        &mut command,
        &format!("browser smoke {}", node_args.join(" ")),
    )
}

/// Starts the real `vink-identity` (email-code authority) on its own scratch database and runs
/// the Auth Web browser phase against an App that has no fixture principal. The local outbox is
/// the only mail transport, so the browser check reads codes from that directory; no production
/// transport is exercised. The runtime role is created here with a throwaway password and owns
/// nothing (`verify postgres` is the evidence for `services/postgres` provisioning itself).
fn auth_web(
    root: &Path,
    server: &Server,
    admin_url: &str,
    chrome: &str,
    state: &Path,
) -> Result<(), String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let dir = state.parent().ok_or("state file has no directory")?;
    let (sock_dir, outbox) = (dir.join("sock"), dir.join("outbox"));
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
    let mut migrate = Command::new(binary(root, "vink-identity"));
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
    let log = std::fs::File::create(dir.join("identity.log")).map_err(|e| e.to_string())?;
    let child = Command::new(binary(root, "vink-identity"))
        .current_dir(root)
        .arg("serve")
        .env(
            "VINK_IDENTITY_DATABASE_URL",
            admin.with(IDENTITY_ROLE, &password, IDENTITY_DATABASE),
        )
        .env("VINK_IDENTITY_SOCKET", &socket)
        .env("VINK_IDENTITY_CALLER_UID", uid.to_string())
        .env("VINK_IDENTITY_AUTHORITY", "email-code")
        .env("VINK_IDENTITY_LOCAL_OUTBOX", &outbox)
        .env("VINK_LOG_FORMAT", "text")
        .stdout(Stdio::null())
        .stderr(log)
        .spawn()
        .map_err(|e| format!("spawn vink-identity: {e}"))?;
    let mut identity = Guard(Some(child));
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
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
    let app = server.with_identity(free_port()?, &socket, uid);
    let mut guard = app.start(root)?;
    let origin = format!("http://127.0.0.1:{}", app.port);
    let outbox = outbox.display().to_string();
    let result = smoke(
        root,
        &["--phase", "auth", "--outbox", &outbox],
        chrome,
        &origin,
        state,
    );
    let _ = guard.terminate(Duration::from_secs(15));
    let _ = identity.terminate(Duration::from_secs(10));
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

pub fn verify(report: &mut Report, env: &Env) -> bool {
    let root = root();
    let Some(url) = env.get(crate::TEST_DATABASE_URL) else {
        report.fail("prerequisites", crate::TEST_DATABASE_URL_MISSING);
        return false;
    };
    let chrome = doctor::chrome_binary(env);
    if !report.step(
        "prerequisites (PostgreSQL, wasm-bindgen, Node 22+, Chrome)",
        || {
            doctor::probe_database(&url)?;
            doctor::require_wasm_bindgen(&root)?;
            doctor::node_version()?;
            chrome
                .as_ref()
                .map(|_| ())
                .ok_or_else(|| "Chrome/Chromium not found; set VINK_CHROME".to_string())
        },
    ) {
        return false;
    }
    let chrome = chrome.unwrap_or_default();
    if !report.step("build server + browser bundle", || crate::build(env, false)) {
        return false;
    }
    if !report.step(
        "browser bundle carries no database URL, credential setting or local password",
        || bundle_secrets(&root.join("target/site/pkg")),
    ) {
        return false;
    }

    let out_dir = root.join("target/verify-web");
    let _ = std::fs::remove_dir_all(&out_dir);
    if let Err(e) = std::fs::create_dir_all(&out_dir) {
        report.fail("prepare target/verify-web", &e.to_string());
        return false;
    }
    let mut scratch_url = String::new();
    if !report.step("scratch database: create, migrate, seed", || {
        scratch_url = recreate_scratch_database(&url)?;
        Ok(())
    }) {
        return false;
    }
    let port = match free_port() {
        Ok(port) => port,
        Err(e) => {
            report.fail("free port", &e);
            return false;
        }
    };
    let origin = format!("http://127.0.0.1:{port}");
    let server = Server {
        port,
        env: vec![
            // The test harness credential owns the scratch database; this lane checks web
            // behavior, not role separation (`verify postgres` covers that).
            ("DATABASE_URL".into(), scratch_url.clone()),
            ("VINK_MIGRATE_DATABASE_URL".into(), scratch_url.clone()),
            ("VINK_PROFILE".into(), "local".into()),
            ("VINK_BIND".into(), format!("127.0.0.1:{port}")),
            ("VINK_PUBLIC_ORIGIN".into(), origin.clone()),
            (
                "VINK_SITE_ROOT".into(),
                root.join("target/site").display().to_string(),
            ),
            ("VINK_DEV_PRINCIPAL".into(), SAMPLE_AUTHOR.into()),
            ("VINK_LOG_FORMAT".into(), "text".into()),
        ],
        log: out_dir.join("server.log"),
    };
    let ok = run_web_checks(
        report,
        &root,
        &server,
        &chrome,
        &origin,
        &out_dir.join("smoke-state.json"),
    ) && report.step(
        "Auth Web: email code, pen name, draft, sign-out, account switch (real Identity, local outbox)",
        || {
            auth_web(
                &root,
                &server,
                &url,
                &chrome,
                &out_dir.join("smoke-state.json"),
            )
        },
    );
    drop_identity_scratch(&url);
    drop_scratch_database(&url);
    ok
}

/// Tag IDs the browser phases need from the seeded catalog.
struct TagSeed {
    /// Đô thị: saved on a draft in the main phase, retired before the after-restart phase.
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
        sqlx::query("INSERT INTO principals (id, display_name) VALUES ('77777777-7777-4777-8777-777777777777', '[Mẫu] Người duyệt tác phẩm') ON CONFLICT (id) DO NOTHING")
            .execute(&mut connection).await.map_err(|e| e.to_string())?;
        Ok(())
    })
}

fn run_web_checks(
    report: &mut Report,
    root: &Path,
    server: &Server,
    chrome: &str,
    origin: &str,
    state: &Path,
) -> bool {
    for step in ["migrate", "dev-seed"] {
        let mut command = server.command(root, &[step]);
        if let Err(e) = crate::process::run(&mut command, &format!("vink-app {step} (scratch)")) {
            report.fail("scratch database: migrate + seed", &e);
            return false;
        }
    }
    let seed = match seed_tag_catalog(root, server) {
        Ok(seed) => seed,
        Err(e) => {
            report.fail("scratch database: tag catalog through `vink-app tag`", &e);
            return false;
        }
    };
    let mut guard = match server.start(root) {
        Ok(guard) => guard,
        Err(e) => {
            report.fail("server starts and reports ready", &e);
            return false;
        }
    };
    if !report.step(
        "SSR HTML, hydration, CodeMirror editor, save/reload/conflict, leave guard, preview, draft tags",
        || smoke(root, &["--phase", "main"], chrome, origin, state),
    ) {
        return false;
    }
    if !report.step(
        "discovery: SQL fixture corpus, real pagination/SSR/reader return and transport race",
        || {
            seed_discovery_fixture(server)?;
            smoke(
                root,
                &["--phase", "discovery-corpus"],
                chrome,
                origin,
                state,
            )
        },
    ) {
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
    // The curator retires a tag the main phase saved on a draft; it must stay there.
    if let Err(e) = tag_command(
        root,
        server,
        &[
            "retire",
            &seed.version.to_string(),
            &seed.city,
            &seed.season,
        ],
    ) {
        report.fail("scratch database: retire a tag through `vink-app tag`", &e);
        return false;
    }
    let mut restarted = match server.start(root) {
        Ok(guard) => guard,
        Err(e) => {
            report.fail("server restarts", &e);
            return false;
        }
    };
    let ok = report.step(
        "confirmed draft survives server restart (browser reload), retired tag kept, export",
        || smoke(root, &["--phase", "after-restart"], chrome, origin, state),
    );
    let _ = restarted.terminate(Duration::from_secs(15));
    if !ok {
        return false;
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
    let mut seat = reader.command(root, &["admin", "add", OTHER_PRINCIPAL, "verify web O-17"]);
    if let Err(e) = crate::process::run(&mut seat, "vink-app admin add (scratch)") {
        report.fail("scratch database: admin seat", &e);
        return false;
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
    let mut reviewing = match reviewer.start(root) {
        Ok(guard) => guard,
        Err(e) => {
            report.fail("reviewer fixture starts", &e);
            return false;
        }
    };
    let reviewer_origin = format!("http://127.0.0.1:{}", reviewer.port);
    let reader_origin = format!("http://127.0.0.1:{}", reader.port);
    let ok = report.step(
        "literary endorsement: real writes, double tap, lost ACK + replay, 409, keyboard, 401",
        || {
            smoke(
                root,
                &["--phase", "endorsement", "--author-base", origin],
                chrome,
                &reader_origin,
                state,
            )
        },
    );
    let ok = ok
        && report.step(
            "review-withdrawn page (O-17@r2): paused queue, admin approve, intervention reject, SSR noindex/no-store, cache purge, comments",
            || {
                smoke(
                    root,
                    &["--phase", "review-withdrawn", "--author-base", origin, "--reviewer-base", &reviewer_origin],
                    chrome,
                    &reader_origin,
                    state,
                )
            },
        );
    let _ = reviewing.terminate(Duration::from_secs(15));
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
        assert_eq!(strip_ansi("\u{1b}[32m INFO\u{1b}[0m ready"), " INFO ready");
    }
}
