//! One owner-scoped statement, filter/order before limit+1. No body and no per-card reads.
use super::postgres::{classify, integrity, title};
use crate::application::ports::StoreError;
use crate::domain::draft::{DraftSummary, DraftVersion};
use crate::domain::draft_query::{DraftPosition, DraftSearch, DraftSort};
use crate::domain::ids::{PrincipalId, WorkId};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

// Kept public for EXPLAIN of exactly the production SQL. The caller always binds owner from
// authenticated session; position and search key never grant access to another owner's data.
pub const DRAFT_RECENT_SQL: &str = "
    SELECT d.work_id, d.version, d.title, d.updated_at
      FROM works w JOIN working_drafts d ON d.work_id = w.id
     WHERE w.owner_id = $1
       AND ($2 = '' OR strpos(d.desk_title_key, $2) > 0)
       AND ($3::timestamptz IS NULL OR d.updated_at < $3
            OR (d.updated_at = $3 AND d.work_id > $4::uuid))
     ORDER BY d.updated_at DESC, d.work_id
     LIMIT $5";

pub const DRAFT_TITLE_SQL: &str = "
    SELECT d.work_id, d.version, d.title, d.updated_at
      FROM works w JOIN working_drafts d ON d.work_id = w.id
     WHERE w.owner_id = $1
       AND ($2 = '' OR strpos(d.desk_title_key, $2) > 0)
       AND ($3::text IS NULL OR (d.desk_title_key COLLATE \"C\", d.work_id) > ($3 COLLATE \"C\", $4::uuid))
     ORDER BY d.desk_title_key COLLATE \"C\", d.work_id
     LIMIT $5";

pub(super) async fn search(
    pool: &PgPool,
    owner: PrincipalId,
    search: &DraftSearch,
) -> Result<Vec<DraftSummary>, StoreError> {
    let sql = match search.sort() {
        DraftSort::Recent => DRAFT_RECENT_SQL,
        DraftSort::Title => DRAFT_TITLE_SQL,
    };
    let query = sqlx::query_as::<_, (Uuid, i64, String, OffsetDateTime)>(sql)
        .bind(owner.as_uuid())
        .bind(search.query_key());
    let query = match search.after() {
        Some(DraftPosition::Recent {
            updated_at,
            work_id,
        }) => query.bind(Some(*updated_at)).bind(Some(work_id.as_uuid())),
        Some(DraftPosition::Title { key, work_id }) => {
            query.bind(Some(key)).bind(Some(work_id.as_uuid()))
        }
        None => match search.sort() {
            DraftSort::Recent => query.bind(None::<OffsetDateTime>).bind(None::<Uuid>),
            DraftSort::Title => query.bind(None::<&str>).bind(None::<Uuid>),
        },
    };
    query
        .bind(search.fetch())
        .fetch_all(pool)
        .await
        .map_err(classify)?
        .into_iter()
        .map(|(id, raw_version, raw_title, updated_at)| {
            Ok(DraftSummary {
                work_id: WorkId::from_uuid(id),
                version: DraftVersion::new(raw_version)
                    .ok_or_else(|| integrity("stored draft version", raw_version))?,
                title: title(&raw_title)?,
                updated_at,
            })
        })
        .collect()
}
