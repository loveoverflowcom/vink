//! `verify postgres` checks for the real `vink-notifications` executable (#347): configuration
//! errors, the migration lifecycle with provisioned roles, database isolation, the two private
//! sockets whose callers are authenticated by the OS peer user (T-07 CHOICE-21 A), and the App
//! relay of the real `vink-worker` delivering through the ingest socket from the App database to
//! the Notification database — including a service stop and an operator replay. The scratch
//! Notification owner comes from [`crate::provision`].

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

/// A root approval as the App producer stores it: canonical envelope and its digest.
fn root_approval(
    event_id: &str,
    actor: &str,
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
        "recipients": [OWNER],
        "target": {
            "kind": "work_comment_thread",
            "work_id": WORK,
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
        if code != Some(0) || !stderr.contains("applied=[20261128000000]") {
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
        let log = std::fs::File::create(self.log("worker")).map_err(|e| e.to_string())?;
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
