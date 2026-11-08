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
//! Standing fence: the principal row is locked FOR UPDATE, which conflicts with the FOR KEY SHARE
//! lock that inserting a suspension takes through `principal_suspensions_principal_id_fkey`. A
//! suspension therefore commits wholly before the save reads its standing, or waits until the
//! save commits. Standing is read in a SEPARATE statement after the lock: in READ COMMITTED a
//! subquery of the locking statement would still use the snapshot taken before the wait. Lifting
//! a suspension is not fenced; a stale "suspended" fails closed. This fences Vink's own
//! suspension only; it says nothing about how fast Identity revokes a session.
//!
//! Draft metadata is created lazily: an absent row is version 1 with no tags. The first save (or a
//! capture) inserts it with `ON CONFLICT DO NOTHING` and then locks it, so two first saves meet on
//! the same row; a refused or conflicting save rolls the insert back.

use sqlx::postgres::{PgConnection, PgPool};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::PgStore;
use super::discussion::{integrity, read_snapshot};
use super::postgres::classify;
use crate::application::ports::{
    PublicTagFacts, PublicTagStore, StoreError, StoredWorkTags, WorkTagStore,
};
use crate::domain::ids::{PrincipalId, RevisionId, TagId, WorkId};
use crate::domain::principal::Standing;
use crate::domain::taxonomy::{
    CatalogCommand, CatalogFacts, CatalogOutcome, CatalogTag, CatalogVersion, CatalogWrite,
    MetadataSnapshot, MetadataVersion, NameKind, SlugHolder, SlugKind, TagChange, TagDescription,
    TagFacet, TagFacts, TagLabel, TagOutcome, TagSlug, TagStatus, TagWrite, WorkTagFacts, WorkTags,
    decide_catalog_change, decide_tag_change,
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

fn slug_kind(raw: &str) -> Result<SlugKind, StoreError> {
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
type SlotRow = (i16, Uuid, String);

fn tags_from_slots(slots: &[SlotRow], owner: Uuid) -> Result<WorkTags, StoreError> {
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

async fn catalog_rows(
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

/// Locks the actor's principal row (the standing fence, see the module docs) and then reads its
/// standing in a new statement.
async fn locked_standing(
    tx: &mut Transaction<'_, Postgres>,
    actor: PrincipalId,
) -> Result<Standing, StoreError> {
    let known: Option<i32> =
        sqlx::query_scalar("SELECT 1 FROM principals WHERE id = $1 FOR UPDATE")
            .bind(actor.as_uuid())
            .fetch_optional(&mut **tx)
            .await
            .map_err(classify)?;
    if known.is_none() {
        tracing::info!("principal has no Vink record");
        return Err(StoreError::UnknownPrincipal);
    }
    let suspended: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM principal_suspensions
                         WHERE principal_id = $1 AND lifted_at IS NULL)",
    )
    .bind(actor.as_uuid())
    .fetch_one(&mut **tx)
    .await
    .map_err(classify)?;
    Ok(if suspended {
        Standing::Suspended
    } else {
        Standing::Active
    })
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
        let catalog = catalog_rows(&mut tx, &ids_of(&tags)).await?;
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

/// Applies one curated catalog change with operator authority, in one transaction: the facts
/// under locks (module docs), `decide_catalog_change` with `can_curate: true`, then exactly the
/// accepted write, committed before answering.
///
/// Operator authority means whoever runs this holds the App database credential, as for
/// `grant_moderator`. Choosing the curator entrypoint is #97's CHOICE-02; a DB-held curator grant
/// must be read under a share lock in this same transaction and turned into `can_curate`, as
/// moderation rechecks its grant at commit. No route calls this.
pub async fn change_catalog(
    pool: &PgPool,
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
        can_curate: true,
        version,
        subject: subject.as_ref().map(TagRow::tag).transpose()?,
        slug_holder,
        successor,
    };
    let outcome = decide_catalog_change(command, &facts);
    if let CatalogOutcome::Accepted(write) = &outcome {
        write_catalog(&mut tx, version, subject.is_some(), write).await?;
        tx.commit().await.map_err(classify)?;
    } else {
        tx.rollback().await.map_err(classify)?;
    }
    Ok(outcome)
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
