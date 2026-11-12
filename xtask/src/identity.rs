//! `verify postgres` checks for the real `vink-identity` executable: configuration errors, the
//! migration lifecycle with provisioned roles, and the private contract over a real Unix socket
//! whose caller is authenticated by the OS peer user. The scratch Identity owner comes from
//! [`crate::provision`]. Without an authority no check expects an authenticated answer; the
//! D-05/S-03 email-code spike has its own end-to-end step ([`email_code`]) with both binaries.

mod email_code;
mod smtp_sink;

use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use sqlx::{AssertSqlSafe, Connection, Executor, PgConnection};

use crate::process::Report;
use crate::web::{self, Guard};

/// A credential marker sent to the contract; it must never reach a log.
const CREDENTIAL: &str = "vink-verify-credential-marker-7f3a";

/// Scratch credentials and databases prepared by the provisioning checks.
pub struct Scratch<'a> {
    pub root: &'a Path,
    /// Test harness credential on the maintenance database.
    pub admin: String,
    /// Test harness credential on the scratch Identity and fresh App databases.
    pub admin_identity: String,
    pub identity_database: &'a str,
    pub admin_app: String,
    pub identity_runtime: String,
    pub identity_migrator: String,
    /// Identity credentials with only the database name changed to the App's.
    pub identity_runtime_on_app: String,
    pub identity_migrator_on_app: String,
    /// The fresh App's own credentials, migrated and seeded.
    pub app_runtime: String,
    pub app_migrator: String,
    /// Values that must not appear in any output.
    pub secrets: Vec<String>,
}

pub fn verify(report: &mut Report, scratch: &Scratch<'_>) -> bool {
    let dir = scratch.root.join("target/verify-identity");
    let run = Run {
        scratch,
        socket: dir.join("sock/identity.sock"),
        dir,
    };
    report.step(
        "identity: invalid configuration exits 2 without echoing credentials",
        || {
            run.prepare()?;
            run.config_errors()
        },
    ) && report.step(
        "identity migrate: runtime refused, migrator applies once, App database refused",
        || run.migrations(),
    ) && report.step(
        "identity serve on its socket: ready, honest capabilities, contract refusals, deadline",
        || run.contract(),
    ) && report.step(
        "identity: wrong or missing caller stays closed; forged headers change nothing",
        || run.callers(),
    ) && report.step(
        "identity: lost connections recover; wrong database or schema is not ready",
        || run.database_failures(),
    ) && report.step(
        "identity: SIGTERM exits 0, restart, second instance refused, stale socket replaced",
        || run.lifecycle(),
    ) && report.step(
        "App + Identity: no credential is anonymous, nothing grants, Identity down is 503 for presented credentials only, public reading stays up, independent restarts",
        || run.app_integration(),
    ) && report.step(
        "App readiness refuses a privileged role (production) and an untrusted Identity server",
        || run.app_refusals(),
    ) && report.step(
        "D-05 email-code authority, two real binaries: activation lock, sign-in through the outbox worker, network limit behind a trusted proxy, binding, isolation, logout, email change, suspension, disable, outage, SMTP delivery with password rotation and relay recovery",
        || run.email_code_end_to_end(),
    ) && report.step(
        "ACC-03 password enrollment, two real binaries: proof, guards, readback and ownership across both login methods",
        || run.password_enrollment_end_to_end(),
    ) && report.step("identity: logs contain no credential or password", || {
        run.no_secrets_in_logs()
    })
}

struct Run<'a> {
    scratch: &'a Scratch<'a>,
    dir: PathBuf,
    socket: PathBuf,
}

/// Minimal HTTP/1.1 over the Unix socket: status code and body.
pub fn request(
    socket: &Path,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Result<(u16, String), String> {
    let mut stream =
        UnixStream::connect(socket).map_err(|e| format!("connect {}: {e}", socket.display()))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    let mut head = format!(
        "{method} {path} HTTP/1.1\r\nHost: identity\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    stream
        .write_all(head.as_bytes())
        .and_then(|()| stream.write_all(body))
        .map_err(|e| format!("write: {e}"))?;
    read_response(&mut stream)
}

fn read_response(stream: &mut UnixStream) -> Result<(u16, String), String> {
    let mut raw = String::new();
    stream
        .read_to_string(&mut raw)
        .map_err(|e| format!("read: {e}"))?;
    let status = raw
        .get(9..12)
        .and_then(|code| code.parse().ok())
        .ok_or_else(|| format!("not an HTTP response: {raw:?}"))?;
    let body = raw
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string())
        .unwrap_or_default();
    Ok((status, body))
}

/// Minimal HTTP/1.1 GET over loopback TCP: status code and body.
/// Account-scoped LVE read: needs a principal, so it follows the drafts outcomes.
const OWN_ENDORSEMENTS: &str = "/api/v1/works/33333333-3333-4333-8333-333333333333/endorsements/me";

fn http(port: u16, path: &str, headers: &[(&str, &str)]) -> Result<(u16, String), String> {
    let mut stream =
        std::net::TcpStream::connect(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    let mut head =
        format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n");
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    stream
        .write_all(head.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).map_err(|e| e.to_string())?;
    let raw = String::from_utf8_lossy(&raw).into_owned();
    let status = raw
        .get(9..12)
        .and_then(|code| code.parse().ok())
        .ok_or_else(|| format!("not an HTTP response: {raw:?}"))?;
    let body = raw
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_string())
        .unwrap_or_default();
    Ok((status, body))
}

fn expect(
    what: &str,
    answer: Result<(u16, String), String>,
    status: u16,
    needle: &str,
) -> Result<(), String> {
    let (got, body) = answer.map_err(|e| format!("{what}: {e}"))?;
    if got == status && body.contains(needle) {
        Ok(())
    } else {
        Err(format!(
            "{what}: expected {status} with `{needle}`, got {got}: {body}"
        ))
    }
}

fn verify_body(credential: &str) -> String {
    format!(r#"{{"credential":"{credential}"}}"#)
}

pub(crate) fn sql(url: &str, statement: &str) -> Result<i64, String> {
    let url = url.to_string();
    let statement = statement.to_string();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?
        .block_on(async move {
            let mut connection = PgConnection::connect(&url)
                .await
                .map_err(|e| format!("connect: {e}"))?;
            if statement.trim_start().starts_with("SELECT") {
                sqlx::query_scalar(AssertSqlSafe(statement))
                    .fetch_one(&mut connection)
                    .await
                    .map_err(|e| e.to_string())
            } else {
                connection
                    .execute(AssertSqlSafe(statement))
                    .await
                    .map(|done| done.rows_affected() as i64)
                    .map_err(|e| e.to_string())
            }
        })
}

impl Run<'_> {
    fn binary(&self) -> Command {
        let mut command = Command::new(web::binary(self.scratch.root, "vink-identity"));
        command
            .current_dir(self.scratch.root)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("VINK_LOG_FORMAT", "text");
        command
    }

    fn uid(&self) -> Result<u32, String> {
        std::fs::metadata(&self.dir)
            .map(|m| m.uid())
            .map_err(|e| e.to_string())
    }

    /// Fresh output directory and a socket directory closed to other users.
    fn prepare(&self) -> Result<(), String> {
        let _ = std::fs::remove_dir_all(&self.dir);
        let sock = self.socket.parent().expect("socket has a directory");
        std::fs::create_dir_all(sock).map_err(|e| e.to_string())?;
        std::fs::set_permissions(sock, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())
    }

    fn log(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.log"))
    }

    fn exit_2(
        &self,
        label: &str,
        args: &[&str],
        env: &[(&str, &str)],
        needle: &str,
    ) -> Result<(), String> {
        let out = self
            .binary()
            .args(args)
            .envs(env.iter().copied())
            .output()
            .map_err(|e| format!("{label}: {e}"))?;
        let stderr = web::strip_ansi(&String::from_utf8_lossy(&out.stderr));
        std::fs::write(self.log(&format!("config-{label}")), &stderr).map_err(|e| e.to_string())?;
        if out.status.code() != Some(2) || !stderr.contains(needle) {
            return Err(format!(
                "{label}: expected exit 2 naming `{needle}`, got {} / {stderr}",
                out.status
            ));
        }
        Ok(())
    }

    fn config_errors(&self) -> Result<(), String> {
        let socket = self.socket.display().to_string();
        let runtime = self.scratch.identity_runtime.as_str();
        self.exit_2(
            "empty",
            &["serve"],
            &[],
            "VINK_IDENTITY_DATABASE_URL: missing",
        )?;
        self.exit_2(
            "empty-socket",
            &["serve"],
            &[],
            "VINK_IDENTITY_SOCKET: missing",
        )?;
        // App variables are not Identity's: the runtime URL never stands in for migrations.
        self.exit_2(
            "migrate-without-own-credential",
            &["migrate"],
            &[
                ("VINK_IDENTITY_DATABASE_URL", runtime),
                ("DATABASE_URL", runtime),
            ],
            "VINK_IDENTITY_MIGRATE_DATABASE_URL: missing",
        )?;
        self.exit_2(
            "caller",
            &["serve"],
            &[
                ("VINK_IDENTITY_DATABASE_URL", runtime),
                ("VINK_IDENTITY_SOCKET", &socket),
                ("VINK_IDENTITY_CALLER_UID", "vink-app"),
            ],
            "VINK_IDENTITY_CALLER_UID: expected a numeric user id",
        )?;
        self.exit_2(
            "scheme",
            &["serve"],
            &[
                ("VINK_IDENTITY_DATABASE_URL", "mysql://u:leaked-secret@db/x"),
                ("VINK_IDENTITY_SOCKET", &socket),
            ],
            "must be a postgres://",
        )?;
        // A socket directory other users can enter is refused before binding.
        let open = self.dir.join("open");
        std::fs::create_dir_all(&open).map_err(|e| e.to_string())?;
        std::fs::set_permissions(&open, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
        let open_socket = open.join("identity.sock").display().to_string();
        self.exit_2(
            "open-directory",
            &["serve"],
            &[
                ("VINK_IDENTITY_DATABASE_URL", runtime),
                ("VINK_IDENTITY_SOCKET", &open_socket),
            ],
            "without access for other users",
        )?;
        std::fs::set_permissions(&open, std::fs::Permissions::from_mode(0o770))
            .map_err(|e| e.to_string())?;
        self.exit_2(
            "group-writable-directory",
            &["serve"],
            &[
                ("VINK_IDENTITY_DATABASE_URL", runtime),
                ("VINK_IDENTITY_SOCKET", &open_socket),
            ],
            "without group write",
        )?;
        // A regular file at the socket path is never replaced.
        std::fs::write(&self.socket, "not a socket").map_err(|e| e.to_string())?;
        let refused = self.exit_2(
            "not-a-socket",
            &["serve"],
            &[
                ("VINK_IDENTITY_DATABASE_URL", runtime),
                ("VINK_IDENTITY_SOCKET", &socket),
            ],
            "is not a socket",
        );
        let kept = std::fs::read_to_string(&self.socket).map_err(|e| e.to_string())?;
        std::fs::remove_file(&self.socket).map_err(|e| e.to_string())?;
        refused?;
        if kept != "not a socket" {
            return Err("the server replaced a regular file at the socket path".into());
        }
        if std::fs::read_dir(&self.dir)
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with("config-"))
            .any(|e| {
                std::fs::read_to_string(e.path())
                    .unwrap_or_default()
                    .contains("leaked-secret")
            })
        {
            return Err("a configuration error echoed a database URL".into());
        }
        println!("==> identity configuration errors exit 2 and name the setting");
        Ok(())
    }

    fn migrate(&self, label: &str, url: &str) -> Result<(Option<i32>, String), String> {
        let out = self
            .binary()
            .arg("migrate")
            .env("VINK_IDENTITY_MIGRATE_DATABASE_URL", url)
            .output()
            .map_err(|e| format!("migrate {label}: {e}"))?;
        let stderr = web::strip_ansi(&String::from_utf8_lossy(&out.stderr));
        std::fs::write(self.log(&format!("migrate-{label}")), &stderr)
            .map_err(|e| e.to_string())?;
        Ok((out.status.code(), stderr))
    }

    fn migrations(&self) -> Result<(), String> {
        let s = self.scratch;
        let (code, stderr) = self.migrate("runtime", &s.identity_runtime)?;
        if code != Some(1) || !stderr.contains("lacks a privilege") {
            return Err(format!(
                "migrate as the Identity runtime: expected exit 1 naming the privilege, got {code:?}: {stderr}"
            ));
        }
        let history =
            "SELECT count(*) FROM pg_class WHERE oid = to_regclass('public._sqlx_migrations')";
        if sql(&s.admin_identity, history)? != 0 {
            return Err("a refused migration created a history table".into());
        }
        let (code, stderr) = self.migrate("first", &s.identity_migrator)?;
        if code != Some(0)
            || !stderr.contains(
                "applied=[20261102000000, 20261107000000, 20261112000000, 20261113000000, 20261114000000]",
            )
        {
            return Err(format!("first Identity migrate: {code:?}: {stderr}"));
        }
        let (code, stderr) = self.migrate("second", &s.identity_migrator)?;
        if code != Some(0) || !stderr.contains("applied=[]") {
            return Err(format!(
                "second Identity migrate must be a no-op: {code:?}: {stderr}"
            ));
        }
        // Only the database name changed: the Identity migrator may not connect to the App.
        let (code, stderr) = self.migrate("app-database", &s.identity_migrator_on_app)?;
        if code != Some(1) || !stderr.contains("permission denied for database") {
            return Err(format!(
                "Identity migrator on the App database: {code:?}: {stderr}"
            ));
        }
        // The App's own migrator reaches the App database, but Identity's registry is foreign.
        let before = sql(&s.admin_app, "SELECT count(*) FROM _sqlx_migrations")?;
        let (code, stderr) = self.migrate("app-history", &s.app_migrator)?;
        if code != Some(1) || !stderr.contains("diverged") {
            return Err(format!(
                "Identity migrate over the App history: {code:?}: {stderr}"
            ));
        }
        if sql(&s.admin_app, "SELECT count(*) FROM _sqlx_migrations")? != before {
            return Err("a refused Identity migration wrote to the App history".into());
        }
        println!("==> identity migrations: runtime refused, applied once, App database untouched");
        Ok(())
    }

    fn start(&self, name: &str, database_url: &str, caller: Option<u32>) -> Result<Guard, String> {
        self.start_with(name, database_url, caller, &[])
    }

    fn start_with(
        &self,
        name: &str,
        database_url: &str,
        caller: Option<u32>,
        extra: &[(&str, &std::ffi::OsStr)],
    ) -> Result<Guard, String> {
        let log = std::fs::File::create(self.log(name)).map_err(|e| e.to_string())?;
        let mut command = self.binary();
        command
            .arg("serve")
            .env("VINK_IDENTITY_DATABASE_URL", database_url)
            .env("VINK_IDENTITY_SOCKET", &self.socket)
            .stdout(Stdio::null())
            .stderr(log);
        for (key, value) in extra {
            command.env(key, value);
        }
        if let Some(uid) = caller {
            command.env("VINK_IDENTITY_CALLER_UID", uid.to_string());
        }
        let child = command
            .spawn()
            .map_err(|e| format!("spawn vink-identity: {e}"))?;
        let mut guard = Guard(Some(child));
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            if let Some(Ok(Some(status))) = guard.0.as_mut().map(|c| c.try_wait()) {
                return Err(format!("vink-identity ({name}) exited early with {status}"));
            }
            if request(&self.socket, "GET", "/health/live", &[], b"").is_ok_and(|(s, _)| s == 200) {
                return Ok(guard);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err(format!(
            "vink-identity ({name}) did not answer on its socket within 20s"
        ))
    }

    fn stop(&self, mut guard: Guard) -> Result<(), String> {
        let status = guard.terminate(Duration::from_secs(10))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("vink-identity exited with {status} after SIGTERM"))
        }
    }

    fn contract(&self) -> Result<(), String> {
        let uid = self.uid()?;
        let server = self.start("serve", &self.scratch.identity_runtime, Some(uid))?;
        let checked = self.contract_answers();
        self.stop(server)?;
        checked
    }

    fn contract_answers(&self) -> Result<(), String> {
        let s = &self.socket;
        expect(
            "ready",
            request(s, "GET", "/health/ready", &[], b""),
            200,
            r#""authority":"not_configured""#,
        )?;
        expect(
            "ready schema",
            request(s, "GET", "/health/ready", &[], b""),
            200,
            r#""schema":"current""#,
        )?;
        expect(
            "ready role",
            request(s, "GET", "/health/ready", &[], b""),
            200,
            r#""role":"restricted""#,
        )?;
        // The unit-test fixture's accepted credential means nothing to the real binary.
        expect(
            "fixture credential",
            request(
                s,
                "POST",
                "/identity/v1/verify-credential",
                &[],
                verify_body("fixture-credential-accepted").as_bytes(),
            ),
            501,
            "authority_not_configured",
        )?;
        expect(
            "capabilities",
            request(s, "GET", "/identity/v1/capabilities", &[], b""),
            200,
            r#""verify_credential":"not_configured""#,
        )?;
        let body = verify_body(CREDENTIAL);
        expect(
            "verify without an authority",
            request(
                s,
                "POST",
                "/identity/v1/verify-credential",
                &[],
                body.as_bytes(),
            ),
            501,
            "authority_not_configured",
        )?;
        for (label, body) in [
            ("malformed", "{\"credential\":".to_string()),
            ("empty", r#"{"credential":""}"#.to_string()),
            (
                "forged subject",
                format!(
                    r#"{{"credential":"{CREDENTIAL}","subject":{{"authority":"x","id":"admin"}}}}"#
                ),
            ),
        ] {
            expect(
                label,
                request(
                    s,
                    "POST",
                    "/identity/v1/verify-credential",
                    &[],
                    body.as_bytes(),
                ),
                400,
                "malformed_request",
            )?;
        }
        let oversized = verify_body(&"c".repeat(9000));
        expect(
            "oversized",
            request(
                s,
                "POST",
                "/identity/v1/verify-credential",
                &[],
                oversized.as_bytes(),
            ),
            413,
            "payload_too_large",
        )?;
        expect(
            "version 2",
            request(
                s,
                "POST",
                "/identity/v2/verify-credential",
                &[],
                body.as_bytes(),
            ),
            404,
            "unsupported_version",
        )?;
        // Headers announce 64 bytes, two arrive, then the caller stalls.
        let mut stream = UnixStream::connect(s).map_err(|e| e.to_string())?;
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .map_err(|e| e.to_string())?;
        stream
            .write_all(b"POST /identity/v1/verify-credential HTTP/1.1\r\nHost: identity\r\nConnection: close\r\nContent-Length: 64\r\n\r\n{\"")
            .map_err(|e| e.to_string())?;
        let started = Instant::now();
        expect(
            "stalled upload",
            read_response(&mut stream),
            503,
            "deadline_exceeded",
        )?;
        if started.elapsed() > Duration::from_secs(4) {
            return Err(format!("deadline answered after {:?}", started.elapsed()));
        }
        println!("==> identity contract over the socket: 200/501/400/413/404/503 as specified");
        Ok(())
    }

    fn callers(&self) -> Result<(), String> {
        let uid = self.uid()?;
        let forged = [
            ("x-user-id", "11111111-1111-4111-8111-111111111111"),
            ("x-vink-principal", "11111111-1111-4111-8111-111111111111"),
            ("x-forwarded-for", "127.0.0.1"),
            ("authorization", "Bearer vink-app"),
        ];
        let body = verify_body(CREDENTIAL);
        for (name, caller, ready) in [
            ("other-caller", Some(uid.wrapping_add(1)), 200),
            ("no-caller", None, 503),
        ] {
            let server = self.start(name, &self.scratch.identity_runtime, caller)?;
            let checked = (|| {
                for path in [
                    "/identity/v1/capabilities",
                    "/identity/v1/verify-credential",
                ] {
                    let method = if path.ends_with("capabilities") {
                        "GET"
                    } else {
                        "POST"
                    };
                    expect(
                        &format!("{name} {path}"),
                        request(&self.socket, method, path, &forged, body.as_bytes()),
                        401,
                        "caller_unauthenticated",
                    )?;
                }
                expect(
                    &format!("{name} ready"),
                    request(&self.socket, "GET", "/health/ready", &[], b""),
                    ready,
                    if caller.is_some() {
                        r#""callers":"configured""#
                    } else {
                        r#""callers":"not_configured""#
                    },
                )
            })();
            self.stop(server)?;
            checked?;
        }
        // The allowed caller's forged headers do not change the answer.
        let server = self.start("forged-headers", &self.scratch.identity_runtime, Some(uid))?;
        let checked = expect(
            "allowed caller with forged headers",
            request(
                &self.socket,
                "POST",
                "/identity/v1/verify-credential",
                &forged,
                body.as_bytes(),
            ),
            501,
            "authority_not_configured",
        );
        self.stop(server)?;
        checked?;
        println!("==> identity callers: another uid and no configuration are refused (401)");
        Ok(())
    }

    fn database_failures(&self) -> Result<(), String> {
        let uid = self.uid()?;
        let s = self.scratch;
        let server = self.start("connections", &s.identity_runtime, Some(uid))?;
        let checked = (|| {
            expect(
                "ready before",
                request(&self.socket, "GET", "/health/ready", &[], b""),
                200,
                "ready",
            )?;
            let dropped = sql(
                &s.admin,
                &format!(
                    "SELECT count(pg_terminate_backend(pid)) FROM pg_stat_activity
                      WHERE application_name = 'vink-identity' AND datname = '{}'",
                    s.identity_database
                ),
            )?;
            if dropped == 0 {
                return Err("no Identity connection to terminate".into());
            }
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                match request(&self.socket, "GET", "/health/ready", &[], b"") {
                    Ok((200, _)) => break,
                    _ if Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(200))
                    }
                    other => {
                        return Err(format!(
                            "not ready after connections were terminated: {other:?}"
                        ));
                    }
                }
            }
            println!("==> identity recovered after {dropped} terminated connection(s)");
            Ok(())
        })();
        self.stop(server)?;
        checked?;
        for (name, url, needle) in [
            // The migration credential can change the schema: never ready to serve with it.
            (
                "migrator-credential",
                s.identity_migrator.as_str(),
                r#""role":"privileged""#,
            ),
            (
                "unreachable",
                "postgres://nobody:unused@127.0.0.1:1/vink_identity",
                r#""database":"unavailable""#,
            ),
            (
                "app-database",
                s.identity_runtime_on_app.as_str(),
                r#""database":"unavailable""#,
            ),
            (
                "app-schema",
                s.app_runtime.as_str(),
                r#""schema":"diverged""#,
            ),
        ] {
            let server = self.start(name, url, Some(uid))?;
            let checked = expect(
                &format!("{name} ready"),
                request(&self.socket, "GET", "/health/ready", &[], b""),
                503,
                needle,
            )
            .and_then(|()| {
                expect(
                    &format!("{name} live"),
                    request(&self.socket, "GET", "/health/live", &[], b""),
                    200,
                    "live",
                )
            });
            self.stop(server)?;
            checked?;
        }
        println!(
            "==> identity reports a privileged role, unreachable or wrong database and foreign schema as not ready"
        );
        Ok(())
    }

    fn lifecycle(&self) -> Result<(), String> {
        let uid = self.uid()?;
        let runtime = &self.scratch.identity_runtime;
        let first = self.start("lifecycle", runtime, Some(uid))?;
        let second = self
            .binary()
            .arg("serve")
            .env("VINK_IDENTITY_DATABASE_URL", runtime)
            .env("VINK_IDENTITY_SOCKET", &self.socket)
            .output()
            .map_err(|e| e.to_string())?;
        let second_err = web::strip_ansi(&String::from_utf8_lossy(&second.stderr));
        std::fs::write(self.log("second-instance"), &second_err).map_err(|e| e.to_string())?;
        let still = request(&self.socket, "GET", "/health/ready", &[], b"");
        self.stop(first)?;
        if second.status.code() != Some(1) || !second_err.contains("already serving") {
            return Err(format!("second instance: {} / {second_err}", second.status));
        }
        expect(
            "first instance after the refused second",
            still,
            200,
            "ready",
        )?;
        let log = std::fs::read_to_string(self.log("lifecycle")).unwrap_or_default();
        if !log.contains("vink-identity stopped") || self.socket.exists() {
            return Err("SIGTERM did not stop cleanly and remove the socket".into());
        }
        // A peer that never finishes its request head is closed and cannot hold shutdown open.
        let stalled = self.start("stalled-head", runtime, Some(uid))?;
        let mut head = UnixStream::connect(&self.socket).map_err(|e| e.to_string())?;
        head.write_all(b"POST /identity/v1/verify-credential HTTP/1.1\r\n")
            .map_err(|e| e.to_string())?;
        head.set_read_timeout(Some(Duration::from_secs(6)))
            .map_err(|e| e.to_string())?;
        let started = Instant::now();
        let closed =
            head.read(&mut [0u8; 64]).is_ok() && started.elapsed() < Duration::from_secs(5);
        let mut second_head = UnixStream::connect(&self.socket).map_err(|e| e.to_string())?;
        second_head
            .write_all(b"POST /identity/v1/verify-credential HTTP/1.1\r\n")
            .map_err(|e| e.to_string())?;
        let stopping = Instant::now();
        self.stop(stalled)?;
        if !closed {
            return Err("a stalled request head was not closed within 5 s".into());
        }
        if stopping.elapsed() > Duration::from_secs(9) {
            return Err(format!(
                "SIGTERM with a stalled request head took {:?}",
                stopping.elapsed()
            ));
        }
        // A crash leaves the socket behind; the next start replaces it.
        let mut crashed = self.start("crash", runtime, Some(uid))?;
        if let Some(mut child) = crashed.0.take() {
            child.kill().map_err(|e| e.to_string())?;
            child.wait().map_err(|e| e.to_string())?;
        }
        if !self.socket.exists() {
            return Err("expected a stale socket after SIGKILL".into());
        }
        let restarted = self.start("restart", runtime, Some(uid))?;
        let ready = request(&self.socket, "GET", "/health/ready", &[], b"");
        self.stop(restarted)?;
        expect("restarted after a crash", ready, 200, "ready")?;
        let log = std::fs::read_to_string(self.log("restart")).unwrap_or_default();
        if !log.contains("replacing a stale socket") {
            return Err("restart did not report replacing the stale socket".into());
        }
        println!(
            "==> identity lifecycle: SIGTERM 0 (also with a stalled head), second instance refused, stale socket replaced"
        );
        Ok(())
    }

    fn start_app(
        &self,
        name: &str,
        database_url: &str,
        server_uid: u32,
    ) -> Result<(Guard, u16), String> {
        self.start_app_with(name, database_url, server_uid, &[])
    }

    fn start_app_with(
        &self,
        name: &str,
        database_url: &str,
        server_uid: u32,
        extra: &[(&str, &str)],
    ) -> Result<(Guard, u16), String> {
        let port = web::free_port()?;
        let log = std::fs::File::create(self.log(name)).map_err(|e| e.to_string())?;
        let child = Command::new(web::binary(self.scratch.root, "vink-app"))
            .arg("serve")
            .current_dir(self.scratch.root)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("DATABASE_URL", database_url)
            .env("VINK_PROFILE", "production")
            .env("VINK_BIND", format!("127.0.0.1:{port}"))
            .env("VINK_PUBLIC_ORIGIN", format!("http://127.0.0.1:{port}"))
            .env("VINK_SITE_ROOT", self.scratch.root.join("target/site"))
            .env("VINK_IDENTITY_SOCKET", &self.socket)
            .env("VINK_IDENTITY_SERVER_UID", server_uid.to_string())
            .env("VINK_LOG_FORMAT", "text")
            .envs(extra.iter().copied())
            .stdout(Stdio::null())
            .stderr(log)
            .spawn()
            .map_err(|e| format!("spawn vink-app: {e}"))?;
        let mut guard = Guard(Some(child));
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            if let Some(Ok(Some(status))) = guard.0.as_mut().map(|c| c.try_wait()) {
                return Err(format!("vink-app ({name}) exited early with {status}"));
            }
            if http(port, "/health/live", &[]).is_ok_and(|(s, _)| s == 200) {
                return Ok((guard, port));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err(format!("vink-app ({name}) did not start within 30s"))
    }

    fn app_integration(&self) -> Result<(), String> {
        let uid = self.uid()?;
        let s = self.scratch;
        let mut identity = Some(self.start("with-app", &s.identity_runtime, Some(uid))?);
        let (app, port) = self.start_app("app", &s.app_runtime, uid)?;
        let bearer = format!("Bearer {CREDENTIAL}");
        let forged = [
            ("authorization", bearer.as_str()),
            ("x-user-id", "11111111-1111-4111-8111-111111111111"),
            ("x-vink-principal", "11111111-1111-4111-8111-111111111111"),
        ];
        let checked = (|| {
            expect(
                "app ready",
                http(port, "/health/ready", &[]),
                200,
                r#""role":"restricted""#,
            )?;
            expect(
                "app ready identity",
                http(port, "/health/ready", &[]),
                200,
                r#""identity":"no_authority""#,
            )?;
            expect(
                "drafts without credential",
                http(port, "/api/v1/drafts", &[]),
                401,
                "unauthenticated",
            )?;
            expect(
                "drafts with credential",
                http(port, "/api/v1/drafts", &forged),
                401,
                "unauthenticated",
            )?;
            expect(
                "own endorsements with credential",
                http(port, OWN_ENDORSEMENTS, &forged),
                401,
                "unauthenticated",
            )?;
            self.public_reading(port)?;

            // Identity stops: only presented credentials depend on it.
            if let Some(server) = identity.take() {
                self.stop(server)?;
            }
            expect(
                "ready while Identity is down",
                http(port, "/health/ready", &[]),
                200,
                r#""identity":"unavailable""#,
            )?;
            expect(
                "no credential while Identity is down",
                http(port, "/api/v1/drafts", &[]),
                401,
                "unauthenticated",
            )?;
            expect(
                "credential while Identity is down",
                http(port, "/api/v1/drafts", &forged),
                503,
                "dependency_unavailable",
            )?;
            expect(
                "own endorsements while Identity is down",
                http(port, OWN_ENDORSEMENTS, &forged),
                503,
                "dependency_unavailable",
            )?;
            self.public_reading(port)?;

            // Identity restarts on its own; the App follows without a restart.
            identity = Some(self.start("with-app-restarted", &s.identity_runtime, Some(uid))?);
            expect(
                "credential after Identity restart",
                http(port, "/api/v1/drafts", &forged),
                401,
                "unauthenticated",
            )?;
            Ok::<_, String>(())
        })();
        let mut app = app;
        let stopped = app.terminate(Duration::from_secs(15));
        let result = checked.and_then(|()| {
            match stopped {
                Ok(status) if status.success() => {}
                other => return Err(format!("vink-app did not stop cleanly: {other:?}")),
            }
            // The App restarts on its own while Identity keeps serving.
            let (restarted, port) = self.start_app("app-restarted", &s.app_runtime, uid)?;
            let mut restarted = restarted;
            let ready = expect(
                "App ready after restart",
                http(port, "/health/ready", &[]),
                200,
                r#""identity":"no_authority""#,
            );
            let _ = restarted.terminate(Duration::from_secs(15));
            ready
        });
        if let Some(server) = identity {
            self.stop(server)?;
        }
        result?;
        println!(
            "==> App + Identity: 401 without/with credential, 503 only when Identity is down, public reading 200 throughout"
        );
        Ok(())
    }

    /// Public list, reader page, endorsement summaries and SSR never need Identity; their HTML
    /// carries no credential.
    fn public_reading(&self, port: u16) -> Result<(), String> {
        expect(
            "public works",
            http(port, "/api/v1/works", &[]),
            200,
            "works",
        )?;
        // The runtime role reads the endorsement tables of the provisioned, migrated database.
        expect(
            "public endorsement summary",
            http(
                port,
                "/api/v1/works/33333333-3333-4333-8333-333333333333/endorsements",
                &[],
            ),
            200,
            r#""status":"available""#,
        )?;
        let (status, home) = http(port, "/", &[])?;
        let (reader_status, reader) =
            http(port, "/tac-pham/33333333-3333-4333-8333-333333333333", &[])?;
        if status != 200 || reader_status != 200 {
            return Err(format!("public pages: {status}/{reader_status}"));
        }
        let mut needles = self.scratch.secrets.clone();
        needles.extend([
            CREDENTIAL.to_string(),
            "postgres://".into(),
            "_DATABASE_URL".into(),
        ]);
        if needles
            .iter()
            .any(|n| home.contains(n.as_str()) || reader.contains(n.as_str()))
        {
            return Err("public HTML contains a credential or database setting".into());
        }
        Ok(())
    }

    fn app_refusals(&self) -> Result<(), String> {
        let uid = self.uid()?;
        let s = self.scratch;
        let identity = self.start("app-refusals", &s.identity_runtime, Some(uid))?;
        let checked = (|| {
            // The migration credential can change the schema: a production App is not ready.
            let (mut app, port) = self.start_app("app-migrator", &s.app_migrator, uid)?;
            let privileged = expect(
                "App with the migrator credential",
                http(port, "/health/ready", &[]),
                503,
                r#""role":"privileged""#,
            );
            let _ = app.terminate(Duration::from_secs(15));
            privileged?;
            // A socket served by an unexpected user is not trusted.
            let (mut app, port) =
                self.start_app("app-wrong-server", &s.app_runtime, uid.wrapping_add(1))?;
            let bearer = format!("Bearer {CREDENTIAL}");
            let refused = expect(
                "credential with an untrusted Identity server",
                http(
                    port,
                    "/api/v1/drafts",
                    &[("authorization", bearer.as_str())],
                ),
                503,
                "dependency_unavailable",
            )
            .and_then(|()| {
                expect(
                    "ready reports it",
                    http(port, "/health/ready", &[]),
                    200,
                    r#""identity":"unavailable""#,
                )
            });
            let _ = app.terminate(Duration::from_secs(15));
            refused
        })();
        self.stop(identity)?;
        checked?;
        println!(
            "==> App refuses the migrator credential (503 privileged) and an Identity socket served by another user"
        );
        Ok(())
    }

    fn no_secrets_in_logs(&self) -> Result<(), String> {
        let mut needles = self.scratch.secrets.clone();
        needles.push(CREDENTIAL.into());
        for entry in std::fs::read_dir(&self.dir).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().is_some_and(|ext| ext == "log") {
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                if needles.iter().any(|needle| text.contains(needle.as_str())) {
                    return Err(format!("{} contains a credential", path.display()));
                }
            }
        }
        println!("==> identity logs: no password or presented credential");
        Ok(())
    }
}
