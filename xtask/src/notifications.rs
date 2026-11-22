//! `verify postgres` checks for the real `vink-notifications` executable (#347): configuration
//! errors, the migration lifecycle with provisioned roles, database isolation, the two private
//! sockets whose callers are authenticated by the OS peer user (T-07 CHOICE-21 A), and the App
//! relay of the real `vink-worker` delivering through the ingest socket from the App database to
//! the Notification database — including a service stop and an operator replay — and the App
//! Gateway of real `vink-app` processes reading the inbox socket (#348 AC-06, [`gateway`]). The
//! scratch Notification owner comes from [`crate::provision`].

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Connection, Executor, PgConnection};

use crate::identity::{request, sql};
use crate::lifecycle::{self, Guard};
use crate::process::Report;

mod gateway;

/// Scratch credentials and databases prepared by the provisioning checks.
pub struct Scratch<'a> {
    pub root: &'a Path,
    /// Test harness credential on the scratch Notification database.
    pub admin_notifications: String,
    pub runtime: String,
    pub migrator: String,
    /// (label, URL) pairs that must be refused at connect: a role on another owner's database.
    pub crossings: Vec<(String, String)>,
    /// The fresh App owner: harness URL, runtime URL (for the relay worker).
    pub admin_app: String,
    pub app_runtime: String,
    /// The fresh App database name (restored by the App-behind drill).
    pub app_database: &'a str,
    pub notifications_database: &'a str,
    /// Scratch copy for the backout drill, and the harness URL on it.
    pub copy_database: &'a str,
    pub admin_copy: String,
    /// libpq settings of the harness credential, for psql/pg_dump/pg_restore.
    pub pg_env: Vec<(&'static str, String)>,
    /// Values that must not appear in any output.
    pub secrets: Vec<String>,
}

pub fn verify(report: &mut Report, scratch: &Scratch<'_>) -> bool {
    let dir = match lifecycle::run_directory(scratch.root, "verify-notifications") {
        Ok(dir) => dir,
        Err(error) => {
            report.fail("Notification evidence directory", &error);
            return false;
        }
    };
    let sockets = match lifecycle::SocketDir::new() {
        Ok(owner) => owner,
        Err(error) => {
            report.fail("Notification socket directory", &error);
            return false;
        }
    };
    let run = Run {
        scratch,
        ingest: sockets.0.join("ingest.sock"),
        inbox: sockets.0.join("inbox.sock"),
        dir,
    };
    report.step(
        "notifications: invalid configuration exits 2 without echoing credentials",
        || run.config_errors(),
    ) && report.step(
        "notifications migrate: runtime refused, migrator applies once, other owners unreachable",
        || run.migrations(),
    ) && report.step(
        "notifications runtime: DML only; no DDL or TEMP; App/Identity roles cannot connect to it",
        || run.isolation(),
    ) && report.step(
        "notifications serve: two sockets ready with the runtime role; each answers only its peer user; SIGTERM removes both",
        || run.sockets(),
    ) && report.step(
        "App relay → Notification Service, two real binaries and two databases: applied, service stop retried then delivered, operator replay already_applied",
        || run.relay_end_to_end(),
    ) && report.step(
        "kill -9 of the service and the worker under load: every event applied exactly once",
        || run.crash_under_load(),
    ) && report.step(
        "Notification restored behind the App's ACK: gate closed, replay restores the projection exactly",
        || run.restore_behind_app(),
    ) && report.step(
        "App restored behind Notification: Notification restored from the earlier dump and replayed; lost App events never appear",
        || run.app_behind_notification(),
    ) && report.step(
        "migration backout and forward on a restored copy",
        || run.backout_on_copy(),
    ) && report.step(
        "App Gateway ↔ Notification Service, real processes: business commits → relay → badge/list/Seen/Read/MarkAllRead; removal and private work filtered; recovering → 503/stale",
        || run.gateway_end_to_end(),
    ) && report.step("notifications: logs contain no credential", || {
        run.no_secrets_in_logs()
    })
}

struct Run<'a> {
    scratch: &'a Scratch<'a>,
    ingest: PathBuf,
    inbox: PathBuf,
    dir: PathBuf,
}

/// Canonical JSON of the notification contract: keys sorted at every level, compact.
fn canonical(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_unstable();
            let fields: Vec<String> = keys
                .into_iter()
                .map(|key| format!("{}:{}", Value::String(key.clone()), canonical(&map[key])))
                .collect();
            format!("{{{}}}", fields.join(","))
        }
        Value::Array(items) => format!(
            "[{}]",
            items.iter().map(canonical).collect::<Vec<_>>().join(",")
        ),
        scalar => scalar.to_string(),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

const OWNER: &str = "0a0a0a0a-0a0a-4a0a-8a0a-0a0a0a0a0a0a";
const WORK: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";

/// A root approval for the default owner and work.
fn root_approval(
    event_id: &str,
    actor: &str,
    comment: &str,
    occurred_at: &str,
) -> (String, String) {
    approval(event_id, actor, OWNER, WORK, comment, occurred_at)
}

/// A root approval as the App producer stores it: canonical envelope and its digest.
fn approval(
    event_id: &str,
    actor: &str,
    owner: &str,
    work: &str,
    comment: &str,
    occurred_at: &str,
) -> (String, String) {
    let envelope = canonical(&json!({
        "event_id": event_id,
        "event_type": "comment.root.approved",
        "schema_version": 1,
        "source_owner": "discussion",
        "activation_epoch": 1,
        "source_order": 2,
        "publication_stamp": {"epoch": 1, "readable": true},
        "occurred_at": occurred_at,
        "actor_id": actor,
        "recipients": [owner],
        "target": {
            "kind": "work_comment_thread",
            "work_id": work,
            "comment_id": comment,
            "moderation_version": 2
        },
        "semantic_kind": "approved",
        "action_required": false
    }));
    let digest = sha256_hex(envelope.as_bytes());
    (envelope, digest)
}

fn query_text(url: &str, statement: &str) -> Result<String, String> {
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
            sqlx::query_scalar(sqlx::AssertSqlSafe(statement))
                .fetch_one(&mut connection)
                .await
                .map_err(|e| e.to_string())
        })
}

impl Run<'_> {
    fn binary(&self, name: &str) -> Command {
        let mut command = Command::new(lifecycle::binary(self.scratch.root, name));
        command
            .current_dir(self.scratch.root)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("VINK_LOG_FORMAT", "text")
            .env("RUST_LOG", "info,sqlx=warn");
        command
    }

    fn uid(&self) -> Result<u32, String> {
        std::fs::metadata(&self.dir)
            .map(|m| m.uid())
            .map_err(|e| e.to_string())
    }

    fn log(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.log"))
    }

    fn config_errors(&self) -> Result<(), String> {
        let out = self
            .binary("vink-notifications")
            .arg("serve")
            .env(
                "VINK_NOTIFICATIONS_DATABASE_URL",
                "mysql://secret-marker@x/y",
            )
            .env("VINK_NOTIFICATIONS_INGEST_SOCKET", &self.ingest)
            .env("VINK_NOTIFICATIONS_INBOX_SOCKET", &self.ingest)
            .output()
            .map_err(|e| e.to_string())?;
        let stderr = String::from_utf8_lossy(&out.stderr);
        std::fs::write(self.log("config"), stderr.as_bytes()).map_err(|e| e.to_string())?;
        if out.status.code() != Some(2)
            || !stderr.contains("VINK_NOTIFICATIONS_DATABASE_URL")
            || !stderr.contains("must differ")
            || stderr.contains("secret-marker")
        {
            return Err(format!(
                "serve with bad settings: {:?}: {stderr}",
                out.status.code()
            ));
        }
        let out = self
            .binary("vink-notifications")
            .arg("migrate")
            .env("VINK_NOTIFICATIONS_DATABASE_URL", &self.scratch.runtime)
            .output()
            .map_err(|e| e.to_string())?;
        if out.status.code() != Some(2)
            || !String::from_utf8_lossy(&out.stderr)
                .contains("VINK_NOTIFICATIONS_MIGRATE_DATABASE_URL")
        {
            return Err("migrate must not fall back to the runtime credential".into());
        }
        println!("==> notifications configuration errors exit 2 and name the setting");
        Ok(())
    }

    fn migrate(&self, label: &str, url: &str) -> Result<(Option<i32>, String), String> {
        let out = self
            .binary("vink-notifications")
            .arg("migrate")
            .env("VINK_NOTIFICATIONS_MIGRATE_DATABASE_URL", url)
            .output()
            .map_err(|e| format!("migrate {label}: {e}"))?;
        let stderr = lifecycle::strip_ansi(&String::from_utf8_lossy(&out.stderr));
        std::fs::write(self.log(&format!("migrate-{label}")), &stderr)
            .map_err(|e| e.to_string())?;
        Ok((out.status.code(), stderr))
    }

    fn migrations(&self) -> Result<(), String> {
        let s = self.scratch;
        let (code, stderr) = self.migrate("runtime", &s.runtime)?;
        if code != Some(1) || !stderr.contains("lacks a privilege") {
            return Err(format!(
                "migrate as the Notification runtime: expected exit 1 naming the privilege, got {code:?}: {stderr}"
            ));
        }
        let (code, stderr) = self.migrate("first", &s.migrator)?;
        if code != Some(0) || !stderr.contains("applied=[20261128000000, 20261129000000]") {
            return Err(format!("first Notification migrate: {code:?}: {stderr}"));
        }
        let (code, stderr) = self.migrate("second", &s.migrator)?;
        if code != Some(0) || !stderr.contains("applied=[]") {
            return Err(format!(
                "second migrate must be a no-op: {code:?}: {stderr}"
            ));
        }
        println!("==> notification migrations: runtime refused, applied once");
        Ok(())
    }

    fn isolation(&self) -> Result<(), String> {
        let s = self.scratch;
        if sql(
            &s.runtime,
            "INSERT INTO inboxes (recipient, created_at, updated_at) VALUES ('0b0b0b0b-0b0b-4b0b-8b0b-0b0b0b0b0b0b', now(), now())",
        )? != 1
        {
            return Err("the runtime role cannot write an inbox".into());
        }
        sql(
            &s.runtime,
            "DELETE FROM inboxes WHERE recipient = '0b0b0b0b-0b0b-4b0b-8b0b-0b0b0b0b0b0b'",
        )?;
        for ddl in [
            "CREATE TABLE vink_probe (id int)",
            "CREATE TEMP TABLE vink_probe (id int)",
        ] {
            match sql(&s.runtime, ddl) {
                Ok(_) => return Err(format!("runtime role ran `{ddl}`")),
                Err(error) if error.contains("permission denied") => {}
                Err(error) => return Err(format!("`{ddl}`: unexpected {error}")),
            }
        }
        for (label, url) in &s.crossings {
            let url = url.clone();
            let refused = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| e.to_string())?
                .block_on(async move {
                    match PgConnection::connect(&url).await {
                        Ok(mut connection) => {
                            let _ = connection.execute("SELECT 1").await;
                            false
                        }
                        Err(_) => true,
                    }
                });
            if !refused {
                return Err(format!("{label}: connected across owners"));
            }
        }
        println!(
            "==> notification runtime is DML-only; {} cross-owner connections refused",
            s.crossings.len()
        );
        Ok(())
    }

    fn start(&self, name: &str, relay: Option<u32>, gateway: Option<u32>) -> Result<Guard, String> {
        let log = std::fs::File::create(self.log(name)).map_err(|e| e.to_string())?;
        let mut command = self.binary("vink-notifications");
        command
            .arg("serve")
            .env("VINK_NOTIFICATIONS_DATABASE_URL", &self.scratch.runtime)
            .env("VINK_NOTIFICATIONS_INGEST_SOCKET", &self.ingest)
            .env("VINK_NOTIFICATIONS_INBOX_SOCKET", &self.inbox)
            .stdout(Stdio::null())
            .stderr(log);
        if let Some(uid) = relay {
            command.env("VINK_NOTIFICATIONS_RELAY_UID", uid.to_string());
        }
        if let Some(uid) = gateway {
            command.env("VINK_NOTIFICATIONS_GATEWAY_UID", uid.to_string());
        }
        let child = command
            .spawn()
            .map_err(|e| format!("spawn vink-notifications: {e}"))?;
        let mut guard = Guard(Some(child));
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            if let Some(Ok(Some(status))) = guard.0.as_mut().map(|c| c.try_wait()) {
                return Err(format!(
                    "vink-notifications ({name}) exited early with {status}"
                ));
            }
            let live = |socket: &Path| {
                request(socket, "GET", "/health/live", &[], b"").is_ok_and(|(s, _)| s == 200)
            };
            if live(&self.ingest) && live(&self.inbox) {
                return Ok(guard);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err(format!(
            "vink-notifications ({name}) did not answer on both sockets within 20s"
        ))
    }

    fn stop(&self, mut guard: Guard) -> Result<(), String> {
        let status = guard.terminate(Duration::from_secs(10))?;
        if !status.success() {
            return Err(format!(
                "vink-notifications exited with {status} after SIGTERM"
            ));
        }
        if self.ingest.exists() || self.inbox.exists() {
            return Err("a socket file survived shutdown".into());
        }
        Ok(())
    }

    fn sockets(&self) -> Result<(), String> {
        let uid = self.uid()?;
        // The Gateway socket is configured for another user: it is closed to this process.
        let guard = self.start("serve-wrong-gateway", Some(uid), Some(uid.wrapping_add(1)))?;
        for (socket, scope) in [(&self.ingest, "ingest"), (&self.inbox, "inbox")] {
            let (status, body) = request(socket, "GET", "/health/ready", &[], b"")?;
            if status != 200 || !body.contains("\"ready\"") || !body.contains(scope) {
                return Err(format!(
                    "{scope} readiness with the runtime role: {status} {body}"
                ));
            }
        }
        let count = format!("/v1/inboxes/{OWNER}/count");
        let (status, body) = request(&self.inbox, "GET", &count, &[], b"")?;
        if status != 401 || !body.contains("caller_unauthenticated") {
            return Err(format!("inbox call from the wrong user: {status} {body}"));
        }
        // A route of the other scope does not exist here, whatever the caller.
        let (status, _) = request(&self.ingest, "GET", &count, &[], b"")?;
        if status != 404 {
            return Err(format!(
                "inbox route on the ingest socket answered {status}"
            ));
        }
        let (status, body) = request(&self.ingest, "POST", "/v1/ingest", &[], b"{\"events\":[]}")?;
        if status != 200 || !body.contains("\"results\":[]") {
            return Err(format!("empty ingest from the relay user: {status} {body}"));
        }
        self.stop(guard)?;
        // No Gateway user at all: that socket is not ready and refuses everything.
        let guard = self.start("serve-no-gateway", Some(uid), None)?;
        let (status, body) = request(&self.inbox, "GET", "/health/ready", &[], b"")?;
        if status != 503 || !body.contains("not_configured") {
            return Err(format!("inbox readiness without a caller: {status} {body}"));
        }
        self.stop(guard)?;
        println!(
            "==> both sockets ready; wrong or missing caller refused; shutdown removed the sockets"
        );
        Ok(())
    }

    fn insert_event(&self, event_id: &str, actor: &str, comment: &str) -> Result<(), String> {
        let occurred = query_text(
            &self.scratch.admin_app,
            "SELECT to_char(now() AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')",
        )?;
        let (envelope, digest) = root_approval(event_id, actor, comment, &occurred);
        let statement = format!(
            "WITH e AS (INSERT INTO notification_events
                 (id, event_type, schema_version, source_owner, dedup_key, activation_epoch, envelope, payload_digest)
               VALUES ('{event_id}', 'comment.root.approved', 1, 'discussion',
                       'comment.root.approved/comment/{comment}/2', 1, '{envelope}', '{digest}')
               RETURNING id)
             INSERT INTO notification_deliveries (event_id) SELECT id FROM e"
        );
        if sql(&self.scratch.admin_app, &statement)? != 1 {
            return Err("the App event was not stored".into());
        }
        Ok(())
    }

    fn delivery(&self, event_id: &str) -> Result<String, String> {
        query_text(
            &self.scratch.admin_app,
            &format!(
                "SELECT state || '/' || coalesce(outcome, '-') || '/' || coalesce(last_error, '-') || '/' || attempts
                   FROM notification_deliveries WHERE event_id = '{event_id}'"
            ),
        )
    }

    fn wait_delivery(
        &self,
        event_id: &str,
        wanted: &str,
        within: Duration,
    ) -> Result<String, String> {
        let deadline = Instant::now() + within;
        let mut last = String::new();
        while Instant::now() < deadline {
            last = self.delivery(event_id)?;
            if last.starts_with(wanted) {
                return Ok(last);
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        Err(format!(
            "delivery {event_id} never reached `{wanted}` (last `{last}`)"
        ))
    }

    fn start_worker(&self, uid: u32) -> Result<Guard, String> {
        self.start_worker_named(uid, "worker")
    }

    fn start_worker_named(&self, uid: u32, name: &str) -> Result<Guard, String> {
        let log = std::fs::File::create(self.log(name)).map_err(|e| e.to_string())?;
        let mut command = Command::new(lifecycle::binary(self.scratch.root, "vink-worker"));
        crate::env::clear_capabilities(&mut command);
        command
            .current_dir(self.scratch.root)
            .env("DATABASE_URL", &self.scratch.app_runtime)
            .env("VINK_PROFILE", "local")
            .env("VINK_LOG_FORMAT", "text")
            .env("RUST_LOG", "info,sqlx=warn")
            .env("VINK_WORKER_OUTBOX", "disabled")
            .env("VINK_NOTIFICATIONS_INGEST_SOCKET", &self.ingest)
            .env("VINK_NOTIFICATIONS_SERVER_UID", uid.to_string())
            .env_remove("VINK_DEV_PRINCIPAL")
            .stdout(Stdio::null())
            .stderr(log);
        let child = command
            .spawn()
            .map_err(|e| format!("spawn vink-worker: {e}"))?;
        Ok(Guard(Some(child)))
    }

    fn groups(&self) -> Result<(i64, i64), String> {
        let admin = &self.scratch.admin_notifications;
        Ok((
            sql(admin, "SELECT count(*) FROM notification_groups")?,
            sql(
                admin,
                "SELECT coalesce(max(actor_count), 0)::bigint FROM notification_groups",
            )?,
        ))
    }

    fn relay_end_to_end(&self) -> Result<(), String> {
        let uid = self.uid()?;
        let first = "019a9b10-0347-7000-8000-000000000001";
        let second = "019a9b10-0347-7000-8000-000000000002";
        self.insert_event(
            first,
            "11111111-1111-4111-8111-111111111111",
            "31313131-3131-4131-8131-313131313131",
        )?;
        let service = self.start("serve-relay", Some(uid), Some(uid))?;
        let mut worker = self.start_worker(uid)?;
        let delivered = self.wait_delivery(first, "succeeded/applied", Duration::from_secs(60))?;
        println!("==> first event: {delivered}");
        if self.groups()? != (1, 1) {
            return Err(format!(
                "expected one group with one actor, found {:?}",
                self.groups()?
            ));
        }
        let (status, body) = request(
            &self.inbox,
            "GET",
            &format!("/v1/inboxes/{OWNER}/count"),
            &[],
            b"",
        )?;
        if status != 200 || !body.contains("\"unseen_group_count\":1") {
            return Err(format!("badge through the inbox socket: {status} {body}"));
        }
        // The service stops: the relay keeps the obligation and retries; nothing is lost.
        self.stop(service)?;
        self.insert_event(
            second,
            "22222222-2222-4222-8222-222222222222",
            "32323232-3232-4232-8232-323232323232",
        )?;
        let retried = self.wait_delivery(second, "ready/-/transport", Duration::from_secs(60))?;
        println!("==> second event while the service is down: {retried}");
        let service = self.start("serve-relay-restarted", Some(uid), Some(uid))?;
        let delivered = self.wait_delivery(second, "succeeded/applied", Duration::from_secs(90))?;
        println!("==> second event after restart: {delivered}");
        if self.groups()? != (1, 2) {
            return Err(format!(
                "expected the second actor in the same group, found {:?}",
                self.groups()?
            ));
        }
        // Operator replay (as after a Notification restore): already applied, no second effect.
        let out = Command::new(lifecycle::binary(self.scratch.root, "vink-app"))
            .args([
                "notifications",
                "replay",
                "2000-01-01T00:00:00Z",
                "verify postgres replay drill",
            ])
            .current_dir(self.scratch.root)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("DATABASE_URL", &self.scratch.app_runtime)
            .env("VINK_PROFILE", "local")
            .env("VINK_LOG_FORMAT", "text")
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!(
                "vink-app notifications replay: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        for event in [first, second] {
            let replayed =
                self.wait_delivery(event, "succeeded/already_applied", Duration::from_secs(90))?;
            println!("==> replayed {event}: {replayed}");
        }
        let receipts = sql(
            &self.scratch.admin_notifications,
            "SELECT count(*) FROM ingestion_receipts",
        )?;
        if self.groups()? != (1, 2) || receipts != 2 {
            return Err(format!(
                "replay changed the inbox: {:?}, {receipts} receipts",
                self.groups()?
            ));
        }
        let status = worker.terminate(Duration::from_secs(15))?;
        if !status.success() {
            return Err(format!("vink-worker exited with {status}"));
        }
        let log =
            lifecycle::strip_ansi(&std::fs::read_to_string(self.log("worker")).unwrap_or_default());
        if !log.contains("notification_relay=\"socket\"") {
            return Err("the worker did not report its socket relay".into());
        }
        self.stop(service)?;
        println!(
            "==> relay delivered across two processes and two databases; replay had no second effect"
        );
        Ok(())
    }

    /// psql/pg_dump/pg_restore with the harness credential (`VINK_PG_BIN` or PATH).
    fn pg(&self, name: &str) -> Command {
        let program = std::env::var_os("VINK_PG_BIN")
            .map(|dir| Path::new(&dir).join(name))
            .unwrap_or_else(|| name.into());
        let mut command = Command::new(program);
        command
            .current_dir(self.scratch.root)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .envs(self.scratch.pg_env.iter().map(|(k, v)| (*k, v.as_str())));
        command
    }

    fn dump(&self, database: &str, label: &str) -> Result<PathBuf, String> {
        let file = self.dir.join(format!("{label}.dump"));
        let out = self
            .pg("pg_dump")
            .args(["--format=custom", "--file"])
            .arg(&file)
            .args(["--dbname", database])
            .output()
            .map_err(|e| format!("pg_dump {database}: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "pg_dump {database}: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        Ok(file)
    }

    /// Restores `file` over the existing database in one transaction; database-level grants stay.
    fn restore(&self, database: &str, file: &Path) -> Result<(), String> {
        let out = self
            .pg("pg_restore")
            .args([
                "--clean",
                "--if-exists",
                "--single-transaction",
                "--exit-on-error",
                "--dbname",
                database,
            ])
            .arg(file)
            .output()
            .map_err(|e| format!("pg_restore {database}: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "pg_restore {database}: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        Ok(())
    }

    fn psql_file(&self, database: &str, file: &Path) -> Result<(bool, String), String> {
        let out = self
            .pg("psql")
            .args([
                "-X",
                "-q",
                "-v",
                "ON_ERROR_STOP=1",
                "--dbname",
                database,
                "-f",
            ])
            .arg(file)
            .output()
            .map_err(|e| format!("psql: {e}"))?;
        Ok((
            out.status.success(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        ))
    }

    /// `count` root approvals of drill `drill`, spread over ten owners and seven works, each by a
    /// new actor on a new comment, stored as the App producer stores them.
    fn insert_batch(&self, drill: u32, from: u32, count: u32) -> Result<(), String> {
        let occurred = query_text(
            &self.scratch.admin_app,
            "SELECT to_char(now() AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')",
        )?;
        let mut rows = Vec::new();
        for i in from..from + count {
            let event_id = format!("019a9b10-0347-7{drill:03x}-8000-{i:012x}");
            let actor = format!("a1a1a1a1-{drill:04x}-4000-8000-{i:012x}");
            let owner = format!("0b0b0b0b-{drill:04x}-4000-8000-{:012x}", i % 10);
            let work = format!("aaaaaaaa-{drill:04x}-4000-8000-{:012x}", i % 7);
            let comment = format!("c0c0c0c0-{drill:04x}-4000-8000-{i:012x}");
            let (envelope, digest) =
                approval(&event_id, &actor, &owner, &work, &comment, &occurred);
            rows.push(format!(
                "('{event_id}', 'comment.root.approved', 1, 'discussion', 'comment.root.approved/comment/{comment}/2', 1, '{envelope}', '{digest}')"
            ));
        }
        let statement = format!(
            "WITH e AS (INSERT INTO notification_events
                 (id, event_type, schema_version, source_owner, dedup_key, activation_epoch, envelope, payload_digest)
               VALUES {} RETURNING id)
             INSERT INTO notification_deliveries (event_id) SELECT id FROM e",
            rows.join(",")
        );
        if sql(&self.scratch.admin_app, &statement)? != i64::from(count) {
            return Err("the App events were not stored".into());
        }
        Ok(())
    }

    fn drill_like(drill: u32) -> String {
        format!("019a9b10-0347-7{drill:03x}-%")
    }

    /// App deliveries of a drill not yet succeeded, and succeeded with `already_applied`.
    fn pending_and_replayed(&self, drill: u32) -> Result<(i64, i64), String> {
        let like = Self::drill_like(drill);
        Ok((
            sql(
                &self.scratch.admin_app,
                &format!(
                    "SELECT count(*) FROM notification_deliveries WHERE event_id::text LIKE '{like}' AND state <> 'succeeded'"
                ),
            )?,
            sql(
                &self.scratch.admin_app,
                &format!(
                    "SELECT count(*) FROM notification_deliveries WHERE event_id::text LIKE '{like}' AND outcome = 'already_applied'"
                ),
            )?,
        ))
    }

    fn all_pending(&self) -> Result<i64, String> {
        sql(
            &self.scratch.admin_app,
            "SELECT count(*) FROM notification_deliveries WHERE state <> 'succeeded'",
        )
    }

    fn receipts(&self, drill: u32) -> Result<i64, String> {
        sql(
            &self.scratch.admin_notifications,
            &format!(
                "SELECT count(*) FROM ingestion_receipts WHERE event_id::text LIKE '{}'",
                Self::drill_like(drill)
            ),
        )
    }

    fn wait_for(
        &self,
        what: &str,
        within: Duration,
        mut done: impl FnMut() -> Result<bool, String>,
    ) -> Result<Duration, String> {
        let started = Instant::now();
        while started.elapsed() < within {
            if done()? {
                return Ok(started.elapsed());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        Err(format!("{what}: not reached within {within:?}"))
    }

    /// The inbox projection without generated IDs: every group's key, counts, active actors,
    /// read state and creation time, in a stable order.
    fn projection(&self) -> Result<String, String> {
        query_text(
            &self.scratch.admin_notifications,
            "SELECT coalesce(json_agg(p ORDER BY p.recipient, p.event_type, p.target_key, p.created_at), '[]')::text
               FROM (SELECT g.recipient, g.event_type, g.target_key, g.actor_count, g.item_count,
                            g.read_through > 0 AS read, g.created_at,
                            (SELECT array_agg(m.actor_id ORDER BY m.actor_id) FROM group_members m
                              WHERE m.group_id = g.group_id AND m.active) AS actors
                       FROM notification_groups g) p",
        )
    }

    fn kill(guard: &mut Guard) -> Result<(), String> {
        let mut child = guard.0.take().ok_or("process already stopped")?;
        child.kill().map_err(|e| e.to_string())?;
        child.wait().map_err(|e| e.to_string())?;
        Ok(())
    }

    fn operator(
        &self,
        binary: &str,
        args: &[&str],
        database_env: (&str, &str),
    ) -> Result<String, String> {
        let out = Command::new(lifecycle::binary(self.scratch.root, binary))
            .args(args)
            .current_dir(self.scratch.root)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env(database_env.0, database_env.1)
            .env("VINK_PROFILE", "local")
            .env("VINK_LOG_FORMAT", "text")
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!(
                "{binary} {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    fn recovery(&self, phase: &str, note: &str) -> Result<(), String> {
        self.operator(
            "vink-notifications",
            &["maintenance", "recovery", phase, note],
            ("VINK_NOTIFICATIONS_DATABASE_URL", &self.scratch.runtime),
        )
        .map(|_| ())
    }

    fn replay(&self, since: &str, note: &str) -> Result<(), String> {
        self.operator(
            "vink-app",
            &["notifications", "replay", since, note],
            ("DATABASE_URL", &self.scratch.app_runtime),
        )
        .map(|_| ())
    }

    fn inbox_count(&self, owner: &str) -> Result<(u16, String), String> {
        request(
            &self.inbox,
            "GET",
            &format!("/v1/inboxes/{owner}/count"),
            &[],
            b"",
        )
    }

    fn crash_under_load(&self) -> Result<(), String> {
        const DRILL: u32 = 1;
        const EVENTS: u32 = 400;
        let uid = self.uid()?;
        self.insert_batch(DRILL, 0, EVENTS)?;
        let mut service = self.start("serve-crash-1", Some(uid), Some(uid))?;
        let mut worker = self.start_worker_named(uid, "worker-crash-1")?;
        self.wait_for("first deliveries", Duration::from_secs(60), || {
            Ok(self.pending_and_replayed(DRILL)?.0 <= i64::from(EVENTS) - 50)
        })?;
        Self::kill(&mut service)?;
        let pending = self.pending_and_replayed(DRILL)?.0;
        println!("==> service killed (SIGKILL) with {pending} of {EVENTS} deliveries outstanding");
        let _service = self.start("serve-crash-2", Some(uid), Some(uid))?;
        self.wait_for("progress after restart", Duration::from_secs(90), || {
            Ok(self.pending_and_replayed(DRILL)?.0 < pending)
        })?;
        Self::kill(&mut worker)?;
        let pending = self.pending_and_replayed(DRILL)?.0;
        println!(
            "==> worker killed (SIGKILL) with {pending} deliveries outstanding; its leases must expire"
        );
        let mut worker = self.start_worker_named(uid, "worker-crash-2")?;
        let waited = self.wait_for("every delivery", Duration::from_secs(150), || {
            Ok(self.pending_and_replayed(DRILL)?.0 == 0)
        })?;
        let receipts = self.receipts(DRILL)?;
        let members = sql(
            &self.scratch.admin_notifications,
            &format!(
                "SELECT coalesce(sum(actor_count), 0)::bigint FROM notification_groups
              WHERE recipient::text LIKE '0b0b0b0b-{DRILL:04x}-%'"
            ),
        )?;
        let replayed = self.pending_and_replayed(DRILL)?.1;
        println!(
            "==> all {EVENTS} delivered {:.1}s after the worker restart; {replayed} answered already_applied (committed before a lost ACK)",
            waited.as_secs_f64()
        );
        if receipts != i64::from(EVENTS) || members != i64::from(EVENTS) {
            return Err(format!(
                "expected {EVENTS} receipts and members, found {receipts} and {members}"
            ));
        }
        let status = worker.terminate(Duration::from_secs(15))?;
        if !status.success() {
            return Err(format!("vink-worker exited with {status}"));
        }
        self.stop(_service)
    }

    fn restore_behind_app(&self) -> Result<(), String> {
        const DRILL: u32 = 2;
        let uid = self.uid()?;
        let service = self.start("serve-restore-1", Some(uid), Some(uid))?;
        let mut worker = self.start_worker_named(uid, "worker-restore")?;
        self.insert_batch(DRILL, 0, 500)?;
        let drained = self.wait_for("first 500", Duration::from_secs(120), || {
            Ok(self.pending_and_replayed(DRILL)?.0 == 0)
        })?;
        println!(
            "==> relay drained 500 events in {:.2}s ({:.0}/s end to end)",
            drained.as_secs_f64(),
            500.0 / drained.as_secs_f64()
        );
        let since = query_text(
            &self.scratch.admin_app,
            "SELECT to_char((now() - interval '1 minute') AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')",
        )?;
        let dump = self.dump(self.scratch.notifications_database, "notifications-before")?;
        self.insert_batch(DRILL, 500, 100)?;
        self.wait_for("next 100", Duration::from_secs(60), || {
            Ok(self.pending_and_replayed(DRILL)?.0 == 0)
        })?;
        let reference = self.projection()?;
        // Notification is lost and restored to the dump: behind what the App acknowledged.
        self.stop(service)?;
        self.restore(self.scratch.notifications_database, &dump)?;
        if self.receipts(DRILL)? != 500 {
            return Err("the restore did not go back to the dump".into());
        }
        self.recovery("begin", "verify: Notification restored behind the App")?;
        let service = self.start("serve-restore-2", Some(uid), Some(uid))?;
        let owner = format!("0b0b0b0b-{DRILL:04x}-4000-8000-000000000000");
        let (status, body) = self.inbox_count(&owner)?;
        if status != 503 || !body.contains("recovering") {
            return Err(format!("inbox while recovering: {status} {body}"));
        }
        self.replay(&since, "verify: replay after Notification restore")?;
        self.wait_for("replay", Duration::from_secs(120), || {
            Ok(self.all_pending()? == 0)
        })?;
        self.recovery("end", "verify: replay drained")?;
        let (status, body) = self.inbox_count(&owner)?;
        if status != 200 {
            return Err(format!("inbox after recovery: {status} {body}"));
        }
        if self.projection()? != reference || self.receipts(DRILL)? != 600 {
            return Err("the replayed projection differs from the one before the loss".into());
        }
        let replayed = self.pending_and_replayed(DRILL)?.1;
        println!(
            "==> restore + replay: projection identical; {replayed} replayed events were already applied"
        );
        let status = worker.terminate(Duration::from_secs(15))?;
        if !status.success() {
            return Err(format!("vink-worker exited with {status}"));
        }
        self.stop(service)
    }

    fn app_behind_notification(&self) -> Result<(), String> {
        const DRILL: u32 = 3;
        let uid = self.uid()?;
        let service = self.start("serve-app-behind-1", Some(uid), Some(uid))?;
        let mut worker = self.start_worker_named(uid, "worker-app-behind-1")?;
        // A backup set as backup.sh takes it: Notification first, then the App.
        let since = query_text(
            &self.scratch.admin_app,
            "SELECT to_char((now() - interval '1 minute') AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')",
        )?;
        let notifications = self.dump(self.scratch.notifications_database, "set-notifications")?;
        let app = self.dump(self.scratch.app_database, "set-app")?;
        let reference = self.projection()?;
        // Work after the backup reaches Notification, then the App is lost and restored.
        self.insert_batch(DRILL, 0, 50)?;
        self.wait_for("events after the backup", Duration::from_secs(60), || {
            Ok(self.pending_and_replayed(DRILL)?.0 == 0)
        })?;
        if self.receipts(DRILL)? != 50 {
            return Err("Notification did not apply the events after the backup".into());
        }
        self.recovery("begin", "verify: App restore in progress")?;
        let status = worker.terminate(Duration::from_secs(15))?;
        if !status.success() {
            return Err(format!("vink-worker exited with {status}"));
        }
        self.restore(self.scratch.app_database, &app)?;
        // Notification is ahead of the App: go back to the set's earlier Notification dump.
        self.stop(service)?;
        self.restore(self.scratch.notifications_database, &notifications)?;
        self.recovery("begin", "verify: Notification restored from the same set")?;
        let service = self.start("serve-app-behind-2", Some(uid), Some(uid))?;
        let mut worker = self.start_worker_named(uid, "worker-app-behind-2")?;
        self.replay(&since, "verify: replay after App restore")?;
        self.wait_for("replay", Duration::from_secs(120), || {
            Ok(self.all_pending()? == 0)
        })?;
        self.recovery("end", "verify: reconciled with the App")?;
        let lost = sql(
            &self.scratch.admin_app,
            &format!(
                "SELECT count(*) FROM notification_events WHERE id::text LIKE '{}'",
                Self::drill_like(DRILL)
            ),
        )?;
        if lost != 0 || self.receipts(DRILL)? != 0 || self.projection()? != reference {
            return Err("an event the App lost is still visible in Notifications".into());
        }
        println!(
            "==> App restored behind Notification: the 50 lost App events left no inbox effect"
        );
        let status = worker.terminate(Duration::from_secs(15))?;
        if !status.success() {
            return Err(format!("vink-worker exited with {status}"));
        }
        self.stop(service)
    }

    fn backout_on_copy(&self) -> Result<(), String> {
        let s = self.scratch;
        let dump = self.dump(s.notifications_database, "backout-source")?;
        sql(
            &s.admin_app,
            &format!(
                "CREATE DATABASE {} WITH TEMPLATE template0 ENCODING 'UTF8'",
                s.copy_database
            ),
        )
        .or_else(|_| {
            sql(
                &s.admin_notifications,
                &format!(
                    "CREATE DATABASE {} WITH TEMPLATE template0 ENCODING 'UTF8'",
                    s.copy_database
                ),
            )
        })?;
        let out = self
            .pg("pg_restore")
            .args(["--exit-on-error", "--dbname", s.copy_database])
            .arg(&dump)
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!(
                "restore into the copy: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        let rollback = s
            .root
            .join("services/vink-notifications/migrations-rollback");
        let (ok, stderr) = self.psql_file(
            s.copy_database,
            &rollback.join("20261129000000_notification_recovery.down.sql"),
        )?;
        if !ok {
            return Err(format!("recovery backout: {stderr}"));
        }
        let inbox_down = rollback.join("20261128000000_notification_inbox.down.sql");
        let (ok, stderr) = self.psql_file(s.copy_database, &inbox_down)?;
        if ok || !stderr.contains("within retention") {
            return Err(format!(
                "inbox backout with live groups must refuse: {stderr}"
            ));
        }
        sql(&s.admin_copy, "DELETE FROM notification_groups")?;
        let (ok, stderr) = self.psql_file(s.copy_database, &inbox_down)?;
        if !ok {
            return Err(format!("inbox backout after purge: {stderr}"));
        }
        if sql(
            &s.admin_copy,
            "SELECT count(*) FROM pg_tables WHERE schemaname = 'public' AND tablename <> '_sqlx_migrations'",
        )? != 0
            || sql(&s.admin_copy, "SELECT count(*) FROM _sqlx_migrations")? != 0
        {
            return Err("backout left tables or history behind".into());
        }
        let (code, stderr) = self.migrate("copy-forward", &s.admin_copy)?;
        if code != Some(0) || !stderr.contains("applied=[20261128000000, 20261129000000]") {
            return Err(format!("forward migration on the copy: {code:?}: {stderr}"));
        }
        println!(
            "==> backout on a copy: refused with live groups, clean after purge, forward migration reapplied"
        );
        Ok(())
    }

    fn no_secrets_in_logs(&self) -> Result<(), String> {
        let mut needles = self.scratch.secrets.clone();
        needles.push("secret-marker".into());
        for entry in std::fs::read_dir(&self.dir).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().is_some_and(|ext| ext == "log") {
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                if needles
                    .iter()
                    .any(|needle| !needle.is_empty() && text.contains(needle.as_str()))
                {
                    return Err(format!("{} contains a credential", path.display()));
                }
                // Envelopes and principal IDs are never logged by either process.
                if text.contains(OWNER) {
                    return Err(format!("{} contains a principal ID", path.display()));
                }
            }
        }
        println!("==> notification and relay logs: no credential, no principal ID");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The App's contract example digest, recomputed by this checker's canonical form.
    #[test]
    fn the_canonical_form_matches_the_contract_example() {
        let example = json!({
            "event_id": "019a9b10-0002-7000-8000-000000000102",
            "event_type": "work.endorsement.tham.changed",
            "schema_version": 1,
            "source_owner": "endorsement",
            "activation_epoch": 1,
            "source_order": 4,
            "publication_stamp": {"epoch": 1, "readable": false},
            "occurred_at": "2026-11-20T02:11:12Z",
            "actor_id": "44444444-4444-4444-8444-44444444444d",
            "recipients": ["55555555-5555-4555-8555-55555555555e"],
            "target": {
                "kind": "work_endorsement",
                "work_id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                "revision_id": "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee"
            },
            "semantic_kind": "remove",
            "action_required": false
        });
        assert_eq!(
            sha256_hex(canonical(&example).as_bytes()),
            "c922b2f3355c071bf1cf9fca5ac1ac73031b34579808a0aa87ce438e35859140"
        );
    }
}
