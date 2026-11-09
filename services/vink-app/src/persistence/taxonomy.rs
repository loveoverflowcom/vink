//! PostgreSQL adapter of work taxonomy (TAG-PR-02, #96). Every write is one transaction: current
//! facts under locks → the exact pure decision of #95 (`decide_tag_change`,
//! `decide_catalog_change`) → a conditional write of exactly what it accepted → commit → the
//! durable answer. The schema (`migrations/20261108000000_work_taxonomy.sql`) holds the
//! structural rules on its own: one row per slug forever, one canonical name per tag, facet by
//! composite foreign key, at most one form and three themes by slot, versions that only advance,
//! catalog rows and snapshots never deleted, snapshots sealed by their digest at commit.
//!
//! Draft save lock order: principal (FOR UPDATE) → work (FOR SHARE) → selected catalog tags (FOR
//! SHARE, by id) → draft metadata row (FOR UPDATE). Catalog change lock order: catalog version row
//! (FOR UPDATE) → subject tag (FOR NO KEY UPDATE) → successor (FOR SHARE) → claimed slug row (FOR
//! NO KEY UPDATE). A catalog change never locks principals, works or draft rows, and a save takes
//! only shared locks on catalog rows, so the two orders cannot form a cycle. No network call
//! happens inside these transactions.
//!
//! Standing fence: [`locked_standing`] (shared with discussion submissions, see
//! `persistence/principal.rs`) locks the principal row FOR UPDATE, so a suspension commits wholly
//! before the save reads its standing, or waits until the save commits. Lifting a suspension is
//! not fenced; a stale "suspended" fails closed. This fences Vink's own suspension only; it says
//! nothing about how fast Identity revokes a session.
//!
//! Draft metadata is created lazily: an absent row is version 1 with no tags. The first save (or a
//! capture) inserts it with `ON CONFLICT DO NOTHING` and then locks it, so two first saves meet on
//! the same row; a refused or conflicting save rolls the insert back.
//!
//! Catalog reads (TAG-PR-03, #97) run in one read-only snapshot each and never touch principals,
//! drafts or the audit. Lookup loads the active catalog (tags + aliases of active tags) and ranks
//! it in Rust: no stored search key or index exists, which is right for a small curated catalog
//! and is revisited when it is not (docs/development/taxonomy-api.md). Every accepted catalog
//! change writes its `tag_catalog_changes` row in the change's transaction; the schema refuses a
//! version step without one.

use sqlx::postgres::{PgConnection, PgPool};
use sqlx::{Postgres, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

use super::PgStore;
use super::discussion::{integrity, read_snapshot};
use super::postgres::classify;
use super::principal::locked_standing;
use crate::application::catalog::{CatalogOperation, CuratorAuthority};
use crate::application::ports::{
    ActiveCatalog, CatalogAuditRecord, CatalogEntry, CatalogListing, CatalogName, CatalogReadStore,
    CatalogStore, PublicTagFacts, PublicTagStore, SlugFacts, StoreError, StoredWorkTags,
    WorkTagStore,
};
use crate::domain::ids::{PrincipalId, RevisionId, TagId, WorkId};
use crate::domain::principal::Standing;
use crate::domain::taxonomy::{
    CatalogCommand, CatalogFacts, CatalogOutcome, CatalogTag, CatalogVersion, CatalogWrite,
    MetadataSnapshot, MetadataVersion, NameKind, SlugHolder, SlugKind, TagAlias, TagChange,
    TagDescription, TagFacet, TagFacts, TagLabel, TagOutcome, TagSlug, TagStatus, TagWrite,
    WorkTagFacts, WorkTags, decide_catalog_change, decide_tag_change,
};

/// Column list of [`TagRow`], as a literal so that statements stay `&'static str`.
macro_rules! tag_columns {
    () => {
        "id, facet, label, slug, description, status, successor_id"
    };
}

fn facet_name(facet: TagFacet) -> &'static str {
    match facet {
        TagFacet::Form => "form",
        TagFacet::Theme => "theme",
    }
}

fn facet(raw: &str) -> Result<TagFacet, StoreError> {
    match raw {
        "form" => Ok(TagFacet::Form),
        "theme" => Ok(TagFacet::Theme),
        other => Err(integrity("stored tag facet", other)),
    }
}

fn slug_kind_name(kind: SlugKind) -> &'static str {
    match kind {
        SlugKind::Canonical => "canonical",
        SlugKind::Alias => "alias",
        SlugKind::Former => "former",
    }
}

pub(super) fn slug_kind(raw: &str) -> Result<SlugKind, StoreError> {
    match raw {
        "canonical" => Ok(SlugKind::Canonical),
        "alias" => Ok(SlugKind::Alias),
        "former" => Ok(SlugKind::Former),
        other => Err(integrity("stored slug kind", other)),
    }
}

fn status_columns(status: TagStatus) -> (&'static str, Option<Uuid>) {
    match status {
        TagStatus::Active => ("active", None),
        TagStatus::Retired { successor } => ("retired", successor.map(|id| id.as_uuid())),
    }
}

/// Stored text must already be in the canonical form the core produced; anything a parse would
/// still change was not written by an accepted change.
fn exact<T>(stored: &str, parsed: Option<T>, text: impl Fn(&T) -> &str) -> Option<T> {
    parsed.filter(|value| text(value) == stored)
}

#[derive(sqlx::FromRow)]
struct TagRow {
    id: Uuid,
    facet: String,
    label: String,
    slug: String,
    description: Option<String>,
    status: String,
    successor_id: Option<Uuid>,
}

impl TagRow {
    fn status(&self) -> Result<TagStatus, StoreError> {
        match (self.status.as_str(), self.successor_id) {
            ("active", None) => Ok(TagStatus::Active),
            ("retired", successor) => Ok(TagStatus::Retired {
                successor: successor.map(TagId::from_uuid),
            }),
            _ => Err(integrity("stored tag status", self.id)),
        }
    }

    fn facts(&self) -> Result<TagFacts, StoreError> {
        Ok(TagFacts {
            id: TagId::from_uuid(self.id),
            facet: facet(&self.facet)?,
            status: self.status()?,
        })
    }

    fn tag(&self) -> Result<CatalogTag, StoreError> {
        let broken = |what| integrity(what, self.id);
        let label = exact(
            &self.label,
            TagLabel::parse(&self.label).ok(),
            TagLabel::as_str,
        )
        .ok_or_else(|| broken("stored tag label"))?;
        let slug = exact(&self.slug, TagSlug::parse(&self.slug).ok(), TagSlug::as_str)
            .ok_or_else(|| broken("stored tag slug"))?;
        let description = match &self.description {
            None => None,
            Some(raw) => Some(
                exact(raw, TagDescription::parse(raw).ok(), TagDescription::as_str)
                    .ok_or_else(|| broken("stored tag description"))?,
            ),
        };
        CatalogTag::restore(
            TagId::from_uuid(self.id),
            facet(&self.facet)?,
            label,
            slug,
            description,
            self.status()?,
        )
        .ok_or_else(|| broken("stored tag"))
    }
}

/// One stored tag slot: (slot, tag, facet). Slot 0 is the form, 1..3 the themes.
pub(super) type SlotRow = (i16, Uuid, String);

pub(super) fn tags_from_slots(slots: &[SlotRow], owner: Uuid) -> Result<WorkTags, StoreError> {
    let mut form = None;
    let mut themes = Vec::with_capacity(slots.len());
    for (slot, id, raw_facet) in slots {
        match (*slot, facet(raw_facet)?) {
            (0, TagFacet::Form) if form.is_none() => form = Some(TagId::from_uuid(*id)),
            (1..=3, TagFacet::Theme) => themes.push(TagId::from_uuid(*id)),
            _ => return Err(integrity("stored tag slot", owner)),
        }
    }
    WorkTags::restore(form, themes).ok_or_else(|| integrity("stored tags", owner))
}

/// Columns for `unnest`: the form in slot 0, then themes from slot 1 in order.
fn slot_columns(tags: &WorkTags) -> (Vec<i16>, Vec<Uuid>, Vec<&'static str>) {
    let mut slots = Vec::new();
    let mut ids = Vec::new();
    let mut facets = Vec::new();
    if let Some(form) = tags.form() {
        slots.push(0);
        ids.push(form.as_uuid());
        facets.push(facet_name(TagFacet::Form));
    }
    for (slot, theme) in (1..).zip(tags.themes()) {
        slots.push(slot);
        ids.push(theme.as_uuid());
        facets.push(facet_name(TagFacet::Theme));
    }
    (slots, ids, facets)
}

fn ids_of(tags: &WorkTags) -> Vec<Uuid> {
    tags.ids().map(|id| id.as_uuid()).collect()
}

pub(super) async fn catalog_rows(
    conn: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<CatalogTag>, StoreError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows: Vec<TagRow> = sqlx::query_as(concat!(
        "SELECT ",
        tag_columns!(),
        " FROM tags WHERE id = ANY($1::uuid[])"
    ))
    .bind(ids)
    .fetch_all(&mut *conn)
    .await
    .map_err(classify)?;
    rows.iter().map(TagRow::tag).collect()
}

async fn draft_slots(conn: &mut PgConnection, work_id: WorkId) -> Result<Vec<SlotRow>, StoreError> {
    sqlx::query_as(
        "SELECT slot, tag_id, facet FROM work_draft_tags WHERE work_id = $1 ORDER BY slot",
    )
    .bind(work_id.as_uuid())
    .fetch_all(&mut *conn)
    .await
    .map_err(classify)
}

fn metadata_version(raw: i64, work_id: WorkId) -> Result<MetadataVersion, StoreError> {
    MetadataVersion::new(raw).ok_or_else(|| integrity("stored metadata version", work_id))
}

/// Materializes the initial draft metadata row (version 1, no tags) if absent. Semantically a
/// no-op: an absent row already means exactly that. It lets the caller lock a real row.
async fn ensure_draft_metadata(conn: &mut PgConnection, work_id: WorkId) -> Result<(), StoreError> {
    sqlx::query(
        "INSERT INTO work_draft_metadata (work_id) VALUES ($1) ON CONFLICT (work_id) DO NOTHING",
    )
    .bind(work_id.as_uuid())
    .execute(&mut *conn)
    .await
    .map_err(classify)?;
    Ok(())
}

/// The owner's facts, read after the owner check so another owner's metadata is never read.
async fn owner_facts(
    tx: &mut Transaction<'_, Postgres>,
    actor: PrincipalId,
    standing: Standing,
    work_id: WorkId,
    change: &TagChange,
) -> Result<WorkTagFacts, StoreError> {
    let selection = change.selection();
    let selected: Vec<Uuid> = selection
        .form()
        .into_iter()
        .chain(selection.themes().iter().copied())
        .map(|id| id.as_uuid())
        .collect();
    // FOR SHARE: a concurrent retirement (or any catalog change of these rows) commits before
    // this read, which then returns the new row, or waits until this save commits.
    let rows: Vec<TagRow> = if selected.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as(concat!(
            "SELECT ",
            tag_columns!(),
            " FROM tags WHERE id = ANY($1::uuid[]) ORDER BY id FOR SHARE"
        ))
        .bind(&selected)
        .fetch_all(&mut **tx)
        .await
        .map_err(classify)?
    };
    let catalog = rows
        .iter()
        .map(TagRow::facts)
        .collect::<Result<Vec<_>, _>>()?;
    ensure_draft_metadata(tx, work_id).await?;
    let version: i64 =
        sqlx::query_scalar("SELECT version FROM work_draft_metadata WHERE work_id = $1 FOR UPDATE")
            .bind(work_id.as_uuid())
            .fetch_one(&mut **tx)
            .await
            .map_err(classify)?;
    let slots = draft_slots(tx, work_id).await?;
    Ok(WorkTagFacts {
        owner: actor,
        standing,
        version: metadata_version(version, work_id)?,
        current: tags_from_slots(&slots, work_id.as_uuid())?,
        catalog,
    })
}

/// Writes an accepted save: the version step (conditional on the version decided from) and, when
/// the tags changed, the slots as a whole. Nothing else.
async fn write_tags(
    tx: &mut Transaction<'_, Postgres>,
    work_id: WorkId,
    expected: MetadataVersion,
    write: &TagWrite,
) -> Result<(), StoreError> {
    let stepped = sqlx::query(
        "UPDATE work_draft_metadata SET version = $3, updated_at = now()
          WHERE work_id = $1 AND version = $2",
    )
    .bind(work_id.as_uuid())
    .bind(expected.get())
    .bind(write.version().get())
    .execute(&mut **tx)
    .await
    .map_err(classify)?
    .rows_affected();
    if stepped != 1 {
        return Err(integrity("draft metadata moved under its lock", work_id));
    }
    if !write.changed() {
        return Ok(());
    }
    sqlx::query("DELETE FROM work_draft_tags WHERE work_id = $1")
        .bind(work_id.as_uuid())
        .execute(&mut **tx)
        .await
        .map_err(classify)?;
    let (slots, ids, facets) = slot_columns(write.tags());
    if !slots.is_empty() {
        sqlx::query(
            "INSERT INTO work_draft_tags (work_id, slot, tag_id, facet)
             SELECT $1, s, t, f FROM unnest($2::smallint[], $3::uuid[], $4::text[]) AS x (s, t, f)",
        )
        .bind(work_id.as_uuid())
        .bind(&slots)
        .bind(&ids)
        .bind(&facets)
        .execute(&mut **tx)
        .await
        .map_err(classify)?;
    }
    Ok(())
}

impl WorkTagStore for PgStore {
    async fn work_tags(
        &self,
        owner: PrincipalId,
        work_id: WorkId,
    ) -> Result<Option<StoredWorkTags>, StoreError> {
        // One read-only snapshot: the version, the slots and their catalog rows agree.
        let mut tx = read_snapshot(self.pool()).await?;
        let row: Option<(Uuid, Option<i64>)> = sqlx::query_as(
            "SELECT w.owner_id, m.version FROM works w
               LEFT JOIN work_draft_metadata m ON m.work_id = w.id
              WHERE w.id = $1",
        )
        .bind(work_id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        let stored_version = match row {
            Some((found, version)) if found == owner.as_uuid() => version,
            // Missing or another owner's: indistinguishable, and nothing else is read.
            _ => {
                tx.commit().await.map_err(classify)?;
                return Ok(None);
            }
        };
        let slots = draft_slots(&mut tx, work_id).await?;
        let tags = tags_from_slots(&slots, work_id.as_uuid())?;
        // The assigned rows and, in the same snapshot, the successor hints of retired ones.
        let mut catalog = catalog_rows(&mut tx, &ids_of(&tags)).await?;
        let successors: Vec<Uuid> = catalog
            .iter()
            .filter_map(|tag| match tag.status() {
                TagStatus::Retired {
                    successor: Some(id),
                } if !tags.contains(id) => Some(id.as_uuid()),
                _ => None,
            })
            .collect();
        let mut successors = catalog_rows(&mut tx, &successors).await?;
        successors.retain(|row| !catalog.iter().any(|known| known.id() == row.id()));
        catalog.extend(successors);
        tx.commit().await.map_err(classify)?;
        let version = match stored_version {
            None => MetadataVersion::FIRST,
            Some(raw) => metadata_version(raw, work_id)?,
        };
        Ok(Some(StoredWorkTags {
            version,
            tags,
            catalog,
        }))
    }

    async fn save_work_tags(
        &self,
        actor: PrincipalId,
        work_id: WorkId,
        change: &TagChange,
    ) -> Result<TagOutcome, StoreError> {
        let mut tx = self.pool().begin().await.map_err(classify)?;
        let standing = locked_standing(&mut tx, actor).await?;
        // FOR SHARE: the owner cannot change while the save holds the row (no operation moves a
        // work today; this keeps the check honest if one is added).
        let owner: Option<Uuid> =
            sqlx::query_scalar("SELECT owner_id FROM works WHERE id = $1 FOR SHARE")
                .bind(work_id.as_uuid())
                .fetch_optional(&mut *tx)
                .await
                .map_err(classify)?;
        let facts = match owner {
            Some(owner) if owner == actor.as_uuid() => {
                Some(owner_facts(&mut tx, actor, standing, work_id, change).await?)
            }
            // A missing work and another owner's work are the same refusal; neither is read.
            _ => None,
        };
        let outcome = decide_tag_change(actor, change, facts.as_ref());
        if let TagOutcome::Accepted(write) = &outcome {
            write_tags(&mut tx, work_id, change.expected(), write).await?;
            tx.commit().await.map_err(classify)?;
        } else {
            tx.rollback().await.map_err(classify)?;
        }
        Ok(outcome)
    }
}

impl PublicTagStore for PgStore {
    async fn public_work_tags(
        &self,
        work_id: WorkId,
    ) -> Result<Option<PublicTagFacts>, StoreError> {
        // One read-only snapshot taken at the first statement: the pointer, its bound snapshot,
        // the slots and the catalog rows belong to ONE publication state. A withdrawal or
        // republish committed before that statement is seen; one committed later is not, and a
        // read already in progress finishes with the state it started with.
        let mut tx = read_snapshot(self.pool()).await?;
        let row: Option<(Uuid, Option<Uuid>, Option<String>)> = sqlx::query_as(
            "SELECT p.revision_id, p.metadata_snapshot_id, s.digest
               FROM publications p
               LEFT JOIN work_metadata_snapshots s
                      ON s.work_id = p.work_id AND s.id = p.metadata_snapshot_id
              WHERE p.work_id = $1",
        )
        .bind(work_id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
        let Some((revision_id, bound, digest)) = row else {
            tx.commit().await.map_err(classify)?;
            return Ok(None);
        };
        let (snapshot, catalog) = match (bound, digest) {
            (None, _) => (None, Vec::new()),
            (Some(snapshot_id), Some(digest)) => {
                let slots: Vec<SlotRow> = sqlx::query_as(
                    "SELECT slot, tag_id, facet FROM work_metadata_snapshot_tags
                      WHERE snapshot_id = $1 ORDER BY slot",
                )
                .bind(snapshot_id)
                .fetch_all(&mut *tx)
                .await
                .map_err(classify)?;
                let tags = tags_from_slots(&slots, snapshot_id)?;
                let catalog = catalog_rows(&mut tx, &ids_of(&tags)).await?;
                let snapshot = MetadataSnapshot::restore(tags, &digest)
                    .ok_or_else(|| integrity("metadata snapshot digest", snapshot_id))?;
                (Some(snapshot), catalog)
            }
            (Some(snapshot_id), None) => {
                return Err(integrity("bound metadata snapshot is missing", snapshot_id));
            }
        };
        tx.commit().await.map_err(classify)?;
        Ok(Some(PublicTagFacts {
            revision_id: RevisionId::from_uuid(revision_id),
            snapshot,
            catalog,
        }))
    }
}

/// What [`capture_metadata_snapshot`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotCapture {
    /// A new, unbound snapshot row and its slots, written in the caller's transaction.
    Captured {
        id: Uuid,
        snapshot: MetadataSnapshot,
    },
    /// The draft metadata is no longer at the expected version; nothing was written.
    Conflict { current: MetadataVersion },
}

/// Publication-owner seam, not a product command: no route or use case calls it yet, because no
/// publish/moderation workflow exists (PUB-01..03). The publication transaction calls it on its
/// own connection, after rechecking author intent, authority and the content revision, and
/// before moving the pointer. It materializes and locks the draft metadata row FOR SHARE (a
/// concurrent tag save waits until the publication transaction ends), checks the version the
/// author confirmed, and inserts an immutable snapshot of exactly those tags; the database
/// verifies its digest at the caller's commit. It never binds, publishes or checks ownership.
/// Binding is the caller's pointer UPDATE: `revision_id` and `metadata_snapshot_id` in one row.
pub async fn capture_metadata_snapshot(
    conn: &mut PgConnection,
    work_id: WorkId,
    expected: MetadataVersion,
) -> Result<SnapshotCapture, StoreError> {
    ensure_draft_metadata(conn, work_id).await?;
    let raw: i64 =
        sqlx::query_scalar("SELECT version FROM work_draft_metadata WHERE work_id = $1 FOR SHARE")
            .bind(work_id.as_uuid())
            .fetch_one(&mut *conn)
            .await
            .map_err(classify)?;
    let current = metadata_version(raw, work_id)?;
    if current != expected {
        return Ok(SnapshotCapture::Conflict { current });
    }
    let slots = draft_slots(conn, work_id).await?;
    let snapshot = MetadataSnapshot::capture(tags_from_slots(&slots, work_id.as_uuid())?);
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO work_metadata_snapshots (id, work_id, metadata_version, digest)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(id)
    .bind(work_id.as_uuid())
    .bind(current.get())
    .bind(snapshot.digest().as_str())
    .execute(&mut *conn)
    .await
    .map_err(classify)?;
    let (slots, ids, facets) = slot_columns(snapshot.tags());
    if !slots.is_empty() {
        sqlx::query(
            "INSERT INTO work_metadata_snapshot_tags (snapshot_id, slot, tag_id, facet)
             SELECT $1, s, t, f FROM unnest($2::smallint[], $3::uuid[], $4::text[]) AS x (s, t, f)",
        )
        .bind(id)
        .bind(&slots)
        .bind(&ids)
        .bind(&facets)
        .execute(&mut *conn)
        .await
        .map_err(classify)?;
    }
    Ok(SnapshotCapture::Captured { id, snapshot })
}

/// Applies one curated catalog change in one transaction: the facts under locks (module docs),
/// `decide_catalog_change` with `can_curate` decided from `authority`, then exactly the accepted
/// write and its audit row, committed before answering. A refusal or conflict writes nothing.
///
/// [`CuratorAuthority::operator`] means whoever runs this holds the App database credential, as
/// for `grant_moderator`; the operator command `vink-app tag …` is the only caller in production
/// (#97 CHOICE-02 A). A future principal grant must be read under a share lock in this same
/// transaction and turned into `can_curate`, as moderation rechecks its grant at commit.
pub async fn change_catalog(
    pool: &PgPool,
    authority: CuratorAuthority,
    command: &CatalogCommand,
) -> Result<CatalogOutcome, StoreError> {
    let change = &command.change;
    let mut tx = pool.begin().await.map_err(classify)?;
    let raw: Option<i64> = sqlx::query_scalar("SELECT version FROM tag_catalog FOR UPDATE")
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
    let version = raw
        .and_then(CatalogVersion::new)
        .ok_or_else(|| integrity("stored catalog version", raw.unwrap_or_default()))?;
    let subject: Option<TagRow> = sqlx::query_as(concat!(
        "SELECT ",
        tag_columns!(),
        " FROM tags WHERE id = $1 FOR NO KEY UPDATE"
    ))
    .bind(change.subject().as_uuid())
    .fetch_optional(&mut *tx)
    .await
    .map_err(classify)?;
    let successor = match change.successor() {
        None => None,
        Some(id) => {
            let row: Option<TagRow> = sqlx::query_as(concat!(
                "SELECT ",
                tag_columns!(),
                " FROM tags WHERE id = $1 FOR SHARE"
            ))
            .bind(id.as_uuid())
            .fetch_optional(&mut *tx)
            .await
            .map_err(classify)?;
            row.as_ref().map(TagRow::facts).transpose()?
        }
    };
    let slug_holder = match change.claimed_slug() {
        None => None,
        Some(slug) => {
            let row: Option<(Uuid, String)> = sqlx::query_as(
                "SELECT tag_id, kind FROM tag_names WHERE slug = $1 FOR NO KEY UPDATE",
            )
            .bind(slug.as_str())
            .fetch_optional(&mut *tx)
            .await
            .map_err(classify)?;
            match row {
                None => None,
                Some((tag, kind)) => Some(SlugHolder {
                    tag: TagId::from_uuid(tag),
                    kind: slug_kind(&kind)?,
                }),
            }
        }
    };
    let facts = CatalogFacts {
        // The operator is the only authority, and holding its witness is the whole check; a
        // principal grant would be read here, under a share lock.
        can_curate: authority == CuratorAuthority::operator(),
        version,
        subject: subject.as_ref().map(TagRow::tag).transpose()?,
        slug_holder,
        successor,
    };
    let outcome = decide_catalog_change(command, &facts);
    if let CatalogOutcome::Accepted(write) = &outcome {
        write_catalog(&mut tx, version, subject.is_some(), write).await?;
        audit_change(&mut tx, authority, command, write).await?;
        tx.commit().await.map_err(classify)?;
    } else {
        tx.rollback().await.map_err(classify)?;
    }
    Ok(outcome)
}

/// The audit row of an accepted change, in the change's transaction. The schema checks at commit
/// that it names exactly the version the change produced.
async fn audit_change(
    tx: &mut Transaction<'_, Postgres>,
    authority: CuratorAuthority,
    command: &CatalogCommand,
    write: &CatalogWrite,
) -> Result<(), StoreError> {
    let change = &command.change;
    sqlx::query(
        "INSERT INTO tag_catalog_changes
                (catalog_version, operation, tag_id, slug, successor_id, changed, authority)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(write.version().get())
    .bind(CatalogOperation::of(change).name())
    .bind(change.subject().as_uuid())
    .bind(change.claimed_slug().as_ref().map(TagSlug::as_str))
    .bind(change.successor().map(|id| id.as_uuid()))
    .bind(write.changed())
    .bind(authority.audit_name())
    .execute(&mut **tx)
    .await
    .map_err(classify)?;
    Ok(())
}

async fn write_catalog(
    tx: &mut Transaction<'_, Postgres>,
    version: CatalogVersion,
    existed: bool,
    write: &CatalogWrite,
) -> Result<(), StoreError> {
    let stepped =
        sqlx::query("UPDATE tag_catalog SET version = $2, updated_at = now() WHERE version = $1")
            .bind(version.get())
            .bind(write.version().get())
            .execute(&mut **tx)
            .await
            .map_err(classify)?
            .rows_affected();
    if stepped != 1 {
        return Err(integrity(
            "catalog version moved under its lock",
            version.get(),
        ));
    }
    if let Some(tag) = write.tag() {
        let (status, successor) = status_columns(tag.status());
        let statement = if existed {
            "UPDATE tags SET label = $3, slug = $4, description = $5, status = $6,
                    successor_id = $7, updated_at = now()
              WHERE id = $1 AND facet = $2"
        } else {
            "INSERT INTO tags (id, facet, label, slug, description, status, successor_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7)"
        };
        let written = sqlx::query(statement)
            .bind(tag.id().as_uuid())
            .bind(facet_name(tag.facet()))
            .bind(tag.label().as_str())
            .bind(tag.slug().as_str())
            .bind(tag.description().map(TagDescription::as_str))
            .bind(status)
            .bind(successor)
            .execute(&mut **tx)
            .await
            .map_err(classify)?
            .rows_affected();
        if written != 1 {
            return Err(integrity("catalog tag row", tag.id()));
        }
    }
    // Demotions and aliases first, the canonical claim last, so the one-canonical index never
    // sees two canonical names of a tag inside a rename.
    let mut slugs: Vec<_> = write.slugs().iter().collect();
    slugs.sort_by_key(|slug| matches!(slug.kind, NameKind::Canonical));
    for slug in slugs {
        let alias_label = match &slug.kind {
            NameKind::Alias(label) => Some(label.as_str()),
            NameKind::Canonical | NameKind::Former => None,
        };
        let kind = slug_kind_name(slug.kind.slug_kind());
        let written = match slug.replaces {
            None => sqlx::query(
                "INSERT INTO tag_names (slug, tag_id, kind, alias_label) VALUES ($1, $2, $3, $4)",
            )
            .bind(slug.slug.as_str())
            .bind(slug.tag.as_uuid())
            .bind(kind)
            .bind(alias_label),
            Some(previous) => sqlx::query(
                "UPDATE tag_names SET kind = $3, alias_label = $4, updated_at = now()
                  WHERE slug = $1 AND tag_id = $2 AND kind = $5",
            )
            .bind(slug.slug.as_str())
            .bind(slug.tag.as_uuid())
            .bind(kind)
            .bind(alias_label)
            .bind(slug_kind_name(previous)),
        }
        .execute(&mut **tx)
        .await
        .map_err(classify)?
        .rows_affected();
        if written != 1 {
            return Err(integrity("catalog slug row", slug.slug.as_str()));
        }
    }
    Ok(())
}

fn stored_label(raw: &str, what: &'static str) -> Result<TagLabel, StoreError> {
    exact(raw, TagLabel::parse(raw).ok(), TagLabel::as_str).ok_or_else(|| integrity(what, raw))
}

fn stored_slug(raw: &str) -> Result<TagSlug, StoreError> {
    exact(raw, TagSlug::parse(raw).ok(), TagSlug::as_str)
        .ok_or_else(|| integrity("stored slug", raw))
}

fn catalog_version_of(raw: i64) -> Result<CatalogVersion, StoreError> {
    CatalogVersion::new(raw).ok_or_else(|| integrity("stored catalog version", raw))
}

impl CatalogReadStore for PgStore {
    async fn active_catalog(&self) -> Result<ActiveCatalog, StoreError> {
        // One snapshot: an alias is listed only with the active tag it names.
        let mut tx = read_snapshot(self.pool()).await?;
        let rows: Vec<TagRow> = sqlx::query_as(concat!(
            "SELECT ",
            tag_columns!(),
            " FROM tags WHERE status = 'active' ORDER BY id"
        ))
        .fetch_all(&mut *tx)
        .await
        .map_err(classify)?;
        let names: Vec<(String, Uuid, Option<String>)> = sqlx::query_as(
            "SELECT n.slug, n.tag_id, n.alias_label
               FROM tag_names n JOIN tags t ON t.id = n.tag_id
              WHERE n.kind = 'alias' AND t.status = 'active'
              ORDER BY n.slug",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(classify)?;
        tx.commit().await.map_err(classify)?;
        let tags = rows
            .iter()
            .map(TagRow::tag)
            .collect::<Result<Vec<_>, _>>()?;
        let aliases = names
            .iter()
            .map(|(slug, target, label)| {
                let label = stored_label(
                    label
                        .as_deref()
                        .ok_or_else(|| integrity("alias without label", slug))?,
                    "stored alias label",
                )?;
                TagAlias::restore(label, stored_slug(slug)?, TagId::from_uuid(*target))
                    .ok_or_else(|| integrity("stored alias", slug))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ActiveCatalog { tags, aliases })
    }

    async fn slug_facts(&self, slug: &TagSlug) -> Result<SlugFacts, StoreError> {
        let mut tx = read_snapshot(self.pool()).await?;
        let holder: Option<(Uuid, String)> =
            sqlx::query_as("SELECT tag_id, kind FROM tag_names WHERE slug = $1")
                .bind(slug.as_str())
                .fetch_optional(&mut *tx)
                .await
                .map_err(classify)?;
        let Some((tag_id, kind)) = holder else {
            tx.commit().await.map_err(classify)?;
            return Ok(SlugFacts {
                holder: None,
                tag: None,
                successor: None,
            });
        };
        let holder = SlugHolder {
            tag: TagId::from_uuid(tag_id),
            kind: slug_kind(&kind)?,
        };
        let mut rows = catalog_rows(&mut tx, &[tag_id]).await?;
        let tag = rows.pop();
        let successor = match tag.as_ref().map(CatalogTag::status) {
            Some(TagStatus::Retired {
                successor: Some(id),
            }) => catalog_rows(&mut tx, &[id.as_uuid()]).await?.pop(),
            _ => None,
        };
        tx.commit().await.map_err(classify)?;
        Ok(SlugFacts {
            holder: Some(holder),
            tag,
            successor,
        })
    }
}

#[derive(sqlx::FromRow)]
struct AuditRow {
    catalog_version: i64,
    operation: String,
    tag_id: Uuid,
    slug: Option<String>,
    successor_id: Option<Uuid>,
    changed: bool,
    authority: String,
    database_role: String,
    recorded_at: OffsetDateTime,
}

impl AuditRow {
    fn record(self) -> Result<CatalogAuditRecord, StoreError> {
        let authority = match self.authority.as_str() {
            "operator" => CuratorAuthority::operator().audit_name(),
            other => return Err(integrity("stored audit authority", other)),
        };
        Ok(CatalogAuditRecord {
            version: catalog_version_of(self.catalog_version)?,
            operation: CatalogOperation::parse(&self.operation)
                .ok_or_else(|| integrity("stored audit operation", &self.operation))?,
            tag: TagId::from_uuid(self.tag_id),
            slug: self.slug.as_deref().map(stored_slug).transpose()?,
            successor: self.successor_id.map(TagId::from_uuid),
            changed: self.changed,
            authority,
            database_role: self.database_role,
            recorded_at: self.recorded_at,
        })
    }
}

impl CatalogStore for PgStore {
    async fn change_catalog(
        &self,
        authority: CuratorAuthority,
        command: &CatalogCommand,
    ) -> Result<CatalogOutcome, StoreError> {
        change_catalog(self.pool(), authority, command).await
    }

    async fn catalog_listing(&self) -> Result<CatalogListing, StoreError> {
        let mut tx = read_snapshot(self.pool()).await?;
        let version: i64 = sqlx::query_scalar("SELECT version FROM tag_catalog")
            .fetch_one(&mut *tx)
            .await
            .map_err(classify)?;
        let rows: Vec<TagRow> = sqlx::query_as(concat!(
            "SELECT ",
            tag_columns!(),
            " FROM tags ORDER BY facet, slug COLLATE \"C\", id"
        ))
        .fetch_all(&mut *tx)
        .await
        .map_err(classify)?;
        let names: Vec<(Uuid, String, String, Option<String>)> = sqlx::query_as(
            "SELECT tag_id, slug, kind, alias_label FROM tag_names
              ORDER BY tag_id, kind, slug COLLATE \"C\"",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(classify)?;
        tx.commit().await.map_err(classify)?;
        let mut entries = Vec::with_capacity(rows.len());
        for row in &rows {
            let tag = row.tag()?;
            let names = names
                .iter()
                .filter(|(owner, ..)| *owner == row.id)
                .map(|(_, slug, kind, alias_label)| {
                    Ok(CatalogName {
                        slug: stored_slug(slug)?,
                        kind: slug_kind(kind)?,
                        alias_label: alias_label
                            .as_deref()
                            .map(|raw| stored_label(raw, "stored alias label"))
                            .transpose()?,
                    })
                })
                .collect::<Result<Vec<_>, StoreError>>()?;
            entries.push(CatalogEntry { tag, names });
        }
        Ok(CatalogListing {
            version: catalog_version_of(version)?,
            entries,
        })
    }

    async fn catalog_history(&self, limit: i64) -> Result<Vec<CatalogAuditRecord>, StoreError> {
        let rows: Vec<AuditRow> = sqlx::query_as(
            "SELECT catalog_version, operation, tag_id, slug, successor_id, changed, authority,
                    database_role, recorded_at
               FROM tag_catalog_changes ORDER BY catalog_version DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(self.pool())
        .await
        .map_err(classify)?;
        rows.into_iter().map(AuditRow::record).collect()
    }
}
