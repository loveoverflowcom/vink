//! The recovery gate (AC-06, contract §1.3 step 5): while closed, the Gateway socket never serves
//! a projection that may disagree with the App; ingest stays open for the replay.

use super::*;

const OWNER: u8 = 0x0a;

#[sqlx::test(migrator = "vink_notifications::persistence::MIGRATOR")]
async fn a_closed_gate_refuses_inbox_calls_but_keeps_ingest_open(pool: PgPool) {
    let inbox = app(pool.clone(), Scope::Inbox);
    let ingest = app(pool.clone(), Scope::Ingest);
    let count = format!("/v1/inboxes/{}/count", id(OWNER));

    let begun = store::set_recovery(&pool, true, "restore drill")
        .await
        .unwrap();
    assert_eq!(
        begun,
        store::RecoveryChange::Changed(store::Recovery {
            recovering: true,
            epoch: 1
        })
    );
    // Idempotent: a second begin changes nothing and writes no audit row.
    assert!(matches!(
        store::set_recovery(&pool, true, "again").await.unwrap(),
        store::RecoveryChange::Unchanged(_)
    ));
    for (method, uri, body) in [
        ("GET", count.clone(), Value::Null),
        (
            "GET",
            format!("/v1/inboxes/{}/groups", id(OWNER)),
            Value::Null,
        ),
        (
            "POST",
            format!("/v1/inboxes/{}/seen", id(OWNER)),
            json!({"command_key": "k", "up_to_snapshot": "s0"}),
        ),
    ] {
        let (status, answer) = call(&inbox, method, &uri, body).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{uri}");
        assert_eq!(answer["error"]["code"], "recovering");
    }
    // A malformed request is still refused on its own terms first: the gate runs after the
    // request checks, so the answer never depends on the projection state.
    let malformed = format!("/v1/inboxes/{}/count", id(OWNER).to_string().to_uppercase());
    let (status, answer) = call(&inbox, "GET", &malformed, Value::Null).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{answer}");
    // The replay still lands while the gate is closed.
    let event = event_id();
    let text = root(event, id(1), id(OWNER), id(0xaa), id(0x31), 2, T0);
    let (status, answer) = call(
        &ingest,
        "POST",
        "/v1/ingest",
        json!({"events": [{"event_id": event, "envelope": text}]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(answer["results"][0]["status"], "applied");
    let (_, ready) = call(&ingest, "GET", "/health/ready", Value::Null).await;
    assert_eq!(ready["checks"]["recovery"], "recovering");
    assert!(store::status(&pool, T0).await.unwrap().recovering);

    store::set_recovery(&pool, false, "replay drained")
        .await
        .unwrap();
    let (status, answer) = call(&inbox, "GET", &count, Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["unseen_group_count"], 1);
    let audit: Vec<(bool, i64, String)> =
        sqlx::query_as("SELECT recovering, epoch, note FROM recovery_changes ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        audit,
        [
            (true, 1, "restore drill".to_string()),
            (false, 1, "replay drained".to_string())
        ]
    );
    // A second recovery starts a new epoch.
    let again = store::set_recovery(&pool, true, "second").await.unwrap();
    assert_eq!(
        again,
        store::RecoveryChange::Changed(store::Recovery {
            recovering: true,
            epoch: 2
        })
    );
}
