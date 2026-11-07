//! Subject → principal bindings and suspensions on real PostgreSQL (D-05/S-03 spike). Identity is
//! not involved here: these tests check the App-owned half — exactly one principal per subject,
//! no implicit linking, immutable bindings, suspension standing and isolation between accounts.

use vink_app::application::ports::{PrincipalStore, StoreError};
use vink_app::domain::principal::{PenName, Standing, SubjectKey, SuspensionReason};
use vink_app::persistence::{
    SuspensionOutcome, active_suspensions, reinstate_principal, suspend_principal,
};

use super::*;

fn subject(authority: &str, id: &str) -> SubjectKey {
    SubjectKey::new(authority, id).unwrap()
}

async fn count(pool: &PgPool, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn concurrent_first_requests_provision_exactly_one_principal_without_orphans(pool: PgPool) {
    let vink = Arc::new(Vink::new(PgStore::new(pool.clone())));
    let key = subject("vink-identity", "018f3c2e-0000-7000-8000-00000000a001");
    let before = count(&pool, "SELECT count(*) FROM principals").await;
    let tasks = (0..12).map(|i| {
        let (vink, key) = (vink.clone(), key.clone());
        tokio::spawn(async move {
            vink.provision_principal(&key, &format!("Người viết {i}"))
                .await
                .unwrap()
        })
    });
    let mut principals = Vec::new();
    let mut created = 0;
    for task in tasks {
        let provisioned = task.await.unwrap();
        created += usize::from(provisioned.created);
        assert_eq!(provisioned.bound.standing, Standing::Active);
        principals.push(provisioned.bound.principal);
    }
    principals.dedup();
    assert_eq!(principals.len(), 1, "{principals:?}");
    assert_eq!(created, 1);
    // Losers rolled back their candidate principal: no orphan rows.
    assert_eq!(
        count(&pool, "SELECT count(*) FROM principals").await,
        before + 1
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM principal_bindings").await,
        1
    );
    // A retry after a lost answer returns the same principal and keeps the first pen name.
    let again = vink.provision_principal(&key, "Tên khác").await.unwrap();
    assert!(!again.created);
    assert_eq!(again.bound.principal, principals[0]);
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn distinct_subjects_never_share_a_principal(pool: PgPool) {
    let vink = Vink::new(PgStore::new(pool.clone()));
    // Same pen name, same subject id under two authorities, two ids under one authority: three
    // principals. The App never sees an email, so it cannot link by one.
    let mut principals = Vec::new();
    for key in [
        subject("vink-identity", "a"),
        subject("vink-identity", "b"),
        subject("oidc:https://issuer.example", "a"),
    ] {
        principals.push(
            vink.provision_principal(&key, "Trùng Tên")
                .await
                .unwrap()
                .bound
                .principal,
        );
    }
    principals.sort();
    principals.dedup();
    assert_eq!(principals.len(), 3);
    assert_eq!(
        vink.principal_for_subject(&subject("vink-identity", "c"))
            .await
            .unwrap(),
        None
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn bindings_cannot_move_be_deleted_or_attach_a_second_subject(pool: PgPool) {
    let vink = Vink::new(PgStore::new(pool.clone()));
    let mine = vink
        .provision_principal(&subject("vink-identity", "mine"), "Của tôi")
        .await
        .unwrap()
        .bound
        .principal;
    let theirs = vink
        .provision_principal(&subject("vink-identity", "theirs"), "Của họ")
        .await
        .unwrap()
        .bound
        .principal;
    for statement in [
        "UPDATE principal_bindings SET principal_id = $1 WHERE subject_id = 'mine'",
        "DELETE FROM principal_bindings WHERE principal_id = $1",
    ] {
        let target = if statement.starts_with("UPDATE") {
            theirs
        } else {
            mine
        };
        assert!(
            sqlx::query(statement)
                .bind(target.as_uuid())
                .execute(&pool)
                .await
                .is_err(),
            "{statement}"
        );
    }
    // A second subject for an existing principal is refused by the schema (no implicit linking).
    let linked = sqlx::query(
        "INSERT INTO principal_bindings (authority, subject_id, principal_id) VALUES ('x', 'y', $1)",
    )
    .bind(mine.as_uuid())
    .execute(&pool)
    .await;
    assert!(linked.is_err());
    // A provisioning that fails part-way leaves nothing: a candidate id that collides with an
    // existing principal must not bind the new subject to that principal.
    let store = PgStore::new(pool.clone());
    let failed = store
        .provision_principal(
            &subject("vink-identity", "hijack"),
            mine,
            &PenName::parse("Chiếm quyền").unwrap(),
        )
        .await;
    assert!(
        matches!(failed, Err(StoreError::Integrity(_))),
        "{failed:?}"
    );
    assert_eq!(
        store
            .principal_for_subject(&subject("vink-identity", "hijack"))
            .await
            .unwrap(),
        None
    );
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn a_pen_name_change_keeps_the_principal_its_binding_and_its_drafts(pool: PgPool) {
    let vink = Vink::new(PgStore::new(pool.clone()));
    let key = subject("vink-identity", "doi-ten");
    let principal = vink
        .provision_principal(&key, "Bút Danh Cũ")
        .await
        .unwrap()
        .bound
        .principal;
    let draft = vink.create_draft(Some(principal)).await.unwrap();
    // No rename command exists yet; the row is changed directly as a labelled fixture.
    sqlx::query("UPDATE principals SET display_name = 'Bút Danh Mới' WHERE id = $1")
        .bind(principal.as_uuid())
        .execute(&pool)
        .await
        .unwrap();
    let bound = vink.principal_for_subject(&key).await.unwrap().unwrap();
    assert_eq!(bound.principal, principal);
    let drafts = vink.list_drafts(Some(principal)).await.unwrap();
    assert_eq!(drafts.len(), 1);
    assert_eq!(drafts[0].work_id, draft.work_id);
    // Another account's principal cannot see or write it.
    let other = vink
        .provision_principal(&subject("vink-identity", "nguoi-khac"), "Người Khác")
        .await
        .unwrap()
        .bound
        .principal;
    assert_eq!(
        vink.load_draft(Some(other), draft.work_id).await,
        Err(DraftError::NotFound)
    );
    assert!(vink.list_drafts(Some(other)).await.unwrap().is_empty());
}

#[sqlx::test(migrator = "vink_app::persistence::MIGRATOR")]
async fn suspension_closes_the_principal_until_lifted_and_keeps_history(pool: PgPool) {
    let vink = Vink::new(PgStore::new(pool.clone()));
    let key = subject("vink-identity", "bi-dinh-chi");
    let principal = vink
        .provision_principal(&key, "Bị Đình Chỉ")
        .await
        .unwrap()
        .bound
        .principal;
    let reason = SuspensionReason::parse("Spam lặp lại (báo cáo #12)").unwrap();
    let SuspensionOutcome::Suspended(first) =
        suspend_principal(&pool, principal, &reason).await.unwrap()
    else {
        panic!("not suspended");
    };
    assert_eq!(
        suspend_principal(&pool, principal, &reason).await.unwrap(),
        SuspensionOutcome::AlreadySuspended(first)
    );
    assert_eq!(
        vink.principal_for_subject(&key)
            .await
            .unwrap()
            .unwrap()
            .standing,
        Standing::Suspended
    );
    // Provisioning again (a retry) cannot hand back an active principal.
    let again = vink.provision_principal(&key, "Tên mới").await.unwrap();
    assert_eq!(again.bound.standing, Standing::Suspended);
    let listed = active_suspensions(&pool).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].reason, "Spam lặp lại (báo cáo #12)");

    assert_eq!(
        reinstate_principal(&pool, principal).await.unwrap(),
        Some(first)
    );
    assert_eq!(reinstate_principal(&pool, principal).await.unwrap(), None);
    assert_eq!(
        vink.principal_for_subject(&key)
            .await
            .unwrap()
            .unwrap()
            .standing,
        Standing::Active
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM principal_suspensions WHERE lifted_at IS NOT NULL"
        )
        .await,
        1
    );
    assert_eq!(
        suspend_principal(&pool, PrincipalId::from_uuid(Uuid::now_v7()), &reason).await,
        Err(StoreError::UnknownPrincipal)
    );
}
