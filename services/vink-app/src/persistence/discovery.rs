//! PostgreSQL adapter of public reads: discovery (TAG-PR-04, #98) and the single publication.
//!
//! Every read is ONE read-only snapshot (`REPEATABLE READ, READ ONLY`, taken at its first
//! statement): filter slugs, matching works, the metadata snapshots bound in their publication
//! rows and the catalog rows of those tags belong to one database state. A withdrawal, republish,
//! rename or retirement committed before that statement is seen whole; one committed later is
//! not seen at all, and the response's validator is computed from exactly what was read.
//!
//! Visibility is the join itself: a work is public only through `publications` (pointer →
//! revision of the same work → owner principal). `working_drafts`, `work_draft_metadata` and
//! `work_draft_tags` are never named here, so a private title, private tags or an unpublished work
//! cannot match, be counted or change a page.
//!
//! The text match folds both sides with the SQL `vink_search_key` (migration
//! `20261110000000_public_discovery`), mirroring `SearchKey::of`: the stored side through the
//! generated columns `title_search_key`/`display_name_search_key`, the query in its own statement.
//! Cursor scope uses that same query key. The word-prefix test (`strpos` of `' ' || key`) treats
//! the reader's text as a literal. Tag
//! filters are canonical identities decided by `filter_tags` from the same snapshot's slug rows;
//! a filter that can match nothing is answered without searching. Pages are keyset (newest first,
//! then work id), read with one row more than a page; there is no OFFSET and no count.

use std::collections::{HashMap, HashSet};

use sqlx::postgres::PgConnection;
use sqlx::types::Json;
use time::OffsetDateTime;
use uuid::Uuid;

use super::PgStore;
use super::discussion::{integrity, read_snapshot};
use super::postgres::{classify, document, title};
use super::taxonomy::{SlotRow, catalog_rows, slug_kind, tags_from_slots};
use crate::application::ports::{
    DiscoveryFacts, DiscoveryRequest, PublicCardFacts, PublicationFacts, PublicationStore,
    SlugFacts, StoreError,
};
use crate::domain::discovery::filter_tags;
use crate::domain::document::DocumentData;
use crate::domain::ids::{RevisionId, TagId, WorkId};
use crate::domain::taxonomy::{
    CatalogTag, MetadataSnapshot, SlugHolder, TagSlug, TagStatus, resolve_slug,
};

/// The search. `$1` is the folded query key (or NULL), `$2` the canonical filter tags (distinct;
/// all must be on the bound snapshot: the unique key (snapshot_id, tag_id) makes the count exact),
/// `$3`/`$4` the position to start after (or NULL), `$5` the rows to read. `published_at <= $3`
/// lets the order index start at the cursor.
/// Public only so that the measurement test can `EXPLAIN` exactly this statement.
pub const DISCOVERY_SEARCH_SQL: &str = "
    SELECT p.work_id, r.title, a.display_name, p.published_at, p.metadata_snapshot_id, s.digest
      FROM publications p
      JOIN revisions r ON r.work_id = p.work_id AND r.id = p.revision_id
      JOIN works w ON w.id = p.work_id
      JOIN principals a ON a.id = w.owner_id
      LEFT JOIN work_metadata_snapshots s
             ON s.work_id = p.work_id AND s.id = p.metadata_snapshot_id
     WHERE ($1::text IS NULL
            OR strpos(' ' || r.title_search_key, ' ' || $1) > 0
            OR strpos(' ' || a.display_name_search_key, ' ' || $1) > 0)
       AND (cardinality($2::uuid[]) = 0
            OR p.metadata_snapshot_id IN (
                SELECT t.snapshot_id FROM work_metadata_snapshot_tags t
                 WHERE t.tag_id = ANY($2::uuid[])
                 GROUP BY t.snapshot_id
                HAVING count(*) = cardinality($2::uuid[])))
       AND ($3::timestamptz IS NULL
            OR (p.published_at <= $3 AND (p.published_at < $3 OR p.work_id > $4::uuid)))
     ORDER BY p.published_at DESC, p.work_id
     LIMIT $5";

#[derive(sqlx::FromRow)]
struct CardRow {
    work_id: Uuid,
    title: String,
    display_name: String,
    published_at: OffsetDateTime,
    metadata_snapshot_id: Option<Uuid>,
    digest: Option<String>,
}

/// A bound snapshot: its id and stored digest, read from the publication row's join.
fn bound(
    id: Option<Uuid>,
    digest: Option<String>,
    work: Uuid,
) -> Result<Option<(Uuid, String)>, StoreError> {
    match (id, digest) {
        (None, _) => Ok(None),
        (Some(id), Some(digest)) => Ok(Some((id, digest))),
        (Some(id), None) => Err(integrity(
            "bound metadata snapshot is missing",
            format!("{work}/{id}"),
        )),
    }
}

/// The digest-verified snapshots `bound`, keyed by id. A snapshot without slots is a valid empty
/// one; a slot set that does not match its digest is corruption.
async fn snapshots(
    conn: &mut PgConnection,
    bound: &[(Uuid, String)],
) -> Result<HashMap<Uuid, MetadataSnapshot>, StoreError> {
    if bound.is_empty() {
        return Ok(HashMap::new());
    }
    let ids: Vec<Uuid> = bound.iter().map(|(id, _)| *id).collect();
    let rows: Vec<(Uuid, i16, Uuid, String)> = sqlx::query_as(
        "SELECT snapshot_id, slot, tag_id, facet FROM work_metadata_snapshot_tags
          WHERE snapshot_id = ANY($1::uuid[]) ORDER BY snapshot_id, slot",
    )
    .bind(&ids)
    .fetch_all(&mut *conn)
    .await
    .map_err(classify)?;
    let mut slots: HashMap<Uuid, Vec<SlotRow>> = HashMap::new();
    for (snapshot, slot, tag, facet) in rows {
        slots.entry(snapshot).or_default().push((slot, tag, facet));
    }
    bound
        .iter()
        .map(|(id, digest)| {
            let tags = tags_from_slots(slots.get(id).map_or(&[][..], Vec::as_slice), *id)?;
            let snapshot = MetadataSnapshot::restore(tags, digest)
                .ok_or_else(|| integrity("metadata snapshot digest", id))?;
            Ok((*id, snapshot))
        })
        .collect()
}

/// Catalog rows of every tag on `snapshots`, in one statement.
async fn snapshot_catalog<'a>(
    conn: &mut PgConnection,
    snapshots: impl Iterator<Item = &'a MetadataSnapshot>,
) -> Result<Vec<CatalogTag>, StoreError> {
    let mut seen = HashSet::new();
    let ids: Vec<Uuid> = snapshots
        .flat_map(|snapshot| snapshot.tags().ids())
        .map(|id| id.as_uuid())
        .filter(|id| seen.insert(*id))
        .collect();
    catalog_rows(conn, &ids).await
}

/// Slug facts of every filter slug, in the filter's order: the slug row, the tag it names and a
/// retired tag's successor row, as `CatalogReadStore::slug_facts` reads one.
async fn filter_facts(
    conn: &mut PgConnection,
    slugs: &[TagSlug],
) -> Result<Vec<SlugFacts>, StoreError> {
    if slugs.is_empty() {
        return Ok(Vec::new());
    }
    let raw: Vec<&str> = slugs.iter().map(TagSlug::as_str).collect();
    let rows: Vec<(String, Uuid, String)> =
        sqlx::query_as("SELECT slug, tag_id, kind FROM tag_names WHERE slug = ANY($1::text[])")
            .bind(&raw)
            .fetch_all(&mut *conn)
            .await
            .map_err(classify)?;
    let mut holders = HashMap::new();
    for (slug, tag, kind) in rows {
        holders.insert(
            slug,
            SlugHolder {
                tag: TagId::from_uuid(tag),
                kind: slug_kind(&kind)?,
            },
        );
    }
    let mut wanted: Vec<Uuid> = holders.values().map(|h| h.tag.as_uuid()).collect();
    wanted.sort();
    wanted.dedup();
    let mut rows = catalog_rows(conn, &wanted).await?;
    let successors: Vec<Uuid> = rows
        .iter()
        .filter_map(|tag| match tag.status() {
            TagStatus::Retired {
                successor: Some(id),
            } if !rows.iter().any(|known| known.id() == id) => Some(id.as_uuid()),
            _ => None,
        })
        .collect();
    rows.extend(catalog_rows(conn, &successors).await?);
    let row = |id: TagId| rows.iter().find(|tag| tag.id() == id).cloned();
    Ok(slugs
        .iter()
        .map(|slug| {
            let holder = holders.get(slug.as_str()).copied();
            let tag = holder.and_then(|h| row(h.tag));
            let successor = match tag.as_ref().map(CatalogTag::status) {
                Some(TagStatus::Retired {
                    successor: Some(id),
                }) => row(id),
                _ => None,
            };
            SlugFacts {
                holder,
                tag,
                successor,
            }
        })
        .collect())
}

impl PublicationStore for PgStore {
    async fn discover_works(
        &self,
        request: &DiscoveryRequest<'_>,
    ) -> Result<DiscoveryFacts, StoreError> {
        let mut tx = read_snapshot(self.pool()).await?;
        let slugs = request.filter.slugs();
        let filters = filter_facts(&mut tx, slugs).await?;
        let constraint = {
            let resolutions = slugs
                .iter()
                .zip(&filters)
                .map(|(slug, facts)| {
                    resolve_slug(slug, facts.holder, facts.tag.as_ref())
                        .map_err(|_| integrity("tag slug resolution", slug.as_str()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            filter_tags(&resolutions)
        };
        let nothing = |filters, query_key| DiscoveryFacts {
            query_key,
            filters,
            works: Vec::new(),
            catalog: Vec::new(),
        };
        // An unknown or retired filter tag: no public work can match, so none is searched.
        let Some(tags) = constraint else {
            tx.commit().await.map_err(classify)?;
            return Ok(nothing(filters, None));
        };
        let key: Option<String> = match request.query {
            None => None,
            Some(query) => Some(
                sqlx::query_scalar("SELECT vink_search_key($1)")
                    .bind(query.text())
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(classify)?,
            ),
        };
        // The domain refused queries without a letter or digit; the SQL key can still be empty
        // for characters the two foldings classify differently (outside Vietnamese/Latin). An
        // empty key would match every title, so it matches none.
        if key.as_deref() == Some("") {
            tx.commit().await.map_err(classify)?;
            return Ok(nothing(filters, key));
        }
        let tags: Vec<Uuid> = tags.iter().map(TagId::as_uuid).collect();
        let (after_at, after_work) = match request.after {
            Some(position) => (
                Some(position.published_at),
                Some(position.work_id.as_uuid()),
            ),
            None => (None, None),
        };
        let fetch = i64::try_from(request.size.fetch())
            .map_err(|_| integrity("page size", request.size.fetch()))?;
        // Plan with the actual parameters each time: a generic plan cannot know which optional
        // parts are absent and was measured far slower for some shapes (docs, measurements).
        sqlx::query("SET LOCAL plan_cache_mode = force_custom_plan")
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
        let rows: Vec<CardRow> = sqlx::query_as(DISCOVERY_SEARCH_SQL)
            .bind(key.as_deref())
            .bind(&tags)
            .bind(after_at)
            .bind(after_work)
            .bind(fetch)
            .fetch_all(&mut *tx)
            .await
            .map_err(classify)?;
        let bound_rows = rows
            .iter()
            .filter_map(|row| {
                bound(row.metadata_snapshot_id, row.digest.clone(), row.work_id).transpose()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let snapshots = snapshots(&mut tx, &bound_rows).await?;
        let catalog = snapshot_catalog(&mut tx, snapshots.values()).await?;
        tx.commit().await.map_err(classify)?;
        let works = rows
            .into_iter()
            .map(|row| {
                Ok(PublicCardFacts {
                    work_id: WorkId::from_uuid(row.work_id),
                    title: title(&row.title)?,
                    author_display_name: row.display_name,
                    published_at: row.published_at,
                    snapshot: row
                        .metadata_snapshot_id
                        .and_then(|id| snapshots.get(&id).cloned()),
                })
            })
            .collect::<Result<Vec<_>, StoreError>>()?;
        Ok(DiscoveryFacts {
            query_key: key,
            filters,
            works,
            catalog,
        })
    }

    async fn publication(&self, work_id: WorkId) -> Result<Option<PublicationFacts>, StoreError> {
        let mut tx = read_snapshot(self.pool()).await?;
        // Reads the pointed revision only; working_drafts is never joined on a public path.
        #[allow(clippy::type_complexity)]
        let row: Option<(
            Uuid,
            Uuid,
            String,
            String,
            OffsetDateTime,
            Json<DocumentData>,
            Option<Uuid>,
            Option<String>,
        )> = sqlx::query_as(
            "SELECT p.work_id, r.id, r.title, a.display_name, p.published_at, r.document,
                    p.metadata_snapshot_id, s.digest
               FROM publications p
               JOIN revisions r ON r.work_id = p.work_id AND r.id = p.revision_id
               JOIN works w ON w.id = p.work_id
               JOIN principals a ON a.id = w.owner_id
               LEFT JOIN work_metadata_snapshots s
                      ON s.work_id = p.work_id AND s.id = p.metadata_snapshot_id
              WHERE p.work_id = $1",
        )
        .bind(work_id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        let Some((work, revision, raw_title, author, published_at, raw_document, id, digest)) = row
        else {
            tx.commit().await.map_err(classify)?;
            return Ok(None);
        };
        let bound_snapshot = bound(id, digest, work)?;
        let mut snapshots = snapshots(&mut tx, bound_snapshot.as_slice()).await?;
        let catalog = snapshot_catalog(&mut tx, snapshots.values()).await?;
        tx.commit().await.map_err(classify)?;
        Ok(Some(PublicationFacts {
            work_id: WorkId::from_uuid(work),
            revision_id: RevisionId::from_uuid(revision),
            title: title(&raw_title)?,
            author_display_name: author,
            published_at,
            document: document(raw_document)?,
            snapshot: id.and_then(|id| snapshots.remove(&id)),
            catalog,
        }))
    }
}
