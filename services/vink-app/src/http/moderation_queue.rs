//! Versioned bounded queue envelopes; legacy arrays remain bounded and advertise Link.
use super::ApiError;
use crate::domain::{
    ids::PrincipalId,
    moderation_queue::{QueueKind, QueueQuery},
};
use axum::{
    Json,
    extract::RawQuery,
    http::{HeaderMap, HeaderValue, header},
};
use serde::Serialize;
#[derive(Serialize)]
pub struct QueuePage<T> {
    pub authority_key: String,
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}
pub fn query(
    actor: PrincipalId,
    kind: QueueKind,
    RawQuery(raw): RawQuery,
) -> Result<QueueQuery, ApiError> {
    let invalid = || ApiError::Validation {
        field: "query",
        message: "invalid queue query".into(),
    };
    let raw = raw.unwrap_or_default();
    if raw.len() > 1024 {
        return Err(invalid());
    }
    let (mut limit, mut cursor) = (None, None);
    if !raw.is_empty() {
        for part in raw.split('&') {
            let (key, value) = part.split_once('=').ok_or_else(invalid)?;
            let key = super::drafts::decode(key)?;
            let slot = match key.as_str() {
                "limit" => &mut limit,
                "cursor" => &mut cursor,
                _ => return Err(invalid()),
            };
            if slot.is_some() {
                return Err(invalid());
            }
            *slot = Some(super::drafts::decode(value)?);
        }
    }
    QueueQuery::parse(actor, kind, limit.as_deref(), cursor.as_deref()).map_err(|field| {
        ApiError::Validation {
            field,
            message: "invalid queue position or limit; restart from first page".into(),
        }
    })
}
pub fn legacy<T>(page: QueuePage<T>, endpoint: &str, size: usize) -> (HeaderMap, Json<Vec<T>>) {
    let mut headers = HeaderMap::new();
    if let Some(cursor) = page.next_cursor {
        // Cursor is canonical lowercase hex, and endpoint is a server constant.
        if let Ok(link) = HeaderValue::from_str(&format!(
            "<{endpoint}?limit={size}&cursor={cursor}>; rel=\"next\""
        )) {
            headers.insert(header::LINK, link);
        }
    }
    (headers, Json(page.items))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queue_query_rejects_unknown_duplicate_and_malformed_inputs() {
        let actor = PrincipalId::from_uuid(uuid::Uuid::from_u128(1));
        for s in [
            "q=a",
            "limit=1&limit=2",
            "cursor=",
            "limit=%",
            "limit=%ff",
            "limit=+2",
            "limit=101",
        ] {
            assert!(
                query(actor, QueueKind::Works, RawQuery(Some(s.into()))).is_err(),
                "{s}"
            );
        }
    }
    #[test]
    fn moderation_queue_provider_envelope_and_legacy_continuation_match_fixture() {
        use crate::{domain::moderation_queue::*, http::contract::CaseSummaryDto};
        let at = time::OffsetDateTime::from_unix_timestamp(1794787200).unwrap();
        let actor = PrincipalId::parse("22222222-2222-4222-8222-222222222222").unwrap();
        let id = "00000000-0000-4000-8000-000000000001";
        let q = QueueQuery::parse(actor, QueueKind::Works, None, None).unwrap();
        let cursor = q.cursor_after(at, uuid::Uuid::parse_str(id).unwrap());
        let page = QueuePage {
            authority_key: q.authority_key(true, &[]),
            items: vec![CaseSummaryDto {
                title: Some("[Mẫu] Hàng chờ ỹ ỷ".into()),
                id: id.into(),
                subject_kind: "work_revision".into(),
                work_id: "33333333-3333-4333-8333-333333333333".into(),
                revision_id: Some("44444444-4444-4444-8444-444444444444".into()),
                decision_version: 1,
                origin: "automation_paused".into(),
                version: 1,
                state: "queued".into(),
                claimed_by: None,
                created_at: at,
            }],
            next_cursor: Some(cursor.clone()),
        };
        let golden: serde_json::Value = serde_json::from_str(include_str!(
            "../../contracts/http/v1/fixtures/moderation.works.page.json"
        ))
        .unwrap();
        assert_eq!(serde_json::to_value(&page).unwrap(), golden);
        let (headers, Json(array)) = legacy(page, "/api/v1/moderation/works/queue", 50);
        assert_eq!(array.len(), 1);
        assert_eq!(
            headers[header::LINK],
            format!("</api/v1/moderation/works/queue?limit=50&cursor={cursor}>; rel=\"next\"")
        );
    }
}
