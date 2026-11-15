//! #203: typed, optional persistence below the public read owner. Never authority or HTTP cache.
use crate::contract::{Publication, WorkList};
use crate::public_reads::{PublicKey, Snapshot};
use serde::{Deserialize, Serialize};

/// POLICY-01/02 are OPEN. No production read/write or offline display until exact signoff.
pub const ENABLED: bool = false;
pub const SCHEMA: u32 = 1;
pub const POLICY: &str = "public-snapshot-v1";
pub const MAX_RECORD_BYTES: usize = 512 * 1024;
pub const RETENTION_MS: u64 = 24 * 60 * 60 * 1000;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum Payload {
    Works(WorkList),
    Published(Publication),
}

/// Whole page, including its cursor. No entity references or private DTO variant.
/// Fields are private: callers can only construct this from the public snapshot allowlist.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    schema: u32,
    origin: String,
    policy: String,
    key: PublicKey,
    provenance: String,
    validated_at_ms: u64,
    payload: Payload,
}

impl Envelope {
    /// Only call for an accepted origin response, using wall time captured BEFORE its request.
    /// SSR/cache seeds have unknown cross-process age and are deliberately not persisted.
    pub fn from_origin(
        key: PublicKey,
        snapshot: &Snapshot,
        origin: String,
        validated_at_ms: u64,
    ) -> Option<Self> {
        let payload = match snapshot {
            Snapshot::Works(v) => Payload::Works(v.clone()),
            Snapshot::Publication(v) => Payload::Published(v.clone()),
            Snapshot::Control(_) => return None,
        };
        let envelope = Self {
            schema: SCHEMA,
            origin,
            policy: POLICY.into(),
            key,
            provenance: "origin_response".into(),
            validated_at_ms,
            payload,
        };
        envelope.valid().then_some(envelope)
    }

    fn valid(&self) -> bool {
        if self.schema != SCHEMA
            || self.policy != POLICY
            || self.provenance != "origin_response"
            || self.origin.is_empty()
            || self.validated_at_ms == 0
            || self.validated_at_ms > MAX_SAFE_INTEGER
        {
            return false;
        }
        match (&self.key, &self.payload) {
            (PublicKey::Reader(id), Payload::Published(p)) => {
                !id.is_empty()
                    && id == &p.work_id
                    && !p.revision_id.is_empty()
                    && p.document.schema_version == 1
            }
            (PublicKey::Works(q), Payload::Works(p)) => {
                let canonical = crate::discovery::ExploreQuery::new(
                    q.q.clone(),
                    q.tags.clone(),
                    q.cursor.clone(),
                );
                let mut ids = std::collections::BTreeSet::new();
                q == &canonical
                    && p.works.len() <= 100
                    && p.works
                        .iter()
                        .all(|w| !w.work_id.is_empty() && ids.insert(&w.work_id))
                    && p.next_cursor
                        .as_ref()
                        .is_none_or(|c| !c.is_empty() && Some(c) != q.cursor.as_ref())
            }
            _ => false,
        }
    }

    pub fn encode(&self) -> Option<String> {
        let json = serde_json::to_string(self).ok()?;
        (self.valid() && json.len() <= MAX_RECORD_BYTES).then_some(json)
    }

    /// Storage is untrusted. Roundtrip equality also rejects unknown or omitted nested fields
    /// in the network DTOs without changing the network consumer's compatibility rules.
    pub fn decode(json: &str, origin: &str, key: &PublicKey) -> Option<Self> {
        if json.len() > MAX_RECORD_BYTES {
            return None;
        }
        let raw: serde_json::Value = serde_json::from_str(json).ok()?;
        let value: Self = serde_json::from_str(json).ok()?;
        (value.valid()
            && value.origin == origin
            && &value.key == key
            && serde_json::to_value(&value).ok()? == raw)
            .then_some(value)
    }

    /// Wall time is only a conservative restart-age bound, never a grant of freshness.
    /// Clock rollback, missing/invalid time and expiry fail closed. Reads never renew this date.
    pub fn age_ms(&self, now_ms: u64) -> Option<u64> {
        if now_ms > MAX_SAFE_INTEGER {
            return None;
        }
        now_ms
            .checked_sub(self.validated_at_ms)
            .filter(|age| *age < RETENTION_MS)
    }

    pub fn snapshot(&self) -> Snapshot {
        match &self.payload {
            Payload::Works(v) => Snapshot::Works(v.clone()),
            Payload::Published(v) => Snapshot::Publication(v.clone()),
        }
    }
}

#[cfg(feature = "hydrate")]
pub mod browser {
    use super::*;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(module = "/public-cache/indexeddb.js")]
    extern "C" {
        #[wasm_bindgen(js_name = listen)]
        fn js_listen(callback: &js_sys::Function);
        #[wasm_bindgen(catch, js_name = capture)]
        async fn js_capture() -> Result<JsValue, JsValue>;
        #[wasm_bindgen(catch, js_name = restore)]
        async fn js_read(ticket: &JsValue, key: &str) -> Result<JsValue, JsValue>;
        #[wasm_bindgen(catch, js_name = write)]
        async fn js_write(ticket: &JsValue, envelope: &str) -> Result<JsValue, JsValue>;
        #[wasm_bindgen(catch, js_name = current)]
        async fn js_current(ticket: &JsValue) -> Result<JsValue, JsValue>;
        #[wasm_bindgen(catch, js_name = barrier)]
        async fn js_barrier(work_id: Option<String>) -> Result<JsValue, JsValue>;
    }

    /// Installed by the existing tab owner once; broadcasts only trigger durable rechecks.
    pub fn listen(callback: impl FnMut() + 'static) {
        if !ENABLED {
            return;
        }
        let callback = Closure::<dyn FnMut()>::new(callback);
        js_listen(callback.as_ref().unchecked_ref());
        callback.forget(); // one listener for the tab lifetime, like the read event owner
    }

    /// Captured before request/restore, never obtained lazily when a delayed response arrives.
    #[derive(Clone)]
    pub struct Lease(JsValue);
    pub async fn capture() -> Option<Lease> {
        if !ENABLED {
            return None;
        }
        let token = js_capture().await.ok()?;
        (!token.is_null() && !token.is_undefined()).then_some(Lease(token))
    }
    pub async fn current(lease: &Lease) -> bool {
        js_current(&lease.0)
            .await
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
    /// Expired is deliberately distinct from missing/unavailable; its UX is a later slice.
    pub enum RestoreRead {
        Hit(Box<Envelope>),
        Expired,
        Miss,
        Unavailable,
    }
    #[derive(Deserialize)]
    #[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
    enum WireRead {
        Hit { text: String },
        Expired,
        Miss,
        Unavailable,
    }
    pub async fn read(lease: &Lease, key: &PublicKey, origin: &str) -> RestoreRead {
        let Ok(key_json) = serde_json::to_string(key) else {
            return RestoreRead::Miss;
        };
        let Some(text) = js_read(&lease.0, &key_json)
            .await
            .ok()
            .and_then(|v| v.as_string())
        else {
            return RestoreRead::Unavailable;
        };
        match serde_json::from_str::<WireRead>(&text) {
            Ok(WireRead::Hit { text }) => Envelope::decode(&text, origin, key)
                .map(|envelope| RestoreRead::Hit(Box::new(envelope)))
                .unwrap_or(RestoreRead::Miss),
            Ok(WireRead::Expired) => RestoreRead::Expired,
            Ok(WireRead::Miss) => RestoreRead::Miss,
            _ => RestoreRead::Unavailable,
        }
    }
    /// True only for IDB transaction.oncomplete. Failure has no effect on the active RAM data.
    pub async fn write(lease: &Lease, envelope: &Envelope) -> bool {
        let Some(text) = envelope.encode() else {
            return false;
        };
        js_write(&lease.0, &text)
            .await
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
    pub async fn barrier(work_id: Option<String>) -> bool {
        if !ENABLED {
            return false;
        }
        js_barrier(work_id)
            .await
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn envelope() -> Envelope {
        let p: Publication = serde_json::from_str(include_str!(
            "../../../services/vink-app/contracts/http/v1/fixtures/publication.get.json"
        ))
        .unwrap();
        Envelope::from_origin(
            PublicKey::Reader(p.work_id.clone()),
            &Snapshot::Publication(p),
            "https://vink.test".into(),
            10_000,
        )
        .unwrap()
    }
    #[test]
    fn envelope_rejects_private_control_unknown_nested_fields_and_wrong_identity() {
        let e = envelope();
        let json = e.encode().unwrap();
        assert_eq!(Envelope::decode(&json, &e.origin, &e.key), Some(e.clone()));
        assert!(Envelope::decode(&json, "https://other.test", &e.key).is_none());
        assert!(Envelope::decode(&json, &e.origin, &PublicKey::Reader("other".into())).is_none());
        for field in [
            "token",
            "own",
            "password",
            "capabilities",
            "draft",
            "pending",
            "moderation",
        ] {
            let mut raw: serde_json::Value = serde_json::from_str(&json).unwrap();
            raw["payload"]["data"][field] = "private".into();
            assert!(Envelope::decode(&raw.to_string(), &e.origin, &e.key).is_none());
        }
        let control = Snapshot::Control(crate::contract::ReaderResponse::Hidden {
            work_id: "w".into(),
        });
        assert!(
            Envelope::from_origin(
                PublicKey::Reader("w".into()),
                &control,
                e.origin.clone(),
                10
            )
            .is_none()
        );
        let mut changed = e.clone();
        changed.schema = 2;
        assert!(changed.encode().is_none());
    }
    #[test]
    fn page_decode_requires_whole_members_cursor_scope_and_bounded_payload() {
        let list: WorkList = serde_json::from_str(include_str!(
            "../../../services/vink-app/contracts/http/v1/fixtures/works.list.json"
        ))
        .unwrap();
        let key = PublicKey::Works(crate::discovery::ExploreQuery::default());
        let e = Envelope::from_origin(
            key.clone(),
            &Snapshot::Works(list.clone()),
            "https://vink.test".into(),
            10_000,
        )
        .unwrap();
        let json = e.encode().unwrap();
        assert!(Envelope::decode(&json, &e.origin, &key).is_some());
        let mut raw: serde_json::Value = serde_json::from_str(&json).unwrap();
        raw["payload"]["data"]
            .as_object_mut()
            .unwrap()
            .remove("next_cursor");
        assert!(Envelope::decode(&raw.to_string(), &e.origin, &key).is_none());
        let mut duplicate = list.clone();
        duplicate.works.push(list.works[0].clone());
        assert!(
            Envelope::from_origin(
                key.clone(),
                &Snapshot::Works(duplicate),
                e.origin.clone(),
                10_000
            )
            .is_none()
        );
        let mut orphan = list;
        orphan.next_cursor = Some("same".into());
        let input = PublicKey::Works(crate::discovery::ExploreQuery::new(
            "".into(),
            vec![],
            Some("same".into()),
        ));
        assert!(
            Envelope::from_origin(input, &Snapshot::Works(orphan), e.origin.clone(), 10_000)
                .is_none()
        );
        let mut big = envelope();
        if let Payload::Published(p) = &mut big.payload {
            p.title = "x".repeat(MAX_RECORD_BYTES);
        }
        assert!(big.encode().is_none());
    }

    #[test]
    fn restart_age_never_renews_and_rollback_or_unknown_age_fails_closed() {
        let e = envelope();
        assert_eq!(e.age_ms(10_200), Some(200));
        assert_eq!(e.age_ms(10_200), Some(200));
        assert_eq!(e.age_ms(9_999), None);
        assert_eq!(e.age_ms(0), None);
        assert_eq!(e.age_ms(10_000 + RETENTION_MS), None);
        assert_eq!(e.age_ms(u64::MAX), None);
    }
}
