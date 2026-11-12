//! Moderation persistence: directory, grants, groups, cases and automation (#159, MOD-09..15).
//!
//! Every command runs in one transaction that takes the shared lock order of
//! [`super::moderation_write`], places the standing fence (`principal::require_active`) on the
//! acting principal before it reads the directory, and writes its audit in the same commit.

use sqlx::{
    Connection, PgConnection, PgPool, Postgres, Transaction, query, query_as, query_scalar,
};
use time::OffsetDateTime;
use uuid::Uuid;

use super::moderation_write::{
    DecisionSource, apply_pointer_effect, lock_work, reroute_open_jobs, revalidate_claims,
    sync_job_states, write_case, write_decision, write_publication_pointers,
};
use super::postgres::{classify, integrity};
use super::principal::require_active;
use crate::application::moderation::{ModerationActionError, case_refusal_to_action_error};
use crate::application::ports::StoreError;
use crate::domain::document::DocumentData;
use crate::domain::ids::{CaseId, GrantId, GroupId, PrincipalId, RevisionId, WorkId};
use crate::domain::moderation::authority::{
    AdminSeat, AuthorityBasis, DirectoryAudit, DirectoryChange, DirectoryError, DirectoryFacts,
    GrantFacts, GrantOrigin, GrantScope, GrantSpec, Grantee, GroupFacts, ModerationDirectory,
    ReviewCapability, ReviewTarget,
};
use crate::domain::moderation::automation::{
    Automation, AutomationError, AutomationMode, AutomationVersion,
};
use crate::domain::moderation::case::{
    CaseEvent, CaseFacts, CaseOrigin, CaseRefusal, CaseState, CaseTransition, CaseVerdict,
    CaseVersion, CloseReason, InternalNote, NoteFacts, ReviewCase, ReviewSubject,
};
use crate::domain::moderation::{
    AdminTransition, DecisionReason, DecisionVersion, ModerationError,
};

// ---------------------------------------------------------------------------
// 1. Directory and admin seats
// ---------------------------------------------------------------------------

pub async fn load_moderation_directory(
    conn: &mut PgConnection,
    _now: OffsetDateTime,
) -> Result<ModerationDirectory, StoreError> {
    // 1. Admins
    let admin_rows: Vec<Uuid> = query_scalar("SELECT principal_id FROM moderation_admin_seats")
        .fetch_all(&mut *conn)
        .await
        .map_err(classify)?;
    let admins: Vec<PrincipalId> = admin_rows.into_iter().map(PrincipalId::from_uuid).collect();

    // 2. Suspended principals
    let suspended_rows: Vec<Uuid> =
        query_scalar("SELECT principal_id FROM principal_suspensions WHERE lifted_at IS NULL")
            .fetch_all(&mut *conn)
            .await
            .map_err(classify)?;
    let suspended: Vec<PrincipalId> = suspended_rows
        .into_iter()
        .map(PrincipalId::from_uuid)
        .collect();

    // 3. Grants from moderation_grants
    #[derive(sqlx::FromRow)]
    struct GrantRow {
        id: Uuid,
        grantee_kind: String,
        grantee_id: Uuid,
        capability: String,
        scope_kind: String,
        scope_work_id: Option<Uuid>,
        expires_at: Option<OffsetDateTime>,
        revoked: bool,
        origin: String,
    }

    let grant_rows: Vec<GrantRow> = query_as(
        "SELECT id, grantee_kind, grantee_id, capability, scope_kind,
                scope_work_id, expires_at, revoked, origin
           FROM moderation_grants",
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(classify)?;

    let mut grants = Vec::with_capacity(grant_rows.len());
    for row in grant_rows {
        let grantee = match row.grantee_kind.as_str() {
            "principal" => Grantee::Principal(PrincipalId::from_uuid(row.grantee_id)),
            "group" => Grantee::Group(GroupId::from_uuid(row.grantee_id)),
            other => return Err(integrity("unknown grantee kind", other)),
        };
        let capability = match row.capability.as_str() {
            "works" => ReviewCapability::Works,
            "comments" => ReviewCapability::Comments,
            other => return Err(integrity("unknown capability", other)),
        };
        let scope = match row.scope_kind.as_str() {
            "global" => GrantScope::Global,
            "work" => {
                let work_id = row
                    .scope_work_id
                    .ok_or_else(|| integrity("scoped grant missing work id", row.id))?;
                GrantScope::Work(WorkId::from_uuid(work_id))
            }
            other => return Err(integrity("unknown scope kind", other)),
        };
        let origin = match row.origin.as_str() {
            "admin" => GrantOrigin::Admin,
            "inherited_comment" => GrantOrigin::InheritedComment,
            other => return Err(integrity("unknown grant origin", other)),
        };
        grants.push(GrantFacts {
            id: GrantId::from_uuid(row.id),
            grantee,
            capability,
            scope,
            expires_at: row.expires_at,
            revoked: row.revoked,
            origin,
        });
    }

    // 4. Inherited comment grants from discussion_moderator_grants
    #[derive(sqlx::FromRow)]
    struct InheritedGrantRow {
        id: Uuid,
        principal_id: Uuid,
        revoked_at: Option<OffsetDateTime>,
    }

    let inherited_rows: Vec<InheritedGrantRow> =
        query_as("SELECT id, principal_id, revoked_at FROM discussion_moderator_grants")
            .fetch_all(&mut *conn)
            .await
            .map_err(classify)?;

    for row in inherited_rows {
        grants.push(GrantFacts {
            id: GrantId::from_uuid(row.id),
            grantee: Grantee::Principal(PrincipalId::from_uuid(row.principal_id)),
            capability: ReviewCapability::Comments,
            scope: GrantScope::Global,
            expires_at: None,
            revoked: row.revoked_at.is_some(),
            origin: GrantOrigin::InheritedComment,
        });
    }

    // 5. Groups and members
    #[derive(sqlx::FromRow)]
    struct GroupRow {
        id: Uuid,
        members: Vec<Uuid>,
    }

    let group_rows: Vec<GroupRow> = query_as(
        "SELECT g.id, coalesce(array_agg(m.principal_id) FILTER (WHERE m.principal_id IS NOT NULL), '{}') AS members
           FROM moderation_groups g
           LEFT JOIN moderation_group_members m ON m.group_id = g.id
          GROUP BY g.id",
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(classify)?;

    let groups = group_rows
        .into_iter()
        .map(|r| GroupFacts {
            id: GroupId::from_uuid(r.id),
            members: r.members.into_iter().map(PrincipalId::from_uuid).collect(),
        })
        .collect();

    ModerationDirectory::restore(DirectoryFacts {
        admins,
        suspended,
        grants,
        groups,
    })
    .map_err(|e| integrity("moderation directory restore failed", format!("{e:?}")))
}

/// The admin prefix of an admin-only command: standing fence, then directory and seat.
async fn admin_seat(
    tx: &mut Transaction<'_, Postgres>,
    admin: PrincipalId,
    now: OffsetDateTime,
) -> Result<(ModerationDirectory, AdminSeat), ModerationActionError> {
    require_active(tx, admin).await?;
    let directory = load_moderation_directory(tx, now).await?;
    let seat = directory
        .require_admin(admin, now)
        .map_err(|e| ModerationActionError::Forbidden(e.as_str()))?;
    Ok((directory, seat))
}

async fn write_directory_audit(
    conn: &mut PgConnection,
    audit: DirectoryAudit,
) -> Result<(), StoreError> {
    let (change, grant, group, member) = match audit.change {
        DirectoryChange::Granted(g) => ("granted", Some(g.as_uuid()), None, None),
        DirectoryChange::Revoked(g) => ("revoked", Some(g.as_uuid()), None, None),
        DirectoryChange::Joined { group, member } => (
            "joined",
            None,
            Some(group.as_uuid()),
            Some(member.as_uuid()),
        ),
        DirectoryChange::Left { group, member } => {
            ("left", None, Some(group.as_uuid()), Some(member.as_uuid()))
        }
    };
    query(
        "INSERT INTO moderation_directory_audit (id, admin_id, change, grant_id, group_id, member_id, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(Uuid::now_v7())
    .bind(audit.admin.as_uuid())
    .bind(change)
    .bind(grant)
    .bind(group)
    .bind(member)
    .bind(audit.at)
    .execute(&mut *conn)
    .await
    .map_err(classify)?;
    Ok(())
}

/// Operator CLI: add an admin seat. Returns whether the seat was new; only a change is audited.
pub async fn add_admin_seat(
    conn: &mut PgConnection,
    operator: &str,
    principal: PrincipalId,
    reason: Option<&str>,
) -> Result<bool, StoreError> {
    let mut tx = conn.begin().await.map_err(classify)?;
    let known: Option<i32> = query_scalar("SELECT 1 FROM principals WHERE id = $1 FOR SHARE")
        .bind(principal.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
    if known.is_none() {
        return Err(StoreError::UnknownPrincipal);
    }
    let added: Option<Uuid> = query_scalar(
        "INSERT INTO moderation_admin_seats (principal_id, created_at)
         VALUES ($1, now())
         ON CONFLICT (principal_id) DO NOTHING
         RETURNING principal_id",
    )
    .bind(principal.as_uuid())
    .fetch_optional(&mut *tx)
    .await
    .map_err(classify)?;
    if added.is_some() {
        insert_admin_audit(&mut tx, operator, "add", principal, reason).await?;
    }
    tx.commit().await.map_err(classify)?;
    Ok(added.is_some())
}

/// Operator CLI: remove an admin seat. A case the former admin held only as admin goes back to
/// the queue in the same commit. Returns whether a seat was removed.
pub async fn remove_admin_seat(
    conn: &mut PgConnection,
    operator: &str,
    principal: PrincipalId,
    reason: Option<&str>,
) -> Result<bool, StoreError> {
    let mut tx = conn.begin().await.map_err(classify)?;
    let removed: Option<Uuid> = query_scalar(
        "DELETE FROM moderation_admin_seats WHERE principal_id = $1 RETURNING principal_id",
    )
    .bind(principal.as_uuid())
    .fetch_optional(&mut *tx)
    .await
    .map_err(classify)?;
    if removed.is_some() {
        insert_admin_audit(&mut tx, operator, "remove", principal, reason).await?;
        let now = OffsetDateTime::now_utc();
        let directory = load_moderation_directory(&mut tx, now).await?;
        revalidate_claims(&mut tx, &directory, Some(principal), now).await?;
    }
    tx.commit().await.map_err(classify)?;
    Ok(removed.is_some())
}

async fn insert_admin_audit(
    conn: &mut PgConnection,
    operator: &str,
    action: &str,
    principal: PrincipalId,
    reason: Option<&str>,
) -> Result<(), StoreError> {
    query(
        "INSERT INTO moderation_admin_audit (id, operator, action, principal_id, reason, created_at)
         VALUES ($1, $2, $3, $4, $5, now())",
    )
    .bind(Uuid::now_v7())
    .bind(operator)
    .bind(action)
    .bind(principal.as_uuid())
    .bind(reason)
    .execute(&mut *conn)
    .await
    .map_err(classify)?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct AdminSeatRecord {
    pub principal_id: PrincipalId,
    pub display_name: String,
    pub created_at: OffsetDateTime,
}

pub async fn list_admin_seats(conn: &mut PgConnection) -> Result<Vec<AdminSeatRecord>, StoreError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        principal_id: Uuid,
        display_name: String,
        created_at: OffsetDateTime,
    }

    let rows: Vec<Row> = query_as(
        "SELECT s.principal_id, p.display_name, s.created_at
           FROM moderation_admin_seats s
           JOIN principals p ON p.id = s.principal_id
          ORDER BY s.created_at ASC",
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(classify)?;

    Ok(rows
        .into_iter()
        .map(|r| AdminSeatRecord {
            principal_id: PrincipalId::from_uuid(r.principal_id),
            display_name: r.display_name,
            created_at: r.created_at,
        })
        .collect())
}

// ---------------------------------------------------------------------------
// 2. Automation mode (MOD-14)
// ---------------------------------------------------------------------------

pub async fn load_automation(conn: &mut PgConnection) -> Result<Automation, StoreError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        mode: String,
        version: i64,
    }

    let row: Row = query_as("SELECT mode, version FROM moderation_automation_state WHERE id = 1")
        .fetch_one(&mut *conn)
        .await
        .map_err(classify)?;

    let mode = match row.mode.as_str() {
        "enabled" => AutomationMode::Enabled,
        "paused" => AutomationMode::Paused,
        other => return Err(integrity("unknown automation mode", other)),
    };
    let version = AutomationVersion::new(row.version)
        .ok_or_else(|| integrity("invalid automation version", row.version))?;

    Ok(Automation::restore(mode, version))
}

/// Pause or re-enable automatic moderation with version, actor and reason. A pause reroutes
/// every open or leased job to the human queue in the same commit, so a result that arrives
/// later is refused (`AutomationPaused`) and a restart has nothing left to finish. Re-enabling
/// applies to submissions made afterwards.
pub async fn set_automation_mode(
    pool: &PgPool,
    admin: PrincipalId,
    expected: AutomationVersion,
    mode: AutomationMode,
    reason: Option<DecisionReason>,
    now: OffsetDateTime,
) -> Result<Automation, ModerationActionError> {
    let mut tx = pool.begin().await.map_err(classify)?;
    require_active(&mut tx, admin).await?;
    let directory = load_moderation_directory(&mut tx, now).await?;
    // Exclusive lock on the mode row: workers applying a result hold it in share mode.
    let _: i32 = query_scalar("SELECT id FROM moderation_automation_state WHERE id = 1 FOR UPDATE")
        .fetch_one(&mut *tx)
        .await
        .map_err(classify)?;
    let current = load_automation(&mut tx).await?;

    let (next, audit) = current
        .set(&directory, admin, expected, mode, reason, now)
        .map_err(|e| match e {
            AutomationError::Authority(a) => ModerationActionError::Forbidden(a.as_str()),
            AutomationError::Stale => ModerationActionError::Conflict("automation_version_stale"),
            AutomationError::Unchanged => ModerationActionError::ValidationFailed("mode_unchanged"),
            AutomationError::ReasonRequired => {
                ModerationActionError::ValidationFailed("reason_required")
            }
            AutomationError::VersionExhausted => ModerationActionError::Internal,
        })?;

    query(
        "UPDATE moderation_automation_state
            SET mode = $1, version = $2, updated_at = now()
          WHERE id = 1",
    )
    .bind(mode.as_str())
    .bind(next.version().get())
    .execute(&mut *tx)
    .await
    .map_err(classify)?;

    query(
        "INSERT INTO moderation_automation_audit (id, admin_id, mode, version, reason, created_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(Uuid::now_v7())
    .bind(admin.as_uuid())
    .bind(mode.as_str())
    .bind(next.version().get())
    .bind(audit.reason.as_str())
    .bind(audit.at)
    .execute(&mut *tx)
    .await
    .map_err(classify)?;

    if mode == AutomationMode::Paused {
        reroute_open_jobs(&mut tx).await?;
    }
    tx.commit().await.map_err(classify)?;
    Ok(next)
}

// ---------------------------------------------------------------------------
// 3. Review case reads
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CaseSummary {
    pub id: CaseId,
    pub subject_kind: String,
    pub work_id: WorkId,
    pub revision_id: Option<RevisionId>,
    pub decision_version: i64,
    pub origin: String,
    pub version: i64,
    pub state: String,
    pub claimed_by: Option<PrincipalId>,
    pub content_owner: PrincipalId,
    pub work_owner: PrincipalId,
    pub created_at: OffsetDateTime,
}

impl CaseSummary {
    pub fn target(&self) -> ReviewTarget {
        ReviewTarget {
            capability: ReviewCapability::Works,
            work: self.work_id,
            content_owner: self.content_owner,
            work_owner: self.work_owner,
        }
    }
}

pub async fn list_open_work_cases(conn: &mut PgConnection) -> Result<Vec<CaseSummary>, StoreError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        subject_kind: String,
        work_id: Uuid,
        revision_id: Option<Uuid>,
        decision_version: i64,
        origin: String,
        version: i64,
        state: String,
        claimed_by: Option<Uuid>,
        target_content_owner: Uuid,
        target_work_owner: Uuid,
        created_at: OffsetDateTime,
    }

    let rows: Vec<Row> = query_as(
        "SELECT id, subject_kind, work_id, revision_id, decision_version, origin, version, state,
                claimed_by, target_content_owner, target_work_owner, created_at
           FROM moderation_cases
          WHERE state IN ('queued', 'claimed', 'escalated') AND subject_kind = 'work_revision'
          ORDER BY created_at ASC, id ASC",
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(classify)?;

    Ok(rows
        .into_iter()
        .map(|r| CaseSummary {
            id: CaseId::from_uuid(r.id),
            subject_kind: r.subject_kind,
            work_id: WorkId::from_uuid(r.work_id),
            revision_id: r.revision_id.map(RevisionId::from_uuid),
            decision_version: r.decision_version,
            origin: r.origin,
            version: r.version,
            state: r.state,
            claimed_by: r.claimed_by.map(PrincipalId::from_uuid),
            content_owner: PrincipalId::from_uuid(r.target_content_owner),
            work_owner: PrincipalId::from_uuid(r.target_work_owner),
            created_at: r.created_at,
        })
        .collect())
}

#[derive(Debug, Clone)]
pub struct NoteRecord {
    pub id: Uuid,
    pub author_id: PrincipalId,
    pub note: String,
    pub created_at: OffsetDateTime,
}

/// A case with the exact submitted snapshot the reviewer decides on.
#[derive(Debug, Clone)]
pub struct CaseDetail {
    pub id: CaseId,
    pub subject_kind: String,
    pub work_id: WorkId,
    pub revision_id: Option<RevisionId>,
    pub content_digest: Option<String>,
    pub title: Option<String>,
    pub document: Option<DocumentData>,
    pub author_id: PrincipalId,
    pub author_name: String,
    pub work_owner: PrincipalId,
    pub decision_version: i64,
    pub origin: String,
    pub version: i64,
    pub state: String,
    pub claimed_by: Option<PrincipalId>,
    pub notes: Vec<NoteRecord>,
    pub created_at: OffsetDateTime,
}

impl CaseDetail {
    pub fn target(&self) -> ReviewTarget {
        ReviewTarget {
            capability: ReviewCapability::Works,
            work: self.work_id,
            content_owner: self.author_id,
            work_owner: self.work_owner,
        }
    }
}

pub async fn get_case_detail(
    conn: &mut PgConnection,
    case_id: CaseId,
) -> Result<Option<CaseDetail>, StoreError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        subject_kind: String,
        work_id: Uuid,
        revision_id: Option<Uuid>,
        content_digest: Option<String>,
        decision_version: i64,
        origin: String,
        version: i64,
        state: String,
        claimed_by: Option<Uuid>,
        created_at: OffsetDateTime,
        target_content_owner: Uuid,
        target_work_owner: Uuid,
        author_name: String,
        title: Option<String>,
        document: Option<sqlx::types::Json<DocumentData>>,
        revision_hash: Option<String>,
    }

    let row: Option<Row> = query_as(
        "SELECT c.id, c.subject_kind, c.work_id, c.revision_id, c.content_digest,
                c.decision_version, c.origin, c.version, c.state, c.claimed_by, c.created_at,
                c.target_content_owner, c.target_work_owner, p.display_name AS author_name,
                r.title, r.document, r.content_hash AS revision_hash
           FROM moderation_cases c
           JOIN principals p ON p.id = c.target_content_owner
           LEFT JOIN revisions r ON r.work_id = c.work_id AND r.id = c.revision_id
          WHERE c.id = $1",
    )
    .bind(case_id.as_uuid())
    .fetch_optional(&mut *conn)
    .await
    .map_err(classify)?;

    let Some(r) = row else { return Ok(None) };
    // The revision is immutable; a digest that differs from the case binding is corruption.
    if r.content_digest.is_some() && r.revision_hash != r.content_digest {
        return Err(integrity("case digest does not match its revision", r.id));
    }

    #[derive(sqlx::FromRow)]
    struct NoteRow {
        id: Uuid,
        author_id: Uuid,
        note: String,
        created_at: OffsetDateTime,
    }

    let note_rows: Vec<NoteRow> = query_as(
        "SELECT id, author_id, note, created_at
           FROM moderation_case_notes
          WHERE case_id = $1
          ORDER BY created_at ASC, id ASC",
    )
    .bind(case_id.as_uuid())
    .fetch_all(&mut *conn)
    .await
    .map_err(classify)?;

    let notes = note_rows
        .into_iter()
        .map(|n| NoteRecord {
            id: n.id,
            author_id: PrincipalId::from_uuid(n.author_id),
            note: n.note,
            created_at: n.created_at,
        })
        .collect();

    Ok(Some(CaseDetail {
        id: CaseId::from_uuid(r.id),
        subject_kind: r.subject_kind,
        work_id: WorkId::from_uuid(r.work_id),
        revision_id: r.revision_id.map(RevisionId::from_uuid),
        content_digest: r.content_digest,
        title: r.title,
        document: r.document.map(|d| d.0),
        author_id: PrincipalId::from_uuid(r.target_content_owner),
        author_name: r.author_name,
        work_owner: PrincipalId::from_uuid(r.target_work_owner),
        decision_version: r.decision_version,
        origin: r.origin,
        version: r.version,
        state: r.state,
        claimed_by: r.claimed_by.map(PrincipalId::from_uuid),
        notes,
        created_at: r.created_at,
    }))
}

// ---------------------------------------------------------------------------
// 4. Case commands
// ---------------------------------------------------------------------------

pub async fn load_case_locked(
    conn: &mut PgConnection,
    case_id: CaseId,
) -> Result<Option<ReviewCase>, StoreError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        subject_kind: String,
        work_id: Uuid,
        revision_id: Option<Uuid>,
        content_digest: Option<String>,
        comment_id: Option<Uuid>,
        decision_version: i64,
        target_content_owner: Uuid,
        target_work_owner: Uuid,
        origin: String,
        version: i64,
        state: String,
        claimed_by: Option<Uuid>,
        claimed_basis_kind: Option<String>,
        claimed_basis_grant_id: Option<Uuid>,
        concluded_by: Option<Uuid>,
        concluded_basis_kind: Option<String>,
        concluded_basis_grant_id: Option<Uuid>,
        concluded_verdict: Option<String>,
        close_reason: Option<String>,
    }

    let row: Option<Row> = query_as(
        "SELECT id, subject_kind, work_id, revision_id, content_digest, comment_id,
                decision_version, target_content_owner, target_work_owner, origin,
                version, state, claimed_by, claimed_basis_kind, claimed_basis_grant_id,
                concluded_by, concluded_basis_kind, concluded_basis_grant_id,
                concluded_verdict, close_reason
           FROM moderation_cases
          WHERE id = $1
          FOR UPDATE",
    )
    .bind(case_id.as_uuid())
    .fetch_optional(&mut *conn)
    .await
    .map_err(classify)?;

    let Some(r) = row else { return Ok(None) };

    #[derive(sqlx::FromRow)]
    struct NoteRow {
        author_id: Uuid,
        note: String,
    }

    let note_rows: Vec<NoteRow> = query_as(
        "SELECT author_id, note FROM moderation_case_notes WHERE case_id = $1 ORDER BY created_at ASC",
    )
    .bind(case_id.as_uuid())
    .fetch_all(&mut *conn)
    .await
    .map_err(classify)?;

    let mut notes = Vec::with_capacity(note_rows.len());
    for n in note_rows {
        let note = InternalNote::parse(&n.note)
            .map_err(|e| integrity("internal note parse failed", format!("{e:?}")))?;
        notes.push(NoteFacts {
            author: PrincipalId::from_uuid(n.author_id),
            note,
        });
    }

    let subject = match r.subject_kind.as_str() {
        "work_revision" => {
            let rev = r
                .revision_id
                .ok_or_else(|| integrity("work_revision case missing revision_id", r.id))?;
            let digest_str = r
                .content_digest
                .as_deref()
                .ok_or_else(|| integrity("work_revision case missing content_digest", r.id))?;
            let digest = crate::domain::revision::ContentHash::parse(digest_str)
                .ok_or_else(|| integrity("invalid content digest in case", digest_str))?;
            let dec = DecisionVersion::new(r.decision_version)
                .ok_or_else(|| integrity("invalid decision version in case", r.decision_version))?;
            ReviewSubject::WorkRevision {
                work: WorkId::from_uuid(r.work_id),
                revision: RevisionId::from_uuid(rev),
                digest,
                decision: dec,
            }
        }
        "comment" => {
            let comm = r
                .comment_id
                .ok_or_else(|| integrity("comment case missing comment_id", r.id))?;
            let dec =
                crate::domain::discussion::lifecycle::ModerationVersion::new(r.decision_version)
                    .ok_or_else(|| {
                        integrity("invalid moderation version in case", r.decision_version)
                    })?;
            ReviewSubject::Comment {
                work: WorkId::from_uuid(r.work_id),
                comment: crate::domain::ids::CommentId::from_uuid(comm),
                decision: dec,
            }
        }
        other => return Err(integrity("unknown subject kind in case", other)),
    };

    let target = ReviewTarget {
        capability: match r.subject_kind.as_str() {
            "work_revision" => ReviewCapability::Works,
            _ => ReviewCapability::Comments,
        },
        work: WorkId::from_uuid(r.work_id),
        content_owner: PrincipalId::from_uuid(r.target_content_owner),
        work_owner: PrincipalId::from_uuid(r.target_work_owner),
    };

    let origin = match r.origin.as_str() {
        "evaluator_uncertain" => CaseOrigin::EvaluatorUncertain,
        "invalid_output" => CaseOrigin::InvalidOutput,
        "retries_exhausted" => CaseOrigin::RetriesExhausted,
        "automation_paused" => CaseOrigin::AutomationPaused,
        "policy_rereview" => CaseOrigin::PolicyRereview,
        "reviewer_intervention" => CaseOrigin::ReviewerIntervention,
        other => return Err(integrity("unknown case origin", other)),
    };

    let version =
        CaseVersion::new(r.version).ok_or_else(|| integrity("invalid case version", r.version))?;

    let state = match r.state.as_str() {
        "queued" => CaseState::Queued,
        "claimed" => {
            let by = r
                .claimed_by
                .ok_or_else(|| integrity("claimed case missing claimant", r.id))?;
            let basis = match r.claimed_basis_kind.as_deref() {
                Some("admin") => AuthorityBasis::Admin,
                Some("grant") => {
                    let g = r
                        .claimed_basis_grant_id
                        .ok_or_else(|| integrity("grant basis missing grant id", r.id))?;
                    AuthorityBasis::Grant(GrantId::from_uuid(g))
                }
                other => return Err(integrity("unknown claimed basis kind", other.unwrap_or(""))),
            };
            CaseState::Claimed {
                by: PrincipalId::from_uuid(by),
                basis,
            }
        }
        "escalated" => CaseState::Escalated,
        "concluded" => {
            let by = r
                .concluded_by
                .ok_or_else(|| integrity("concluded case missing concluded_by", r.id))?;
            let basis = match r.concluded_basis_kind.as_deref() {
                Some("admin") => AuthorityBasis::Admin,
                Some("grant") => {
                    let g = r
                        .concluded_basis_grant_id
                        .ok_or_else(|| integrity("grant basis missing grant id", r.id))?;
                    AuthorityBasis::Grant(GrantId::from_uuid(g))
                }
                other => {
                    return Err(integrity(
                        "unknown concluded basis kind",
                        other.unwrap_or(""),
                    ));
                }
            };
            let verdict = match r.concluded_verdict.as_deref() {
                Some("approve") => CaseVerdict::Approve,
                Some("reject") => CaseVerdict::Reject,
                other => return Err(integrity("unknown concluded verdict", other.unwrap_or(""))),
            };
            CaseState::Concluded {
                by: PrincipalId::from_uuid(by),
                basis,
                verdict,
            }
        }
        "closed" => {
            let close_reason = match r.close_reason.as_deref() {
                Some("withdrawn") => CloseReason::Withdrawn,
                Some("target_gone") => CloseReason::TargetGone,
                Some("superseded") => CloseReason::Superseded,
                other => return Err(integrity("unknown close reason", other.unwrap_or(""))),
            };
            CaseState::Closed(close_reason)
        }
        other => return Err(integrity("unknown case state", other)),
    };

    let case = ReviewCase::restore(CaseFacts {
        id: CaseId::from_uuid(r.id),
        subject,
        target,
        origin,
        version,
        state,
        notes,
    })
    .map_err(|e| integrity("review case restore failed", format!("{e:?}")))?;

    Ok(Some(case))
}

/// A case command the core decides on the locked case (claim, release, note, escalate,
/// takeover).
pub enum CaseCommand {
    Claim,
    Release,
    Note(InternalNote),
    Escalate,
    TakeOver(DecisionReason),
}

/// Run one case command in one commit: standing fence, directory, locked case, core decision,
/// fenced write (and the note row only after the fenced write). When the core refuses because
/// the claimant lost every basis, the case goes back to the queue in its own commit and the
/// command still answers forbidden (MOD-11).
pub async fn case_command(
    pool: &PgPool,
    actor: PrincipalId,
    case_id: CaseId,
    expected: CaseVersion,
    command: CaseCommand,
    now: OffsetDateTime,
) -> Result<ReviewCase, ModerationActionError> {
    let mut tx = pool.begin().await.map_err(classify)?;
    require_active(&mut tx, actor).await?;
    let directory = load_moderation_directory(&mut tx, now).await?;
    let case = load_case_locked(&mut tx, case_id)
        .await?
        .ok_or(ModerationActionError::NotFound)?;
    let note = match &command {
        CaseCommand::Note(note) => Some(note.clone()),
        _ => None,
    };
    let decided: Result<CaseTransition, CaseRefusal> = match command {
        CaseCommand::Claim => case.claim(&directory, actor, expected, now),
        CaseCommand::Release => case.release(actor, expected),
        CaseCommand::Note(note) => case.note(&directory, actor, expected, note, now),
        CaseCommand::Escalate => case.escalate(actor, expected),
        CaseCommand::TakeOver(reason) => {
            case.take_over(&directory, actor, expected, Some(&reason), now)
        }
    };
    let transition = match decided {
        Ok(transition) => transition,
        Err(refusal) => {
            requeue_if_authority_lost(tx, &case, &directory, actor, refusal, now).await?;
            return Err(case_refusal_to_action_error(refusal));
        }
    };
    if !write_case(&mut tx, &transition.next, expected.get()).await? {
        return Err(ModerationActionError::Conflict("case_version_stale"));
    }
    if let Some(note) = note {
        query(
            "INSERT INTO moderation_case_notes (id, case_id, author_id, note, created_at)
             VALUES ($1, $2, $3, $4, now())",
        )
        .bind(Uuid::now_v7())
        .bind(case_id.as_uuid())
        .bind(actor.as_uuid())
        .bind(note.as_str())
        .execute(&mut *tx)
        .await
        .map_err(classify)?;
    }
    tx.commit().await.map_err(classify)?;
    Ok(transition.next)
}

/// On an authority refusal against the actor's own claim, persist the core's revalidation:
/// lost every basis -> queued. Any other refusal leaves the case unchanged (rolled back).
async fn requeue_if_authority_lost(
    mut tx: Transaction<'_, Postgres>,
    case: &ReviewCase,
    directory: &ModerationDirectory,
    actor: PrincipalId,
    refusal: CaseRefusal,
    now: OffsetDateTime,
) -> Result<(), StoreError> {
    let claimed_by_actor = matches!(case.state(), CaseState::Claimed { by, .. } if by == actor);
    if matches!(refusal, CaseRefusal::Authority(_))
        && claimed_by_actor
        && let Ok(revalidated) = case.revalidate(directory, now)
        && revalidated.event == CaseEvent::AuthorityLost
        && write_case(&mut tx, &revalidated.next, case.version().get()).await?
    {
        tx.commit().await.map_err(classify)?;
        tracing::info!(case = %case.facts().id, "claim returned to the queue: authority lost");
        return Ok(());
    }
    tx.rollback().await.map_err(classify)?;
    Ok(())
}

/// MOD-13: a reviewer with authority on the work opens a case on one submitted revision (for
/// example the currently public one) at its current decision version. One open case per
/// revision; recusal and standing are checked in this commit.
pub async fn open_intervention_case(
    pool: &PgPool,
    actor: PrincipalId,
    work: WorkId,
    revision: RevisionId,
    now: OffsetDateTime,
) -> Result<CaseId, ModerationActionError> {
    let mut tx = pool.begin().await.map_err(classify)?;
    let owner = lock_work(&mut tx, work)
        .await?
        .ok_or(ModerationActionError::NotFound)?;
    require_active(&mut tx, actor).await?;
    let directory = load_moderation_directory(&mut tx, now).await?;
    directory
        .resolve(actor, &ReviewTarget::work(work, owner), now)
        .map_err(|refusal| ModerationActionError::Forbidden(refusal.as_str()))?;
    let (publication, moderation, _) =
        super::publication::load_paired_state(&mut tx, owner, work).await?;
    if !publication.facts().lifecycle.is_active() {
        return Err(ModerationActionError::NotFound);
    }
    let decision = moderation
        .facts()
        .revisions
        .iter()
        .find(|r| r.revision == revision)
        .ok_or(ModerationActionError::NotFound)?;
    let id = CaseId::from_uuid(Uuid::now_v7());
    let opened: Option<Uuid> = query_scalar(
        "INSERT INTO moderation_cases (
             id, subject_kind, work_id, revision_id, content_digest, decision_version,
             target_content_owner, target_work_owner, origin, version, state,
             created_at, updated_at
         )
         VALUES ($1, 'work_revision', $2, $3, $4, $5, $6, $6, 'reviewer_intervention', 1, 'queued', now(), now())
         ON CONFLICT (work_id, revision_id) WHERE (state IN ('queued', 'claimed', 'escalated') AND subject_kind = 'work_revision')
         DO NOTHING
         RETURNING id",
    )
    .bind(id.as_uuid())
    .bind(work.as_uuid())
    .bind(revision.as_uuid())
    .bind(decision.digest.as_str())
    .bind(decision.version.get())
    .bind(owner.as_uuid())
    .fetch_optional(&mut *tx)
    .await
    .map_err(classify)?;
    if opened.is_none() {
        return Err(ModerationActionError::Conflict("case_already_open"));
    }
    tx.commit().await.map_err(classify)?;
    Ok(id)
}

/// Conclude a work-revision case in one commit (MOD-11..13): the work prefix of the lock order,
/// the standing fence, the locked case, the core's conclusion against the decision read in this
/// commit, then decision, history, pointers, job states and the case.
pub async fn conclude_case(
    pool: &PgPool,
    actor: PrincipalId,
    case_id: CaseId,
    expected_case_version: CaseVersion,
    verdict: CaseVerdict,
    reason: Option<DecisionReason>,
    now: OffsetDateTime,
) -> Result<AdminTransition, ModerationActionError> {
    let mut tx = pool.begin().await.map_err(classify)?;

    // The case's work, read unlocked only to know which work prefix to lock first.
    let work: Option<Uuid> = query_scalar("SELECT work_id FROM moderation_cases WHERE id = $1")
        .bind(case_id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
    let work_id = WorkId::from_uuid(work.ok_or(ModerationActionError::NotFound)?);
    let owner = lock_work(&mut tx, work_id)
        .await?
        .ok_or(ModerationActionError::NotFound)?;
    require_active(&mut tx, actor).await?;

    let case = load_case_locked(&mut tx, case_id)
        .await?
        .ok_or(ModerationActionError::NotFound)?;
    let rev_id = match &case.facts().subject {
        ReviewSubject::WorkRevision { work, revision, .. } if *work == work_id => *revision,
        ReviewSubject::WorkRevision { .. } => {
            return Err(integrity("case work changed", case_id).into());
        }
        ReviewSubject::Comment { .. } => {
            return Err(ModerationActionError::ValidationFailed(
                "only_work_revision_cases_supported",
            ));
        }
    };

    let (pub_state, mod_state, _) =
        super::publication::load_paired_state(&mut tx, owner, work_id).await?;
    let rev_facts = pub_state
        .facts()
        .revisions
        .iter()
        .find(|r| r.snapshot.revision == rev_id)
        .ok_or(ModerationActionError::NotFound)?;
    let dec_facts = mod_state
        .facts()
        .revisions
        .iter()
        .find(|r| r.revision == rev_id)
        .ok_or(ModerationActionError::NotFound)?;
    let current = ReviewSubject::WorkRevision {
        work: work_id,
        revision: rev_id,
        digest: rev_facts.snapshot.content.hash().clone(),
        decision: dec_facts.version,
    };

    let directory = load_moderation_directory(&mut tx, now).await?;
    let (transition, conclusion) = match case.conclude(
        &directory,
        actor,
        expected_case_version,
        &current,
        verdict,
        reason,
        false,
        now,
    ) {
        Ok(concluded) => concluded,
        Err(refusal) => {
            requeue_if_authority_lost(tx, &case, &directory, actor, refusal, now).await?;
            return Err(case_refusal_to_action_error(refusal));
        }
    };

    let admin_trans = mod_state
        .apply_case_conclusion(&pub_state, &conclusion)
        .map_err(|e| match e {
            ModerationError::StaleDecision => ModerationActionError::Conflict("stale_decision"),
            ModerationError::WrongAuthority => ModerationActionError::Forbidden("wrong_authority"),
            ModerationError::NotAdmin => ModerationActionError::Forbidden("not_admin"),
            ModerationError::Publication(_) => {
                ModerationActionError::Conflict("publication_changed")
            }
            _ => ModerationActionError::Internal,
        })?;

    if !write_case(&mut tx, &transition.next, expected_case_version.get()).await? {
        return Err(ModerationActionError::Conflict("case_version_stale"));
    }
    let next_pub = admin_trans.publication.facts();
    let next_mod = admin_trans.moderation.facts();
    write_decision(
        &mut tx,
        next_pub,
        next_mod,
        rev_id,
        DecisionSource {
            evaluation: None,
            case: Some(case_id),
        },
    )
    .await?;
    write_publication_pointers(&mut tx, next_pub).await?;
    apply_pointer_effect(&mut tx, work_id, admin_trans.effects.pointer).await?;
    sync_job_states(&mut tx, next_mod).await?;

    tx.commit().await.map_err(classify)?;
    Ok(admin_trans)
}

// ---------------------------------------------------------------------------
// 5. Grants and groups (MOD-11)
// ---------------------------------------------------------------------------

pub async fn create_moderation_grant(
    pool: &PgPool,
    admin: PrincipalId,
    spec: GrantSpec,
    now: OffsetDateTime,
) -> Result<GrantId, ModerationActionError> {
    let mut tx = pool.begin().await.map_err(classify)?;
    let (directory, seat) = admin_seat(&mut tx, admin, now).await?;
    if spec.expires_at.is_some_and(|at| at <= now) {
        return Err(ModerationActionError::ValidationFailed("expires_in_past"));
    }
    if let Grantee::Principal(p) = spec.grantee {
        let known: Option<i32> = query_scalar("SELECT 1 FROM principals WHERE id = $1")
            .bind(p.as_uuid())
            .fetch_optional(&mut *tx)
            .await
            .map_err(classify)?;
        if known.is_none() {
            return Err(ModerationActionError::ValidationFailed("unknown_principal"));
        }
    }
    if let GrantScope::Work(w) = spec.scope {
        let known: Option<i32> = query_scalar("SELECT 1 FROM works WHERE id = $1")
            .bind(w.as_uuid())
            .fetch_optional(&mut *tx)
            .await
            .map_err(classify)?;
        if known.is_none() {
            return Err(ModerationActionError::ValidationFailed("unknown_work"));
        }
    }
    let id = GrantId::from_uuid(Uuid::now_v7());
    let (_, audit) = directory.grant(seat, id, spec).map_err(|e| match e {
        DirectoryError::UnknownGroup => ModerationActionError::ValidationFailed("unknown_group"),
        DirectoryError::DuplicateGrant => ModerationActionError::Conflict("duplicate_grant"),
        DirectoryError::Refused(a) => ModerationActionError::Forbidden(a.as_str()),
        _ => ModerationActionError::Internal,
    })?;
    let (grantee_kind, grantee_id) = match spec.grantee {
        Grantee::Principal(p) => ("principal", p.as_uuid()),
        Grantee::Group(g) => ("group", g.as_uuid()),
    };
    let (scope_kind, scope_work) = match spec.scope {
        GrantScope::Global => ("global", None),
        GrantScope::Work(w) => ("work", Some(w.as_uuid())),
    };
    query(
        "INSERT INTO moderation_grants (
             id, grantee_kind, grantee_id, capability, scope_kind, scope_work_id,
             expires_at, revoked, origin, created_by, created_at
         )
         VALUES ($1, $2, $3, $4, $5, $6, $7, false, 'admin', $8, now())",
    )
    .bind(id.as_uuid())
    .bind(grantee_kind)
    .bind(grantee_id)
    .bind(spec.capability.as_str())
    .bind(scope_kind)
    .bind(scope_work)
    .bind(spec.expires_at)
    .bind(admin.as_uuid())
    .execute(&mut *tx)
    .await
    .map_err(classify)?;
    write_directory_audit(&mut tx, audit).await?;
    tx.commit().await.map_err(classify)?;
    Ok(id)
}

/// Revoke an admin-issued grant. Claims that rested on it are revalidated in the same commit:
/// another live basis keeps the claimant, none returns the case to the queue.
pub async fn revoke_moderation_grant(
    pool: &PgPool,
    admin: PrincipalId,
    grant_id: GrantId,
    now: OffsetDateTime,
) -> Result<(), ModerationActionError> {
    let mut tx = pool.begin().await.map_err(classify)?;
    let (_, seat) = admin_seat(&mut tx, admin, now).await?;
    let row: Option<String> =
        query_scalar("SELECT origin FROM moderation_grants WHERE id = $1 FOR UPDATE")
            .bind(grant_id.as_uuid())
            .fetch_optional(&mut *tx)
            .await
            .map_err(classify)?;
    // Inherited comment grants are revoked through `vink-app moderator revoke`.
    if row.as_deref() != Some("admin") {
        return Err(ModerationActionError::NotFound);
    }
    let directory = load_moderation_directory(&mut tx, now).await?;
    let (after, audit) = directory.revoke(seat, grant_id).map_err(|e| match e {
        DirectoryError::UnknownGrant => ModerationActionError::NotFound,
        DirectoryError::Unchanged => ModerationActionError::Conflict("already_revoked"),
        DirectoryError::Refused(a) => ModerationActionError::Forbidden(a.as_str()),
        _ => ModerationActionError::Internal,
    })?;
    query(
        "UPDATE moderation_grants
            SET revoked = true, revoked_at = now(), revoked_by = $1
          WHERE id = $2 AND revoked = false",
    )
    .bind(admin.as_uuid())
    .bind(grant_id.as_uuid())
    .execute(&mut *tx)
    .await
    .map_err(classify)?;
    write_directory_audit(&mut tx, audit).await?;
    revalidate_claims(&mut tx, &after, None, now).await?;
    tx.commit().await.map_err(classify)?;
    Ok(())
}

pub async fn create_moderation_group(
    pool: &PgPool,
    admin: PrincipalId,
    name: &str,
    now: OffsetDateTime,
) -> Result<GroupId, ModerationActionError> {
    let name = name.trim();
    let length = name.chars().count();
    if !(1..=80).contains(&length) || name.chars().any(char::is_control) {
        return Err(ModerationActionError::ValidationFailed("group_name"));
    }
    let mut tx = pool.begin().await.map_err(classify)?;
    let (_, seat) = admin_seat(&mut tx, admin, now).await?;
    let id = GroupId::from_uuid(Uuid::now_v7());
    let created: Option<Uuid> = query_scalar(
        "INSERT INTO moderation_groups (id, name, created_by, created_at)
         VALUES ($1, $2, $3, now())
         ON CONFLICT (name) DO NOTHING
         RETURNING id",
    )
    .bind(id.as_uuid())
    .bind(name)
    .bind(admin.as_uuid())
    .fetch_optional(&mut *tx)
    .await
    .map_err(classify)?;
    if created.is_none() {
        return Err(ModerationActionError::Conflict("duplicate_group"));
    }
    query(
        "INSERT INTO moderation_directory_audit (id, admin_id, change, group_id, created_at)
         VALUES ($1, $2, 'group_created', $3, $4)",
    )
    .bind(Uuid::now_v7())
    .bind(admin.as_uuid())
    .bind(id.as_uuid())
    .bind(seat.checked_at())
    .execute(&mut *tx)
    .await
    .map_err(classify)?;
    tx.commit().await.map_err(classify)?;
    Ok(id)
}

/// Add or remove a group member. A removed member's claims are revalidated in the same commit.
pub async fn set_group_membership(
    pool: &PgPool,
    admin: PrincipalId,
    group: GroupId,
    member: PrincipalId,
    present: bool,
    now: OffsetDateTime,
) -> Result<(), ModerationActionError> {
    let mut tx = pool.begin().await.map_err(classify)?;
    let (_, seat) = admin_seat(&mut tx, admin, now).await?;
    let locked: Option<Uuid> =
        query_scalar("SELECT id FROM moderation_groups WHERE id = $1 FOR UPDATE")
            .bind(group.as_uuid())
            .fetch_optional(&mut *tx)
            .await
            .map_err(classify)?;
    if locked.is_none() {
        return Err(ModerationActionError::NotFound);
    }
    let known: Option<i32> = query_scalar("SELECT 1 FROM principals WHERE id = $1")
        .bind(member.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(classify)?;
    if known.is_none() {
        return Err(ModerationActionError::ValidationFailed("unknown_principal"));
    }
    let directory = load_moderation_directory(&mut tx, now).await?;
    let (after, audit) = directory
        .set_membership(seat, group, member, present)
        .map_err(|e| match e {
            DirectoryError::UnknownGroup => ModerationActionError::NotFound,
            DirectoryError::Unchanged => ModerationActionError::Conflict("membership_unchanged"),
            DirectoryError::Refused(a) => ModerationActionError::Forbidden(a.as_str()),
            _ => ModerationActionError::Internal,
        })?;
    if present {
        query(
            "INSERT INTO moderation_group_members (group_id, principal_id, added_by, created_at)
             VALUES ($1, $2, $3, now())",
        )
        .bind(group.as_uuid())
        .bind(member.as_uuid())
        .bind(admin.as_uuid())
        .execute(&mut *tx)
        .await
        .map_err(classify)?;
    } else {
        query("DELETE FROM moderation_group_members WHERE group_id = $1 AND principal_id = $2")
            .bind(group.as_uuid())
            .bind(member.as_uuid())
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
    }
    write_directory_audit(&mut tx, audit).await?;
    if !present {
        revalidate_claims(&mut tx, &after, Some(member), now).await?;
    }
    tx.commit().await.map_err(classify)?;
    Ok(())
}

pub async fn list_moderation_grants(
    conn: &mut PgConnection,
) -> Result<Vec<GrantFacts>, StoreError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        grantee_kind: String,
        grantee_id: Uuid,
        capability: String,
        scope_kind: String,
        scope_work_id: Option<Uuid>,
        expires_at: Option<OffsetDateTime>,
        revoked: bool,
        origin: String,
    }

    let rows: Vec<Row> = query_as(
        "SELECT id, grantee_kind, grantee_id, capability, scope_kind,
                scope_work_id, expires_at, revoked, origin
           FROM moderation_grants
          ORDER BY created_at ASC",
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(classify)?;

    let mut grants = Vec::with_capacity(rows.len());
    for r in rows {
        let grantee = match r.grantee_kind.as_str() {
            "principal" => Grantee::Principal(PrincipalId::from_uuid(r.grantee_id)),
            "group" => Grantee::Group(GroupId::from_uuid(r.grantee_id)),
            other => return Err(integrity("unknown grantee kind", other)),
        };
        let capability = match r.capability.as_str() {
            "works" => ReviewCapability::Works,
            "comments" => ReviewCapability::Comments,
            other => return Err(integrity("unknown capability", other)),
        };
        let scope = match r.scope_kind.as_str() {
            "global" => GrantScope::Global,
            "work" => {
                let w = r
                    .scope_work_id
                    .ok_or_else(|| integrity("work scope missing work_id", r.id))?;
                GrantScope::Work(WorkId::from_uuid(w))
            }
            other => return Err(integrity("unknown scope kind", other)),
        };
        let origin = match r.origin.as_str() {
            "admin" => GrantOrigin::Admin,
            "inherited_comment" => GrantOrigin::InheritedComment,
            other => return Err(integrity("unknown grant origin", other)),
        };
        grants.push(GrantFacts {
            id: GrantId::from_uuid(r.id),
            grantee,
            capability,
            scope,
            expires_at: r.expires_at,
            revoked: r.revoked,
            origin,
        });
    }

    Ok(grants)
}
