//! Notifications through the App Gateway (#348) on real PostgreSQL: authorization and hydration
//! of inbox groups against current App facts, bounded scans with finite continuations,
//! session-bound tokens, the standing fence of ACK commands, fresh/stale/unavailable counts, the
//! private decision details and the public Thấm list. The Notification Service is replaced by an
//! in-process fake of its socket contract (the App never links the service crate); the real
//! socket client is covered by `notification_inbox` unit tests.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use vink_app::application::discussion::{ModerateComment, ModerationAction, SubmitComment};
use vink_app::application::endorsement::SetWorkEndorsement;
use vink_app::domain::discussion::moderation::GrantNote;
use vink_app::domain::ids::CommentId;
use vink_app::domain::revision::RevisionContent;
use vink_app::notification_inbox::{
    InboxCommand, InboxFailure, InboxFuture, NotificationInbox, WireCount, WireGroup, WirePage,
};
use vink_app::persistence::grant_moderator;

use super::*;

const MODERATOR: Uuid = Uuid::from_u128(0x6666_6666_6666_4666_8666_6666_6666_0348);
const SESSION: (&str, &str) = ("cookie", "__Host-vink_session=notif-session-one");
const SESSION_B: (&str, &str) = ("cookie", "__Host-vink_session=notif-session-two");
const R1: Uuid = dev_seed::SAMPLE_REVISION;

fn reader_id(n: u128) -> Uuid {
    Uuid::from_u128(0x7777_7777_7777_4777_8777_0348_0000_0000 + n)
}

/// The service's inbox contract, in memory: groups per recipient, newest signal first, keyset
/// cursor `c<seq>.<uuid>`, recorded commands, and an outage switch.
#[derive(Default)]
struct FakeInbox {
    groups: Mutex<Vec<(Uuid, Value)>>,
    commands: Mutex<Vec<(Uuid, String, InboxCommand)>>,
    pages: AtomicUsize,
    down: AtomicBool,
    seen: Mutex<i64>,
}

impl FakeInbox {
    fn add(&self, recipient: Uuid, group: Value) {
        self.groups.lock().unwrap().push((recipient, group));
    }
    fn last_seq(&self) -> i64 {
        let groups = self.groups.lock().unwrap();
        groups
            .iter()
            .map(|(_, g)| g["signal_version"].as_i64().unwrap())
            .max()
            .unwrap_or(0)
    }
}

impl NotificationInbox for FakeInbox {
    fn count(&self, recipient: PrincipalId) -> InboxFuture<'_, WireCount> {
        let down = self.down.load(Ordering::SeqCst);
        let seen = *self.seen.lock().unwrap();
        let unseen = self
            .groups
            .lock()
            .unwrap()
            .iter()
            .filter(|(r, g)| {
                *r == recipient.as_uuid() && g["signal_version"].as_i64().unwrap() > seen
            })
            .count() as i64;
        let last = self.last_seq();
        Box::pin(async move {
            if down {
                return Err(InboxFailure::Unavailable);
            }
            Ok(WireCount {
                status: "fresh".into(),
                unseen_group_count: unseen,
                as_of_snapshot: format!("s{last}"),
                max_staleness_seconds: 60,
            })
        })
    }

    fn page<'a>(
        &'a self,
        recipient: PrincipalId,
        limit: u8,
        cursor: Option<&'a str>,
    ) -> InboxFuture<'a, WirePage> {
        self.pages.fetch_add(1, Ordering::SeqCst);
        let down = self.down.load(Ordering::SeqCst);
        let seen = *self.seen.lock().unwrap();
        let mut mine: Vec<Value> = self
            .groups
            .lock()
            .unwrap()
            .iter()
            .filter(|(r, _)| *r == recipient.as_uuid())
            .map(|(_, g)| g.clone())
            .collect();
        let last = self.last_seq();
        let after = cursor.map(|c| {
            let (seq, id) = c.strip_prefix('c').unwrap().split_once('.').unwrap();
            (seq.parse::<i64>().unwrap(), Uuid::parse_str(id).unwrap())
        });
        Box::pin(async move {
            if down {
                return Err(InboxFailure::Unavailable);
            }
            let key = |g: &Value| {
                (
                    g["signal_version"].as_i64().unwrap(),
                    Uuid::parse_str(g["group_id"].as_str().unwrap()).unwrap(),
                )
            };
            mine.sort_by_key(|g| std::cmp::Reverse(key(g)));
            let mut rest: Vec<Value> = mine
                .into_iter()
                .filter(|g| after.is_none_or(|a| key(g) < a))
                .collect();
            for g in &mut rest {
                g["unseen"] = json!(g["signal_version"].as_i64().unwrap() > seen);
            }
            let more = rest.len() > usize::from(limit);
            rest.truncate(usize::from(limit));
            let next_cursor = more.then(|| {
                let (seq, id) = key(rest.last().unwrap());
                format!("c{seq}.{id}")
            });
            let groups: Vec<WireGroup> = rest
                .into_iter()
                .map(|g| serde_json::from_value(g).unwrap())
                .collect();
            Ok(WirePage {
                inbox_snapshot: format!("s{last}"),
                seen_snapshot: format!("s{seen}"),
                groups,
                next_cursor,
            })
        })
    }

    fn command<'a>(
        &'a self,
        recipient: PrincipalId,
        command_key: &'a str,
        command: &'a InboxCommand,
    ) -> InboxFuture<'a, Value> {
        self.commands.lock().unwrap().push((
            recipient.as_uuid(),
            command_key.to_string(),
            command.clone(),
        ));
        let answer = match command {
            InboxCommand::Seen { up_to } => {
                let mut seen = self.seen.lock().unwrap();
                *seen = (*seen).max(*up_to);
                json!({ "seen_snapshot": format!("s{}", *seen) })
            }
            InboxCommand::Read {
                group_id,
                observed_version,
            } => {
                json!({ "group_id": group_id, "group_version": observed_version + 1, "unread": false })
            }
            InboxCommand::MarkAllRead { up_to } => {
                json!({ "up_to_snapshot": format!("s{up_to}"), "groups_marked": 1 })
            }
        };
        Box::pin(async move { Ok(answer) })
    }
}

#[allow(clippy::too_many_arguments)]
fn group(
    seq: i64,
    event_type: &str,
    target: Value,
    actors: &[Uuid],
    contributions: Option<&[(Uuid, Uuid)]>,
    epoch: Option<i64>,
    action_required: bool,
) -> Value {
    let counted = matches!(
        event_type,
        "comment.root.approved"
            | "comment.reply.approved"
            | "work.endorsement.tham.changed"
            | "comment.endorsement.ung.changed"
    );
    json!({
        "group_id": Uuid::from_u128(0x019a_9b20_0000_7000_8000_0000_0000_0000 + seq as u128),
        "event_type": event_type,
        "target": target,
        "group_version": seq,
        "signal_version": seq,
        "unread": true,
        "unseen": true,
        "actor_count": if counted { actors.len() as i32 } else { 0 },
        "item_count": if counted { 0 } else { 1 },
        "actors_preview": actors.iter().take(3).collect::<Vec<_>>(),
        "contributions": contributions.map(|c| json!({
            "newest": c.iter().map(|(comment, actor)| json!({"comment_id": comment, "actor_id": actor})).collect::<Vec<_>>(),
            "total": c.len(),
        })),
        "action_required": action_required,
        "publication_epoch": epoch,
        "created_at": "2026-11-20T02:00:00Z",
        "last_signal_at": "2026-11-20T02:10:00Z",
        "expires_at": "2027-05-19T02:00:00Z",
    })
}

fn thread(comment: Uuid) -> Value {
    json!({"kind": "work_comment_thread", "work_id": dev_seed::SAMPLE_WORK.to_string(),
           "comment_id": comment.to_string(), "moderation_version": 2})
}

fn tham_target() -> Value {
    json!({"kind": "work_endorsement", "work_id": dev_seed::SAMPLE_WORK.to_string(),
           "revision_id": R1.to_string()})
}

struct Fixture {
    pool: PgPool,
    vink: Arc<Vink<PgStore>>,
    inbox: Arc<FakeInbox>,
    app: Router,
    epoch: i64,
}

async fn fixture(pool: PgPool) -> Fixture {
    let vink = Arc::new(seeded(&pool).await);
    sqlx::query("INSERT INTO principals (id, display_name) VALUES ($1, 'Kiểm duyệt viên #348')")
        .bind(MODERATOR)
        .execute(&pool)
        .await
        .unwrap();
    for n in 1..=8 {
        sqlx::query("INSERT INTO principals (id, display_name) VALUES ($1, $2)")
            .bind(reader_id(n))
            .bind(format!("Bạn đọc {n} (dữ liệu kiểm thử)"))
            .execute(&pool)
            .await
            .unwrap();
    }
    let note = GrantNote::parse("Kiểm duyệt viên của bộ test #348").unwrap();
    grant_moderator(&pool, PrincipalId::from_uuid(MODERATOR), &note)
        .await
        .unwrap();
    let epoch: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT public_epoch FROM work_publications WHERE work_id = $1), 0)",
    )
    .bind(dev_seed::SAMPLE_WORK)
    .fetch_one(&pool)
    .await
    .unwrap();
    let inbox = Arc::new(FakeInbox::default());
    let app = router(
        AppState::new(
            Vink::new(PgStore::new(pool.clone())),
            Arc::new(HeaderIdentity),
            ORIGIN,
            std::path::Path::new("target/site"),
        )
        .with_notification_inbox(Some(inbox.clone() as Arc<dyn NotificationInbox>)),
    );
    Fixture {
        pool,
        vink,
        inbox,
        app,
        epoch,
    }
}

async fn comment(f: &Fixture, writer: Uuid, body: &str, action: Option<ModerationAction>) -> Uuid {
    let id = f
        .vink
        .submit_comment(
            principal(writer),
            SubmitComment {
                work_id: sample_work(),
                target: None,
                body_format: Default::default(),
                body: body.into(),
                command_key: format!("k-{}", Uuid::now_v7().simple()),
            },
        )
        .await
        .unwrap()
        .comment_id;
    if let Some(action) = action {
        moderate(f, id, 1, action).await;
    }
    id.as_uuid()
}

async fn moderate(f: &Fixture, id: CommentId, expected: i64, action: ModerationAction) {
    let reason = (action != ModerationAction::Approve)
        .then(|| "Lời lẽ công kích cá nhân (dữ liệu kiểm thử).".to_string());
    f.vink
        .moderate_comment(
            principal(MODERATOR),
            ModerateComment {
                comment_id: id,
                expected_version: expected,
                action,
                reason,
            },
        )
        .await
        .unwrap();
}

struct Answer {
    status: StatusCode,
    cache: Option<String>,
    etag: Option<String>,
    body: Value,
}

#[allow(clippy::too_many_arguments)]
async fn send(
    app: &Router,
    method: &str,
    path: &str,
    actor: Option<Uuid>,
    session: Option<(&str, &str)>,
    origin: bool,
    body: Option<Value>,
    if_none_match: Option<&str>,
) -> Answer {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(actor) = actor {
        request = request.header("x-test-principal", actor.to_string());
    }
    if let Some((name, value)) = session {
        request = request.header(name, value);
    }
    if origin {
        request = request.header(header::ORIGIN, ORIGIN);
    }
    if let Some(tag) = if_none_match {
        request = request.header(header::IF_NONE_MATCH, tag);
    }
    let request = match body {
        Some(body) => request
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string())),
        None => request.body(Body::empty()),
    }
    .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let header = |name| {
        response
            .headers()
            .get(name)
            .map(|v: &header::HeaderValue| v.to_str().unwrap().to_string())
    };
    let cache = header(header::CACHE_CONTROL);
    let etag = header(header::ETAG);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    Answer {
        status,
        cache,
        etag,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    }
}

async fn get_as(f: &Fixture, path: &str, actor: Uuid, session: (&str, &str)) -> Answer {
    send(
        &f.app,
        "GET",
        path,
        Some(actor),
        Some(session),
        false,
        None,
        None,
    )
    .await
}

async fn post_as(
    f: &Fixture,
    path: &str,
    actor: Uuid,
    session: (&str, &str),
    body: Value,
) -> Answer {
    send(
        &f.app,
        "POST",
        path,
        Some(actor),
        Some(session),
        true,
        Some(body),
        None,
    )
    .await
}

const OWNER: Uuid = dev_seed::SAMPLE_AUTHOR;

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn the_inbox_shows_only_what_the_viewer_may_see_now(pool: PgPool) {
    let f = fixture(pool).await;
    let (r1, r2, r3) = (reader_id(1), reader_id(2), reader_id(3));
    let c1 = comment(
        &f,
        r1,
        "Câu thơ\n\nthứ  hai làm tôi nhớ mãi",
        Some(ModerationAction::Approve),
    )
    .await;
    let c2 = comment(&f, r2, "Bị gỡ sau đó", Some(ModerationAction::Approve)).await;
    moderate(&f, CommentId::from_uuid(c2), 2, ModerationAction::Remove).await;
    let c3 = comment(&f, r3, "Đang chờ duyệt", None).await;
    let epoch = f.epoch;
    // Root group whose newest contribution was removed (its invalidation not delivered yet) and
    // one pending comment the service could never have had: falls back to c1.
    f.inbox.add(
        OWNER,
        group(
            10,
            "comment.root.approved",
            thread(c2),
            &[r2, r3, r1],
            Some(&[(c2, r2), (c3, r3), (c1, r1)]),
            Some(epoch),
            false,
        ),
    );
    f.inbox.add(
        OWNER,
        group(
            9,
            "work.endorsement.tham.changed",
            tham_target(),
            &[r1, r2],
            None,
            Some(epoch),
            false,
        ),
    );
    // A signal stamped in another epoch (COM-11) and a reserved admin type: never shown.
    f.inbox.add(
        OWNER,
        group(
            8,
            "comment.root.approved",
            thread(c1),
            &[r1],
            Some(&[(c1, r1)]),
            Some(epoch + 1),
            false,
        ),
    );
    f.inbox.add(
        OWNER,
        group(
            7,
            "admin.notice.reserved",
            json!({"kind": "admin_notice_reserved", "notice_id": Uuid::now_v7().to_string()}),
            &[],
            None,
            None,
            false,
        ),
    );
    let page = get_as(&f, "/api/v1/me/notifications", OWNER, SESSION).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert_eq!(page.cache.as_deref(), Some("no-store"));
    let items = page.body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2, "{}", page.body);
    let root = &items[0];
    assert_eq!(root["target"]["comment_id"], c1.to_string());
    assert_eq!(root["target"]["work_title"], dev_seed::SAMPLE_TITLE);
    assert_eq!(root["excerpt"]["text"], "Câu thơ thứ hai làm tôi nhớ mãi");
    assert_eq!(root["actor_count"], 1, "only r1 still has a valid comment");
    assert_eq!(root["actors_preview"].as_array().unwrap().len(), 1);
    let rendered = page.body.to_string();
    assert!(!rendered.contains("Bị gỡ sau đó") && !rendered.contains("Đang chờ duyệt"));
    assert!(
        !rendered.contains(&r2.to_string()),
        "no principal id reaches the client"
    );
    assert_eq!(items[1]["type"], "work.endorsement.tham.changed");
    assert_eq!(page.body["next_cursor"], Value::Null);
    // The badge counts only what may be shown (2 of 4 unseen candidates).
    let count = get_as(&f, "/api/v1/me/notifications/count", OWNER, SESSION).await;
    assert_eq!(count.body["freshness"]["status"], "fresh");
    assert_eq!(count.body["freshness"]["unseen_group_count"], 2);
    // Another principal reads its own (empty) inbox: never the owner's groups.
    let other = get_as(&f, "/api/v1/me/notifications", r1, SESSION).await;
    assert_eq!(other.body["items"], json!([]));
    // The work stops being public: interaction rows disappear; a decision row stays private.
    f.inbox.add(
        OWNER,
        group(
            11,
            "work.moderation.decided",
            json!({"kind": "work_moderation_decision", "work_id": dev_seed::SAMPLE_WORK.to_string(),
                     "revision_id": R1.to_string(), "decision_version": 1}),
            &[],
            None,
            None,
            false,
        ),
    );
    sqlx::query("DELETE FROM publications WHERE work_id = $1")
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&f.pool)
        .await
        .unwrap();
    let hidden = get_as(&f, "/api/v1/me/notifications", OWNER, SESSION).await;
    let items = hidden.body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{}", hidden.body);
    assert_eq!(items[0]["type"], "work.moderation.decided");
    assert_eq!(items[0]["target"]["work_title"], dev_seed::SAMPLE_TITLE);
    assert!(!hidden.body.to_string().contains("Câu thơ"));
    // The same decision row is never another principal's.
    f.inbox.add(
        r1,
        group(
            12,
            "work.moderation.decided",
            items[0]["target"].clone(),
            &[],
            None,
            None,
            false,
        ),
    );
    let foreign = get_as(&f, "/api/v1/me/notifications", r1, SESSION).await;
    assert_eq!(foreign.body["items"], json!([]));
}

fn ung_target(comment: Uuid) -> Value {
    json!({"kind": "work_comment_endorsement", "work_id": dev_seed::SAMPLE_WORK.to_string(),
           "comment_id": comment.to_string()})
}

/// #365 AC-05/06: an Ưng row is hydrated for the comment's writer only, while the comment is
/// public on a readable work; held/removed comments and other viewers never see it, and no
/// excerpt or principal ID reaches the client.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn ung_rows_are_the_writers_while_the_comment_is_public(pool: PgPool) {
    let f = fixture(pool).await;
    let (writer, a, b) = (reader_id(1), reader_id(2), reader_id(3));
    let kept = comment(
        &f,
        writer,
        "Nhận xét được Ưng",
        Some(ModerationAction::Approve),
    )
    .await;
    let removed = comment(
        &f,
        writer,
        "Nhận xét sẽ bị gỡ",
        Some(ModerationAction::Approve),
    )
    .await;
    let epoch = f.epoch;
    for (seq, target) in [(20, kept), (21, removed)] {
        f.inbox.add(
            writer,
            group(
                seq,
                "comment.endorsement.ung.changed",
                ung_target(target),
                &[b, a],
                None,
                Some(epoch),
                false,
            ),
        );
    }
    // The same row addressed to the work's owner (a contract violation) is never shown.
    f.inbox.add(
        OWNER,
        group(
            22,
            "comment.endorsement.ung.changed",
            ung_target(kept),
            &[a],
            None,
            Some(epoch),
            false,
        ),
    );
    let page = get_as(&f, "/api/v1/me/notifications", writer, SESSION).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    let items = page.body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2, "{}", page.body);
    let row = &items[1];
    assert_eq!(row["type"], "comment.endorsement.ung.changed");
    assert_eq!(row["target"]["kind"], "work_comment_endorsement");
    assert_eq!(row["target"]["comment_id"], kept.to_string());
    assert_eq!(row["target"]["work_title"], dev_seed::SAMPLE_TITLE);
    assert_eq!(row["actor_count"], 2);
    assert_eq!(row["summary"]["kind"], "comment_ung_rollup");
    assert!(row.get("excerpt").is_none(), "{row}");
    let rendered = page.body.to_string();
    assert!(!rendered.contains("Nhận xét được Ưng") && !rendered.contains(&a.to_string()));
    let count = get_as(&f, "/api/v1/me/notifications/count", writer, SESSION).await;
    assert_eq!(count.body["freshness"]["unseen_group_count"], 2);

    // A removed comment hides its Ưng row at once (no invalidation needed for this type).
    moderate(
        &f,
        CommentId::from_uuid(removed),
        2,
        ModerationAction::Remove,
    )
    .await;
    let page = get_as(&f, "/api/v1/me/notifications", writer, SESSION).await;
    let items = page.body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{}", page.body);
    assert_eq!(items[0]["target"]["comment_id"], kept.to_string());
    let owner = get_as(&f, "/api/v1/me/notifications", OWNER, SESSION).await;
    assert_eq!(owner.body["items"], json!([]));

    // The work stops being public: the Ưng row disappears too.
    sqlx::query("DELETE FROM publications WHERE work_id = $1")
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&f.pool)
        .await
        .unwrap();
    let hidden = get_as(&f, "/api/v1/me/notifications", writer, SESSION).await;
    assert_eq!(hidden.body["items"], json!([]), "{}", hidden.body);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_densely_filtered_inbox_continues_with_a_finite_cursor(pool: PgPool) {
    let f = fixture(pool).await;
    let r1 = reader_id(1);
    let c1 = comment(&f, r1, "Hợp lệ", Some(ModerationAction::Approve)).await;
    // One valid group at the bottom under 160 groups of an older epoch.
    f.inbox.add(
        OWNER,
        group(
            1,
            "comment.root.approved",
            thread(c1),
            &[r1],
            Some(&[(c1, r1)]),
            Some(f.epoch),
            false,
        ),
    );
    for seq in 2..=161 {
        f.inbox.add(
            OWNER,
            group(
                seq,
                "comment.root.approved",
                thread(c1),
                &[r1],
                Some(&[(c1, r1)]),
                Some(f.epoch + 1),
                false,
            ),
        );
    }
    let first = get_as(&f, "/api/v1/me/notifications?limit=20", OWNER, SESSION).await;
    assert_eq!(first.status, StatusCode::OK);
    assert_eq!(first.body["items"], json!([]));
    assert_eq!(
        first.body["continuation_reason"],
        "filtered_page_scan_bound"
    );
    assert_eq!(
        f.inbox.pages.load(Ordering::SeqCst),
        3,
        "bounded scan: three service batches"
    );
    let cursor = first.body["next_cursor"].as_str().unwrap().to_string();
    // The cursor belongs to this session and principal only.
    let path = format!("/api/v1/me/notifications?cursor={cursor}");
    assert_eq!(
        get_as(&f, &path, OWNER, SESSION_B).await.status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        get_as(&f, &path, r1, SESSION).await.status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let second = get_as(&f, &path, OWNER, SESSION).await;
    let items = second.body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{}", second.body);
    assert_eq!(items[0]["target"]["comment_id"], c1.to_string());
    assert_eq!(second.body["next_cursor"], Value::Null);
    assert_eq!(f.inbox.pages.load(Ordering::SeqCst), 4);
    // The badge stops after two batches and says its count is a lower bound.
    let count = get_as(&f, "/api/v1/me/notifications/count", OWNER, SESSION).await;
    assert_eq!(count.body["freshness"]["count_is_lower_bound"], true);
    assert_eq!(count.body["freshness"]["unseen_group_count"], 0);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn acks_are_bound_to_the_session_and_fenced_by_standing(pool: PgPool) {
    let f = fixture(pool).await;
    f.inbox.add(
        OWNER,
        group(
            5,
            "work.endorsement.tham.changed",
            tham_target(),
            &[reader_id(1)],
            None,
            Some(f.epoch),
            false,
        ),
    );
    let page = get_as(&f, "/api/v1/me/notifications", OWNER, SESSION).await;
    let snapshot = page.body["inbox_snapshot"].as_str().unwrap().to_string();
    let group_id = page.body["items"][0]["group_id"].clone();
    let seen = json!({"command_key": "seen-1", "up_to_snapshot": snapshot});
    let ok = post_as(
        &f,
        "/api/v1/me/notifications/seen",
        OWNER,
        SESSION,
        seen.clone(),
    )
    .await;
    assert_eq!(ok.status, StatusCode::OK, "{}", ok.body);
    assert!(
        ok.body["seen_snapshot"]
            .as_str()
            .unwrap()
            .starts_with("s1.5.")
    );
    assert_eq!(
        f.inbox.commands.lock().unwrap().last().unwrap().2,
        InboxCommand::Seen { up_to: 5 }
    );
    // A token of another session or principal never reaches the service (A → B → A).
    let before = f.inbox.commands.lock().unwrap().len();
    for (actor, session) in [(OWNER, SESSION_B), (reader_id(1), SESSION)] {
        let refused = post_as(
            &f,
            "/api/v1/me/notifications/seen",
            actor,
            session,
            seen.clone(),
        )
        .await;
        assert_eq!(refused.status, StatusCode::UNPROCESSABLE_ENTITY);
    }
    let forged = json!({"command_key": "seen-2", "up_to_snapshot": "s5"});
    assert_eq!(
        post_as(&f, "/api/v1/me/notifications/seen", OWNER, SESSION, forged)
            .await
            .status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(f.inbox.commands.lock().unwrap().len(), before);
    // Read passes the observed version; mark-all-read the authenticated snapshot.
    let read = post_as(
        &f,
        "/api/v1/me/notifications/read",
        OWNER,
        SESSION,
        json!({"command_key": "read-1", "group_id": group_id, "observed_group_version": 5}),
    )
    .await;
    assert_eq!(read.status, StatusCode::OK);
    assert_eq!(read.body["is_read"], true);
    let all = post_as(
        &f,
        "/api/v1/me/notifications/mark-all-read",
        OWNER,
        SESSION,
        json!({"command_key": "all-1", "up_to_snapshot": snapshot}),
    )
    .await;
    assert_eq!(all.status, StatusCode::OK);
    assert_eq!(
        f.inbox.commands.lock().unwrap().last().unwrap().2,
        InboxCommand::MarkAllRead { up_to: 5 }
    );
    // Guest, missing Origin, and a suspended principal: nothing reaches the service.
    let before = f.inbox.commands.lock().unwrap().len();
    let guest = send(
        &f.app,
        "POST",
        "/api/v1/me/notifications/seen",
        None,
        None,
        true,
        Some(json!({"command_key": "g", "up_to_snapshot": snapshot})),
        None,
    )
    .await;
    assert_eq!(guest.status, StatusCode::UNAUTHORIZED);
    let no_origin = send(
        &f.app,
        "POST",
        "/api/v1/me/notifications/seen",
        Some(OWNER),
        Some(SESSION),
        false,
        Some(json!({"command_key": "o", "up_to_snapshot": snapshot})),
        None,
    )
    .await;
    assert_eq!(no_origin.status, StatusCode::FORBIDDEN);
    sqlx::query("INSERT INTO principal_suspensions (id, principal_id, reason) VALUES ($1, $2, 'Kiểm thử #348')")
        .bind(Uuid::now_v7())
        .bind(OWNER)
        .execute(&f.pool)
        .await
        .unwrap();
    let suspended = post_as(
        &f,
        "/api/v1/me/notifications/seen",
        OWNER,
        SESSION,
        json!({"command_key": "s", "up_to_snapshot": snapshot}),
    )
    .await;
    assert_eq!(suspended.status, StatusCode::FORBIDDEN);
    assert_eq!(suspended.body["error"]["code"], "principal_suspended");
    assert_eq!(f.inbox.commands.lock().unwrap().len(), before);
    // Private reads need a session: a guest is 401 before any service call.
    let pages = f.inbox.pages.load(Ordering::SeqCst);
    let guest = send(
        &f.app,
        "GET",
        "/api/v1/me/notifications",
        None,
        None,
        false,
        None,
        None,
    )
    .await;
    assert_eq!(guest.status, StatusCode::UNAUTHORIZED);
    assert_eq!(guest.cache.as_deref(), Some("no-store"));
    assert_eq!(f.inbox.pages.load(Ordering::SeqCst), pages);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn the_badge_is_fresh_stale_or_unavailable_and_never_a_false_zero(pool: PgPool) {
    let f = fixture(pool).await;
    f.inbox.add(
        OWNER,
        group(
            3,
            "work.endorsement.tham.changed",
            tham_target(),
            &[reader_id(1)],
            None,
            Some(f.epoch),
            false,
        ),
    );
    let fresh = get_as(&f, "/api/v1/me/notifications/count", OWNER, SESSION).await;
    assert_eq!(fresh.body["freshness"]["unseen_group_count"], 1);
    f.inbox.down.store(true, Ordering::SeqCst);
    let stale = get_as(&f, "/api/v1/me/notifications/count", OWNER, SESSION).await;
    assert_eq!(stale.status, StatusCode::OK);
    assert_eq!(stale.body["freshness"]["status"], "stale");
    assert_eq!(stale.body["freshness"]["last_known_count"], 1);
    // No verified count for this principal: an outage, not zero.
    let none = get_as(&f, "/api/v1/me/notifications/count", reader_id(2), SESSION).await;
    assert_eq!(none.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(none.body["error"]["code"], "dependency_unavailable");
    let list = get_as(&f, "/api/v1/me/notifications", OWNER, SESSION).await;
    assert_eq!(list.status, StatusCode::SERVICE_UNAVAILABLE);
    // A Gateway without an inbox transport answers 503 too.
    let bare = router(AppState::new(
        Vink::new(PgStore::new(f.pool.clone())),
        Arc::new(HeaderIdentity),
        ORIGIN,
        std::path::Path::new("target/site"),
    ));
    let answer = send(
        &bare,
        "GET",
        "/api/v1/me/notifications/count",
        Some(OWNER),
        Some(SESSION),
        false,
        None,
        None,
    )
    .await;
    assert_eq!(answer.status, StatusCode::SERVICE_UNAVAILABLE);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn decision_details_are_private_to_the_owner_and_the_writer(pool: PgPool) {
    let f = fixture(pool).await;
    let (r1, r2) = (reader_id(1), reader_id(2));
    let rejected = comment(&f, r1, "Bị từ chối", Some(ModerationAction::Reject)).await;
    let removed = comment(
        &f,
        r2,
        "Thân bị gỡ không được trả lại",
        Some(ModerationAction::Approve),
    )
    .await;
    moderate(
        &f,
        CommentId::from_uuid(removed),
        2,
        ModerationAction::Remove,
    )
    .await;
    let path = |c: Uuid, v: i64| format!("/api/v1/me/notifications/decisions/comments/{c}/{v}");
    let own = get_as(&f, &path(rejected, 2), r1, SESSION).await;
    assert_eq!(own.status, StatusCode::OK, "{}", own.body);
    assert_eq!(own.body["outcome"], "rejected");
    assert_eq!(own.body["comment_body"], "Bị từ chối");
    assert_eq!(
        own.body["reason"]["message"],
        "Lời lẽ công kích cá nhân (dữ liệu kiểm thử)."
    );
    assert_eq!(own.body["is_current"], true);
    assert!(!own.body.to_string().contains(&MODERATOR.to_string()));
    let gone = get_as(&f, &path(removed, 3), r2, SESSION).await;
    assert_eq!(gone.body["outcome"], "removed");
    assert_eq!(gone.body["comment_body"], Value::Null);
    assert!(!gone.body.to_string().contains("Thân bị gỡ"));
    // The approval version of the same comment is not an adverse decision.
    assert_eq!(
        get_as(&f, &path(removed, 2), r2, SESSION).await.status,
        StatusCode::NOT_FOUND
    );
    // The work's owner, another reader and a guest get the same 404.
    for actor in [OWNER, r2] {
        assert_eq!(
            get_as(&f, &path(rejected, 2), actor, SESSION).await.status,
            StatusCode::NOT_FOUND
        );
    }
    let guest = send(
        &f.app,
        "GET",
        &path(rejected, 2),
        None,
        None,
        false,
        None,
        None,
    )
    .await;
    assert_eq!(guest.status, StatusCode::UNAUTHORIZED);
    // Work decisions: only the owner, at an existing concluded version.
    let version: Option<i64> = sqlx::query_scalar(
        "SELECT max(decision_version) FROM work_revision_decision_history
         WHERE work_id = $1 AND revision_id = $2 AND moderation IN ('approved', 'rejected')",
    )
    .bind(dev_seed::SAMPLE_WORK)
    .bind(R1)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    let work = |v: i64| {
        format!(
            "/api/v1/me/notifications/decisions/works/{}/revisions/{R1}/{v}",
            dev_seed::SAMPLE_WORK
        )
    };
    if let Some(version) = version {
        let detail = get_as(&f, &work(version), OWNER, SESSION).await;
        assert_eq!(detail.status, StatusCode::OK, "{}", detail.body);
        assert_eq!(detail.body["work_title"], dev_seed::SAMPLE_TITLE);
        assert_eq!(
            get_as(&f, &work(version), r1, SESSION).await.status,
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        get_as(&f, &work(999), OWNER, SESSION).await.status,
        StatusCode::NOT_FOUND
    );
}

async fn new_revision(pool: &PgPool) -> Uuid {
    let id = Uuid::now_v7();
    let (raw_title, raw_document): (String, sqlx::types::Json<DocumentData>) =
        sqlx::query_as("SELECT title, document FROM revisions WHERE id = $1")
            .bind(R1)
            .fetch_one(pool)
            .await
            .unwrap();
    let content =
        RevisionContent::new(titled(&format!("{raw_title} (bản sửa)"), raw_document.0)).unwrap();
    sqlx::query(
        "INSERT INTO revisions (id, work_id, title, document, content_hash, created_by)
         SELECT $1, work_id, title || ' (bản sửa)', document, $3, created_by
           FROM revisions WHERE id = $2",
    )
    .bind(id)
    .bind(R1)
    .bind(content.hash().as_str())
    .execute(pool)
    .await
    .unwrap();
    id
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn the_public_tham_list_pages_current_membership_for_guests(pool: PgPool) {
    let f = fixture(pool).await;
    for n in 1..=5 {
        f.vink
            .set_work_endorsement(
                principal(reader_id(n)),
                SetWorkEndorsement {
                    work_id: sample_work(),
                    revision: R1.to_string(),
                    command_key: Uuid::now_v7().to_string(),
                    expected_order: 1,
                    endorsed: true,
                },
            )
            .await
            .unwrap();
    }
    let base = format!(
        "/api/v1/works/{}/revisions/{R1}/endorsers",
        dev_seed::SAMPLE_WORK
    );
    let guest = |path: String| {
        let app = f.app.clone();
        async move { send(&app, "GET", &path, None, None, false, None, None).await }
    };
    let first = guest(format!("{base}?limit=2")).await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.body);
    assert_eq!(first.cache.as_deref(), Some("private, no-cache"));
    assert_eq!(first.body["total_count"], 5);
    assert_eq!(first.body["items"].as_array().unwrap().len(), 2);
    assert!(
        !first
            .body
            .to_string()
            .contains(&reader_id(1).to_string()[..13])
    );
    // Revalidation re-reads membership: unchanged → 304.
    let tag = first.etag.clone().unwrap();
    let again = send(
        &f.app,
        "GET",
        &format!("{base}?limit=2"),
        None,
        None,
        false,
        None,
        Some(&tag),
    )
    .await;
    assert_eq!(again.status, StatusCode::NOT_MODIFIED);
    // Walk every page; one reader not yet listed unlikes mid-way and leaves the list.
    let mut seen = first.body["items"].as_array().unwrap().len();
    let mut cursor = first.body["next_cursor"].as_str().map(str::to_owned);
    let mut unliked = false;
    while let Some(next) = cursor {
        if !unliked {
            unliked = true;
            sqlx::query(
                "UPDATE work_endorsements SET selected = false, version = version + 1,
                        accepted_order = accepted_order + 1
                 WHERE work_id = $1 AND revision_id = $2 AND cursor_key = (
                     SELECT cursor_key FROM work_endorsements
                     WHERE work_id = $1 AND revision_id = $2 AND selected
                     ORDER BY cursor_key DESC LIMIT 1)",
            )
            .bind(dev_seed::SAMPLE_WORK)
            .bind(R1)
            .execute(&f.pool)
            .await
            .unwrap();
        }
        let page = guest(format!("{base}?limit=2&cursor={next}")).await;
        assert_eq!(page.status, StatusCode::OK, "{}", page.body);
        assert_eq!(page.body["total_count"], 4);
        seen += page.body["items"].as_array().unwrap().len();
        cursor = page.body["next_cursor"].as_str().map(str::to_owned);
    }
    assert_eq!(
        seen, 4,
        "every current member exactly once, beyond the 3 preview names"
    );
    // A new public revision: a continued cursor is 409, a first page of the old one 404.
    let cursor = first.body["next_cursor"].as_str().unwrap().to_string();
    let r2 = new_revision(&f.pool).await;
    sqlx::query(
        "UPDATE publications SET revision_id = $2, generation = generation + 1 WHERE work_id = $1",
    )
    .bind(dev_seed::SAMPLE_WORK)
    .bind(r2)
    .execute(&f.pool)
    .await
    .unwrap();
    let changed = guest(format!("{base}?cursor={cursor}")).await;
    assert_eq!(changed.status, StatusCode::CONFLICT);
    assert_eq!(changed.body["error"]["code"], "revision_changed");
    assert_eq!(changed.cache.as_deref(), Some("no-store"));
    assert_eq!(guest(base.clone()).await.status, StatusCode::NOT_FOUND);
    let fresh = guest(format!(
        "/api/v1/works/{}/revisions/{r2}/endorsers",
        dev_seed::SAMPLE_WORK
    ))
    .await;
    assert_eq!(fresh.body["total_count"], 0);
    // Not public at all: 404.
    sqlx::query("DELETE FROM publications WHERE work_id = $1")
        .bind(dev_seed::SAMPLE_WORK)
        .execute(&f.pool)
        .await
        .unwrap();
    assert_eq!(
        guest(format!(
            "/api/v1/works/{}/revisions/{r2}/endorsers",
            dev_seed::SAMPLE_WORK
        ))
        .await
        .status,
        StatusCode::NOT_FOUND
    );
}

/// Query budget (T-06/AC-08): with skewed data (20 000 members, 18 000 selected, all on one
/// revision) a public Thấm page is a bounded index scan that stops after `limit + 1` members,
/// never a sequential scan. The plan is printed for the handoff (`-- --nocapture`).
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn endorser_pages_and_batch_facts_stay_on_their_indexes_under_skew(pool: PgPool) {
    let f = fixture(pool).await;
    sqlx::query(
        "INSERT INTO principals (id, display_name)
         SELECT gen_random_uuid(), 'Độc giả ' || n FROM generate_series(1, 20000) AS n",
    )
    .execute(&f.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO work_endorsements (work_id, revision_id, actor_id, selected, version, accepted_order)
         SELECT $1, $2, id, (row_number() OVER ()) % 10 <> 0, CASE WHEN (row_number() OVER ()) % 10 <> 0 THEN 2 ELSE 3 END, 3
         FROM principals WHERE display_name LIKE 'Độc giả %'",
    )
    .bind(dev_seed::SAMPLE_WORK)
    .bind(R1)
    .execute(&f.pool)
    .await
    .unwrap();
    sqlx::query("ANALYZE work_endorsements")
        .execute(&f.pool)
        .await
        .unwrap();
    let plan: Vec<String> = sqlx::query_scalar(
        "EXPLAIN (ANALYZE, BUFFERS)
         SELECT e.cursor_key, a.display_name, a.handle, a.retired
         FROM work_endorsements e
         LEFT JOIN public_author_attribution a ON a.id = e.actor_id
         WHERE e.work_id = $1 AND e.revision_id = $2 AND e.selected AND e.cursor_key > $3
         ORDER BY e.cursor_key LIMIT 51",
    )
    .bind(dev_seed::SAMPLE_WORK)
    .bind(R1)
    .bind(Uuid::nil())
    .fetch_all(&f.pool)
    .await
    .unwrap();
    println!("endorser page plan:\n{}", plan.join("\n"));
    // Either the (work, revision, position) range or the position order with a filter: both are
    // bounded index scans that stop after `limit + 1` members.
    assert!(
        plan.iter()
            .any(|line| line.contains("Index Scan using work_endorsements_")),
        "{}",
        plan.join("\n")
    );
    assert!(
        !plan
            .iter()
            .any(|line| line.contains("Seq Scan on work_endorsements"))
    );
    // Through the route: total over 18 000 members, a bounded page.
    let page = send(
        &f.app,
        "GET",
        &format!(
            "/api/v1/works/{}/revisions/{R1}/endorsers?limit=50",
            dev_seed::SAMPLE_WORK
        ),
        None,
        None,
        false,
        None,
        None,
    )
    .await;
    assert_eq!(page.body["total_count"], 18000);
    assert_eq!(page.body["items"].as_array().unwrap().len(), 50);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn web_scope_rejects_reads_and_writes_under_a_replaced_cookie(pool: PgPool) {
    let f = fixture(pool).await;
    let scope = get_as(&f, "/api/v1/auth/session/account", OWNER, SESSION)
        .await
        .body["notification_scope"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(scope.len(), 32);
    let page = |actor: Uuid, session: (&str, &str), path: &str, body: Option<Value>| {
        let method = if body.is_some() { "POST" } else { "GET" };
        let request = Request::builder()
            .method(method)
            .uri(path)
            .header("x-test-principal", actor.to_string())
            .header(session.0, session.1)
            .header("x-vink-notification-scope", &scope)
            .header(header::ORIGIN, ORIGIN)
            .header(header::CONTENT_TYPE, "application/json")
            .body(body.map_or_else(Body::empty, |body| Body::from(body.to_string())))
            .unwrap();
        f.app.clone().oneshot(request)
    };
    assert_eq!(
        page(OWNER, SESSION, "/api/v1/me/notifications/count", None)
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    for (actor, replacement) in [(OWNER, SESSION_B), (reader_id(1), SESSION)] {
        let count = page(actor, replacement, "/api/v1/me/notifications/count", None)
            .await
            .unwrap();
        assert_eq!(count.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(count.headers()[header::CACHE_CONTROL], "no-store");
        let result = page(
            actor, replacement, "/api/v1/me/notifications/read",
            Some(json!({"command_key": "old-web-read", "group_id": Uuid::now_v7(), "observed_group_version": 1})),
        ).await.unwrap();
        assert_eq!(result.status(), StatusCode::UNAUTHORIZED);
    }
    assert!(f.inbox.commands.lock().unwrap().is_empty());
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn exact_reader_target_reuses_current_public_discussion_visibility(pool: PgPool) {
    let f = fixture(pool).await;
    let root = comment(
        &f,
        reader_id(1),
        "Chữ gốc sẽ bị gỡ",
        Some(ModerationAction::Approve),
    )
    .await;
    let reply = f
        .vink
        .submit_comment(
            principal(reader_id(2)),
            SubmitComment {
                work_id: sample_work(),
                target: Some(CommentId::from_uuid(root)),
                body_format: Default::default(),
                body: "Phản hồi vẫn công khai".into(),
                command_key: "notification-exact-reader-reply".into(),
            },
        )
        .await
        .unwrap()
        .comment_id;
    let path = format!(
        "/api/v1/works/{}/discussion/public/comments/{}",
        sample_work(),
        reply
    );
    let root_path = format!(
        "/api/v1/works/{}/discussion/public/comments/{root}",
        sample_work()
    );
    assert_eq!(
        send(&f.app, "GET", &path, None, None, false, None, None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    moderate(&f, reply, 1, ModerationAction::Approve).await;
    let shown = send(&f.app, "GET", &path, None, None, false, None, None).await;
    assert_eq!(shown.status, StatusCode::OK);
    assert_eq!(shown.cache.as_deref(), Some("no-store"));
    assert_eq!(shown.body["preview"][0]["body"], "Phản hồi vẫn công khai");
    moderate(&f, CommentId::from_uuid(root), 2, ModerationAction::Remove).await;
    assert_eq!(
        send(&f.app, "GET", &root_path, None, None, false, None, None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    let anchored = send(&f.app, "GET", &path, None, None, false, None, None).await;
    assert_eq!(anchored.body["root"]["kind"], "anchor");
    assert!(!anchored.body.to_string().contains("Chữ gốc sẽ bị gỡ"));
    moderate(&f, reply, 2, ModerationAction::Remove).await;
    assert_eq!(
        send(&f.app, "GET", &path, None, None, false, None, None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
}
