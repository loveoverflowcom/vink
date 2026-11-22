//! At most three bounded SELECTs in one repeatable-read snapshot: the page, then two batch
//! statements over the page's IDs (Thấm/Nhận xét of public anchors; work views of every row).
//! Each batch statement runs in its own savepoint: a failure rolls back to it, so known
//! titles/state and the other counters remain usable, and only the failed counters are
//! unavailable.
use super::{
    PgStore,
    discussion::read_snapshot,
    postgres::{classify, integrity, title},
};
use crate::application::{
    author_works::{AuthorWorkSummary, CurrentPublicSummary, PublicationSummary},
    ports::{AuthorWorkStore, StoreError},
};
use crate::domain::{
    draft::DraftVersion,
    draft_query::{DraftPosition, DraftSearch, DraftSort},
    ids::{PrincipalId, RevisionId, WorkId},
    work_view::count::{FirstPublic, ViewCount, ViewCountFacts, read_view_count},
};
use sqlx::{Acquire, AssertSqlSafe, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

/// Exactly the production page SQL, exposed for reproducible EXPLAIN evidence.
pub fn author_page_sql(sort: DraftSort) -> String {
    let (after, order) = match sort {
        DraftSort::Recent => (
            "($3::timestamptz IS NULL OR updated_at < $3 OR (updated_at = $3 AND work_id > $4::uuid))",
            "updated_at DESC, work_id",
        ),
        DraftSort::Title => (
            "($3::text IS NULL OR (title_key COLLATE \"C\", work_id) > ($3 COLLATE \"C\", $4::uuid))",
            "title_key COLLATE \"C\", work_id",
        ),
    };
    format!(
        r#"
WITH candidates AS NOT MATERIALIZED (
    SELECT w.id AS work_id, d.version, coalesce(d.title, fallback.title, '') AS title,
           coalesce(d.updated_at, w.created_at) AS updated_at,
           coalesce(d.desk_title_key, vink_desk_key(coalesce(fallback.title, 'Chưa đặt tên'))) AS title_key
      FROM works w LEFT JOIN working_drafts d ON d.work_id = w.id
      LEFT JOIN LATERAL (SELECT r.title FROM revisions r WHERE r.work_id = w.id AND d.work_id IS NULL
                         ORDER BY r.created_at DESC, r.id DESC LIMIT 1) fallback ON true
     WHERE w.owner_id = $1
), page AS MATERIALIZED (
    SELECT * FROM candidates WHERE ($2 = '' OR strpos(title_key, $2) > 0) AND {after}
     ORDER BY {order} LIMIT $5
)
SELECT page.work_id, page.version, page.title, page.updated_at,
       wp.audience, wp.lifecycle, wp.generation, wp.pointer_revision_id,
       wp.intent_revision_id, im.moderation AS intent_moderation,
       latest.id AS latest_revision_id, lm.moderation AS latest_moderation,
       wp.review_withdrawn_revision_id IS NOT NULL AS review_withdrawn,
       p.revision_id AS public_revision_id, pr.title AS public_title,
       CASE WHEN d.work_id IS NULL OR pr.id IS NULL THEN NULL ELSE
           d.title IS DISTINCT FROM pr.title OR d.document IS DISTINCT FROM pr.document OR
           CASE WHEN pr.metadata_snapshot_id IS NULL THEN NULL ELSE
             ARRAY(SELECT slot::text || ':' || tag_id::text FROM work_draft_tags WHERE work_id = page.work_id ORDER BY slot)
             IS DISTINCT FROM
             ARRAY(SELECT slot::text || ':' || tag_id::text FROM work_metadata_snapshot_tags WHERE snapshot_id = pr.metadata_snapshot_id ORDER BY slot)
           END END AS has_draft_changes
  FROM page
  LEFT JOIN working_drafts d ON d.work_id = page.work_id
  LEFT JOIN work_publications wp ON wp.work_id = page.work_id
  LEFT JOIN publications p ON p.work_id = page.work_id
  LEFT JOIN revisions pr ON pr.work_id = page.work_id AND pr.id = p.revision_id
  LEFT JOIN work_revision_moderation im ON im.work_id = page.work_id AND im.revision_id = wp.intent_revision_id
  LEFT JOIN LATERAL (SELECT id FROM revisions r WHERE r.work_id = page.work_id ORDER BY created_at DESC, id DESC LIMIT 1) latest ON true
  LEFT JOIN work_revision_moderation lm ON lm.work_id = page.work_id AND lm.revision_id = latest.id
 ORDER BY {order}
"#
    )
}

/// Page IDs + exact public revision anchors only. Aggregates are independent: no fan-out.
/// The comment predicate is the actor-free public projection (approved, no removal),
/// including replies whose root is only an anchor; comments_open does not change visibility.
pub const AUTHOR_METRICS_SQL: &str = r#"
WITH anchors AS (SELECT * FROM unnest($1::uuid[], $2::uuid[]) AS a(work_id, revision_id)),
tham AS (
    SELECT a.work_id, count(e.actor_id) AS n FROM anchors a
      LEFT JOIN work_endorsements e ON e.work_id = a.work_id AND e.revision_id = a.revision_id AND e.selected
     GROUP BY a.work_id
), discussion AS (
    SELECT a.work_id, count(c.id) AS n FROM anchors a
      LEFT JOIN discussion_threads t ON t.work_id = a.work_id
      LEFT JOIN comments c ON c.thread_id = t.id AND c.review = 'approved' AND c.removal IS NULL
     GROUP BY a.work_id
)
SELECT a.work_id, tham.n AS tham, discussion.n AS comments FROM anchors a
 JOIN tham USING(work_id) JOIN discussion USING(work_id)
"#;

/// Work views of the page (#342): the stored total, the first publication and the counting
/// start of each page ID, in the page's snapshot. One statement whatever the page size (the IDs
/// are at most one page); the owner predicate repeats the page's scope so the statement can
/// never read another owner's work, whatever IDs it is given. No full text, no reader data:
/// the read rule (`domain::work_view::count`) turns these facts into the author's number.
pub const AUTHOR_VIEWS_SQL: &str = r#"
SELECT w.id AS work_id, t.views, f.provenance, f.first_published_at, c.started_at
  FROM unnest($1::uuid[]) AS page(work_id)
  JOIN works w ON w.id = page.work_id AND w.owner_id = $2
  LEFT JOIN work_view_totals t ON t.work_id = w.id
  LEFT JOIN work_first_publications f ON f.work_id = w.id
  LEFT JOIN work_view_counting c ON true
"#;

type ViewRow = (
    Uuid,
    Option<i64>,
    Option<String>,
    Option<OffsetDateTime>,
    Option<OffsetDateTime>,
);

/// The read rule's facts from one row, or `None` when the stored history is not one the schema
/// allows (the work's number is then unavailable, never guessed).
fn view_count(
    (_, total, provenance, first_published_at, started_at): &ViewRow,
) -> Option<ViewCount> {
    let first_public = match (provenance.as_deref(), first_published_at) {
        (Some("never_public"), None) => FirstPublic::Never,
        (Some("exact"), Some(at)) => FirstPublic::At(*at),
        (Some("legacy"), None) => FirstPublic::Legacy,
        (Some("unknown"), None) => FirstPublic::Unknown,
        _ => return None,
    };
    read_view_count(ViewCountFacts {
        total: *total,
        first_public,
        counting_started_at: *started_at,
    })
    .ok()
}

#[derive(sqlx::FromRow)]
struct Row {
    work_id: Uuid,
    version: Option<i64>,
    title: String,
    updated_at: OffsetDateTime,
    audience: Option<String>,
    lifecycle: Option<String>,
    generation: Option<i64>,
    pointer_revision_id: Option<Uuid>,
    intent_revision_id: Option<Uuid>,
    intent_moderation: Option<String>,
    latest_revision_id: Option<Uuid>,
    latest_moderation: Option<String>,
    review_withdrawn: bool,
    public_revision_id: Option<Uuid>,
    public_title: Option<String>,
    has_draft_changes: Option<bool>,
}
impl Row {
    fn summary(self) -> Result<AuthorWorkSummary, StoreError> {
        let consistent = self.pointer_revision_id == self.public_revision_id
            && (self.public_revision_id.is_none()
                || (self.audience.as_deref() == Some("public")
                    && self.lifecycle.as_deref() == Some("active")));
        let publication = consistent.then(|| PublicationSummary {
            audience: self.audience.unwrap_or_else(|| "private".into()),
            lifecycle: self.lifecycle.unwrap_or_else(|| "active".into()),
            generation: self.generation.unwrap_or(1),
            intent_revision_id: self.intent_revision_id.map(RevisionId::from_uuid),
            intent_moderation: self.intent_moderation,
            latest_revision_id: self.latest_revision_id.map(RevisionId::from_uuid),
            latest_moderation: self.latest_moderation,
            review_withdrawn: self.review_withdrawn,
            has_draft_changes: self.has_draft_changes,
            current_public: self
                .public_revision_id
                .zip(self.public_title)
                .map(|(id, title)| CurrentPublicSummary {
                    revision_id: RevisionId::from_uuid(id),
                    title,
                    tham: None,
                    comments: None,
                }),
        });
        Ok(AuthorWorkSummary {
            work_id: WorkId::from_uuid(self.work_id),
            title: title(&self.title)?,
            draft_version: self
                .version
                .map(|v| DraftVersion::new(v).ok_or_else(|| integrity("stored draft version", v)))
                .transpose()?,
            updated_at: self.updated_at,
            publication,
            views: None,
        })
    }
}

impl AuthorWorkStore for PgStore {
    async fn search_author_works(
        &self,
        owner: PrincipalId,
        query: &DraftSearch,
    ) -> Result<Vec<AuthorWorkSummary>, StoreError> {
        search(self.pool(), owner, query).await
    }
}
async fn search(
    pool: &PgPool,
    owner: PrincipalId,
    search: &DraftSearch,
) -> Result<Vec<AuthorWorkSummary>, StoreError> {
    let mut tx = read_snapshot(pool).await?;
    let query = sqlx::query_as::<_, Row>(AssertSqlSafe(author_page_sql(search.sort())))
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
    let mut rows = query
        .bind(search.fetch())
        .fetch_all(&mut *tx)
        .await
        .map_err(classify)?
        .into_iter()
        .map(Row::summary)
        .collect::<Result<Vec<_>, _>>()?;
    let anchors: Vec<_> = rows
        .iter()
        .take(search.size())
        .filter_map(|w| {
            w.publication
                .as_ref()?
                .current_public
                .as_ref()
                .map(|p| (w.work_id.as_uuid(), p.revision_id.as_uuid()))
        })
        .collect();
    if !anchors.is_empty() {
        let mut metrics = tx.begin().await.map_err(classify)?;
        let counts = sqlx::query_as::<_, (Uuid, i64, i64)>(AUTHOR_METRICS_SQL)
            .bind(anchors.iter().map(|a| a.0).collect::<Vec<_>>())
            .bind(anchors.iter().map(|a| a.1).collect::<Vec<_>>())
            .fetch_all(&mut *metrics)
            .await;
        match counts {
            Ok(counts) => {
                metrics.commit().await.map_err(classify)?;
                for w in &mut rows {
                    if let Some(p) = w
                        .publication
                        .as_mut()
                        .and_then(|p| p.current_public.as_mut())
                        && let Some((_, tham, comments)) =
                            counts.iter().find(|c| c.0 == w.work_id.as_uuid())
                    {
                        p.tham = Some(*tham);
                        p.comments = Some(*comments);
                    }
                }
            }
            Err(_) => {
                tracing::warn!("author work metrics unavailable");
                metrics.rollback().await.map_err(classify)?;
            }
        }
    }
    let page: Vec<Uuid> = rows
        .iter()
        .take(search.size())
        .map(|w| w.work_id.as_uuid())
        .collect();
    if !page.is_empty() {
        let mut views = tx.begin().await.map_err(classify)?;
        let counts = sqlx::query_as::<_, ViewRow>(AUTHOR_VIEWS_SQL)
            .bind(&page)
            .bind(owner.as_uuid())
            .fetch_all(&mut *views)
            .await;
        match counts {
            Ok(counts) => {
                views.commit().await.map_err(classify)?;
                let mut anomalies = 0_usize;
                for w in rows.iter_mut().take(search.size()) {
                    let found = counts.iter().find(|c| c.0 == w.work_id.as_uuid());
                    w.views = found.and_then(view_count);
                    anomalies += usize::from(w.views.is_none());
                }
                if anomalies > 0 {
                    tracing::warn!(anomalies, "author work views inconsistent");
                }
            }
            Err(_) => {
                tracing::warn!("author work views unavailable");
                views.rollback().await.map_err(classify)?;
            }
        }
    }
    tx.commit().await.map_err(classify)?;
    Ok(rows)
}
