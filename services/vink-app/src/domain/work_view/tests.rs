//! Pure work-view rule: shared decision vectors (`tests/fixtures/work-views/decisions.json`),
//! identity types, digests and the horizon/retention bound.

use std::collections::HashMap;

use serde::{Deserialize, Deserializer};
use time::format_description::well_known::Rfc3339;
use uuid::{NoContext, Timestamp};

use super::*;

fn at(raw: &str) -> OffsetDateTime {
    OffsetDateTime::parse(raw, &Rfc3339).unwrap()
}

/// A UUIDv7 event minted at `time` (millisecond precision, random tail).
fn event_at(time: OffsetDateTime) -> ViewEventId {
    let nanos = time.unix_timestamp_nanos();
    let seconds = u64::try_from(nanos.div_euclid(1_000_000_000)).unwrap();
    let subsec = u32::try_from(nanos.rem_euclid(1_000_000_000)).unwrap();
    ViewEventId::from_uuid(Uuid::new_v7(Timestamp::from_unix(
        NoContext, seconds, subsec,
    )))
    .unwrap()
}

const KEY: ViewDigestKey = ViewDigestKey::from_bytes([7; 32]);

fn work(n: u128) -> WorkId {
    WorkId::from_uuid(Uuid::from_u128(n))
}

fn member(n: u128) -> ViewReader {
    ViewReader::Member(PrincipalId::from_uuid(Uuid::from_u128(n)))
}

/// `absent` → `None`, `null` → `Some(None)`, a value → `Some(Some(v))`.
fn present<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Some(Option::deserialize(deserializer)?))
}

#[derive(Deserialize)]
struct Vectors {
    actors: HashMap<String, Uuid>,
    defaults: Defaults,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Defaults {
    work: String,
    owner: String,
    published: String,
}

#[derive(Deserialize)]
struct StoredReceipt {
    work: String,
    verdict: String,
    basis: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    examples: Vec<String>,
    rules: Vec<String>,
    #[serde(default)]
    note: Option<String>,
    now: String,
    reader: String,
    event_issued_at: String,
    #[serde(default, deserialize_with = "present")]
    work: Option<Option<String>>,
    #[serde(default, deserialize_with = "present")]
    published: Option<Option<String>>,
    #[serde(default)]
    receipt: Option<StoredReceipt>,
    #[serde(default)]
    window_counted_at: Option<String>,
    expect: Expect,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expect {
    decision: String,
    #[serde(default)]
    verdict: Option<String>,
    #[serde(default, deserialize_with = "present")]
    basis: Option<Option<String>>,
    #[serde(default)]
    day: Option<String>,
    #[serde(default, deserialize_with = "present")]
    replaces: Option<Option<String>>,
    #[serde(default)]
    refusal: Option<String>,
}

fn refusal_name(refusal: ViewRefusal) -> &'static str {
    match refusal {
        ViewRefusal::OutsideHorizon => "outside_horizon",
        ViewRefusal::EventReused => "event_reused",
        ViewRefusal::WorkNotFound => "work_not_found",
    }
}

#[test]
fn shared_decision_vectors() {
    let vectors: Vectors = serde_json::from_str(include_str!(
        "../../../tests/fixtures/work-views/decisions.json"
    ))
    .unwrap();
    let id = |name: &str| *vectors.actors.get(name).unwrap_or_else(|| panic!("{name}"));
    let revision = |name: &str| RevisionId::from_uuid(id(name));
    assert!(vectors.cases.len() >= 25);
    let mut decisions = std::collections::HashSet::new();
    for case in &vectors.cases {
        let name = &case.id;
        assert!(!case.rules.is_empty(), "{name}: rules");
        let _ = (&case.examples, &case.note);
        let reader = if case.reader.starts_with('G') {
            ViewReader::Guest(GuestToken::parse(&id(&case.reader).to_string()).unwrap())
        } else {
            ViewReader::Member(PrincipalId::from_uuid(id(&case.reader)))
        };
        let target = WorkId::from_uuid(id(&vectors.defaults.work));
        let exists = case
            .work
            .clone()
            .unwrap_or(Some(vectors.defaults.work.clone()));
        let published = case
            .published
            .clone()
            .unwrap_or(Some(vectors.defaults.published.clone()));
        let facts = ViewFacts {
            work: exists.map(|_| WorkFacts {
                owner: PrincipalId::from_uuid(id(&vectors.defaults.owner)),
                published: published.as_deref().map(revision),
            }),
            receipt: case.receipt.as_ref().map(|r| {
                ViewReceipt::restore(
                    WorkId::from_uuid(id(&r.work)),
                    ViewVerdict::parse(&r.verdict).unwrap(),
                    r.basis.as_deref().map(revision),
                )
                .unwrap()
            }),
            window: case.window_counted_at.as_deref().map(at),
            now: at(&case.now),
        };
        let event = ViewEvent {
            work: target,
            id: event_at(at(&case.event_issued_at)),
            reader,
        };
        let decision = decide_view(&event, facts);
        let expect = &case.expect;
        let basis = || {
            expect
                .basis
                .clone()
                .unwrap_or_else(|| panic!("{name}: basis"))
                .as_deref()
                .map(revision)
        };
        match (expect.decision.as_str(), decision) {
            ("count", ViewDecision::Count(write)) => {
                assert_eq!(write.counted_at, facts.now, "{name}");
                assert_eq!(write.receipt.verdict(), ViewVerdict::Counted, "{name}");
                assert_eq!(write.receipt.work(), target, "{name}");
                assert_eq!(write.receipt.basis(), basis(), "{name}");
                assert_eq!(
                    write.day.to_string(),
                    expect.day.clone().unwrap(),
                    "{name}: day"
                );
                assert_eq!(
                    write.replaces,
                    expect.replaces.clone().unwrap().as_deref().map(at),
                    "{name}: replaces"
                );
                assert_eq!(decision.delta(), 1);
            }
            ("skip", ViewDecision::Skip(receipt)) => {
                assert_eq!(
                    receipt.verdict().as_str(),
                    expect.verdict.as_deref().unwrap(),
                    "{name}"
                );
                assert_eq!(receipt.work(), target, "{name}");
                assert_eq!(receipt.basis(), basis(), "{name}");
                assert_eq!(decision.delta(), 0);
            }
            ("replay", ViewDecision::Replay(receipt)) => {
                assert_eq!(
                    receipt.verdict().as_str(),
                    expect.verdict.as_deref().unwrap(),
                    "{name}"
                );
                assert_eq!(receipt.basis(), basis(), "{name}");
                assert_eq!(
                    decision.new_receipt(),
                    None,
                    "{name}: a replay writes nothing"
                );
            }
            ("refuse", ViewDecision::Refuse(refusal)) => {
                assert_eq!(
                    refusal_name(refusal),
                    expect.refusal.as_deref().unwrap(),
                    "{name}"
                );
                assert_eq!(
                    decision.new_receipt(),
                    None,
                    "{name}: a refusal writes nothing"
                );
            }
            (expected, got) => panic!("{name}: expected {expected}, got {got:?}"),
        }
        decisions.insert(expect.decision.clone());
    }
    assert_eq!(decisions.len(), 4, "every decision kind is exercised");
}

#[test]
fn event_ids_are_uuidv7_only() {
    let now = at("2026-12-01T01:00:00Z");
    let event = event_at(now);
    assert_eq!(
        ViewEventId::parse(&event.as_uuid().to_string()),
        Some(event)
    );
    assert_eq!(
        ViewEventId::parse(&event.as_uuid().simple().to_string()),
        Some(event)
    );
    assert_eq!(event.issued_at(), Some(now));
    for raw in [
        "",
        "not-a-uuid",
        "00000000-0000-0000-0000-000000000000",
        "5b1f0c7e-9d43-4a2e-8f61-2c7d9e0a4b11", // v4
        "0193a000-0000-7000-c000-00000000000a", // v7 bits, non-RFC variant
    ] {
        assert_eq!(ViewEventId::parse(raw), None, "{raw}");
    }
}

#[test]
fn an_event_time_beyond_the_calendar_is_outside_the_horizon() {
    // 48 bits of milliseconds reach past year 9999.
    let forged = ViewEventId::parse("ffffffff-ffff-7fff-bfff-ffffffffffff").unwrap();
    assert_eq!(forged.issued_at(), None);
    assert!(!forged.within_horizon(at("2026-12-01T01:00:00Z")));
}

#[test]
fn horizon_bounds_are_inclusive() {
    let now = at("2026-12-01T12:00:00Z");
    assert!(event_at(now - MAX_EVENT_AGE).within_horizon(now));
    assert!(!event_at(now - MAX_EVENT_AGE - Duration::milliseconds(1)).within_horizon(now));
    assert!(event_at(now + MAX_EVENT_LEAD).within_horizon(now));
    assert!(!event_at(now + MAX_EVENT_LEAD + Duration::milliseconds(1)).within_horizon(now));
}

/// Any replay that the horizon still admits comes within `RECEIPT_RETENTION` of the first
/// decision, so retention (which removes receipts strictly older than that) never lets a decided
/// event be decided again.
#[test]
fn a_replay_inside_the_horizon_always_finds_its_receipt() {
    let first = at("2026-12-01T12:00:00Z");
    let step = Duration::minutes(30);
    let mut issued = first - MAX_EVENT_AGE;
    while issued <= first + MAX_EVENT_LEAD {
        let event = event_at(issued);
        assert!(event.within_horizon(first));
        let mut later = first;
        while later <= first + Duration::hours(48) {
            if event.within_horizon(later) {
                assert!(later - first <= RECEIPT_RETENTION, "{issued} {later}");
            }
            later += step;
        }
        issued += step;
    }
}

#[test]
fn window_is_open_through_exactly_24_hours() {
    let counted = at("2026-12-01T01:00:00Z");
    assert!(window_open(counted, counted));
    assert!(window_open(counted, counted + VIEW_WINDOW));
    assert!(!window_open(
        counted,
        counted + VIEW_WINDOW + Duration::microseconds(1)
    ));
    // A clock that went backwards never reopens counting.
    assert!(window_open(counted, counted - Duration::minutes(5)));
}

#[test]
fn guest_tokens_are_random_uuids_and_never_printed() {
    let token = GuestToken::generate();
    assert_eq!(GuestToken::parse(&token.expose()), Some(token));
    assert_eq!(format!("{token:?}"), "GuestToken(..)");
    assert!(!format!("{:?}", ViewReader::Guest(token)).contains(&token.expose()));
    for raw in [
        "",
        "0193a000-0000-7000-8000-00000000000a", // v7
        "00000000-0000-0000-0000-000000000000",
        "5b1f0c7e-9d43-4a2e-cf61-2c7d9e0a4b11", // v4 bits, non-RFC variant
    ] {
        assert_eq!(GuestToken::parse(raw), None, "{raw}");
    }
}

#[test]
fn reader_digests_bind_reader_kind_work_and_key() {
    let raw = Uuid::parse_str("5b1f0c7e-9d43-4a2e-8f61-2c7d9e0a4b11").unwrap();
    let guest = ViewReader::Guest(GuestToken::parse(&raw.to_string()).unwrap());
    let same_bytes_member = ViewReader::Member(PrincipalId::from_uuid(raw));
    let w = work(1);
    let d = ReaderDigest::derive(&KEY, guest, w);
    assert_eq!(d, ReaderDigest::derive(&KEY, guest, w), "stable");
    assert_ne!(d, ReaderDigest::derive(&KEY, same_bytes_member, w), "kind");
    assert_ne!(d, ReaderDigest::derive(&KEY, guest, work(2)), "work");
    let other_key = ViewDigestKey::from_bytes([8; 32]);
    assert_ne!(d, ReaderDigest::derive(&other_key, guest, w), "key");
    assert_ne!(
        ReaderDigest::derive(&KEY, member(1), w),
        ReaderDigest::derive(&KEY, member(2), w)
    );
    assert_eq!(format!("{d:?}"), "ReaderDigest(..)");
    assert_eq!(format!("{KEY:?}"), "ViewDigestKey(..)");
}

#[test]
fn event_digests_hide_the_event_id() {
    let event = event_at(at("2026-12-01T01:00:00Z"));
    let digest = EventDigest::of(event);
    assert_eq!(digest, EventDigest::of(event));
    assert_ne!(
        digest,
        EventDigest::of(event_at(at("2026-12-01T01:00:00Z")))
    );
    assert_eq!(digest.hex().len(), 64);
    let simple = event.as_uuid().simple().to_string();
    assert!(!digest.hex().contains(&simple));
    assert!(!format!("{digest:?}").contains(&simple));
}

#[test]
fn digest_keys_parse_from_64_hex_digits() {
    let hex = "00112233445566778899aabbccddeeff00112233445566778899AABBCCDDEEFF";
    let key = ViewDigestKey::parse_hex(hex).unwrap();
    assert_eq!(
        ViewDigestKey::parse_hex(&format!(" {hex}\n")),
        Some(key.clone())
    );
    for raw in ["", &hex[..62], &format!("{hex}00"), &hex.replace('0', "g")] {
        assert_eq!(ViewDigestKey::parse_hex(raw), None, "{raw}");
    }
    let wide = format!("{}é", &hex[..62]);
    assert_eq!(ViewDigestKey::parse_hex(&wide), None);
}

#[test]
fn receipts_and_verdicts_round_trip_only_reachable_states() {
    let w = work(1);
    let r = Some(RevisionId::from_uuid(Uuid::from_u128(9)));
    let all = [
        ViewVerdict::Counted,
        ViewVerdict::Skipped(ViewSkip::WithinWindow),
        ViewVerdict::Skipped(ViewSkip::OwnWork),
        ViewVerdict::Skipped(ViewSkip::NotPublic),
    ];
    for verdict in all {
        assert_eq!(ViewVerdict::parse(verdict.as_str()), Some(verdict));
        let not_public = verdict == ViewVerdict::Skipped(ViewSkip::NotPublic);
        assert_eq!(ViewReceipt::restore(w, verdict, None).is_some(), not_public);
        assert_eq!(ViewReceipt::restore(w, verdict, r).is_some(), !not_public);
    }
    assert_eq!(ViewVerdict::parse("replayed"), None);
}

#[test]
fn a_decided_event_is_never_decided_again() {
    // Whatever the facts are now, a receipt of the same work replays it.
    let w = work(1);
    let now = at("2026-12-01T01:00:00Z");
    let event = ViewEvent {
        work: w,
        id: event_at(now),
        reader: member(5),
    };
    let receipt = ViewReceipt::restore(
        w,
        ViewVerdict::Counted,
        Some(RevisionId::from_uuid(Uuid::from_u128(9))),
    )
    .unwrap();
    for (published, window, later) in [
        (None, None, now),
        (
            Some(RevisionId::from_uuid(Uuid::from_u128(10))),
            None,
            now + Duration::days(3),
        ),
        (
            None,
            Some(now - Duration::days(2)),
            now + Duration::hours(30),
        ),
    ] {
        let facts = ViewFacts {
            work: Some(WorkFacts {
                owner: PrincipalId::from_uuid(Uuid::from_u128(4)),
                published,
            }),
            receipt: Some(receipt),
            window,
            now: later,
        };
        assert_eq!(decide_view(&event, facts), ViewDecision::Replay(receipt));
    }
}

#[test]
fn speculative_requests_are_recognised() {
    assert!(is_speculative(["prefetch"]));
    assert!(is_speculative(["prefetch;prerender"]));
    assert!(is_speculative(["prefetch; anonymous-client-ip"]));
    assert!(is_speculative(["", "PREFETCH"]));
    assert!(!is_speculative([]));
    assert!(!is_speculative(["", "navigate"]));
    assert!(!is_speculative(["prefetcher"]));
}

#[test]
fn the_ingest_budget_admits_up_to_its_limit() {
    assert!(INGEST_LIMIT.admits(0));
    assert!(INGEST_LIMIT.admits(INGEST_LIMIT.requests - 1));
    assert!(!INGEST_LIMIT.admits(INGEST_LIMIT.requests));
}
