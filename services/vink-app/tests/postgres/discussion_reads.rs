//! #120 DISC-02/03 on real PostgreSQL: the public projection and the viewer's private overlay are
//! separate reads; own pending/rejected comments are recoverable wherever they sit; nobody else's
//! state leaks; cursors and HTTP input are bounded; and the remaining write fences serialize with a
//! suspension at commit. Oracles are computed from raw rows through the pure domain, never from the
//! read under test.

use std::collections::BTreeSet;

use vink_app::application::discussion::{OwnCommentsQuery, OwnPlacement, OwnStatus, RootEntry};
use vink_app::domain::discussion::lifecycle::{ModerationVersion, Removal};
use vink_app::domain::discussion::page::{CursorScope, MAX_CURSOR_BYTES, PageCursor};
use vink_app::domain::discussion::visibility::own_overlay;
use vink_app::identity::Resolution;

use super::*;

/// A third reader, so "another viewer" is never the work's author or the moderator.
const READER_B: Uuid = Uuid::from_u128(0x6666_6666_6666_4666_8666_6666_6666_6666);

fn reader_b() -> Option<PrincipalId> {
    Some(PrincipalId::from_uuid(READER_B))
}

async fn with_reader_b(pool: &PgPool) -> Vink<PgStore> {
    let vink = discussion(pool).await;
    sqlx::query("INSERT INTO principals (id, display_name) VALUES ($1, 'Người đọc B')")
        .bind(READER_B)
        .execute(pool)
        .await
        .unwrap();
    vink
}

fn unpublished(limit: Option<u32>, cursor: Option<String>) -> OwnCommentsQuery {
    OwnCommentsQuery {
        cursor,
        limit,
        ids: None,
    }
}

fn exact(ids: &[CommentId]) -> OwnCommentsQuery {
    OwnCommentsQuery {
        ids: Some(
            ids.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(","),
        ),
        ..OwnCommentsQuery::default()
    }
}

/// Every own unpublished comment of `viewer`, following the overlay's own cursor to its end.
async fn whole_overlay(vink: &Vink<PgStore>, viewer: Option<PrincipalId>) -> Vec<CommentId> {
    let mut ids = Vec::new();
    let mut cursor = None;
    loop {
        let page = vink
            .own_comments(viewer, sample_work(), &unpublished(Some(2), cursor))
            .await
            .unwrap();
        ids.extend(page.items.iter().map(|item| item.id));
        match page.next {
            Some(next) => cursor = Some(next.encode_in(CursorScope::Own)),
            None => return ids,
        }
    }
}

/// Every comment ID the public projection lists, roots/anchors, previews and reply pages.
async fn whole_public(vink: &Vink<PgStore>) -> Vec<CommentId> {
    let mut ids = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let public = vink
            .public_discussion(sample_work(), cursor.as_deref(), Some(3))
            .await
            .unwrap();
        let page = public.page();
        for root in &page.roots {
            let root_id = match &root.entry {
                RootEntry::Comment(item) => item.id,
                RootEntry::Anchor { id, .. } => *id,
            };
            ids.push(root_id);
            ids.extend(root.preview.iter().map(|item| item.id));
            let mut after = root.preview_next.map(PageCursor::encode);
            while let Some(position) = after {
                let replies = vink
                    .public_replies(sample_work(), root_id, Some(&position), Some(2))
                    .await
                    .unwrap();
                ids.extend(replies.page().replies.iter().map(|item| item.id));
                after = replies.page().next.map(PageCursor::encode);
            }
        }
        match page.next {
            Some(next) => cursor = Some(next.encode()),
            None => return ids,
        }
    }
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn own_comments_are_recoverable_beyond_the_public_page_and_under_unlisted_roots(
    pool: PgPool,
) {
    let vink = with_reader_b(&pool).await;
    // 21 public roots fill the composed and public first pages (default 20).
    let mut public_roots = Vec::new();
    for n in 0..21 {
        public_roots.push(published(&vink, author(), None, &format!("Gốc công khai {n}")).await);
    }
    let fragile = public_roots[0];
    // A's pending reply under a root that moderation then removes: with no public reply left,
    // the root is not listed for A, so no public or composed list can show the reply.
    let orphan = post(&vink, other(), Some(fragile), "Trả lời đang chờ duyệt").await;
    moderate(&vink, fragile, 2, ModerationAction::Remove)
        .await
        .unwrap();
    // A's pending root, after every public root: off the first page.
    let late_root = post(&vink, other(), None, "Gốc của A ở trang sau").await;
    // A's rejected root with a reason; B's pending root.
    let rejected = post(&vink, other(), None, "Lời bị từ chối").await;
    moderate(&vink, rejected, 1, ModerationAction::Reject)
        .await
        .unwrap();
    let theirs = post(&vink, reader_b(), None, "Của B, đang chờ").await;

    // The composed first page cannot recover either of A's pending comments.
    let composed = vink
        .discussion(other(), sample_work(), None, None)
        .await
        .unwrap();
    let composed_ids: BTreeSet<CommentId> = composed
        .roots
        .iter()
        .flat_map(|root| {
            let id = match &root.entry {
                RootEntry::Comment(item) => item.id,
                RootEntry::Anchor { id, .. } => *id,
            };
            std::iter::once(id).chain(root.preview.iter().map(|item| item.id))
        })
        .collect();
    assert!(!composed_ids.contains(&late_root) && !composed_ids.contains(&orphan));
    assert!(
        vink.replies(other(), sample_work(), fragile, None, None)
            .await
            .is_err(),
        "the unlisted root's replies are not readable"
    );

    // The overlay recovers exactly A's unpublished comments, oldest first.
    let overlay = vink
        .own_comments(other(), sample_work(), &unpublished(None, None))
        .await
        .unwrap();
    let ids: Vec<CommentId> = overlay.items.iter().map(|item| item.id).collect();
    assert_eq!(ids, vec![orphan, late_root, rejected]);
    assert_eq!(whole_overlay(&vink, other()).await, ids, "paged in twos");
    let reply = &overlay.items[0];
    assert_eq!(reply.status, OwnStatus::Pending);
    assert_eq!(reply.body.as_deref(), Some("Trả lời đang chờ duyệt"));
    assert_eq!(
        reply.placement,
        OwnPlacement::Reply {
            root_id: fragile,
            root_listed: false,
            context: vink_app::domain::discussion::visibility::ReplyContext::Root,
            context_author: None,
        }
    );
    assert_eq!(overlay.items[1].placement, OwnPlacement::Root);
    assert_eq!(overlay.items[2].status, OwnStatus::Rejected);
    assert_eq!(overlay.items[2].rejection_reason.as_deref(), Some(REASON));
    assert_eq!(overlay.items[1].rejection_reason, None);

    // B, the work's author and the moderator each see only their own; a guest gets nothing.
    assert_eq!(whole_overlay(&vink, reader_b()).await, vec![theirs]);
    assert!(whole_overlay(&vink, author()).await.is_empty());
    assert!(whole_overlay(&vink, moderator()).await.is_empty());
    assert_eq!(
        vink.own_comments(None, sample_work(), &unpublished(None, None))
            .await,
        Err(DiscussionError::Unauthenticated)
    );
    // Exact lookups of someone else's comments, or of nothing, reveal nothing.
    let peek = vink
        .own_comments(
            reader_b(),
            sample_work(),
            &exact(&[orphan, rejected, late_root]),
        )
        .await
        .unwrap();
    assert!(peek.items.is_empty());
    let by_author = vink
        .own_comments(
            author(),
            sample_work(),
            &exact(&[theirs, CommentId::generate()]),
        )
        .await
        .unwrap();
    assert!(by_author.items.is_empty());

    // The public projection holds none of the unpublished comments.
    let public: BTreeSet<CommentId> = whole_public(&vink).await.into_iter().collect();
    for private in [orphan, late_root, rejected, theirs, fragile] {
        assert!(!public.contains(&private), "{private}");
    }
    assert_eq!(public.len(), 20, "the 20 public roots that remain");
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_comment_moves_from_the_overlay_to_the_public_list_exactly_once(pool: PgPool) {
    let vink = with_reader_b(&pool).await;
    let root = published(&vink, author(), None, "Gốc").await;
    let reply = post(&vink, other(), Some(root), "Trả lời của A").await;
    let gone = post(&vink, other(), None, "Sẽ bị gỡ").await;
    let removed_later = published(&vink, other(), None, "Duyệt rồi gỡ").await;
    assert_eq!(whole_overlay(&vink, other()).await, vec![reply, gone]);
    assert!(!whole_public(&vink).await.contains(&reply));

    moderate(&vink, reply, 1, ModerationAction::Approve)
        .await
        .unwrap();
    // Approved: gone from the overlay list, present once in public, so a composed view keyed by
    // comment ID never shows it twice.
    assert_eq!(whole_overlay(&vink, other()).await, vec![gone]);
    let public = whole_public(&vink).await;
    assert_eq!(public.iter().filter(|id| **id == reply).count(), 1);
    let unique: BTreeSet<_> = public.iter().collect();
    assert_eq!(unique.len(), public.len(), "no ID twice across pages");

    // Write-result reconciliation reads the current row, not the receipt: approved, removed by
    // the writer, removed by moderation after approval.
    vink.remove_own_comment(other(), sample_work(), gone)
        .await
        .unwrap();
    moderate(&vink, removed_later, 2, ModerationAction::Remove)
        .await
        .unwrap();
    let foreign = post(&vink, reader_b(), None, "Của B").await;
    let reconciled = vink
        .own_comments(
            other(),
            sample_work(),
            &exact(&[reply, gone, removed_later, foreign, CommentId::generate()]),
        )
        .await
        .unwrap();
    let found: Vec<(CommentId, OwnStatus, Option<&str>, bool)> = reconciled
        .items
        .iter()
        .map(|item| {
            (
                item.id,
                item.status,
                item.body.as_deref(),
                item.rejection_reason.is_some(),
            )
        })
        .collect();
    assert_eq!(
        found,
        vec![
            (reply, OwnStatus::Approved, Some("Trả lời của A"), false),
            (gone, OwnStatus::Removed, None, false),
            // The moderation-removed text is still stored, but never returned once removed.
            (removed_later, OwnStatus::Removed, None, false),
        ]
    );
    assert!(reconciled.next.is_none());
    assert!(!whole_public(&vink).await.contains(&removed_later));
    // A replayed submit reports the current state as well, never the receipt's history.
    let key = key();
    let first = vink
        .submit_comment(other(), command(None, "Gửi lại", &key))
        .await
        .unwrap();
    moderate(&vink, first.comment_id, 1, ModerationAction::Approve)
        .await
        .unwrap();
    let replay = vink
        .submit_comment(other(), command(None, "Gửi lại", &key))
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.status, OwnStatus::Approved);
}

/// The overlay SQL selects exactly what `own_overlay` keeps for each viewer, over every state and
/// writer (the exact lookup over every row, the list over the unpublished ones).
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn sql_overlay_filters_match_the_domain_projection(pool: PgPool) {
    let vink = with_reader_b(&pool).await;
    published(&vink, author(), None, "Mở thread").await;
    let thread = thread_of_sample(&pool).await;
    for writer in [dev_seed::OTHER_PRINCIPAL, READER_B] {
        for review in ["pending", "approved", "rejected"] {
            for removal in [None, Some("author"), Some("moderation")] {
                let id = Uuid::now_v7();
                sqlx::query(AssertSqlSafe(format!(
                    "INSERT INTO comments (id, thread_id, kind, replied_to_reply, author_id,
                                           body, review, moderation_version,
                                           admission_policy, admission_reason)
                     VALUES ($1, $2, 'root', false, $3, 'Trạng thái', {PREMODERATED})"
                )))
                .bind(id)
                .bind(thread)
                .bind(writer)
                .execute(&pool)
                .await
                .unwrap();
                if (review, removal) != ("pending", None) {
                    review_raw(&pool, id, review, removal).await.unwrap();
                }
            }
        }
    }
    let states: Vec<(Uuid, Uuid, String, Option<String>, i64)> = sqlx::query_as(
        "SELECT id, author_id, review, removal, moderation_version FROM comments
          ORDER BY created_at, id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let all: Vec<CommentId> = states
        .iter()
        .map(|(id, ..)| CommentId::from_uuid(*id))
        .collect();
    for viewer in [other(), reader_b(), author(), moderator()] {
        let me = viewer.unwrap();
        let mut listed = Vec::new();
        let mut known = Vec::new();
        for (id, writer, review, removal, version) in &states {
            let state = CommentState::restore(
                match review.as_str() {
                    "pending" => Review::Pending,
                    "approved" => Review::Approved,
                    _ => Review::Rejected,
                },
                match removal.as_deref() {
                    None => None,
                    Some("author") => Some(Removal::ByAuthor),
                    _ => Some(Removal::ByModeration),
                },
                ModerationVersion::new(*version).unwrap(),
            );
            if let Some(own) = own_overlay(PrincipalId::from_uuid(*writer), state, me) {
                known.push(CommentId::from_uuid(*id));
                if own.unpublished {
                    listed.push(CommentId::from_uuid(*id));
                }
            }
        }
        assert_eq!(whole_overlay(&vink, viewer).await, listed, "{viewer:?}");
        // Exact lookups are bounded to 20 IDs; ask in chunks.
        let mut found = Vec::new();
        for chunk in all.chunks(20) {
            let page = vink
                .own_comments(viewer, sample_work(), &exact(chunk))
                .await
                .unwrap();
            found.extend(page.items.iter().map(|item| item.id));
        }
        assert_eq!(found, known, "{viewer:?}");
    }
}

/// The public projection lists exactly what the composed read shows a guest, with an independent
/// oracle from raw rows: present+approved comments, plus anchors for hidden roots with public
/// replies.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn the_public_projection_equals_the_guest_view_and_an_independent_oracle(pool: PgPool) {
    let vink = with_reader_b(&pool).await;
    let mut roots = Vec::new();
    for n in 0..7 {
        roots.push(published(&vink, author(), None, &format!("Gốc {n}")).await);
    }
    for (n, root) in roots.iter().enumerate() {
        for m in 0..(n % 4) {
            published(&vink, other(), Some(*root), &format!("Trả lời {n}.{m}")).await;
        }
        post(&vink, reader_b(), Some(*root), "B chờ duyệt").await;
    }
    // Hide two roots: one keeps public replies (anchor), one has none (gone).
    moderate(&vink, roots[3], 2, ModerationAction::Remove)
        .await
        .unwrap();
    moderate(&vink, roots[4], 2, ModerationAction::Reject)
        .await
        .unwrap();
    vink.remove_own_comment(author(), sample_work(), roots[0])
        .await
        .unwrap();

    let expected: Vec<CommentId> = sqlx::query_scalar(
        "SELECT c.id FROM comments c
          WHERE c.kind = 'root' AND (
                (c.review = 'approved' AND c.removal IS NULL)
             OR EXISTS (SELECT 1 FROM comments r WHERE r.root_id = c.id
                         AND r.review = 'approved' AND r.removal IS NULL))
          ORDER BY c.created_at, c.id",
    )
    .fetch_all(&pool)
    .await
    .unwrap()
    .into_iter()
    .map(CommentId::from_uuid)
    .collect();
    let public = vink
        .public_discussion(sample_work(), None, Some(50))
        .await
        .unwrap();
    let guest = vink
        .discussion(None, sample_work(), None, Some(50))
        .await
        .unwrap();
    assert_eq!(
        public.page(),
        &guest,
        "same facts as the composed guest read"
    );
    assert_eq!(listed_ids(public.page()), expected);
    let replies_expected: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM comments WHERE kind = 'reply' AND review = 'approved'
            AND removal IS NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let counted: i64 = public
        .page()
        .roots
        .iter()
        .map(|root| root.public_replies)
        .sum();
    assert_eq!(counted, replies_expected);
    let all = whole_public(&vink).await;
    assert_eq!(all.len() as i64, expected.len() as i64 + replies_expected);
}

/// Keyset paging is not a snapshot across requests (D2T-INV-09): a comment approved before the
/// cursor is not shown on the next page, a removed one disappears, and no ID is repeated.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn approval_and_removal_during_pagination_follow_the_keyset_contract(pool: PgPool) {
    let vink = with_reader_b(&pool).await;
    let early = post(&vink, other(), None, "Chờ, trước con trỏ").await;
    let mut roots = Vec::new();
    for n in 0..6 {
        roots.push(published(&vink, author(), None, &format!("Gốc {n}")).await);
    }
    let first = vink
        .public_discussion(sample_work(), None, Some(3))
        .await
        .unwrap();
    let first_ids = listed_ids(first.page());
    assert_eq!(first_ids, roots[..3]);
    let cursor = first.page().next.unwrap().encode();
    // Between the two requests: approve the comment before the cursor, remove one after it.
    moderate(&vink, early, 1, ModerationAction::Approve)
        .await
        .unwrap();
    moderate(&vink, roots[4], 2, ModerationAction::Remove)
        .await
        .unwrap();
    let second = vink
        .public_discussion(sample_work(), Some(&cursor), Some(3))
        .await
        .unwrap();
    let second_ids = listed_ids(second.page());
    assert_eq!(second_ids, vec![roots[3], roots[5]]);
    assert!(!first_ids.iter().any(|id| second_ids.contains(id)));
    assert!(
        !second_ids.contains(&early),
        "approved behind the cursor: seen on refresh"
    );
    let refreshed = listed_ids(
        vink.public_discussion(sample_work(), None, Some(50))
            .await
            .unwrap()
            .page(),
    );
    assert_eq!(refreshed[0], early);
}

struct OutageIdentity;

impl IdentityResolver for OutageIdentity {
    fn resolve<'a>(&'a self, _: &'a Parts, _: Option<SocketAddr>) -> ResolveFuture<'a> {
        Box::pin(std::future::ready(Resolution::Unavailable))
    }
}

fn private_keys(body: &str) -> Vec<&'static str> {
    [
        "visibility",
        "can_remove",
        "rejection_reason",
        "viewer_is_work_author",
        "own_pending",
        "status",
    ]
    .into_iter()
    .filter(|key| body.contains(key))
    .collect()
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn http_keeps_public_and_private_reads_apart(pool: PgPool) {
    let app = app(&pool).await;
    let vink = with_reader_b(&pool).await;
    let root = published(&vink, author(), None, "Gốc công khai").await;
    let pending = post(&vink, other(), Some(root), "Bí mật của A").await;
    let rejected = post(&vink, other(), None, "Bị từ chối").await;
    moderate(&vink, rejected, 1, ModerationAction::Reject)
        .await
        .unwrap();
    let work = dev_seed::SAMPLE_WORK;
    let public = format!("/api/v1/works/{work}/discussion/public");
    let me = format!("/api/v1/works/{work}/discussion/me");

    // Public: same bytes for a guest and for any actor; nothing private; no-store.
    let (status, cache, guest_body) = call(&app, "GET", &public, None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cache.as_deref(), Some("no-store"));
    assert!(private_keys(&guest_body).is_empty(), "{guest_body}");
    assert!(!guest_body.contains("Bí mật") && !guest_body.contains("Bị từ chối"));
    for actor in [
        dev_seed::OTHER_PRINCIPAL,
        dev_seed::SAMPLE_AUTHOR,
        MODERATOR,
    ] {
        let (status, _, body) = call(&app, "GET", &public, Some(actor), None, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, guest_body, "the public read ignores the actor");
    }
    let replies = format!("{public}/roots/{root}/replies");
    let (status, cache, body) = call(
        &app,
        "GET",
        &replies,
        Some(dev_seed::OTHER_PRINCIPAL),
        None,
        None,
    )
    .await;
    assert_eq!(
        (status, cache.as_deref()),
        (StatusCode::OK, Some("no-store"))
    );
    assert_eq!(
        json(&body)["replies"],
        json!([]),
        "A's pending reply is not public"
    );

    // Overlay: A sees theirs (no-store), B and a guest do not; an actor switch on one router
    // never carries the previous viewer's state.
    let (status, cache, body) = call(
        &app,
        "GET",
        &me,
        Some(dev_seed::OTHER_PRINCIPAL),
        None,
        None,
    )
    .await;
    assert_eq!(
        (status, cache.as_deref()),
        (StatusCode::OK, Some("no-store"))
    );
    let mine = json(&body);
    assert_eq!(mine["comments"].as_array().unwrap().len(), 2);
    assert_eq!(mine["comments"][0]["comment_id"], pending.to_string());
    assert_eq!(mine["comments"][0]["placement"]["root_listed"], true);
    assert_eq!(mine["comments"][1]["rejection_reason"], REASON);
    let (status, cache, body) = call(&app, "GET", &me, Some(READER_B), None, None).await;
    assert_eq!(
        (status, cache.as_deref()),
        (StatusCode::OK, Some("no-store"))
    );
    assert_eq!(json(&body)["comments"], json!([]));
    assert!(!body.contains("Bí mật"));
    let (status, cache, body) = call(&app, "GET", &me, None, None, None).await;
    assert_eq!(
        (status, cache.as_deref()),
        (StatusCode::UNAUTHORIZED, Some("no-store"))
    );
    assert_eq!(json(&body)["error"]["code"], "unauthenticated");
    // A guest learns nothing about validity either: authentication comes first.
    let (status, _, _) = call(&app, "GET", &format!("{me}?limit=0"), None, None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Input bounds, all 422 with no-store; never clamped or ignored.
    let a = Some(dev_seed::OTHER_PRINCIPAL);
    let thread_cursor = PageCursor::after(time::OffsetDateTime::now_utc(), pending).encode();
    let own_cursor =
        PageCursor::after(time::OffsetDateTime::now_utc(), pending).encode_in(CursorScope::Own);
    let huge = "9".repeat(MAX_CURSOR_BYTES * 200);
    for (path, actor, field) in [
        (format!("{me}?cursor={thread_cursor}"), a, "cursor"),
        (format!("{public}?cursor={own_cursor}"), None, "cursor"),
        (format!("{public}?cursor=c1.{huge}"), None, "cursor"),
        (
            format!("{public}?cursor=c1.%2B1.{}", pending.as_uuid().simple()),
            None,
            "cursor",
        ),
        (format!("{public}?cursor="), None, "cursor"),
        // Invalid UTF-8 decodes lossily, so it reaches the cursor parser and is refused there.
        (format!("{public}?cursor=%FF%FE"), None, "cursor"),
        (format!("{public}?cursor=a&cursor=b"), None, "query"),
        (format!("{public}?limit=51"), None, "limit"),
        (format!("{public}?limit=-1"), None, "query"),
        (format!("{public}?limit=99999999999"), None, "query"),
        (format!("{me}?ids={pending}&cursor={own_cursor}"), a, "ids"),
        (format!("{me}?ids={pending}&limit=2"), a, "ids"),
        (format!("{me}?ids="), a, "ids"),
        (format!("{me}?ids=not-an-id"), a, "ids"),
        (
            format!("{me}?ids={}", vec![pending.to_string(); 21].join(",")),
            a,
            "ids",
        ),
    ] {
        let (status, cache, body) = call(&app, "GET", &path, actor, None, None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{path}: {body}");
        assert_eq!(cache.as_deref(), Some("no-store"), "{path}");
        assert_eq!(json(&body)["error"]["field"], field, "{path}: {body}");
    }
    // Exact lookup over HTTP; 21 copies of one ID are one ID after dedup, but the raw length
    // bound still refuses them above.
    let (status, _, body) = call(
        &app,
        "GET",
        &format!("{me}?ids={pending},{root}"),
        a,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let found = json(&body);
    assert_eq!(found["comments"].as_array().unwrap().len(), 1, "{body}");
    assert_eq!(found["comments"][0]["status"], "pending");
    assert_eq!(found["next_cursor"], Value::Null);

    // Missing or unpublished work: 404 for both reads, never an empty discussion.
    let missing = Uuid::now_v7();
    for (path, actor) in [
        (format!("/api/v1/works/{missing}/discussion/public"), None),
        (format!("/api/v1/works/{missing}/discussion/me"), a),
        (
            "/api/v1/works/not-a-work/discussion/public".to_string(),
            None,
        ),
        (format!("{public}/roots/{rejected}/replies"), None),
    ] {
        let (status, cache, body) = call(&app, "GET", &path, actor, None, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}: {body}");
        assert_eq!(cache.as_deref(), Some("no-store"));
    }

    // Guest SSR of the reader page renders public facts only, whoever the request names.
    let page = format!("/tac-pham/{work}");
    let (status, _, html) = call(&app, "GET", &page, a, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        html.contains("Gốc công khai"),
        "SSR renders the public root"
    );
    assert!(!html.contains("Bí mật của A") && !html.contains("Bị từ chối"));
}

/// An Identity outage hides only what needs Identity: the public projection keeps answering,
/// the overlay and the composed read are 503, never a guest view in disguise.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn an_identity_outage_never_hides_the_public_projection(pool: PgPool) {
    let vink = with_reader_b(&pool).await;
    published(&vink, author(), None, "Gốc công khai").await;
    post(&vink, other(), None, "Chờ duyệt").await;
    let app = router(AppState::new(
        Vink::new(PgStore::new(pool.clone())),
        Arc::new(OutageIdentity),
        ORIGIN,
        std::path::Path::new("target/site"),
    ));
    let work = dev_seed::SAMPLE_WORK;
    let (status, cache, body) = call(
        &app,
        "GET",
        &format!("/api/v1/works/{work}/discussion/public"),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(
        (status, cache.as_deref()),
        (StatusCode::OK, Some("no-store"))
    );
    assert_eq!(json(&body)["roots"].as_array().unwrap().len(), 1);
    for path in [
        format!("/api/v1/works/{work}/discussion/me"),
        format!("/api/v1/works/{work}/discussion"),
    ] {
        let (status, cache, body) = call(&app, "GET", &path, None, None, None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{path}");
        assert_eq!(cache.as_deref(), Some("no-store"));
        assert_eq!(json(&body)["error"]["code"], "dependency_unavailable");
    }
}

/// The writer's removal holds the comment row, then waits at its standing fence for an
/// uncommitted suspension and refuses; the text stays.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn own_removal_waits_for_a_pending_suspension_and_refuses(pool: PgPool) {
    let vink = Arc::new(discussion(&pool).await);
    let mine = published(&vink, other(), None, "Lời của A").await;
    let mut suspending = pool.begin().await.unwrap();
    let pid = backend_pid(&mut suspending).await;
    suspend_in(&mut suspending, dev_seed::OTHER_PRINCIPAL).await;
    let removing = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move { vink.remove_own_comment(other(), sample_work(), mine).await })
    };
    wait_blocked_by(&pool, pid).await;
    assert!(!removing.is_finished(), "the removal waits at its fence");
    suspending.commit().await.unwrap();
    assert_eq!(removing.await.unwrap(), Err(DiscussionError::Suspended));
    let (removal, body): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT removal, body FROM comments WHERE id = $1")
            .bind(mine.as_uuid())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((removal, body.as_deref()), (None, Some("Lời của A")));
}

/// The author's mode change holds the thread row, then waits for an uncommitted suspension and
/// refuses; the generation does not move.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_mode_change_waits_for_a_pending_suspension_and_refuses(pool: PgPool) {
    let vink = Arc::new(discussion(&pool).await);
    published(&vink, other(), None, "Mở thread").await;
    let mut suspending = pool.begin().await.unwrap();
    let pid = backend_pid(&mut suspending).await;
    suspend_in(&mut suspending, dev_seed::SAMPLE_AUTHOR).await;
    let closing = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move { vink.set_comments_mode(author(), close_at(1)).await })
    };
    wait_blocked_by(&pool, pid).await;
    assert!(!closing.is_finished(), "the mode change waits at its fence");
    suspending.commit().await.unwrap();
    assert_eq!(closing.await.unwrap(), Err(DiscussionError::Suspended));
    let (open, generation): (bool, i64) = sqlx::query_as(
        "SELECT comments_open, settings_generation FROM discussion_threads WHERE work_id = $1",
    )
    .bind(dev_seed::SAMPLE_WORK)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((open, generation), (true, 1));
}

/// A write that already holds the fence makes the suspension wait until it commits: the
/// linearization point is the principal row, in both orders.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_suspension_waits_for_a_write_that_already_holds_the_fence(pool: PgPool) {
    let vink = discussion(&pool).await;
    let mine = published(&vink, other(), None, "Lời của A").await;
    // Stand in for an in-flight removal at its fence: the same row locks, in the same order.
    let mut writing = pool.begin().await.unwrap();
    let pid = backend_pid(&mut writing).await;
    sqlx::query("SELECT 1 FROM comments WHERE id = $1 FOR UPDATE")
        .bind(mine.as_uuid())
        .execute(&mut *writing)
        .await
        .unwrap();
    sqlx::query("SELECT 1 FROM principals WHERE id = $1 FOR UPDATE")
        .bind(dev_seed::OTHER_PRINCIPAL)
        .execute(&mut *writing)
        .await
        .unwrap();
    let suspending = {
        let pool = pool.clone();
        tokio::spawn(async move {
            suspend_principal(
                &pool,
                PrincipalId::from_uuid(dev_seed::OTHER_PRINCIPAL),
                &suspension_reason(),
            )
            .await
        })
    };
    wait_blocked_by(&pool, pid).await;
    assert!(
        !suspending.is_finished(),
        "the suspension waits for the fence holder"
    );
    writing.commit().await.unwrap();
    suspending.await.unwrap().unwrap();
    assert_eq!(
        vink.remove_own_comment(other(), sample_work(), mine).await,
        Err(DiscussionError::Suspended)
    );
}

async fn suspend_in(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, principal: Uuid) {
    sqlx::query(
        "INSERT INTO principal_suspensions (id, principal_id, reason)
         VALUES ($1, $2, 'Tạm khóa trong bộ test thảo luận')",
    )
    .bind(Uuid::now_v7())
    .bind(principal)
    .execute(&mut **tx)
    .await
    .unwrap();
}

/// Mixed concurrent writes on one thread and one comment (submit, reply, moderate, remove,
/// close/open, suspend/reinstate the moderator) never deadlock and never leave an unexpected
/// error: each lands or is refused by a guard. PostgreSQL reports a deadlock as an error, which
/// the store classifies as unavailable, so an `Unavailable` here is the failure signal.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn concurrent_writes_on_one_thread_never_deadlock(pool: PgPool) {
    let vink = Arc::new(with_reader_b(&pool).await);
    let root = published(&vink, author(), None, "Gốc nóng").await;
    let generation = || async {
        sqlx::query_scalar::<_, i64>(
            "SELECT settings_generation FROM discussion_threads WHERE work_id = $1",
        )
        .bind(dev_seed::SAMPLE_WORK)
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    for round in 0..12 {
        // Reopen (a reaffirmation when already open) so the setup can submit.
        vink.set_comments_mode(
            author(),
            SetCommentsMode {
                work_id: sample_work(),
                expected_generation: generation().await,
                mode: CommentsMode::Open,
            },
        )
        .await
        .unwrap();
        let target = post(&vink, other(), Some(root), &format!("Chờ {round}")).await;
        let mine = published(&vink, reader_b(), Some(root), &format!("Của B {round}")).await;
        let generation = generation().await;
        let mode = if round % 2 == 0 {
            CommentsMode::Closed
        } else {
            CommentsMode::Open
        };
        let (submitted, replied, decided, removed, toggled, suspended) = tokio::join!(
            {
                let vink = Arc::clone(&vink);
                async move {
                    vink.submit_comment(reader_b(), command(Some(root), "Gửi đồng thời", &key()))
                        .await
                        .map(drop)
                }
            },
            {
                let vink = Arc::clone(&vink);
                async move {
                    vink.submit_comment(other(), command(Some(mine), "Trả lời B", &key()))
                        .await
                        .map(drop)
                }
            },
            {
                let vink = Arc::clone(&vink);
                async move {
                    moderate(&vink, target, 1, ModerationAction::Approve)
                        .await
                        .map(drop)
                }
            },
            {
                let vink = Arc::clone(&vink);
                async move {
                    vink.remove_own_comment(reader_b(), sample_work(), mine)
                        .await
                        .map(drop)
                }
            },
            {
                let vink = Arc::clone(&vink);
                async move {
                    vink.set_comments_mode(
                        author(),
                        SetCommentsMode {
                            work_id: sample_work(),
                            expected_generation: generation,
                            mode,
                        },
                    )
                    .await
                    .map(drop)
                }
            },
            {
                let pool = pool.clone();
                async move {
                    if round % 3 == 0 {
                        suspend_principal(
                            &pool,
                            PrincipalId::from_uuid(MODERATOR),
                            &suspension_reason(),
                        )
                        .await
                        .map(drop)
                    } else {
                        Ok(())
                    }
                }
            },
        );
        for (name, result) in [
            ("submit", submitted),
            ("reply", replied),
            ("moderate", decided),
            ("remove", removed),
            ("mode", toggled),
        ] {
            assert!(
                !matches!(
                    result,
                    Err(DiscussionError::Unavailable | DiscussionError::Internal)
                ),
                "round {round} {name}: {result:?}"
            );
        }
        suspended.unwrap();
        if round % 3 == 0 {
            reinstate_principal(&pool, PrincipalId::from_uuid(MODERATOR))
                .await
                .unwrap();
        }
    }
}

/// The overlay index is a pure read optimization: its rollback drops only the index and the
/// migration record, and the overlay answers the same rows without it (by a table scan).
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn the_overlay_index_rolls_back_without_changing_answers(pool: PgPool) {
    let vink = with_reader_b(&pool).await;
    let root = published(&vink, author(), None, "Gốc").await;
    let reply = post(&vink, other(), Some(root), "Chờ").await;
    let own = post(&vink, other(), None, "Gốc chờ").await;
    post(&vink, reader_b(), None, "Của B").await;
    let index: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('comments_own_unpublished_idx')::text")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(index.as_deref(), Some("comments_own_unpublished_idx"));
    let before = whole_overlay(&vink, other()).await;
    assert_eq!(before, vec![reply, own]);
    let comments_before: i64 = sqlx::query_scalar("SELECT count(*) FROM comments")
        .fetch_one(&pool)
        .await
        .unwrap();
    let script = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations-rollback/20261113000000_own_comment_overlay.down.sql"),
    )
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::raw_sql(AssertSqlSafe(script))
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let index: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('comments_own_unpublished_idx')::text")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(index, None);
    let recorded: i64 =
        sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations WHERE version = 20261113000000")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(recorded, 0);
    let comments_after: i64 = sqlx::query_scalar("SELECT count(*) FROM comments")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(comments_after, comments_before);
    assert_eq!(whole_overlay(&vink, other()).await, before);
}
