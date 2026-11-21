//! Work-view use case (#341) without a database: input validation, digests handed to storage,
//! error mapping, prune rounds, and the shared timeline oracle
//! (`tests/fixtures/work-views/scenario.json`) replayed through an in-memory store that applies
//! `decide_view` exactly as the PostgreSQL transaction does. `tests/postgres/work_view.rs`
//! replays the same file on real PostgreSQL.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::Deserialize;
use time::format_description::well_known::Rfc3339;
use time::{Date, OffsetDateTime};
use uuid::{NoContext, Timestamp, Uuid};

use super::Vink;
use super::ports::{StoreError, ViewPrune, ViewWrite, WorkViewStore};
use super::work_view::{
    MAX_PRUNE_ROUNDS, RecordWorkView, Viewer, WorkViewError, WorkViewInput, WorkViewResult,
};
use crate::domain::ids::{PrincipalId, RevisionId, WorkId};
use crate::domain::work_view::{
    EventDigest, GuestToken, RECEIPT_RETENTION, ReaderDigest, VIEW_WINDOW, ViewDecision,
    ViewDigestKey, ViewEventId, ViewFacts, ViewReader, ViewReceipt, ViewRefusal, ViewSkip,
    ViewVerdict, WorkFacts, decide_view,
};

const KEY: ViewDigestKey = ViewDigestKey::from_bytes([42; 32]);

fn at(raw: &str) -> OffsetDateTime {
    OffsetDateTime::parse(raw, &Rfc3339).unwrap()
}

fn event_at(time: OffsetDateTime) -> String {
    let nanos = time.unix_timestamp_nanos();
    let seconds = u64::try_from(nanos.div_euclid(1_000_000_000)).unwrap();
    let subsec = u32::try_from(nanos.rem_euclid(1_000_000_000)).unwrap();
    Uuid::new_v7(Timestamp::from_unix(NoContext, seconds, subsec)).to_string()
}

/// The PostgreSQL schema as maps, written only through `decide_view`'s decisions.
#[derive(Default)]
struct Model {
    works: HashMap<WorkId, WorkFacts>,
    windows: HashMap<(WorkId, ReaderDigest), OffsetDateTime>,
    receipts: HashMap<EventDigest, (ViewReceipt, OffsetDateTime)>,
    totals: HashMap<WorkId, i64>,
    days: HashMap<(WorkId, Date), i64>,
    started: Option<OffsetDateTime>,
    writes: Vec<ViewWrite>,
}

#[derive(Default)]
struct MemoryViews(Mutex<Model>);

impl MemoryViews {
    fn with_work(work: WorkId, owner: PrincipalId, published: Option<RevisionId>) -> Self {
        let store = Self::default();
        store
            .0
            .lock()
            .unwrap()
            .works
            .insert(work, WorkFacts { owner, published });
        store
    }

    fn model(&self) -> std::sync::MutexGuard<'_, Model> {
        self.0.lock().unwrap()
    }
}

impl WorkViewStore for MemoryViews {
    async fn record_view(&self, write: ViewWrite) -> Result<ViewDecision, StoreError> {
        let mut m = self.0.lock().unwrap();
        m.writes.push(write);
        let work = m.works.get(&write.event.work).copied();
        let receipt = m.receipts.get(&write.event_digest).map(|(r, _)| *r);
        let window = if work.is_some() && receipt.is_none() {
            m.windows.get(&(write.event.work, write.reader)).copied()
        } else {
            None
        };
        let decision = decide_view(
            &write.event,
            ViewFacts {
                work,
                receipt,
                window,
                now: write.now,
            },
        );
        if let ViewDecision::Count(count) = decision {
            let key = (write.event.work, write.reader);
            assert_eq!(m.windows.get(&key).copied(), count.replaces);
            m.windows.insert(key, count.counted_at);
            *m.totals.entry(write.event.work).or_default() += 1;
            *m.days.entry((write.event.work, count.day)).or_default() += 1;
        }
        if let Some(kept) = decision.new_receipt() {
            assert!(
                m.receipts
                    .insert(write.event_digest, (kept, write.now))
                    .is_none()
            );
            m.started.get_or_insert(write.now);
        }
        Ok(decision)
    }

    async fn prune_views(&self, now: OffsetDateTime, limit: i64) -> Result<ViewPrune, StoreError> {
        let limit = usize::try_from(limit).unwrap();
        let mut m = self.0.lock().unwrap();
        let mut doomed: Vec<_> = m
            .windows
            .iter()
            .filter(|(_, counted)| **counted < now - VIEW_WINDOW)
            .map(|(k, _)| *k)
            .collect();
        doomed.truncate(limit);
        for key in &doomed {
            m.windows.remove(key);
        }
        let mut stale: Vec<_> = m
            .receipts
            .iter()
            .filter(|(_, (_, decided))| *decided < now - RECEIPT_RETENTION)
            .map(|(k, _)| *k)
            .collect();
        stale.truncate(limit);
        for key in &stale {
            m.receipts.remove(key);
        }
        Ok(ViewPrune {
            windows: doomed.len() as u64,
            receipts: stale.len() as u64,
        })
    }
}

fn ids() -> (WorkId, PrincipalId, RevisionId) {
    (
        WorkId::from_uuid(Uuid::from_u128(0xA)),
        PrincipalId::from_uuid(Uuid::from_u128(0xA1)),
        RevisionId::from_uuid(Uuid::from_u128(0xE1)),
    )
}

#[derive(Deserialize)]
struct Scenario {
    readers: HashMap<String, String>,
    steps: Vec<Step>,
    #[serde(rename = "final")]
    end: Final,
}

#[derive(Deserialize)]
struct Step {
    at: String,
    #[serde(rename = "do")]
    action: String,
    #[serde(default)]
    reader: Option<String>,
    #[serde(default)]
    event: Option<String>,
    #[serde(default)]
    expect: Option<String>,
    #[serde(default)]
    basis: Option<String>,
    #[serde(default)]
    total: Option<i64>,
    #[serde(default)]
    pruned: Option<Pruned>,
}

#[derive(Deserialize)]
struct Pruned {
    windows: u64,
    receipts: u64,
}

#[derive(Deserialize)]
struct Final {
    total: i64,
    days: HashMap<String, i64>,
    windows: usize,
    receipts: usize,
    counting_started_at: String,
}

/// The result of a step in the fixture's words.
fn spoken(result: WorkViewResult) -> String {
    let skip = |s: ViewSkip| ViewVerdict::Skipped(s).as_str().to_string();
    match result {
        WorkViewResult::Counted => "counted".into(),
        WorkViewResult::Skipped(s) => skip(s),
        WorkViewResult::Replayed(v) => format!("replayed:{}", v.as_str()),
        WorkViewResult::Ignored(r) => format!(
            "refused:{}",
            match r {
                ViewRefusal::OutsideHorizon => "outside_horizon",
                ViewRefusal::EventReused => "event_reused",
                ViewRefusal::WorkNotFound => "work_not_found",
            }
        ),
    }
}

#[tokio::test]
async fn shared_timeline_oracle_in_memory() {
    let scenario: Scenario = serde_json::from_str(include_str!(
        "../../tests/fixtures/work-views/scenario.json"
    ))
    .unwrap();
    let (work, author, r1) = ids();
    let r2 = RevisionId::from_uuid(Uuid::from_u128(0xE2));
    let vink = Vink::new(MemoryViews::with_work(work, author, Some(r1)));
    let mut viewers: HashMap<String, Viewer> = HashMap::new();
    for (name, kind) in &scenario.readers {
        let viewer = match (name.as_str(), kind.as_str()) {
            ("T", "member") => Viewer::Member(author),
            (_, "member") => Viewer::Member(PrincipalId::from_uuid(Uuid::now_v7())),
            (_, "guest") => Viewer::Guest(GuestToken::generate().expose()),
            other => panic!("{other:?}"),
        };
        viewers.insert(name.clone(), viewer);
    }
    let revisions = HashMap::from([("R1", r1), ("R2", r2)]);
    let mut events: HashMap<String, String> = HashMap::new();
    let mut hidden = None;
    for (n, step) in scenario.steps.iter().enumerate() {
        let now = at(&step.at);
        let label = format!("step {} ({} at {})", n + 1, step.action, step.at);
        match step.action.as_str() {
            "view" => {
                let event = events
                    .entry(step.event.clone().unwrap())
                    .or_insert_with(|| event_at(now))
                    .clone();
                let viewer = viewers[step.reader.as_deref().unwrap()].clone();
                let record = vink
                    .record_work_view(
                        &KEY,
                        viewer,
                        RecordWorkView {
                            work_id: work,
                            event_id: event,
                        },
                        now,
                    )
                    .await
                    .unwrap();
                assert_eq!(
                    &spoken(record.result),
                    step.expect.as_ref().unwrap(),
                    "{label}"
                );
                assert_eq!(
                    record.basis,
                    step.basis.as_deref().map(|b| revisions[b]),
                    "{label}: basis"
                );
                let total = vink.store().model().totals.get(&work).copied().unwrap_or(0);
                assert_eq!(Some(total), step.total, "{label}: total");
            }
            "replace_revision" => {
                vink.store().model().works.get_mut(&work).unwrap().published = Some(r2);
            }
            "set_private" => {
                let mut m = vink.store().model();
                let facts = m.works.get_mut(&work).unwrap();
                hidden = facts.published.take();
            }
            "republish" => {
                vink.store().model().works.get_mut(&work).unwrap().published = hidden.take();
            }
            "prune" => {
                let pruned = vink.prune_work_views(now, 1_000).await.unwrap();
                let expected = step.pruned.as_ref().unwrap();
                assert_eq!(
                    (pruned.windows, pruned.receipts),
                    (expected.windows, expected.receipts),
                    "{label}"
                );
                assert!(pruned.drained);
            }
            other => panic!("unknown action {other}"),
        }
    }
    let m = vink.store().model();
    assert_eq!(m.totals[&work], scenario.end.total);
    let days: HashMap<String, i64> = m
        .days
        .iter()
        .map(|((_, day), views)| (day.to_string(), *views))
        .collect();
    assert_eq!(days, scenario.end.days);
    assert_eq!(days.values().sum::<i64>(), m.totals[&work], "days add up");
    assert_eq!(m.windows.len(), scenario.end.windows);
    assert_eq!(m.receipts.len(), scenario.end.receipts);
    assert_eq!(m.started, Some(at(&scenario.end.counting_started_at)));
}

#[tokio::test]
async fn untrusted_input_is_refused_before_storage() {
    let (work, author, r1) = ids();
    let vink = Vink::new(MemoryViews::with_work(work, author, Some(r1)));
    let now = at("2026-12-01T01:00:00Z");
    let member = Viewer::Member(PrincipalId::from_uuid(Uuid::from_u128(7)));
    for raw in ["", "nope", "5b1f0c7e-9d43-4a2e-8f61-2c7d9e0a4b11"] {
        let error = vink
            .record_work_view(
                &KEY,
                member.clone(),
                RecordWorkView {
                    work_id: work,
                    event_id: raw.into(),
                },
                now,
            )
            .await
            .unwrap_err();
        assert_eq!(error, WorkViewError::Invalid(WorkViewInput::EventId));
        assert_eq!(WorkViewInput::EventId.field(), "event_id");
    }
    for token in ["", "0193a000-0000-7000-8000-00000000000a", "x"] {
        let error = vink
            .record_work_view(
                &KEY,
                Viewer::Guest(token.into()),
                RecordWorkView {
                    work_id: work,
                    event_id: event_at(now),
                },
                now,
            )
            .await
            .unwrap_err();
        assert_eq!(error, WorkViewError::Invalid(WorkViewInput::GuestToken));
    }
    assert!(vink.store().model().writes.is_empty());
    let token = GuestToken::generate().expose();
    assert!(!format!("{:?}", Viewer::Guest(token.clone())).contains(&token));
}

#[tokio::test]
async fn storage_receives_digests_and_a_microsecond_clock() {
    let (work, author, r1) = ids();
    let vink = Vink::new(MemoryViews::with_work(work, author, Some(r1)));
    let now = at("2026-12-01T01:00:00.123456789Z");
    let token = GuestToken::generate();
    let event = event_at(now);
    let record = vink
        .record_work_view(
            &KEY,
            Viewer::Guest(token.expose()),
            RecordWorkView {
                work_id: work,
                event_id: event.clone(),
            },
            now,
        )
        .await
        .unwrap();
    assert_eq!(record.result, WorkViewResult::Counted);
    assert_eq!(record.delta(), 1);
    assert_eq!(record.basis, Some(r1));
    assert_eq!(record.decided_at, at("2026-12-01T01:00:00.123456Z"));
    let m = vink.store().model();
    let write = m.writes[0];
    let id = ViewEventId::parse(&event).unwrap();
    assert_eq!(write.event_digest, EventDigest::of(id));
    assert_eq!(
        write.reader,
        ReaderDigest::derive(&KEY, ViewReader::Guest(token), work)
    );
    assert_eq!(write.now, record.decided_at);
}

struct FailingViews(StoreError);

impl WorkViewStore for FailingViews {
    async fn record_view(&self, _: ViewWrite) -> Result<ViewDecision, StoreError> {
        Err(self.0.clone())
    }

    async fn prune_views(&self, _: OffsetDateTime, _: i64) -> Result<ViewPrune, StoreError> {
        Err(self.0.clone())
    }
}

#[tokio::test]
async fn storage_failures_are_never_a_result() {
    let (work, ..) = ids();
    let now = at("2026-12-01T01:00:00Z");
    for (error, expected) in [
        (
            StoreError::Unavailable("down".into()),
            WorkViewError::Unavailable,
        ),
        (StoreError::Integrity("bad".into()), WorkViewError::Internal),
        (StoreError::UnknownPrincipal, WorkViewError::Internal),
    ] {
        let vink = Vink::new(FailingViews(error));
        let got = vink
            .record_work_view(
                &KEY,
                Viewer::Member(PrincipalId::from_uuid(Uuid::from_u128(7))),
                RecordWorkView {
                    work_id: work,
                    event_id: event_at(now),
                },
                now,
            )
            .await
            .unwrap_err();
        assert_eq!(got, expected);
    }
}

/// Always reports a full batch, so only the round limit stops the loop.
struct BacklogViews(AtomicUsize);

impl WorkViewStore for BacklogViews {
    async fn record_view(&self, _: ViewWrite) -> Result<ViewDecision, StoreError> {
        unreachable!()
    }

    async fn prune_views(&self, _: OffsetDateTime, limit: i64) -> Result<ViewPrune, StoreError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(ViewPrune {
            windows: 0,
            receipts: limit as u64,
        })
    }
}

#[tokio::test]
async fn prune_rounds_stop_when_drained_or_at_the_round_limit() {
    let (work, author, r1) = ids();
    let memory = Vink::new(MemoryViews::with_work(work, author, Some(r1)));
    let quiet = memory
        .prune_work_views(at("2026-12-01T01:00:00Z"), 10)
        .await
        .unwrap();
    assert_eq!((quiet.windows, quiet.receipts, quiet.drained), (0, 0, true));

    let backlog = Vink::new(BacklogViews(AtomicUsize::new(0)));
    let pruned = backlog
        .prune_work_views(at("2026-12-01T01:00:00Z"), 10)
        .await
        .unwrap();
    assert!(!pruned.drained);
    assert_eq!(backlog.store().0.load(Ordering::SeqCst), MAX_PRUNE_ROUNDS);
    assert_eq!(pruned.receipts, 10 * MAX_PRUNE_ROUNDS as u64);
}
