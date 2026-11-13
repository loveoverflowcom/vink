//! Comment moderation governance integration and concurrency tests (`ADM-01`, `ADM-03`, `O-03`,
//! `MOD-09`..`MOD-15`). Exercises HTTP routes, `PgStore`, migration upgrade/backout, SQL schema
//! constraints and deterministic lock-order / revocation barrier tests against real PostgreSQL.

use axum::http::StatusCode;
use serde_json::{Value, json};
use sqlx::migrate::Migrator;
use time::{Duration as TimeDuration, OffsetDateTime};
use vink_app::application::discussion::{
    DiscussionError, ModerateComment, ModerationAction, OwnCommentsQuery, OwnStatus,
};
use vink_app::application::post_review::{
    RequestReview, ReviewAuthority, ReviewResult, ReviewWrite,
};
use vink_app::domain::discussion::admission::PostReviewFinding;
use vink_app::domain::discussion::lifecycle::{ModerationVersion, Review};
use vink_app::domain::discussion::moderation::GrantNote;
use vink_app::domain::discussion::outbox::{Completion, OutboxPolicy};
use vink_app::domain::discussion::post_review::CONSUMER as POST_REVIEW_CONSUMER;
use vink_app::domain::ids::GrantId;
use vink_app::domain::moderation::authority::{GrantScope, GrantSpec, Grantee, ReviewCapability};
use vink_app::evaluation_worker::EvaluationWorker;
use vink_app::persistence::moderation::{
    add_admin_seat, create_moderation_grant, remove_admin_seat, revoke_moderation_grant,
    set_group_membership,
};
use vink_app::persistence::outbox as queue;
use vink_app::persistence::post_review::OperatorPostReview;
use vink_app::persistence::{MIGRATOR, migrate};

use super::*;

const COMMENT_GOVERNANCE_MIGRATION: i64 = 20_261_119_000_000;

async fn insert_principal(pool: &PgPool, id: Uuid, name: &str) {
    sqlx::query("INSERT INTO principals (id, display_name) VALUES ($1, $2)")
        .bind(id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
}

async fn grant_admin(pool: &PgPool, id: Uuid) {
    let mut conn = pool.acquire().await.unwrap();
    add_admin_seat(
        &mut conn,
        "operator-test",
        PrincipalId::from_uuid(id),
        Some("test"),
    )
    .await
    .unwrap();
}

async fn post_on(
    vink: &Vink<PgStore>,
    actor_id: Uuid,
    work_id: WorkId,
    target: Option<CommentId>,
    body: &str,
) -> CommentId {
    let outcome = vink
        .submit_comment(
            Some(PrincipalId::from_uuid(actor_id)),
            SubmitComment {
                work_id,
                target,
                body: body.into(),
                command_key: key(),
            },
        )
        .await
        .unwrap();
    assert_eq!(outcome.status, OwnStatus::Pending);
    outcome.comment_id
}

async fn publish_work(
    pool: &PgPool,
    vink: &Vink<PgStore>,
    owner_id: Uuid,
    title: &str,
) -> (WorkId, Uuid) {
    let draft = vink
        .create_draft(Some(PrincipalId::from_uuid(owner_id)))
        .await
        .unwrap();
    vink.save_draft(
        Some(PrincipalId::from_uuid(owner_id)),
        save(draft.work_id, 1, title, doc(&[&["Khổ thơ mở đầu."]])),
    )
    .await
    .unwrap();
    let snap = vink
        .capture_snapshot(
            Some(PrincipalId::from_uuid(owner_id)),
            draft.work_id,
            &key(),
            2,
            1,
        )
        .await
        .unwrap();
    vink.request_public(
        Some(PrincipalId::from_uuid(owner_id)),
        draft.work_id,
        &key(),
        snap.revision_id,
        1,
    )
    .await
    .unwrap();
    sqlx::query("UPDATE work_evaluation_jobs SET scenario = 'approve' WHERE work_id = $1")
        .bind(draft.work_id.as_uuid())
        .execute(pool)
        .await
        .unwrap();
    assert!(
        EvaluationWorker::run_once(pool, "test-worker")
            .await
            .unwrap()
    );
    (draft.work_id, snap.revision_id.as_uuid())
}

async fn create_grant_http(
    app: &Router,
    admin_id: Uuid,
    grantee_kind: &str,
    grantee_id: Uuid,
    capability: &str,
    scope_work: Option<WorkId>,
) -> Uuid {
    let mut payload = json!({
        "grantee_kind": grantee_kind,
        "grantee_id": grantee_id.to_string(),
        "capability": capability,
        "scope_kind": if scope_work.is_some() { "work" } else { "global" },
    });
    if let Some(w) = scope_work {
        payload["scope_work_id"] = json!(w.to_string());
    }
    let (status, _, body) = call(
        app,
        "POST",
        "/api/v1/moderation/grants",
        Some(admin_id),
        Some(ORIGIN),
        Some(payload),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "create grant failed: {body}");
    Uuid::parse_str(json(&body)["grant_id"].as_str().unwrap()).unwrap()
}

async fn revoke_grant_http(app: &Router, admin_id: Uuid, grant_id: Uuid) {
    let (status, _, body) = call(
        app,
        "POST",
        &format!("/api/v1/moderation/grants/{grant_id}/revoke"),
        Some(admin_id),
        Some(ORIGIN),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "revoke grant failed: {body}"
    );
}

async fn create_group_http(app: &Router, admin_id: Uuid, name: &str) -> Uuid {
    let (status, _, body) = call(
        app,
        "POST",
        "/api/v1/moderation/groups",
        Some(admin_id),
        Some(ORIGIN),
        Some(json!({ "name": name })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "create group failed: {body}");
    Uuid::parse_str(json(&body)["group_id"].as_str().unwrap()).unwrap()
}

async fn add_member_http(app: &Router, admin_id: Uuid, group_id: Uuid, member_id: Uuid) {
    let (status, _, body) = call(
        app,
        "POST",
        &format!("/api/v1/moderation/groups/{group_id}/members"),
        Some(admin_id),
        Some(ORIGIN),
        Some(json!({ "principal_id": member_id.to_string() })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "add member failed: {body}");
}

async fn remove_member_http(app: &Router, admin_id: Uuid, group_id: Uuid, member_id: Uuid) {
    let (status, _, body) = call(
        app,
        "DELETE",
        &format!("/api/v1/moderation/groups/{group_id}/members/{member_id}"),
        Some(admin_id),
        Some(ORIGIN),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "remove member failed: {body}"
    );
}

async fn get_queue_http(
    app: &Router,
    actor_id: Uuid,
    cursor: Option<&str>,
    limit: Option<u32>,
) -> (StatusCode, Option<String>, Value) {
    let mut path = "/api/v1/moderation/comments".to_string();
    let mut params = Vec::new();
    if let Some(c) = cursor {
        params.push(format!("cursor={c}"));
    }
    if let Some(l) = limit {
        params.push(format!("limit={l}"));
    }
    if !params.is_empty() {
        path.push('?');
        path.push_str(&params.join("&"));
    }
    let (status, cache, body) = call(app, "GET", &path, Some(actor_id), None, None).await;
    (status, cache, json(&body))
}

async fn decide_http(
    app: &Router,
    actor_id: Uuid,
    comment_id: CommentId,
    expected_version: i64,
    action: &str,
    reason: Option<&str>,
) -> (StatusCode, Option<String>, Value) {
    let mut payload = json!({
        "expected_version": expected_version,
        "action": action,
    });
    if let Some(r) = reason {
        payload["reason"] = json!(r);
    }
    let (status, cache, body) = call(
        app,
        "POST",
        &format!("/api/v1/moderation/comments/{comment_id}/decisions"),
        Some(actor_id),
        Some(ORIGIN),
        Some(payload),
    )
    .await;
    (status, cache, json(&body))
}

async fn decision_provenance(
    pool: &PgPool,
    comment_id: CommentId,
    version: i64,
) -> (String, Option<Uuid>, Option<Uuid>, Uuid, Option<String>) {
    sqlx::query_as(
        "SELECT authority_kind, grant_id, governance_grant_id, moderator_id, reason
           FROM comment_moderation_decisions
          WHERE comment_id = $1 AND version = $2",
    )
    .bind(comment_id.as_uuid())
    .bind(version)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Admin seat, individual `comments` grant, group `comments` grant and legacy grant each operate
/// queue + decisions without a legacy supplement, record exact provenance, respect standing and
/// lose access immediately upon revocation / removal.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn admin_individual_group_and_legacy_authorities_work_and_record_provenance(pool: PgPool) {
    let app = app(&pool).await;
    let vink = seeded(&pool).await;
    let admin_id = Uuid::now_v7();
    let ind_id = Uuid::now_v7();
    let grp_member_id = Uuid::now_v7();
    let legacy_id = Uuid::now_v7();
    for (id, name) in [
        (admin_id, "Quản trị viên"),
        (ind_id, "Duyệt viên cá nhân"),
        (grp_member_id, "Duyệt viên nhóm"),
        (legacy_id, "Duyệt viên kế thừa"),
    ] {
        insert_principal(&pool, id, name).await;
    }
    grant_admin(&pool, admin_id).await;
    let ind_grant = create_grant_http(&app, admin_id, "principal", ind_id, "comments", None).await;
    let group_id = create_group_http(&app, admin_id, "Tổ bình luận").await;
    let grp_grant = create_grant_http(&app, admin_id, "group", group_id, "comments", None).await;
    add_member_http(&app, admin_id, group_id, grp_member_id).await;
    let legacy_note = GrantNote::parse("Grant bình luận kế thừa").unwrap();
    let GrantOutcome::Granted(legacy_grant) =
        grant_moderator(&pool, PrincipalId::from_uuid(legacy_id), &legacy_note)
            .await
            .unwrap()
    else {
        panic!("expected fresh legacy grant");
    };

    let c1 = post(&vink, other(), None, "Duyệt bởi grant cá nhân").await;
    let c2 = post(&vink, other(), None, "Từ chối bởi grant nhóm").await;
    let c3 = post(&vink, other(), None, "Gỡ bởi ghế admin").await;
    let c4 = post(&vink, other(), None, "Duyệt bởi grant kế thừa").await;

    // All four distinct authorities see all 4 pending items in the queue with no-store.
    for actor in [admin_id, ind_id, grp_member_id, legacy_id] {
        let (st, cache, body) = get_queue_http(&app, actor, None, None).await;
        assert_eq!(st, StatusCode::OK, "queue failed for {actor}: {body}");
        assert_eq!(cache.as_deref(), Some("no-store"));
        assert_eq!(body["pending_total"], 4);
        let items = body["items"].as_array().unwrap();
        assert_eq!(items.len(), 4);
        assert!(items.iter().all(|item| item["can_decide"] == true));
    }

    // Decide with each authority path & verify HTTP + provenance row.
    let (st, cache, res) = decide_http(&app, ind_id, c1, 1, "approve", None).await;
    assert_eq!(st, StatusCode::OK, "{res}");
    assert_eq!(cache.as_deref(), Some("no-store"));
    assert_eq!(res["review"], "approved");
    assert_eq!(res["version"], 2);
    assert_eq!(
        decision_provenance(&pool, c1, 2).await,
        (
            "governance_grant".into(),
            None,
            Some(ind_grant),
            ind_id,
            None
        )
    );

    let (st, _, res) = decide_http(&app, grp_member_id, c2, 1, "reject", Some(REASON)).await;
    assert_eq!(st, StatusCode::OK, "{res}");
    assert_eq!(res["review"], "rejected");
    assert_eq!(
        decision_provenance(&pool, c2, 2).await,
        (
            "governance_grant".into(),
            None,
            Some(grp_grant),
            grp_member_id,
            Some(REASON.into())
        )
    );

    let (st, _, res) = decide_http(&app, admin_id, c3, 1, "remove", Some(REASON)).await;
    assert_eq!(st, StatusCode::OK, "{res}");
    assert_eq!(res["removed"], true);
    let removed_kind: Option<String> =
        sqlx::query_scalar("SELECT removal FROM comments WHERE id = $1")
            .bind(c3.as_uuid())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(removed_kind.as_deref(), Some("moderation"));
    assert_eq!(
        decision_provenance(&pool, c3, 2).await,
        ("admin".into(), None, None, admin_id, Some(REASON.into()))
    );

    let (st, _, res) = decide_http(&app, legacy_id, c4, 1, "approve", None).await;
    assert_eq!(st, StatusCode::OK, "{res}");
    assert_eq!(
        decision_provenance(&pool, c4, 2).await,
        (
            "legacy_grant".into(),
            Some(legacy_grant),
            None,
            legacy_id,
            None
        )
    );

    // Suspended governance moderator is refused (both store and HTTP) and writes nothing.
    let c5 = post(&vink, other(), None, "Sau khi đình chỉ").await;
    suspend_principal(&pool, PrincipalId::from_uuid(ind_id), &suspension_reason())
        .await
        .unwrap();
    assert_eq!(
        vink.moderate_comment(
            principal(ind_id),
            ModerateComment {
                comment_id: c5,
                expected_version: 1,
                action: ModerationAction::Approve,
                reason: None,
            },
        )
        .await,
        Err(DiscussionError::Suspended)
    );
    let (st, _, body) = get_queue_http(&app, ind_id, None, None).await;
    assert_eq!(st, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["error"]["code"], "principal_suspended");
    let (st, _, body) = decide_http(&app, ind_id, c5, 1, "approve", None).await;
    assert_eq!(st, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["error"]["code"], "principal_suspended");
    reinstate_principal(&pool, PrincipalId::from_uuid(ind_id))
        .await
        .unwrap();

    // Removing group membership / revoking individual grant / removing admin seat removes access.
    remove_member_http(&app, admin_id, group_id, grp_member_id).await;
    revoke_grant_http(&app, admin_id, ind_grant).await;
    {
        let mut conn = pool.acquire().await.unwrap();
        assert!(
            remove_admin_seat(&mut conn, "op", PrincipalId::from_uuid(admin_id), None)
                .await
                .unwrap()
        );
    }
    for lost in [grp_member_id, ind_id, admin_id] {
        let (st, _, body) = get_queue_http(&app, lost, None, None).await;
        assert_eq!(st, StatusCode::FORBIDDEN, "lost queue {lost}: {body}");
        assert_eq!(body["error"]["code"], "not_moderator");
        let (st, _, body) = decide_http(&app, lost, c5, 1, "approve", None).await;
        assert_eq!(st, StatusCode::FORBIDDEN, "lost decide {lost}: {body}");
        assert_eq!(body["error"]["code"], "not_moderator");
    }
}

/// Capability separation (`works` != `comments`) and per-`WorkId` scope apply on both queue and
/// decisions; inherited legacy comment grants do not grant work review.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn capability_separation_and_work_scope_enforced_strictly(pool: PgPool) {
    let app = app(&pool).await;
    let vink = seeded(&pool).await;
    let admin_id = Uuid::now_v7();
    let works_only_id = Uuid::now_v7();
    let scoped_mod_id = Uuid::now_v7();
    let author2_id = Uuid::now_v7();
    let legacy_id = Uuid::now_v7();
    for (id, name) in [
        (admin_id, "Admin"),
        (works_only_id, "Chỉ duyệt tác phẩm"),
        (scoped_mod_id, "Duyệt theo bài"),
        (author2_id, "Tác giả hai"),
        (legacy_id, "Kế thừa cũ"),
    ] {
        insert_principal(&pool, id, name).await;
    }
    grant_admin(&pool, admin_id).await;
    create_grant_http(&app, admin_id, "principal", works_only_id, "works", None).await;
    let w1 = sample_work();
    let (w2, _) = publish_work(&pool, &vink, author2_id, "Tác phẩm hai").await;
    let scoped_w1 = create_grant_http(
        &app,
        admin_id,
        "principal",
        scoped_mod_id,
        "comments",
        Some(w1),
    )
    .await;
    let legacy_note = GrantNote::parse("Chỉ bình luận").unwrap();
    grant_moderator(&pool, PrincipalId::from_uuid(legacy_id), &legacy_note)
        .await
        .unwrap();

    let c_w1 = post_on(
        &vink,
        dev_seed::OTHER_PRINCIPAL,
        w1,
        None,
        "Bình luận bài 1",
    )
    .await;
    let c_w2 = post_on(
        &vink,
        dev_seed::OTHER_PRINCIPAL,
        w2,
        None,
        "Bình luận bài 2",
    )
    .await;

    // Works-only grant is refused on comment queue, comment decision and missing comment ID.
    let (st, _, body) = get_queue_http(&app, works_only_id, None, None).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "not_moderator");
    for target in [c_w1, CommentId::from_uuid(Uuid::now_v7())] {
        let (st, _, body) = decide_http(&app, works_only_id, target, 1, "approve", None).await;
        assert_eq!(st, StatusCode::FORBIDDEN);
        assert_eq!(body["error"]["code"], "not_moderator");
    }

    // Inherited comment grant is refused on works moderation queue.
    let (st, _, body) = call(
        &app,
        "GET",
        "/api/v1/moderation/works/queue",
        Some(legacy_id),
        None,
        None,
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN, "works queue for legacy: {body}");

    // A scoped moderator sees only W1. An ID outside W1 has the same outcome as an absent ID.
    let (st, _, body) = get_queue_http(&app, scoped_mod_id, None, None).await;
    assert_eq!(st, StatusCode::OK, "{body}");
    assert_eq!(body["pending_total"], 1);
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["items"][0]["comment_id"], c_w1.to_string());

    let outside = decide_http(&app, scoped_mod_id, c_w2, 99, "approve", None).await;
    let missing = decide_http(
        &app,
        scoped_mod_id,
        CommentId::from_uuid(Uuid::now_v7()),
        99,
        "approve",
        None,
    )
    .await;
    assert_eq!(outside, missing);
    assert_eq!(outside.0, StatusCode::FORBIDDEN);
    assert_eq!(outside.1.as_deref(), Some("no-store"));
    assert_eq!(outside.2["error"]["code"], "not_moderator");

    let (st, _, body) = decide_http(&app, scoped_mod_id, c_w1, 1, "approve", None).await;
    assert_eq!(st, StatusCode::OK, "{body}");
    assert_eq!(
        decision_provenance(&pool, c_w1, 2).await,
        (
            "governance_grant".into(),
            None,
            Some(scoped_w1),
            scoped_mod_id,
            None
        )
    );

    // Granting a group grant scoped to W2 widens queue & allows deciding c_w2; revoking W1 shrinks.
    let grp = create_group_http(&app, admin_id, "Nhóm bài 2").await;
    let scoped_w2 = create_grant_http(&app, admin_id, "group", grp, "comments", Some(w2)).await;
    add_member_http(&app, admin_id, grp, scoped_mod_id).await;
    let c_w1_b = post_on(&vink, dev_seed::OTHER_PRINCIPAL, w1, None, "Bài 1 tiếp").await;

    let (st, _, body) = get_queue_http(&app, scoped_mod_id, None, None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body["pending_total"], 2);
    revoke_grant_http(&app, admin_id, scoped_w1).await;

    let (st, _, body) = get_queue_http(&app, scoped_mod_id, None, None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(body["pending_total"], 1);
    assert_eq!(body["items"][0]["comment_id"], c_w2.to_string());
    let (st, _, body) = decide_http(&app, scoped_mod_id, c_w1_b, 1, "approve", None).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "not_moderator");
    let (st, _, body) = decide_http(&app, scoped_mod_id, c_w2, 1, "approve", None).await;
    assert_eq!(st, StatusCode::OK, "{body}");
    assert_eq!(decision_provenance(&pool, c_w2, 2).await.2, Some(scoped_w2));
}

/// Admin seat, individual grant, work-scoped grant and group grant all enforce strict recusal on
/// own comments and comments on own works (excluded from queue & `pending_total`, `403
/// moderator_recused` on decision).
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn admin_and_governance_moderators_recuse_from_own_comments_and_works(pool: PgPool) {
    let app = app(&pool).await;
    let vink = seeded(&pool).await;
    let admin_id = Uuid::now_v7();
    let grp_mod_id = Uuid::now_v7();
    let peer_id = Uuid::now_v7();
    for (id, name) in [
        (admin_id, "Admin kiêm tác giả"),
        (grp_mod_id, "Duyệt nhóm kiêm tác giả"),
        (peer_id, "Duyệt viên độc lập"),
    ] {
        insert_principal(&pool, id, name).await;
    }
    grant_admin(&pool, admin_id).await;
    let group_id = create_group_http(&app, admin_id, "Nhóm duyệt").await;
    create_grant_http(&app, admin_id, "group", group_id, "comments", None).await;
    add_member_http(&app, admin_id, group_id, grp_mod_id).await;
    create_grant_http(&app, admin_id, "principal", peer_id, "comments", None).await;

    let (w_admin, _) = publish_work(&pool, &vink, admin_id, "Tác phẩm của Admin").await;
    let (w_grp, _) = publish_work(&pool, &vink, grp_mod_id, "Tác phẩm của Mod nhóm").await;

    let admins_comment_on_sample = post(&vink, principal(admin_id), None, "Admin tự viết").await;
    let reader_on_admins_work = post_on(
        &vink,
        dev_seed::OTHER_PRINCIPAL,
        w_admin,
        None,
        "Độc giả ở bài Admin",
    )
    .await;
    let grp_comment_on_sample = post(&vink, principal(grp_mod_id), None, "Mod nhóm tự viết").await;
    let reader_on_grp_work = post_on(
        &vink,
        dev_seed::OTHER_PRINCIPAL,
        w_grp,
        None,
        "Độc giả ở bài Mod nhóm",
    )
    .await;
    let neutral_comment = post(&vink, other(), None, "Bình luận trung lập").await;

    // Admin sees only grp_comment_on_sample, reader_on_grp_work and neutral_comment (total = 3).
    let (st, _, q_admin) = get_queue_http(&app, admin_id, None, None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(q_admin["pending_total"], 3);
    let admin_ids: Vec<&str> = q_admin["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["comment_id"].as_str().unwrap())
        .collect();
    assert!(!admin_ids.contains(&admins_comment_on_sample.to_string().as_str()));
    assert!(!admin_ids.contains(&reader_on_admins_work.to_string().as_str()));

    for mine in [admins_comment_on_sample, reader_on_admins_work] {
        let (st, _, body) = decide_http(&app, admin_id, mine, 1, "approve", None).await;
        assert_eq!(st, StatusCode::FORBIDDEN, "admin recuse on {mine}: {body}");
        assert_eq!(body["error"]["code"], "moderator_recused");
    }

    // Group moderator sees only admins_comment_on_sample, reader_on_admins_work, neutral (total = 3).
    let (st, _, q_grp) = get_queue_http(&app, grp_mod_id, None, None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(q_grp["pending_total"], 3);
    for mine in [grp_comment_on_sample, reader_on_grp_work] {
        let (st, _, body) = decide_http(&app, grp_mod_id, mine, 1, "approve", None).await;
        assert_eq!(st, StatusCode::FORBIDDEN, "grp recuse on {mine}: {body}");
        assert_eq!(body["error"]["code"], "moderator_recused");
    }

    // Independent peer sees and can approve all 5.
    let (st, _, q_peer) = get_queue_http(&app, peer_id, None, None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(q_peer["pending_total"], 5);
    for c in [
        admins_comment_on_sample,
        reader_on_admins_work,
        grp_comment_on_sample,
        reader_on_grp_work,
        neutral_comment,
    ] {
        let (st, _, _) = decide_http(&app, peer_id, c, 1, "approve", None).await;
        assert_eq!(st, StatusCode::OK);
    }
}

/// When an actor holds multiple overlapping bases and one is revoked or expires, decisions continue
/// via a surviving basis and audit records that live basis (never the revoked/expired one).
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn fallback_records_surviving_live_authority_provenance(pool: PgPool) {
    let app = app(&pool).await;
    let vink = seeded(&pool).await;
    let admin_id = Uuid::now_v7();
    let dual_id = Uuid::now_v7();
    insert_principal(&pool, admin_id, "Admin").await;
    insert_principal(&pool, dual_id, "Đa căn cứ").await;
    grant_admin(&pool, admin_id).await;
    grant_admin(&pool, dual_id).await;

    // Give dual_id: admin seat + individual global grant + group grant + legacy grant.
    let ind_grant = create_grant_http(&app, admin_id, "principal", dual_id, "comments", None).await;
    let grp = create_group_http(&app, admin_id, "Nhóm dự phòng").await;
    let grp_grant = create_grant_http(&app, admin_id, "group", grp, "comments", None).await;
    add_member_http(&app, admin_id, grp, dual_id).await;
    let legacy_note = GrantNote::parse("Grant kế thừa").unwrap();
    let GrantOutcome::Granted(legacy_grant) =
        grant_moderator(&pool, PrincipalId::from_uuid(dual_id), &legacy_note)
            .await
            .unwrap()
    else {
        panic!("expected legacy grant");
    };

    let c1 = post(&vink, other(), None, "1: ưu tiên admin").await;
    let c2 = post(&vink, other(), None, "2: rơi về grant cá nhân").await;
    let c3 = post(&vink, other(), None, "3: rơi về grant nhóm").await;
    let c4 = post(&vink, other(), None, "4: rơi về grant kế thừa").await;

    assert_eq!(
        decide_http(&app, dual_id, c1, 1, "approve", None).await.0,
        StatusCode::OK
    );
    assert_eq!(decision_provenance(&pool, c1, 2).await.0, "admin");

    {
        let mut conn = pool.acquire().await.unwrap();
        remove_admin_seat(&mut conn, "op", PrincipalId::from_uuid(dual_id), None)
            .await
            .unwrap();
    }
    assert_eq!(
        decide_http(&app, dual_id, c2, 1, "approve", None).await.0,
        StatusCode::OK
    );
    assert_eq!(
        decision_provenance(&pool, c2, 2).await,
        (
            "governance_grant".into(),
            None,
            Some(ind_grant),
            dual_id,
            None
        )
    );

    revoke_grant_http(&app, admin_id, ind_grant).await;
    assert_eq!(
        decide_http(&app, dual_id, c3, 1, "approve", None).await.0,
        StatusCode::OK
    );
    assert_eq!(
        decision_provenance(&pool, c3, 2).await,
        (
            "governance_grant".into(),
            None,
            Some(grp_grant),
            dual_id,
            None
        )
    );

    revoke_grant_http(&app, admin_id, grp_grant).await;
    assert_eq!(
        decide_http(&app, dual_id, c4, 1, "approve", None).await.0,
        StatusCode::OK
    );
    assert_eq!(
        decision_provenance(&pool, c4, 2).await,
        (
            "legacy_grant".into(),
            Some(legacy_grant),
            None,
            dual_id,
            None
        )
    );
}

/// Queue pagination (`limit`, `cursor`, `pending_total`) stays exact across interleaved scoped,
/// out-of-scope, recused, author-hidden and review-withdrawn works (`O-03@r2`), and refuses stale
/// cursors cleanly when authority ends mid-pagination.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn scoped_pagination_cursors_handle_interleaving_hidden_works_and_stale_cursors(
    pool: PgPool,
) {
    let app = app(&pool).await;
    let vink = seeded(&pool).await;
    let admin_id = Uuid::now_v7();
    let mod_id = Uuid::now_v7();
    let author_other = Uuid::now_v7();
    for (id, name) in [
        (admin_id, "Admin"),
        (mod_id, "Mod phân trang"),
        (author_other, "Tác giả phụ"),
    ] {
        insert_principal(&pool, id, name).await;
    }
    grant_admin(&pool, admin_id).await;

    let w1 = sample_work();
    let (w2_disallowed, _) = publish_work(&pool, &vink, author_other, "Bài ngoài phạm vi").await;
    let (w3_hidden, _) = publish_work(&pool, &vink, author_other, "Bài sẽ tự ẩn").await;
    let (w4_withdrawn, w4_rev) =
        publish_work(&pool, &vink, author_other, "Bài sẽ bị gỡ duyệt").await;
    let (w5_mods_own, _) = publish_work(&pool, &vink, mod_id, "Bài của chính mod").await;

    // Interleave comments across eligible, disallowed and recused works.
    let c_w1_1 = post_on(&vink, dev_seed::OTHER_PRINCIPAL, w1, None, "E1").await;
    let _c_w2_a = post_on(&vink, dev_seed::OTHER_PRINCIPAL, w2_disallowed, None, "D1").await;
    let _c_recused_author = post_on(&vink, mod_id, w1, None, "R1: mod viết").await;
    let c_w3 = post_on(
        &vink,
        dev_seed::OTHER_PRINCIPAL,
        w3_hidden,
        None,
        "E2: trước ẩn",
    )
    .await;
    let _c_recused_owner = post_on(
        &vink,
        dev_seed::OTHER_PRINCIPAL,
        w5_mods_own,
        None,
        "R2: bài mod",
    )
    .await;
    let c_w4 = post_on(
        &vink,
        dev_seed::OTHER_PRINCIPAL,
        w4_withdrawn,
        None,
        "E3: trước rút",
    )
    .await;
    let _c_w2_b = post_on(&vink, dev_seed::OTHER_PRINCIPAL, w2_disallowed, None, "D2").await;
    let c_w1_2 = post_on(&vink, dev_seed::OTHER_PRINCIPAL, w1, None, "E4").await;

    // Hide w3 via author `set_private`, and review-withdraw w4 via moderation case reject.
    vink.set_private(principal(author_other), w3_hidden, &key(), 2)
        .await
        .unwrap();
    let (st, _, case_body) = call(
        &app,
        "POST",
        &format!("/api/v1/moderation/works/{w4_withdrawn}/revisions/{w4_rev}/cases"),
        Some(admin_id),
        Some(ORIGIN),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "open case: {case_body}");
    let case_id = json(&case_body)["case_id"].as_str().unwrap().to_string();
    let (st, _, body) = call(
        &app,
        "POST",
        &format!("/api/v1/moderation/works/cases/{case_id}/claim"),
        Some(admin_id),
        Some(ORIGIN),
        Some(json!({ "expected_version": 1 })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "claim: {body}");
    let (st, _, body) = call(
        &app,
        "POST",
        &format!("/api/v1/moderation/works/cases/{case_id}/conclude"),
        Some(admin_id),
        Some(ORIGIN),
        Some(json!({
            "expected_version": 2,
            "verdict": "rejected",
            "reason": "Vi phạm kiểm thử rút duyệt",
        })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "conclude: {body}");

    let g_w1 = create_grant_http(&app, admin_id, "principal", mod_id, "comments", Some(w1)).await;
    let g_w3 = create_grant_http(
        &app,
        admin_id,
        "principal",
        mod_id,
        "comments",
        Some(w3_hidden),
    )
    .await;
    let g_w4 = create_grant_http(
        &app,
        admin_id,
        "principal",
        mod_id,
        "comments",
        Some(w4_withdrawn),
    )
    .await;
    let _g_w5 = create_grant_http(
        &app,
        admin_id,
        "principal",
        mod_id,
        "comments",
        Some(w5_mods_own),
    )
    .await;

    // Walk limit=2 across the 4 eligible comments: no underfilled pages, no trailing empty page.
    let (st, _, page1) = get_queue_http(&app, mod_id, None, Some(2)).await;
    assert_eq!(st, StatusCode::OK, "{page1}");
    assert_eq!(page1["pending_total"], 4);
    assert_eq!(page1["items"].as_array().unwrap().len(), 2);
    assert_eq!(page1["items"][0]["comment_id"], c_w1_1.to_string());
    assert_eq!(
        page1["items"][0]["work_title"],
        json!(dev_seed::SAMPLE_TITLE)
    );
    assert_eq!(page1["items"][1]["comment_id"], c_w3.to_string());
    assert_eq!(page1["items"][1]["work_title"], Value::Null);
    let cursor1 = page1["next_cursor"].as_str().unwrap().to_string();

    let (st, _, page2) = get_queue_http(&app, mod_id, Some(&cursor1), Some(2)).await;
    assert_eq!(st, StatusCode::OK, "{page2}");
    assert_eq!(page2["pending_total"], 4);
    assert_eq!(page2["items"].as_array().unwrap().len(), 2);
    assert_eq!(page2["items"][0]["comment_id"], c_w4.to_string());
    assert_eq!(page2["items"][0]["work_title"], Value::Null);
    assert_eq!(page2["items"][1]["comment_id"], c_w1_2.to_string());
    assert_eq!(page2["next_cursor"], Value::Null);

    // Pending comments submitted earlier on hidden / review-withdrawn works can still be moderated.
    assert_eq!(
        decide_http(&app, mod_id, c_w3, 1, "approve", None).await.0,
        StatusCode::OK
    );
    assert_eq!(
        decide_http(&app, mod_id, c_w4, 1, "approve", None).await.0,
        StatusCode::OK
    );

    // Narrowing scope mid-pagination with old cursor drops revoked works and adjusts pending_total;
    // losing remaining comments authority fails closed (`403 not_moderator`) even with old cursor.
    revoke_grant_http(&app, admin_id, g_w1).await;
    let (st, _, body) = get_queue_http(&app, mod_id, Some(&cursor1), Some(2)).await;
    assert_eq!(st, StatusCode::OK, "{body}");
    assert_eq!(body["pending_total"], 0);
    assert!(body["items"].as_array().unwrap().is_empty());

    revoke_grant_http(&app, admin_id, g_w3).await;
    revoke_grant_http(&app, admin_id, g_w4).await;
    revoke_grant_http(&app, admin_id, _g_w5).await;
    let (st, _, body) = get_queue_http(&app, mod_id, Some(&cursor1), Some(2)).await;
    assert_eq!(st, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["error"]["code"], "not_moderator");
}

/// SQL schema checks enforce valid provenance tuples, audit failures roll back comment updates
/// atomically, audit rows survive revocations, and migration upgrade/backout matches pre/post
/// state constraints.
#[sqlx::test(migrations = false)]
async fn schema_provenance_atomicity_history_and_upgrade_backout(pool: PgPool) {
    let previous = Migrator::with_migrations(
        MIGRATOR
            .migrations
            .iter()
            .filter(|m| m.version < COMMENT_GOVERNANCE_MIGRATION)
            .cloned()
            .collect(),
    );
    previous.run(&pool).await.unwrap();
    let vink = seeded(&pool).await;
    let legacy_mod = Uuid::now_v7();
    insert_principal(&pool, legacy_mod, "Legacy trước nâng cấp").await;
    let note = GrantNote::parse("Trước nâng cấp").unwrap();
    let GrantOutcome::Granted(legacy_grant) =
        grant_moderator(&pool, PrincipalId::from_uuid(legacy_mod), &note)
            .await
            .unwrap()
    else {
        panic!("expected grant");
    };
    let legacy_comment = post(&vink, other(), None, "Duyệt trước nâng cấp").await;
    sqlx::query("UPDATE comments SET review = 'approved', moderation_version = 2 WHERE id = $1")
        .bind(legacy_comment.as_uuid())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO comment_moderation_decisions
                (comment_id, version, action, moderator_id, grant_id, reason)
         VALUES ($1, 2, 'approve', $2, $3, NULL)",
    )
    .bind(legacy_comment.as_uuid())
    .bind(legacy_mod)
    .bind(legacy_grant)
    .execute(&pool)
    .await
    .unwrap();

    // Upgrade to 20261119000000: old row is preserved with authority_kind = 'legacy_grant'.
    let report = migrate(&pool).await.unwrap();
    assert_eq!(report.applied, vec![COMMENT_GOVERNANCE_MIGRATION]);
    assert_eq!(
        decision_provenance(&pool, legacy_comment, 2).await,
        (
            "legacy_grant".into(),
            Some(legacy_grant),
            None,
            legacy_mod,
            None
        )
    );

    // Pre-activation backout succeeds when only legacy decisions exist, and re-migrates cleanly.
    let down_sql = include_str!(
        "../../migrations-rollback/20261119000000_comment_moderation_governance.down.sql"
    );
    sqlx::raw_sql(down_sql).execute(&pool).await.unwrap();
    let migrated = migrate(&pool).await.unwrap();
    assert_eq!(migrated.applied, vec![COMMENT_GOVERNANCE_MIGRATION]);

    // Governance provenance constraints reject every invalid/contradictory provenance combination.
    let app = app(&pool).await;
    let admin_id = Uuid::now_v7();
    let gov_mod = Uuid::now_v7();
    insert_principal(&pool, admin_id, "Admin schema").await;
    insert_principal(&pool, gov_mod, "Gov schema").await;
    grant_admin(&pool, admin_id).await;
    let gov_grant = create_grant_http(&app, admin_id, "principal", gov_mod, "comments", None).await;
    let work_grant = create_grant_http(&app, admin_id, "principal", gov_mod, "works", None).await;

    let bad_c = post(&vink, other(), None, "Kiểm ràng buộc SQL").await;
    let shape = "comment_moderation_decisions_provenance_required";
    let authority = "comment_moderation_decisions_authority_matches";
    for (kind, actor, legacy, governance, expected) in [
        ("admin", admin_id, Some(legacy_grant), None, shape),
        ("admin", admin_id, None, Some(gov_grant), shape),
        ("governance_grant", gov_mod, None, None, shape),
        (
            "governance_grant",
            gov_mod,
            Some(legacy_grant),
            Some(gov_grant),
            shape,
        ),
        ("legacy_grant", gov_mod, None, Some(gov_grant), shape),
        ("legacy_grant", gov_mod, Some(legacy_grant), None, authority),
        (
            "governance_grant",
            admin_id,
            None,
            Some(gov_grant),
            authority,
        ),
        (
            "governance_grant",
            gov_mod,
            None,
            Some(work_grant),
            authority,
        ),
        ("admin", gov_mod, None, None, authority),
    ] {
        let err = sqlx::query(
            "INSERT INTO comment_moderation_decisions
                     (comment_id, version, action, moderator_id, authority_kind, grant_id, governance_grant_id)
             VALUES ($1, 8, 'approve', $2, $3, $4, $5)",
        )
        .bind(bad_c.as_uuid())
        .bind(actor)
        .bind(kind)
        .bind(legacy)
        .bind(governance)
        .execute(&pool)
        .await
        .unwrap_err();
        assert_eq!(constraint(err), expected);
    }

    // Audit insert failure rolls back the comment state update atomically.
    sqlx::raw_sql(
        "CREATE FUNCTION fail_comment_audit() RETURNS trigger LANGUAGE plpgsql AS $$
         BEGIN RAISE EXCEPTION 'fault audit insert'; END; $$;
         CREATE TRIGGER inject_fail_audit
           BEFORE INSERT ON comment_moderation_decisions
           FOR EACH ROW EXECUTE FUNCTION fail_comment_audit();",
    )
    .execute(&pool)
    .await
    .unwrap();
    let (st, _, _) = decide_http(&app, gov_mod, bad_c, 1, "approve", None).await;
    assert_eq!(st, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(review_of(&pool, bad_c).await, ("pending".into(), 1));
    sqlx::raw_sql(
        "DROP TRIGGER inject_fail_audit ON comment_moderation_decisions;
         DROP FUNCTION fail_comment_audit();",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Commit real governance + admin decisions; history survives revocation/seat removal, and
    // down migration refuses to drop the column once non-legacy provenance exists.
    assert_eq!(
        decide_http(&app, gov_mod, bad_c, 1, "approve", None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        decide_http(&app, admin_id, bad_c, 2, "approve", None)
            .await
            .0,
        StatusCode::OK
    );
    revoke_grant_http(&app, admin_id, gov_grant).await;
    {
        let mut conn = pool.acquire().await.unwrap();
        remove_admin_seat(&mut conn, "op", PrincipalId::from_uuid(admin_id), None)
            .await
            .unwrap();
    }
    assert_eq!(
        decision_provenance(&pool, bad_c, 2).await,
        (
            "governance_grant".into(),
            None,
            Some(gov_grant),
            gov_mod,
            None
        )
    );
    assert_eq!(
        decision_provenance(&pool, bad_c, 3).await,
        ("admin".into(), None, None, admin_id, None)
    );
    let mut conn = pool.acquire().await.unwrap();
    let refused = sqlx::raw_sql(down_sql)
        .execute(&mut *conn)
        .await
        .unwrap_err();
    assert!(refused.to_string().contains("roll forward"), "{refused}");
    sqlx::query("ROLLBACK").execute(&mut *conn).await.unwrap();
    assert_eq!(
        decision_provenance(&pool, bad_c, 2).await.2,
        Some(gov_grant)
    );
}

/// Human reaffirmation advances version cleanly, post-review worker never overrides a newer human
/// governance decision, and public/own overlays reflect decisions accurately (`rejection_reason`
/// reaches only the writer).
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn reaffirmation_post_review_and_public_overlay_behave_honestly(pool: PgPool) {
    let app = app(&pool).await;
    let vink = seeded(&pool).await;
    let admin_id = Uuid::now_v7();
    let mod_id = Uuid::now_v7();
    insert_principal(&pool, admin_id, "Admin").await;
    insert_principal(&pool, mod_id, "Mod PR").await;
    grant_admin(&pool, admin_id).await;
    create_grant_http(&app, admin_id, "principal", mod_id, "comments", None).await;

    let c_app = post(&vink, other(), None, "Duyệt rồi tái xác nhận").await;
    let c_rej = post(&vink, other(), None, "Sẽ bị từ chối").await;

    // Approve -> Reaffirm approve increments version 1 -> 2 -> 3.
    let (st, _, res1) = decide_http(&app, mod_id, c_app, 1, "approve", None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(res1["version"], 2);
    let (st, _, res2) = decide_http(&app, mod_id, c_app, 2, "approve", None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(res2["review"], "approved");
    assert_eq!(res2["version"], 3);

    // Enqueue operator post-review at observed version 3, then supersede with newer human reject
    // before post-review worker finishes: worker records `superseded` and does not overwrite.
    let ReviewWrite::Accepted(ticket) = vink
        .request_review(
            ReviewAuthority::operator(),
            true,
            &RequestReview {
                key: Uuid::now_v7(),
                comment: c_app,
                expected: ModerationVersion::new(3).unwrap(),
                note: GrantNote::parse("Operator kiểm lại").unwrap(),
            },
        )
        .await
        .unwrap()
    else {
        panic!("expected post-review ticket");
    };
    vink.record_review_result(
        ReviewAuthority::operator(),
        &ReviewResult {
            ticket,
            key: Uuid::now_v7(),
            finding: PostReviewFinding::Flagged,
            note: GrantNote::parse("Flagged bởi operator").unwrap(),
        },
    )
    .await
    .unwrap();
    let (st, _, res3) = decide_http(
        &app,
        admin_id,
        c_app,
        3,
        "reject",
        Some("Quyết định người mới hơn"),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(res3["version"], 4);

    let mut batch = queue::claim(&pool, &[POST_REVIEW_CONSUMER], 2, OutboxPolicy::DEFAULT)
        .await
        .unwrap();
    for claimed in &batch.claimed {
        if claimed.event.comment_id == c_app.as_uuid() {
            let done = queue::complete(
                &pool,
                claimed,
                &OperatorPostReview,
                OutboxPolicy::DEFAULT,
                0,
            )
            .await
            .unwrap();
            assert!(matches!(done, Completion::Succeed { .. }));
        }
    }
    let outcome: (String, i64, i64) = sqlx::query_as(
        "SELECT outcome, observed_version, resulting_version
           FROM comment_post_review_receipts WHERE ticket_id = $1",
    )
    .bind(ticket)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(outcome, ("superseded".into(), 4, 4));
    assert_eq!(review_of(&pool, c_app).await, ("rejected".into(), 4));

    // Re-approve c_app and reject c_rej; public shows only c_app, own shows c_rej + reason ONLY to
    // the writer.
    let (st, _, _) = decide_http(&app, mod_id, c_app, 4, "approve", None).await;
    assert_eq!(st, StatusCode::OK);
    let (st, _, _) = decide_http(&app, mod_id, c_rej, 1, "reject", Some(REASON)).await;
    assert_eq!(st, StatusCode::OK);

    let (_, _, pub_body) = call(
        &app,
        "GET",
        &format!("/api/v1/works/{}/discussion/public", sample_work()),
        None,
        None,
        None,
    )
    .await;
    assert!(pub_body.contains("Duyệt rồi tái xác nhận"));
    assert!(!pub_body.contains("Sẽ bị từ chối"));
    assert!(!pub_body.contains(REASON));

    let own = vink
        .own_comments(
            other(),
            sample_work(),
            &OwnCommentsQuery {
                cursor: None,
                limit: None,
                ids: Some(c_rej.to_string()),
            },
        )
        .await
        .unwrap();
    assert_eq!(own.items[0].status, OwnStatus::Rejected);
    assert_eq!(own.items[0].rejection_reason.as_deref(), Some(REASON));

    let other_me = vink
        .own_comments(
            principal(mod_id),
            sample_work(),
            &OwnCommentsQuery {
                cursor: None,
                limit: None,
                ids: Some(c_rej.to_string()),
            },
        )
        .await
        .unwrap();
    assert!(other_me.items.is_empty());

    // Writer own removal remains terminal and blocks late moderator decisions (`409 comment_removed`).
    let c_rm = post(&vink, other(), None, "Sắp tự gỡ").await;
    vink.remove_own_comment(other(), sample_work(), c_rm)
        .await
        .unwrap();
    let (st, _, body) = decide_http(&app, mod_id, c_rm, 2, "approve", None).await;
    assert_eq!(st, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "comment_removed");
    let _ = batch.claimed.drain(..);
}

/// Real PostgreSQL barrier races covering uncommitted revocations, membership removal, seat removal,
/// in-flight decisions blocking self-revocation/group-leave without AB-BA deadlocks, post-lock
/// expiry clock recheck with surviving basis fallback, and concurrent writer removal / post-review.
#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn concurrent_revocations_membership_seat_expiry_and_self_admin_races(pool: PgPool) {
    let vink = Arc::new(seeded(&pool).await);
    let admin_mod = Uuid::now_v7();
    let solo_mod = Uuid::now_v7();
    insert_principal(&pool, admin_mod, "Admin tự thu hồi").await;
    insert_principal(&pool, solo_mod, "Duyệt độc lập").await;
    grant_admin(&pool, admin_mod).await;

    let now = OffsetDateTime::now_utc();
    // 1. Uncommitted governance grant revoke waits -> commits -> moderate refuses (and falls back
    //    when another grant survives).
    let revoke_only_g = create_moderation_grant(
        &pool,
        PrincipalId::from_uuid(admin_mod),
        GrantSpec {
            grantee: Grantee::Principal(PrincipalId::from_uuid(solo_mod)),
            capability: ReviewCapability::Comments,
            scope: GrantScope::Global,
            expires_at: None,
        },
        now,
    )
    .await
    .unwrap();
    let c1 = post(&vink, other(), None, "Race 1: revoke trước").await;
    let mut revoking = pool.begin().await.unwrap();
    let rev_pid = backend_pid(&mut revoking).await;
    sqlx::query("UPDATE moderation_grants SET revoked = true, revoked_at = now() WHERE id = $1")
        .bind(revoke_only_g.as_uuid())
        .execute(&mut *revoking)
        .await
        .unwrap();
    let deciding = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move {
            vink.moderate_comment(
                principal(solo_mod),
                ModerateComment {
                    comment_id: c1,
                    expected_version: 1,
                    action: ModerationAction::Approve,
                    reason: None,
                },
            )
            .await
        })
    };
    wait_blocked_by(&pool, rev_pid).await;
    revoking.commit().await.unwrap();
    assert_eq!(deciding.await.unwrap(), Err(DiscussionError::NotModerator));

    // 2. SAME principal `admin_mod` holds Level 1 FOR SHARE via paused `moderate_comment`, while
    //    `admin_mod` concurrently calls `revoke_moderation_grant` & `set_group_membership(false)`:
    //    zero AB-BA deadlock; decision completes under held grant, then revoke/leave succeed!
    let grp = vink_app::persistence::moderation::create_moderation_group(
        &pool,
        PrincipalId::from_uuid(admin_mod),
        "Nhóm race",
        now,
    )
    .await
    .unwrap();
    set_group_membership(
        &pool,
        PrincipalId::from_uuid(admin_mod),
        grp,
        PrincipalId::from_uuid(admin_mod),
        true,
        now,
    )
    .await
    .unwrap();
    let g_self_grp = create_moderation_grant(
        &pool,
        PrincipalId::from_uuid(admin_mod),
        GrantSpec {
            grantee: Grantee::Group(grp),
            capability: ReviewCapability::Comments,
            scope: GrantScope::Global,
            expires_at: None,
        },
        now,
    )
    .await
    .unwrap();
    // Temporarily remove admin_mod's admin seat so decision rests directly on `g_self_grp`.
    // Use another admin `second_admin` or grant/restore admin seat via SQL/helper:
    // Actually keep admin_mod WITH admin seat during revoke_grant/set_group_membership so BOTH
    // `moderate_comment` (Level 1 -> 2 -> 3(admin_mod)) AND `revoke_moderation_grant` (Level 1 ->
    // 3(admin_mod)) contend on BOTH Level 1 (`g_self_grp`/`grp`) AND Level 3 (`principals(admin_mod)`).
    let g_self_ind = create_moderation_grant(
        &pool,
        PrincipalId::from_uuid(admin_mod),
        GrantSpec {
            grantee: Grantee::Principal(PrincipalId::from_uuid(admin_mod)),
            capability: ReviewCapability::Comments,
            scope: GrantScope::Global,
            expires_at: None,
        },
        now,
    )
    .await
    .unwrap();
    let c2 = post(
        &vink,
        other(),
        None,
        "Race 2: không deadlock với chính admin",
    )
    .await;
    let mut pauser = pool.begin().await.unwrap();
    let pauser_pid = backend_pid(&mut pauser).await;
    sqlx::query("SELECT 1 FROM comments WHERE id = $1 FOR UPDATE")
        .bind(c2.as_uuid())
        .execute(&mut *pauser)
        .await
        .unwrap();
    let deciding = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move {
            vink.moderate_comment(
                principal(admin_mod),
                ModerateComment {
                    comment_id: c2,
                    expected_version: 1,
                    action: ModerationAction::Approve,
                    reason: None,
                },
            )
            .await
        })
    };
    wait_blocked_by(&pool, pauser_pid).await;
    let self_revoke = {
        let pool = pool.clone();
        tokio::spawn(async move {
            revoke_moderation_grant(
                &pool,
                PrincipalId::from_uuid(admin_mod),
                g_self_ind,
                OffsetDateTime::now_utc(),
            )
            .await
        })
    };
    let self_leave = {
        let pool = pool.clone();
        tokio::spawn(async move {
            set_group_membership(
                &pool,
                PrincipalId::from_uuid(admin_mod),
                grp,
                PrincipalId::from_uuid(admin_mod),
                false,
                OffsetDateTime::now_utc(),
            )
            .await
        })
    };
    // Both revocation AND group leave wait cleanly behind the in-flight decision with 0 deadlocks.
    wait_blocked(&pool, 3).await;
    pauser.rollback().await.unwrap();
    assert_eq!(deciding.await.unwrap().unwrap().review(), Review::Approved);
    self_revoke.await.unwrap().unwrap();
    self_leave.await.unwrap().unwrap();
    let _ = g_self_grp;

    // 3. Uncommitted group removal + uncommitted admin seat removal both block then refuse/fallback.
    let g_fallback = create_moderation_grant(
        &pool,
        PrincipalId::from_uuid(admin_mod),
        GrantSpec {
            grantee: Grantee::Principal(PrincipalId::from_uuid(solo_mod)),
            capability: ReviewCapability::Comments,
            scope: GrantScope::Global,
            expires_at: None,
        },
        OffsetDateTime::now_utc(),
    )
    .await
    .unwrap();
    let g_primary = create_moderation_grant(
        &pool,
        PrincipalId::from_uuid(admin_mod),
        GrantSpec {
            grantee: Grantee::Principal(PrincipalId::from_uuid(solo_mod)),
            capability: ReviewCapability::Comments,
            scope: GrantScope::Global,
            expires_at: None,
        },
        OffsetDateTime::now_utc(),
    )
    .await
    .unwrap();
    // Make g_fallback second in creation/directory order vs g_primary first: grant order in table
    // is insertion order, so revoking g_fallback leaves g_primary as fallback cleanly.
    let c3 = post(&vink, other(), None, "Race 3: rơi về grant còn sống").await;
    let mut rev_primary = pool.begin().await.unwrap();
    let rev_primary_pid = backend_pid(&mut rev_primary).await;
    sqlx::query("UPDATE moderation_grants SET revoked = true, revoked_at = now() WHERE id = $1")
        .bind(g_fallback.as_uuid())
        .execute(&mut *rev_primary)
        .await
        .unwrap();
    let deciding = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move {
            vink.moderate_comment(
                principal(solo_mod),
                ModerateComment {
                    comment_id: c3,
                    expected_version: 1,
                    action: ModerationAction::Approve,
                    reason: None,
                },
            )
            .await
        })
    };
    wait_blocked_by(&pool, rev_primary_pid).await;
    rev_primary.commit().await.unwrap();
    assert_eq!(deciding.await.unwrap().unwrap().review(), Review::Approved);
    assert_eq!(
        decision_provenance(&pool, c3, 2).await,
        (
            "governance_grant".into(),
            None,
            Some(g_primary.as_uuid()),
            solo_mod,
            None
        )
    );
    revoke_moderation_grant(
        &pool,
        PrincipalId::from_uuid(admin_mod),
        g_primary,
        OffsetDateTime::now_utc(),
    )
    .await
    .unwrap();

    // Uncommitted group membership removal blocks `moderate_comment`, commits -> NotModerator.
    let grp_barrier = vink_app::persistence::moderation::create_moderation_group(
        &pool,
        PrincipalId::from_uuid(admin_mod),
        "Nhóm barrier",
        OffsetDateTime::now_utc(),
    )
    .await
    .unwrap();
    set_group_membership(
        &pool,
        PrincipalId::from_uuid(admin_mod),
        grp_barrier,
        PrincipalId::from_uuid(solo_mod),
        true,
        OffsetDateTime::now_utc(),
    )
    .await
    .unwrap();
    create_moderation_grant(
        &pool,
        PrincipalId::from_uuid(admin_mod),
        GrantSpec {
            grantee: Grantee::Group(grp_barrier),
            capability: ReviewCapability::Comments,
            scope: GrantScope::Global,
            expires_at: None,
        },
        OffsetDateTime::now_utc(),
    )
    .await
    .unwrap();
    let c3_grp = post(&vink, other(), None, "Race 3b: rời nhóm đang mở").await;
    let mut rem_member = pool.begin().await.unwrap();
    let rem_member_pid = backend_pid(&mut rem_member).await;
    sqlx::query("SELECT id FROM moderation_groups WHERE id = $1 FOR UPDATE")
        .bind(grp_barrier.as_uuid())
        .execute(&mut *rem_member)
        .await
        .unwrap();
    sqlx::query("DELETE FROM moderation_group_members WHERE group_id = $1 AND principal_id = $2")
        .bind(grp_barrier.as_uuid())
        .bind(solo_mod)
        .execute(&mut *rem_member)
        .await
        .unwrap();
    let deciding = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move {
            vink.moderate_comment(
                principal(solo_mod),
                ModerateComment {
                    comment_id: c3_grp,
                    expected_version: 1,
                    action: ModerationAction::Approve,
                    reason: None,
                },
            )
            .await
        })
    };
    wait_blocked_by(&pool, rem_member_pid).await;
    rem_member.commit().await.unwrap();
    assert_eq!(deciding.await.unwrap(), Err(DiscussionError::NotModerator));

    // Uncommitted admin seat removal (when temp_admin ONLY has admin seat) blocks and refuses,
    // AND paused in-flight admin decision blocks `remove_admin_seat` then completes before removal.
    let temp_admin = Uuid::now_v7();
    insert_principal(&pool, temp_admin, "Admin tạm").await;
    grant_admin(&pool, temp_admin).await;
    let c4_pause = post(&vink, other(), None, "Race 4a: admin giữ hàng").await;
    let mut pauser_admin = pool.begin().await.unwrap();
    let pauser_admin_pid = backend_pid(&mut pauser_admin).await;
    sqlx::query("SELECT 1 FROM comments WHERE id = $1 FOR UPDATE")
        .bind(c4_pause.as_uuid())
        .execute(&mut *pauser_admin)
        .await
        .unwrap();
    let deciding_admin = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move {
            vink.moderate_comment(
                principal(temp_admin),
                ModerateComment {
                    comment_id: c4_pause,
                    expected_version: 1,
                    action: ModerationAction::Approve,
                    reason: None,
                },
            )
            .await
        })
    };
    wait_blocked_by(&pool, pauser_admin_pid).await;
    let rm_seat_task = {
        let pool = pool.clone();
        tokio::spawn(async move {
            let mut conn = pool.acquire().await.unwrap();
            remove_admin_seat(&mut conn, "op", PrincipalId::from_uuid(temp_admin), None).await
        })
    };
    wait_blocked(&pool, 2).await;
    pauser_admin.rollback().await.unwrap();
    assert_eq!(
        deciding_admin.await.unwrap().unwrap().review(),
        Review::Approved
    );
    assert!(rm_seat_task.await.unwrap().unwrap());

    grant_admin(&pool, temp_admin).await;
    let c4 = post(&vink, other(), None, "Race 4b: mất ghế admin").await;
    let mut del_seat = pool.begin().await.unwrap();
    let del_seat_pid = backend_pid(&mut del_seat).await;
    sqlx::query("DELETE FROM moderation_admin_seats WHERE principal_id = $1")
        .bind(temp_admin)
        .execute(&mut *del_seat)
        .await
        .unwrap();
    let deciding = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move {
            vink.moderate_comment(
                principal(temp_admin),
                ModerateComment {
                    comment_id: c4,
                    expected_version: 1,
                    action: ModerationAction::Approve,
                    reason: None,
                },
            )
            .await
        })
    };
    wait_blocked_by(&pool, del_seat_pid).await;
    del_seat.commit().await.unwrap();
    assert_eq!(deciding.await.unwrap(), Err(DiscussionError::NotModerator));

    // 4. Expiry advancing WHILE `moderate_comment` waits on `comments FOR UPDATE`: post-lock
    //    clock recheck catches expiration (`clock_timestamp() >= expires_at`), refuses when sole
    //    basis expires AND falls back cleanly to a surviving live grant when available.
    let db_now: OffsetDateTime = sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(&pool)
        .await
        .unwrap();
    let exp = db_now + TimeDuration::milliseconds(800);
    let _g_expiring = create_moderation_grant(
        &pool,
        PrincipalId::from_uuid(admin_mod),
        GrantSpec {
            grantee: Grantee::Principal(PrincipalId::from_uuid(solo_mod)),
            capability: ReviewCapability::Comments,
            scope: GrantScope::Global,
            expires_at: Some(exp),
        },
        db_now - TimeDuration::seconds(1),
    )
    .await
    .unwrap();
    let mut pauser = pool.begin().await.unwrap();
    let pauser_pid = backend_pid(&mut pauser).await;
    sqlx::query("SELECT 1 FROM comments WHERE id = $1 FOR UPDATE")
        .bind(c4.as_uuid())
        .execute(&mut *pauser)
        .await
        .unwrap();
    let deciding = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move {
            vink.moderate_comment(
                principal(solo_mod),
                ModerateComment {
                    comment_id: c4,
                    expected_version: 1,
                    action: ModerationAction::Approve,
                    reason: None,
                },
            )
            .await
        })
    };
    wait_blocked_by(&pool, pauser_pid).await;
    loop {
        let passed: bool = sqlx::query_scalar("SELECT clock_timestamp() >= $1")
            .bind(exp)
            .fetch_one(&pool)
            .await
            .unwrap();
        if passed {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    pauser.rollback().await.unwrap();
    assert_eq!(deciding.await.unwrap(), Err(DiscussionError::NotModerator));

    // Same expiry barrier, now paired with a live surviving grant -> uses live grant provenance.
    let db_now2: OffsetDateTime = sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(&pool)
        .await
        .unwrap();
    let exp2 = db_now2 + TimeDuration::milliseconds(800);
    let g_exp_first = create_moderation_grant(
        &pool,
        PrincipalId::from_uuid(admin_mod),
        GrantSpec {
            grantee: Grantee::Principal(PrincipalId::from_uuid(solo_mod)),
            capability: ReviewCapability::Comments,
            scope: GrantScope::Global,
            expires_at: Some(exp2),
        },
        db_now2 - TimeDuration::seconds(1),
    )
    .await
    .unwrap();
    let g_live = create_moderation_grant(
        &pool,
        PrincipalId::from_uuid(admin_mod),
        GrantSpec {
            grantee: Grantee::Principal(PrincipalId::from_uuid(solo_mod)),
            capability: ReviewCapability::Comments,
            scope: GrantScope::Global,
            expires_at: None,
        },
        db_now2,
    )
    .await
    .unwrap();
    let _ = g_exp_first;
    let mut pauser2 = pool.begin().await.unwrap();
    let pauser2_pid = backend_pid(&mut pauser2).await;
    sqlx::query("SELECT 1 FROM comments WHERE id = $1 FOR UPDATE")
        .bind(c4.as_uuid())
        .execute(&mut *pauser2)
        .await
        .unwrap();
    let deciding2 = {
        let vink = Arc::clone(&vink);
        tokio::spawn(async move {
            vink.moderate_comment(
                principal(solo_mod),
                ModerateComment {
                    comment_id: c4,
                    expected_version: 1,
                    action: ModerationAction::Approve,
                    reason: None,
                },
            )
            .await
        })
    };
    wait_blocked_by(&pool, pauser2_pid).await;
    loop {
        let passed: bool = sqlx::query_scalar("SELECT clock_timestamp() >= $1")
            .bind(exp2)
            .fetch_one(&pool)
            .await
            .unwrap();
        if passed {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    pauser2.rollback().await.unwrap();
    assert_eq!(deciding2.await.unwrap().unwrap().review(), Review::Approved);
    assert_eq!(
        decision_provenance(&pool, c4, 2).await,
        (
            "governance_grant".into(),
            None,
            Some(g_live.as_uuid()),
            solo_mod,
            None
        )
    );

    // 5. Concurrent `moderate_comment` vs writer `remove_own_comment` and vs `PostReview`: 0 deadlocks.
    let c5 = post(&vink, other(), None, "Race 5: đồng thời gỡ & hậu kiểm").await;
    let (res_mod, res_rm) = tokio::join!(
        vink.moderate_comment(
            principal(solo_mod),
            ModerateComment {
                comment_id: c5,
                expected_version: 1,
                action: ModerationAction::Approve,
                reason: None,
            },
        ),
        vink.remove_own_comment(other(), sample_work(), c5),
    );
    assert!(matches!(
        (&res_mod, &res_rm),
        (Ok(_), Ok(_)) | (Err(DiscussionError::Removed), Ok(_)) | (Ok(_), Err(_))
    ));

    let c6 = post(
        &vink,
        other(),
        None,
        "Race 6: đồng thời với post-review worker",
    )
    .await;
    assert_eq!(
        vink.moderate_comment(
            principal(solo_mod),
            ModerateComment {
                comment_id: c6,
                expected_version: 1,
                action: ModerationAction::Approve,
                reason: None,
            },
        )
        .await
        .unwrap()
        .review(),
        Review::Approved
    );
    let ReviewWrite::Accepted(ticket) = vink
        .request_review(
            ReviewAuthority::operator(),
            true,
            &RequestReview {
                key: Uuid::now_v7(),
                comment: c6,
                expected: ModerationVersion::new(2).unwrap(),
                note: GrantNote::parse("Kiểm race worker").unwrap(),
            },
        )
        .await
        .unwrap()
    else {
        panic!("expected post-review ticket");
    };
    vink.record_review_result(
        ReviewAuthority::operator(),
        &ReviewResult {
            ticket,
            key: Uuid::now_v7(),
            finding: PostReviewFinding::Flagged,
            note: GrantNote::parse("Flagged").unwrap(),
        },
    )
    .await
    .unwrap();
    let mut pr_batch = queue::claim(&pool, &[POST_REVIEW_CONSUMER], 5, OutboxPolicy::DEFAULT)
        .await
        .unwrap();
    let claimed = pr_batch
        .claimed
        .drain(..)
        .find(|c| c.event.comment_id == c6.as_uuid())
        .expect("claimed c6 delivery");
    let (res_human, res_worker) = tokio::join!(
        vink.moderate_comment(
            principal(solo_mod),
            ModerateComment {
                comment_id: c6,
                expected_version: 2,
                action: ModerationAction::Approve,
                reason: None,
            },
        ),
        queue::complete(
            &pool,
            &claimed,
            &OperatorPostReview,
            OutboxPolicy::DEFAULT,
            0
        ),
    );
    // Either contender may acquire the comment first. A worker hold advances version 2 → 3,
    // so a human carrying version 2 must then conflict; a human first supersedes the ticket.
    // Require the matching durable state, receipt and audit, never accept an arbitrary error.
    let (review, outcome, observed, audits) = match res_human {
        Ok(state) => {
            assert_eq!(state.review(), Review::Approved);
            assert_eq!(state.version().get(), 3);
            ("approved", "superseded", 3_i64, 2_i64)
        }
        Err(DiscussionError::ModerationConflict { current: 3 }) => ("pending", "held", 2, 1),
        other => panic!("human race: {other:?}"),
    };
    assert!(
        matches!(
            res_worker,
            Ok(Completion::Succeed {
                already_applied: false
            })
        ),
        "worker race: {res_worker:?}"
    );
    let final_state: (String, i64, Option<String>) =
        sqlx::query_as("SELECT review, moderation_version, removal FROM comments WHERE id = $1")
            .bind(c6.as_uuid())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(final_state, (review.into(), 3, None));
    let receipt: (String, i64, i64) = sqlx::query_as(
        "SELECT outcome, observed_version, resulting_version
           FROM comment_post_review_receipts WHERE ticket_id = $1",
    )
    .bind(ticket)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(receipt, (outcome.into(), observed, 3));
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM comment_moderation_decisions WHERE comment_id = $1",
    )
    .bind(c6.as_uuid())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, audits);
    let _: GrantId = g_live;
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_seat_added_while_waiting_is_used_only_by_the_next_decision(pool: PgPool) {
    let app = app(&pool).await;
    let vink = seeded(&pool).await;
    let admin = Uuid::now_v7();
    let reviewer = Uuid::now_v7();
    insert_principal(&pool, admin, "Quản trị viên").await;
    insert_principal(&pool, reviewer, "Người duyệt").await;
    grant_admin(&pool, admin).await;
    let grant = create_grant_http(&app, admin, "principal", reviewer, "comments", None).await;
    let comment = post(&vink, other(), None, "Chờ kiểm duyệt").await;

    let mut barrier = pool.begin().await.unwrap();
    let blocker = backend_pid(&mut barrier).await;
    sqlx::query("SELECT id FROM comments WHERE id = $1 FOR UPDATE")
        .bind(comment.as_uuid())
        .execute(&mut *barrier)
        .await
        .unwrap();
    let deciding = tokio::spawn({
        let app = app.clone();
        async move { decide_http(&app, reviewer, comment, 1, "approve", None).await }
    });
    wait_blocked_by(&pool, blocker).await;
    grant_admin(&pool, reviewer).await;
    barrier.commit().await.unwrap();

    assert_eq!(deciding.await.unwrap().0, StatusCode::OK);
    assert_eq!(
        decision_provenance(&pool, comment, 2).await,
        ("governance_grant".into(), None, Some(grant), reviewer, None)
    );
    assert_eq!(
        decide_http(&app, reviewer, comment, 2, "approve", None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        decision_provenance(&pool, comment, 3).await,
        ("admin".into(), None, None, reviewer, None)
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn governance_revocation_can_wait_on_the_admin_without_blocking_a_case_grant_fk(
    pool: PgPool,
) {
    let app = app(&pool).await;
    let admin = Uuid::now_v7();
    insert_principal(&pool, admin, "Quản trị viên").await;
    grant_admin(&pool, admin).await;
    let grant = create_grant_http(&app, admin, "principal", admin, "works", None).await;

    let mut case_write = pool.begin().await.unwrap();
    let blocker = backend_pid(&mut case_write).await;
    sqlx::query("SELECT id FROM principals WHERE id = $1 FOR UPDATE")
        .bind(admin)
        .execute(&mut *case_write)
        .await
        .unwrap();
    let revoking = tokio::spawn(async move { revoke_grant_http(&app, admin, grant).await });
    wait_blocked_by(&pool, blocker).await;
    // A work-case write already holding its principal/case locks takes this same lock through
    // claimed_basis_grant_id_fkey (or concluded_basis_grant_id_fkey).
    sqlx::query("SET LOCAL lock_timeout = '2s'")
        .execute(&mut *case_write)
        .await
        .unwrap();
    sqlx::query("SELECT id FROM moderation_grants WHERE id = $1 FOR KEY SHARE")
        .bind(grant)
        .execute(&mut *case_write)
        .await
        .unwrap();
    case_write.commit().await.unwrap();
    revoking.await.unwrap();
}
