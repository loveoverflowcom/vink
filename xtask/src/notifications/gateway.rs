//! #348 AC-06: the App Gateway (`/api/v1/me/notifications…`) of real `vink-app` processes reading
//! the real `vink-notifications` inbox socket (peer user checked), fed by the real `vink-worker`
//! relay over the ingest socket. Every signal starts as an App business commit (Thấm, comments
//! approved by a moderator) and every loss of permission is a real command (comment removed, work
//! set private); the recovery gate of #347 must surface as `503`/`stale`, never `0` or an empty
//! list.
//!
//! Each principal is its own `vink-app serve-api` process in the `local` profile with
//! `VINK_DEV_PRINCIPAL` (loopback identity fixture, not authentication); a presented session
//! cookie is still required by the Gateway, which binds its tokens to it. Authentication through
//! Identity is covered by the identity checks of this lane.

use std::cell::Cell;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::{Run, query_text, sql};
use crate::lifecycle::{self, Guard};

const AUTHOR: &str = "11111111-1111-4111-8111-111111111111";
const WORK: &str = "33333333-3333-4333-8333-333333333333";
const REVISION: &str = "44444444-4444-4444-8444-444444444444";
const TITLE: &str = "[Mẫu] Mưa trên phố cũ";
const READER_A: &str = "a3480000-0000-4000-8000-00000000000a";
const READER_B: &str = "a3480000-0000-4000-8000-00000000000b";
const MODERATOR: &str = "a3480000-0000-4000-8000-0000000000aa";
const BODY_A: &str = "Câu thơ thứ hai làm tôi nhớ bến sông quê, kiểm thử Gateway hai process.";
const BODY_B: &str = "Đoạn kết lặng mà ấm; bình luận này sẽ bị gỡ trong bài kiểm.";
const SETTLE: Duration = Duration::from_secs(60);

/// One `vink-app serve-api` acting as one principal, with its own session cookie.
struct App {
    guard: Guard,
    port: u16,
    cookie: String,
}

impl App {
    fn origin(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn get(&self, path: &str) -> Result<(u16, Value), String> {
        exchange(self.port, "GET", path, &[("Cookie", &self.cookie)], None)
    }

    fn guest(&self, path: &str) -> Result<(u16, Value), String> {
        exchange(self.port, "GET", path, &[], None)
    }

    fn send(&self, method: &str, path: &str, body: &Value) -> Result<(u16, Value), String> {
        let origin = self.origin();
        exchange(
            self.port,
            method,
            path,
            &[("Cookie", &self.cookie), ("Origin", &origin)],
            Some(&body.to_string()),
        )
    }
}

/// Minimal HTTP/1.1 over loopback TCP with `Connection: close`: status and JSON body.
fn exchange(
    port: u16,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
) -> Result<(u16, Value), String> {
    let mut stream =
        std::net::TcpStream::connect(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .map_err(|e| e.to_string())?;
    let body = body.unwrap_or("");
    let mut head = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    if !body.is_empty() {
        head.push_str("Content-Type: application/json\r\n");
    }
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    stream
        .write_all(head.as_bytes())
        .and_then(|()| stream.write_all(body.as_bytes()))
        .map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).map_err(|e| e.to_string())?;
    let raw = String::from_utf8_lossy(&raw).into_owned();
    let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((&raw, ""));
    let status = head
        .get(9..12)
        .and_then(|code| code.parse().ok())
        .ok_or_else(|| format!("not an HTTP response: {raw:?}"))?;
    Ok((status, serde_json::from_str(body).unwrap_or(Value::Null)))
}

fn expect(what: &str, answer: (u16, Value), status: u16) -> Result<Value, String> {
    if answer.0 == status {
        Ok(answer.1)
    } else {
        Err(format!(
            "{what}: expected {status}, got {} {}",
            answer.0, answer.1
        ))
    }
}

fn error_code(body: &Value) -> &str {
    body["error"]["code"].as_str().unwrap_or("")
}

/// The Gateway's badge: `(status, count)` where count is `unseen_group_count` (fresh) or
/// `last_known_count` (stale).
fn badge(body: &Value) -> (String, i64) {
    let f = &body["freshness"];
    let status = f["status"].as_str().unwrap_or("").to_string();
    let count = f["unseen_group_count"]
        .as_i64()
        .or_else(|| f["last_known_count"].as_i64())
        .unwrap_or(-1);
    (status, count)
}

fn items(page: &Value) -> Vec<Value> {
    page["items"].as_array().cloned().unwrap_or_default()
}

fn of_type<'a>(page: &'a [Value], kind: &str) -> Vec<&'a Value> {
    page.iter().filter(|item| item["type"] == kind).collect()
}

struct Drill<'r, 'a> {
    run: &'r Run<'a>,
    uid: u32,
    keys: Cell<u64>,
}

impl Drill<'_, '_> {
    /// A fresh command key (UUID-shaped; the scratch database is new each run).
    fn key(&self) -> String {
        let n = self.keys.get() + 1;
        self.keys.set(n);
        format!("019a9b30-0348-7000-8000-{n:012x}")
    }

    fn app(&self, name: &str, principal: &str) -> Result<App, String> {
        let port = lifecycle::free_port()?;
        let log = std::fs::File::create(self.run.log(name)).map_err(|e| e.to_string())?;
        let child = Command::new(lifecycle::binary(self.run.scratch.root, "vink-app"))
            .arg("serve-api")
            .current_dir(self.run.scratch.root)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("DATABASE_URL", &self.run.scratch.app_runtime)
            .env("VINK_PROFILE", "local")
            .env("VINK_BIND", format!("127.0.0.1:{port}"))
            .env("VINK_DEV_PRINCIPAL", principal)
            .env("VINK_NOTIFICATIONS_INBOX_SOCKET", &self.run.inbox)
            .env("VINK_NOTIFICATIONS_SERVER_UID", self.uid.to_string())
            .env("VINK_LOG_FORMAT", "text")
            .env("RUST_LOG", "info,sqlx=warn")
            .stdout(Stdio::null())
            .stderr(log)
            .spawn()
            .map_err(|e| format!("spawn vink-app ({name}): {e}"))?;
        let mut guard = Guard(Some(child));
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            if let Some(Ok(Some(status))) = guard.0.as_mut().map(|c| c.try_wait()) {
                return Err(format!("vink-app ({name}) exited early with {status}"));
            }
            if lifecycle::http_status(port, "/health/live") == Some(200) {
                return Ok(App {
                    guard,
                    port,
                    cookie: format!("__Host-vink_session=gateway-drill-{name}"),
                });
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err(format!("vink-app ({name}) did not start within 30s"))
    }

    fn app_cli(&self, args: &[&str]) -> Result<String, String> {
        self.run.operator(
            "vink-app",
            args,
            ("DATABASE_URL", &self.run.scratch.app_runtime),
        )
    }

    fn count(&self, app: &App) -> Result<(String, i64), String> {
        let body = expect("badge", app.get("/api/v1/me/notifications/count")?, 200)?;
        Ok(badge(&body))
    }

    /// Waits until the Gateway's fresh badge shows `wanted`; returns how long it took.
    fn settle(&self, app: &App, wanted: i64, what: &str) -> Result<Duration, String> {
        let mut last = (String::new(), -1);
        let took = self.run.wait_for(what, SETTLE, || {
            last = self.count(app)?;
            Ok(last == ("fresh".into(), wanted))
        });
        took.map_err(|e| format!("{e} (last badge {last:?})"))
    }

    fn list(&self, app: &App) -> Result<Value, String> {
        expect("inbox page", app.get("/api/v1/me/notifications")?, 200)
    }

    fn comment(&self, app: &App, body: &str) -> Result<String, String> {
        let answer = expect(
            "submit comment",
            app.send(
                "POST",
                &format!("/api/v1/works/{WORK}/discussion/comments"),
                &json!({"target_id": null, "body": body, "command_key": self.key()}),
            )?,
            201,
        )?;
        answer["comment_id"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| format!("submit comment: {answer}"))
    }

    fn moderate(
        &self,
        moderator: &App,
        comment: &str,
        expected: i64,
        action: &str,
    ) -> Result<(), String> {
        let reason =
            (action != "approve").then_some("Lời lẽ công kích cá nhân (dữ liệu kiểm thử).");
        expect(
            &format!("moderation {action}"),
            moderator.send(
                "POST",
                &format!("/api/v1/moderation/comments/{comment}/decisions"),
                &json!({"expected_version": expected, "action": action, "reason": reason}),
            )?,
            200,
        )
        .map(|_| ())
    }

    fn tham(&self, app: &App, endorsed: bool, expected_order: i64) -> Result<(), String> {
        expect(
            "Thấm",
            app.send(
                "PUT",
                &format!("/api/v1/works/{WORK}/endorsement/me"),
                &json!({
                    "command_key": self.key(),
                    "expected_order": expected_order,
                    "endorsed": endorsed,
                    "revision_id": REVISION,
                }),
            )?,
            200,
        )
        .map(|_| ())
    }

    fn snapshot_command(&self, app: &App, verb: &str, snapshot: &Value) -> Result<Value, String> {
        expect(
            verb,
            app.send(
                "POST",
                &format!("/api/v1/me/notifications/{verb}"),
                &json!({"command_key": self.key(), "up_to_snapshot": snapshot}),
            )?,
            200,
        )
    }
}

impl Run<'_> {
    pub(super) fn gateway_end_to_end(&self) -> Result<(), String> {
        let drill = Drill {
            run: self,
            uid: self.uid()?,
            keys: Cell::new(0),
        };
        // App facts: the sample work, two readers, a moderator with a legacy grant, producers on.
        drill.app_cli(&["dev-seed"])?;
        let principals = format!(
            "INSERT INTO principals (id, display_name) VALUES
               ('{READER_A}', 'Bạn đọc A (kiểm thử #348)'),
               ('{READER_B}', 'Bạn đọc B (kiểm thử #348)'),
               ('{MODERATOR}', 'Kiểm duyệt viên (kiểm thử #348)')
             ON CONFLICT (id) DO NOTHING"
        );
        sql(&self.scratch.admin_app, &principals)?;
        drill.app_cli(&[
            "moderator",
            "grant",
            MODERATOR,
            "verify postgres gateway drill",
        ])?;
        drill.app_cli(&["notifications", "activate", "verify postgres gateway drill"])?;

        let uid = drill.uid;
        let service = self.start("gateway-service", Some(uid), Some(uid))?;
        let mut worker = self.start_worker_named(uid, "gateway-worker")?;
        let mut apps = Vec::new();
        let result = (|| {
            for (name, principal) in [
                ("gateway-author", AUTHOR),
                ("gateway-reader-a", READER_A),
                ("gateway-reader-b", READER_B),
                ("gateway-moderator", MODERATOR),
            ] {
                apps.push(drill.app(name, principal)?);
            }
            self.gateway_scenario(&drill, &apps)
        })();
        let mut stopped = Vec::new();
        for mut app in apps {
            stopped.push(app.guard.terminate(Duration::from_secs(15)));
        }
        let worker_status = worker.terminate(Duration::from_secs(15));
        // Leave the gate open whatever happened, so later steps see a serving inbox.
        let _ = self.recovery("end", "gateway drill cleanup");
        self.stop(service)?;
        result?;
        for status in stopped {
            match status {
                Ok(status) if status.success() => {}
                other => return Err(format!("a vink-app did not stop cleanly: {other:?}")),
            }
        }
        match worker_status {
            Ok(status) if status.success() => {}
            other => return Err(format!("vink-worker did not stop cleanly: {other:?}")),
        }
        self.gateway_logs_are_private()
    }

    fn gateway_scenario(&self, drill: &Drill<'_, '_>, apps: &[App]) -> Result<(), String> {
        let [author, reader_a, reader_b, moderator] = apps else {
            return Err("expected four App processes".into());
        };

        // AC-01: the inbox owner comes from the session; no session, no call.
        let guest = author.guest("/api/v1/me/notifications/count")?;
        if guest.0 != 401 || error_code(&guest.1) != "unauthenticated" {
            return Err(format!("guest badge: {} {}", guest.0, guest.1));
        }
        let no_origin = exchange(
            author.port,
            "POST",
            "/api/v1/me/notifications/seen",
            &[("Cookie", &author.cookie)],
            Some(r#"{"command_key":"019a9b30-0348-7000-8000-ffffffffffff","up_to_snapshot":"x"}"#),
        )?;
        if no_origin.0 != 403 {
            return Err(format!(
                "ACK without Origin: {} {}",
                no_origin.0, no_origin.1
            ));
        }
        // An empty inbox is a fresh zero: verified, so it may be shown as none.
        if drill.count(author)? != ("fresh".into(), 0) {
            return Err(format!("empty inbox badge: {:?}", drill.count(author)?));
        }

        // Business commits → App outbox → relay → rollup.
        drill.tham(reader_a, true, 1)?;
        let comment_a = drill.comment(reader_a, BODY_A)?;
        let comment_b = drill.comment(reader_b, BODY_B)?;
        drill.moderate(moderator, &comment_a, 1, "approve")?;
        drill.moderate(moderator, &comment_b, 1, "approve")?;
        let took = drill.settle(author, 2, "Thấm + two approvals reach the Gateway badge")?;
        println!("==> Thấm and two approved comments → badge 2 after {took:?}");

        // #365: an Ưng on reader B's comment reaches B (its writer), never the work's owner.
        expect(
            "Ưng",
            reader_a.send(
                "PUT",
                &format!("/api/v1/works/{WORK}/discussion/comments/{comment_b}/endorsement/me"),
                &json!({"command_key": drill.key(), "expected_order": 1, "endorsed": true}),
            )?,
            200,
        )?;
        drill.settle(reader_b, 1, "an Ưng reaches the comment's writer")?;
        let ung_page = drill.list(reader_b)?;
        let ung_rows = of_type(&items(&ung_page), "comment.endorsement.ung.changed")
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        if ung_rows.len() != 1
            || ung_rows[0]["target"]["kind"] != "work_comment_endorsement"
            || ung_rows[0]["target"]["comment_id"] != comment_b.as_str()
            || ung_rows[0]["actor_count"] != 1
            || ung_rows[0]["summary"]["kind"] != "comment_ung_rollup"
            || ung_rows[0].get("excerpt").is_some()
        {
            return Err(format!("writer's Ưng row: {ung_page}"));
        }

        // AC-02/03: what the Gateway shows is hydrated from current App facts.
        let page = drill.list(author)?;
        let shown = items(&page);
        let comments = of_type(&shown, "comment.root.approved");
        let tham = of_type(&shown, "work.endorsement.tham.changed");
        if shown.len() != 2 || comments.len() != 1 || tham.len() != 1 {
            return Err(format!("first page: {page}"));
        }
        let rollup = comments[0];
        if rollup["actor_count"] != 2
            || rollup["target"]["work_title"] != TITLE
            || rollup["excerpt"]["text"].as_str() != Some(BODY_B)
            || rollup["is_unseen"] != true
            || tham[0]["actor_count"] != 1
        {
            return Err(format!("comment rollup or Thấm group: {page}"));
        }
        let text = page.to_string();
        if [READER_A, READER_B, MODERATOR]
            .iter()
            .any(|id| text.contains(id))
        {
            return Err("the inbox page exposes a principal ID".into());
        }

        // AC-04: Seen up to the page's snapshot, Read by group version.
        drill.snapshot_command(author, "seen", &page["inbox_snapshot"])?;
        if drill.count(author)? != ("fresh".into(), 0) {
            return Err(format!("badge after Seen: {:?}", drill.count(author)?));
        }
        expect(
            "read Thấm group",
            author.send(
                "POST",
                "/api/v1/me/notifications/read",
                &json!({
                    "command_key": drill.key(),
                    "group_id": tham[0]["group_id"],
                    "observed_group_version": tham[0]["group_version"],
                }),
            )?,
            200,
        )?;
        let after_read = items(&drill.list(author)?);
        if of_type(&after_read, "work.endorsement.tham.changed")
            .first()
            .is_none_or(|group| group["is_read"] != true)
        {
            return Err(format!("Thấm group after Read: {after_read:?}"));
        }

        // A signal after Read opens a new group; the read one keeps its history.
        drill.tham(reader_b, true, 1)?;
        drill.settle(author, 1, "a Thấm after Read opens a new unread group")?;
        let page = drill.list(author)?;
        let tham_groups = of_type(&items(&page), "work.endorsement.tham.changed")
            .into_iter()
            .map(|group| (group["is_read"].clone(), group["actor_count"].clone()))
            .collect::<Vec<_>>();
        if tham_groups.len() != 2
            || !tham_groups.contains(&(json!(true), json!(1)))
            || !tham_groups.contains(&(json!(false), json!(1)))
        {
            return Err(format!("Thấm groups after a second reader: {page}"));
        }

        // #347 recovery gate while the badge is 1: never turned into 0 or an empty list.
        self.recovery("begin", "gateway drill: inbox reconciled after a restore")?;
        let listed = author.get("/api/v1/me/notifications")?;
        if listed.0 != 503 || error_code(&listed.1) != "dependency_unavailable" {
            return Err(format!(
                "inbox page while recovering: {} {}",
                listed.0, listed.1
            ));
        }
        let stale = expect(
            "badge while recovering",
            author.get("/api/v1/me/notifications/count")?,
            200,
        )?;
        if badge(&stale) != ("stale".into(), 1) {
            return Err(format!("remembered badge while recovering: {stale}"));
        }
        let unknown = reader_a.get("/api/v1/me/notifications/count")?;
        if unknown.0 != 503 || error_code(&unknown.1) != "dependency_unavailable" {
            return Err(format!(
                "badge without a remembered count while recovering: {} {}",
                unknown.0, unknown.1
            ));
        }
        let ack = author.send(
            "POST",
            "/api/v1/me/notifications/mark-all-read",
            &json!({"command_key": drill.key(), "up_to_snapshot": page["inbox_snapshot"]}),
        )?;
        if ack.0 != 503 {
            return Err(format!("ACK while recovering: {} {}", ack.0, ack.1));
        }
        self.recovery("end", "gateway drill: reconciled")?;
        if drill.count(author)? != ("fresh".into(), 1) {
            return Err(format!("badge after recovery: {:?}", drill.count(author)?));
        }

        // AC-05: the public Thấm list needs no session and reads the Endorsement owner.
        let endorsers = expect(
            "public Thấm list",
            author.guest(&format!(
                "/api/v1/works/{WORK}/revisions/{REVISION}/endorsers"
            ))?,
            200,
        )?;
        if endorsers["total_count"] != 2 || items(&endorsers).len() != 2 {
            return Err(format!("public Thấm list: {endorsers}"));
        }
        let marked = drill.snapshot_command(author, "mark-all-read", &page["inbox_snapshot"])?;
        if items(&drill.list(author)?)
            .iter()
            .any(|group| group["is_read"] != true)
            || drill.count(author)? != ("fresh".into(), 0)
        {
            return Err(format!("after MarkAllRead ({marked}): not all read"));
        }

        // Loss of permission: a removed comment leaves the rollup; its writer gets a decision.
        drill.moderate(moderator, &comment_b, 2, "remove")?;
        self.wait_for("the removed comment leaves the rollup", SETTLE, || {
            let page = drill.list(author)?;
            Ok(of_type(&items(&page), "comment.root.approved")
                .first()
                .is_some_and(|group| {
                    group["actor_count"] == 1 && group["excerpt"]["text"].as_str() == Some(BODY_A)
                }))
        })?;
        // The writer gets the adverse decision; the Ưng row of the removed comment disappears.
        let mut writer_page = Value::Null;
        self.wait_for(
            "the writer's inbox holds only the adverse decision",
            SETTLE,
            || {
                writer_page = drill.list(reader_b)?;
                let rows = items(&writer_page);
                Ok(rows.len() == 1 && rows[0]["type"] == "comment.moderation.adverse")
            },
        )
        .map_err(|e| format!("{e}: {writer_page}"))?;

        // Loss of permission: the work set private hides every interaction group.
        let state = expect(
            "author publication state",
            author.get(&format!("/api/v1/works/{WORK}/publication/state"))?,
            200,
        )?;
        let generation = state["generation"]
            .as_i64()
            .ok_or_else(|| format!("publication state: {state}"))?;
        expect(
            "set private",
            author.send(
                "POST",
                &format!("/api/v1/works/{WORK}/publication/set-private"),
                &json!({"command_key": drill.key(), "expected_generation": generation}),
            )?,
            200,
        )?;
        let hidden = drill.list(author)?;
        if !items(&hidden).is_empty() {
            return Err(format!(
                "groups of a private work are still shown: {hidden}"
            ));
        }

        // AC-01: a suspended principal's commands are fenced before reaching the service.
        drill.app_cli(&["principal", "suspend", READER_B, "Kiểm thử đình chỉ #348"])?;
        let suspended = reader_b.send(
            "POST",
            "/api/v1/me/notifications/seen",
            &json!({"command_key": drill.key(), "up_to_snapshot": writer_page["inbox_snapshot"]}),
        )?;
        if suspended.0 != 403 || error_code(&suspended.1) != "principal_suspended" {
            return Err(format!(
                "Seen by a suspended principal: {} {}",
                suspended.0, suspended.1
            ));
        }
        if drill.count(reader_b)?.1 != 1 {
            return Err("a fenced Seen still reached the service".into());
        }
        // The last commands (set private, suspension) owe invalidations too: every App
        // obligation drains through the relay.
        let mut backlog = String::new();
        self.wait_for("every App delivery succeeded", SETTLE, || {
            backlog = query_text(
                &self.scratch.admin_app,
                "SELECT count(*) FILTER (WHERE state <> 'succeeded')::text || '/' || count(*)::text
                   FROM notification_deliveries",
            )?;
            Ok(backlog.starts_with("0/"))
        })
        .map_err(|e| format!("{e} (last {backlog})"))?;
        println!(
            "==> Gateway over real processes: badge/list/Seen/Read/MarkAllRead; Ưng to the writer; recovering → 503 and stale 1 (never 0); removal and private work filtered; suspended Seen → 403; deliveries {backlog} succeeded"
        );
        Ok(())
    }

    /// No App or service log of the drill carries a comment body or a reader's principal ID.
    fn gateway_logs_are_private(&self) -> Result<(), String> {
        for name in [
            "gateway-author",
            "gateway-reader-a",
            "gateway-reader-b",
            "gateway-moderator",
            "gateway-service",
            "gateway-worker",
        ] {
            let log = std::fs::read_to_string(self.log(name)).map_err(|e| e.to_string())?;
            for needle in [BODY_A, BODY_B, READER_A, READER_B] {
                if log.contains(needle) {
                    return Err(format!(
                        "{name}.log contains a comment body or principal ID"
                    ));
                }
            }
        }
        Ok(())
    }
}
