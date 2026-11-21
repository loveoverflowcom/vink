use super::*;
use serde_json::json;

const REPLY_ID: &str = "019a9b10-0001-7000-8000-000000000101";
const THAM_ID: &str = "019a9b10-0002-7000-8000-000000000102";

/// Contract §3 example 1, as a JSON value (key order irrelevant: the digest is canonical).
fn reply_example() -> Value {
    json!({
        "event_id": REPLY_ID,
        "event_type": "comment.reply.approved",
        "schema_version": 1,
        "source_owner": "discussion",
        "activation_epoch": 1,
        "source_order": 2,
        "publication_stamp": {"epoch": 1, "readable": true},
        "occurred_at": "2026-11-20T02:10:00Z",
        "actor_id": "11111111-1111-4111-8111-11111111111a",
        "recipients": [
            "22222222-2222-4222-8222-22222222222b",
            "33333333-3333-4333-8333-33333333333c"
        ],
        "target": {
            "kind": "work_comment_reply",
            "work_id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            "root_id": "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
            "reply_context_kind": "reply",
            "reply_target_id": "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
            "comment_id": "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
            "moderation_version": 2
        },
        "semantic_kind": "approved",
        "action_required": false,
        "payload_digest": "e6f89afc584f2f9e4464294c5db85d852c4840a8cde79c891bfb038348ed26b6"
    })
}

/// Contract §3 example 1b (Thấm remove on a hidden work).
fn tham_example() -> Value {
    json!({
        "event_id": THAM_ID,
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
        "action_required": false,
        "payload_digest": "c922b2f3355c071bf1cf9fca5ac1ac73031b34579808a0aa87ce438e35859140"
    })
}

fn id(raw: &str) -> Uuid {
    Uuid::parse_str(raw).unwrap()
}

/// Re-signs an edited envelope, as a well-behaved producer would.
fn signed(mut value: Value) -> String {
    let object = value.as_object_mut().unwrap();
    object.remove("payload_digest");
    let digest = sha256_hex(canonical(&Value::Object(object.clone())).as_bytes());
    object.insert("payload_digest".into(), digest.into());
    value.to_string()
}

#[test]
fn both_contract_examples_verify_with_their_published_digests() {
    let reply = parse(&reply_example().to_string(), id(REPLY_ID)).unwrap();
    assert_eq!(reply.event_type, EventType::CommentReplyApproved);
    assert_eq!(
        reply.payload_digest,
        "e6f89afc584f2f9e4464294c5db85d852c4840a8cde79c891bfb038348ed26b6"
    );
    assert_eq!(
        reply.target,
        Target::WorkCommentReply {
            work: id("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"),
            root: id("bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb"),
            context: ReplyContext::Reply(id("cccccccc-cccc-4ccc-8ccc-cccccccccccc")),
            comment: id("dddddddd-dddd-4ddd-8ddd-dddddddddddd"),
            moderation_version: 2,
        }
    );
    assert_eq!(reply.recipients.len(), 2);

    let tham = parse(&tham_example().to_string(), id(THAM_ID)).unwrap();
    assert_eq!(tham.semantic, SemanticKind::Remove);
    assert_eq!(
        tham.stamp,
        Some(Stamp {
            epoch: 1,
            readable: false
        })
    );
    assert_eq!(tham.first_selection, None);
}

#[test]
fn a_tampered_body_or_digest_is_refused_before_anything_else() {
    let mut value = reply_example();
    value["source_order"] = json!(3);
    assert_eq!(
        parse(&value.to_string(), id(REPLY_ID)),
        Err(Invalid::DigestMismatch)
    );
    let mut value = reply_example();
    value["payload_digest"] = json!("00".repeat(32));
    assert_eq!(
        parse(&value.to_string(), id(REPLY_ID)),
        Err(Invalid::DigestMismatch)
    );
    // The batch says one ID, the signed envelope another.
    assert_eq!(
        parse(&reply_example().to_string(), id(THAM_ID)),
        Err(Invalid::EventIdMismatch)
    );
}

#[test]
fn duplicate_keys_unknown_fields_and_wrong_types_are_malformed() {
    let text = reply_example().to_string();
    let duplicated = text.replacen(
        "\"schema_version\":1",
        "\"schema_version\":1,\"schema_version\":1",
        1,
    );
    assert_ne!(duplicated, text);
    assert_eq!(parse(&duplicated, id(REPLY_ID)), Err(Invalid::Malformed));

    let mut extra = reply_example();
    extra["moderator"] = json!("someone");
    assert_eq!(parse(&signed(extra), id(REPLY_ID)), Err(Invalid::Malformed));

    let mut nested = reply_example();
    nested["target"]["comment_body"] = json!("chữ");
    assert_eq!(
        parse(&signed(nested), id(REPLY_ID)),
        Err(Invalid::Malformed)
    );

    let mut float = reply_example();
    float["schema_version"] = json!(1.0);
    assert_eq!(parse(&signed(float), id(REPLY_ID)), Err(Invalid::Malformed));

    assert_eq!(parse("[]", id(REPLY_ID)), Err(Invalid::Malformed));
    assert_eq!(
        parse(&"x".repeat(MAX_ENVELOPE_BYTES + 1), id(REPLY_ID)),
        Err(Invalid::TooLarge)
    );
}

#[test]
fn identifiers_and_recipients_must_be_canonical() {
    let mut upper = reply_example();
    upper["actor_id"] = json!("11111111-1111-4111-8111-11111111111A");
    assert_eq!(
        parse(&signed(upper), id(REPLY_ID)),
        Err(Invalid::NonCanonical)
    );

    let mut unsorted = reply_example();
    unsorted["recipients"] = json!([
        "33333333-3333-4333-8333-33333333333c",
        "22222222-2222-4222-8222-22222222222b"
    ]);
    assert_eq!(
        parse(&signed(unsorted), id(REPLY_ID)),
        Err(Invalid::NonCanonical)
    );

    let mut duplicate = reply_example();
    duplicate["recipients"] = json!([
        "22222222-2222-4222-8222-22222222222b",
        "22222222-2222-4222-8222-22222222222b"
    ]);
    assert_eq!(
        parse(&signed(duplicate), id(REPLY_ID)),
        Err(Invalid::NonCanonical)
    );
}

#[test]
fn the_reserved_admin_type_unknown_types_and_other_schemas_are_refused() {
    let mut admin = reply_example();
    admin["event_type"] = json!("admin.notice.reserved");
    assert_eq!(
        parse(&signed(admin), id(REPLY_ID)),
        Err(Invalid::ReservedType)
    );

    let mut follow = reply_example();
    follow["event_type"] = json!("profile.follow.created");
    assert_eq!(
        parse(&signed(follow), id(REPLY_ID)),
        Err(Invalid::UnknownType)
    );

    let mut helpful = tham_example();
    helpful["event_type"] = json!("comment.endorsement.huu_ich.changed");
    assert_eq!(
        parse(&signed(helpful), id(THAM_ID)),
        Err(Invalid::UnknownType)
    );

    let mut v2 = reply_example();
    v2["schema_version"] = json!(2);
    assert_eq!(parse(&signed(v2), id(REPLY_ID)), Err(Invalid::Schema));
}

#[test]
fn each_type_carries_exactly_its_own_fields() {
    // Thấm add needs first_selection; remove must not carry it.
    let mut add = tham_example();
    add["semantic_kind"] = json!("add");
    assert_eq!(
        parse(&signed(add.clone()), id(THAM_ID)),
        Err(Invalid::Shape)
    );
    add["first_selection"] = json!(true);
    assert!(parse(&signed(add), id(THAM_ID)).is_ok());
    let mut remove = tham_example();
    remove["first_selection"] = json!(false);
    assert_eq!(parse(&signed(remove), id(THAM_ID)), Err(Invalid::Shape));

    // A decision never names its maker.
    let decided = json!({
        "event_id": REPLY_ID,
        "event_type": "work.moderation.decided",
        "schema_version": 1,
        "source_owner": "publication_moderation",
        "activation_epoch": 1,
        "source_order": 3,
        "occurred_at": "2026-11-20T02:10:00Z",
        "actor_id": null,
        "recipients": ["22222222-2222-4222-8222-22222222222b"],
        "target": {
            "kind": "work_moderation_decision",
            "work_id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            "revision_id": "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee",
            "decision_version": 3
        },
        "semantic_kind": "rejected",
        "action_required": true
    });
    assert!(parse(&signed(decided.clone()), id(REPLY_ID)).is_ok());
    let mut named = decided.clone();
    named["actor_id"] = json!("11111111-1111-4111-8111-11111111111a");
    assert_eq!(parse(&signed(named), id(REPLY_ID)), Err(Invalid::Shape));
    let mut order = decided;
    order["source_order"] = json!(4);
    assert_eq!(parse(&signed(order), id(REPLY_ID)), Err(Invalid::Shape));

    // An approval cannot be addressed to its own actor.
    let mut selfish = reply_example();
    selfish["recipients"] = json!(["11111111-1111-4111-8111-11111111111a"]);
    assert_eq!(parse(&signed(selfish), id(REPLY_ID)), Err(Invalid::Shape));

    // Invalidation: no recipients, no actor, a known internal target.
    let invalidation = json!({
        "event_id": REPLY_ID,
        "event_type": "notification.invalidation",
        "schema_version": 1,
        "source_owner": "discussion",
        "activation_epoch": 1,
        "source_order": 5,
        "occurred_at": "2026-11-20T02:10:00Z",
        "actor_id": null,
        "recipients": [],
        "target": {
            "kind": "comment",
            "work_id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            "comment_id": "dddddddd-dddd-4ddd-8ddd-dddddddddddd"
        },
        "semantic_kind": "invalidate",
        "action_required": false
    });
    let parsed = parse(&signed(invalidation.clone()), id(REPLY_ID)).unwrap();
    assert!(!parsed.event_type.grouped());
    let mut addressed = invalidation;
    addressed["recipients"] = json!(["22222222-2222-4222-8222-22222222222b"]);
    assert_eq!(parse(&signed(addressed), id(REPLY_ID)), Err(Invalid::Shape));
}

#[test]
fn reply_contexts_are_root_reply_or_purged() {
    let mut root = reply_example();
    root["target"]["reply_context_kind"] = json!("root");
    assert_eq!(
        parse(&signed(root.clone()), id(REPLY_ID)),
        Err(Invalid::Shape)
    );
    root["target"]["reply_target_id"] = json!("bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb");
    let parsed = parse(&signed(root), id(REPLY_ID)).unwrap();
    assert!(matches!(
        parsed.target,
        Target::WorkCommentReply {
            context: ReplyContext::Root,
            ..
        }
    ));
    let mut purged = reply_example();
    purged["target"]["reply_target_id"] = Value::Null;
    let parsed = parse(&signed(purged), id(REPLY_ID)).unwrap();
    assert!(matches!(
        parsed.target,
        Target::WorkCommentReply {
            context: ReplyContext::Purged,
            ..
        }
    ));
}
