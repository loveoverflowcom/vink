//! Moderation persistence: directory, grants, cases, automation and evaluation jobs (#159, MOD-09..15).

use sqlx::{PgConnection, PgPool, query, query_as, query_scalar};
use time::OffsetDateTime;
use uuid::Uuid;

use super::postgres::{classify, integrity};
use crate::application::moderation::{ModerationActionError, case_refusal_to_action_error};
use crate::application::ports::StoreError;
use crate::domain::ids::{CaseId, GrantId, GroupId, PrincipalId, RevisionId, WorkId};
use crate::domain::moderation::authority::{
    AuthorityBasis, DirectoryError, DirectoryFacts, GrantFacts, GrantOrigin, GrantScope, GrantSpec,
    Grantee, GroupFacts, ModerationDirectory, ReviewCapability, ReviewTarget,
};
use crate::domain::moderation::automation::{
    Automation, AutomationError, AutomationMode, AutomationVersion,
};
use crate::domain::moderation::case::{
    CaseFacts, CaseOrigin, CaseState, CaseVerdict, CaseVersion, CloseReason, InternalNote,
    NoteFacts, ReviewCase, ReviewSubject,
};
use crate::domain::moderation::{
    AdminTransition, DecisionProvenance, DecisionReason, DecisionVersion, ModerationError,
};
use crate::domain::publication::PointerEffect;

// ---------------------------------------------------------------------------
// 1. Directory and Admin Seats
// ---------------------------------------------------------------------------

pub async fn load_moderation_directory(
    conn: &mut PgConnection,
    _now: OffsetDateTime,
) -> Result<ModerationDirectory, StoreError> {
    // 1. Admins
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

// Admin Seats CLI management
pub async fn add_admin_seat(
    conn: &mut PgConnection,
    operator: &str,
    principal: PrincipalId,
    reason: Option<&str>,
) -> Result<(), StoreError> {
    query(
        "INSERT INTO moderation_admin_seats (principal_id, created_at)
         VALUES ($1, now())
         ON CONFLICT (principal_id) DO NOTHING",
    )
    .bind(principal.as_uuid())
    .execute(&mut *conn)
    .await
    .map_err(classify)?;

    query(
        "INSERT INTO moderation_admin_audit (id, operator, action, principal_id, reason, created_at)
         VALUES ($1, $2, 'add', $3, $4, now())",
    )
    .bind(Uuid::now_v7())
    .bind(operator)
    .bind(principal.as_uuid())
    .bind(reason)
    .execute(&mut *conn)
    .await
    .map_err(classify)?;

    Ok(())
}

pub async fn remove_admin_seat(
    conn: &mut PgConnection,
    operator: &str,
    principal: PrincipalId,
    reason: Option<&str>,
) -> Result<(), StoreError> {
    query("DELETE FROM moderation_admin_seats WHERE principal_id = $1")
        .bind(principal.as_uuid())
        .execute(&mut *conn)
        .await
        .map_err(classify)?;

    query(
        "INSERT INTO moderation_admin_audit (id, operator, action, principal_id, reason, created_at)
         VALUES ($1, $2, 'remove', $3, $4, now())",
    )
    .bind(Uuid::now_v7())
    .bind(operator)
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
// 2. Automation Mode
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

pub async fn set_automation_mode(
    conn: &mut PgConnection,
    admin: PrincipalId,
    expected: AutomationVersion,
    mode: AutomationMode,
    reason: Option<DecisionReason>,
    now: OffsetDateTime,
) -> Result<Automation, ModerationActionError> {
    let directory = load_moderation_directory(conn, now).await?;
    let current = load_automation(conn).await?;

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

    // Update state table
    let updated = query(
        "UPDATE moderation_automation_state
            SET mode = $1, version = $2, updated_at = now()
          WHERE id = 1 AND version = $3",
    )
    .bind(mode.as_str())
    .bind(next.version().get())
    .bind(expected.get())
    .execute(&mut *conn)
    .await
    .map_err(classify)?;

    if updated.rows_affected() == 0 {
        return Err(ModerationActionError::Conflict("automation_version_stale"));
    }

    // Audit row
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
    .execute(&mut *conn)
    .await
    .map_err(classify)?;

    // When pausing: open evaluation jobs are rerouted and open human review cases created
    if mode == AutomationMode::Paused {
        #[derive(sqlx::FromRow)]
        struct RerouteRow {
            work_id: Uuid,
            revision_id: Uuid,
            content_digest: String,
        }

        let rerouted: Vec<RerouteRow> = query_as(
            "UPDATE work_evaluation_jobs
                SET state = 'rerouted', updated_at = now()
              WHERE state IN ('open', 'leased')
              RETURNING work_id, revision_id, content_digest",
        )
        .fetch_all(&mut *conn)
        .await
        .map_err(classify)?;

        for job in rerouted {
            // Find owner of work
            let owner: Option<Uuid> = query_scalar("SELECT owner_id FROM works WHERE id = $1")
                .bind(job.work_id)
                .fetch_optional(&mut *conn)
                .await
                .map_err(classify)?;

            if let Some(owner_id) = owner {
                query(
                    "INSERT INTO moderation_cases (
                         id, subject_kind, work_id, revision_id, content_digest, decision_version,
                         target_content_owner, target_work_owner, origin, version, state,
                         created_at, updated_at
                     )
                     VALUES ($1, 'work_revision', $2, $3, $4, 1, $5, $5, 'automation_paused', 1, 'queued', now(), now())
                     ON CONFLICT (work_id, revision_id) WHERE (state IN ('queued', 'claimed', 'escalated') AND subject_kind = 'work_revision')
                     DO NOTHING",
                )
                .bind(Uuid::now_v7())
                .bind(job.work_id)
                .bind(job.revision_id)
                .bind(job.content_digest)
                .bind(owner_id)
                .execute(&mut *conn)
                .await
                .map_err(classify)?;
            }
        }
    }

    Ok(next)
}

// ---------------------------------------------------------------------------
// 3. Review Cases
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
    pub created_at: OffsetDateTime,
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
        created_at: OffsetDateTime,
    }

    let rows: Vec<Row> = query_as(
        "SELECT id, subject_kind, work_id, revision_id, decision_version, origin, version, state, claimed_by, created_at
           FROM moderation_cases
          WHERE state IN ('queued', 'claimed', 'escalated') AND subject_kind = 'work_revision'
          ORDER BY created_at ASC",
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

#[derive(Debug, Clone)]
pub struct CaseDetail {
    pub id: CaseId,
    pub subject_kind: String,
    pub work_id: WorkId,
    pub revision_id: Option<RevisionId>,
    pub title: Option<String>,
    pub author_id: PrincipalId,
    pub author_name: String,
    pub decision_version: i64,
    pub origin: String,
    pub version: i64,
    pub state: String,
    pub claimed_by: Option<PrincipalId>,
    pub notes: Vec<NoteRecord>,
    pub created_at: OffsetDateTime,
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
        decision_version: i64,
        origin: String,
        version: i64,
        state: String,
        claimed_by: Option<Uuid>,
        created_at: OffsetDateTime,
        target_content_owner: Uuid,
        author_name: String,
        title: Option<String>,
    }

    let row: Option<Row> = query_as(
        "SELECT c.id, c.subject_kind, c.work_id, c.revision_id, c.decision_version,
                c.origin, c.version, c.state, c.claimed_by, c.created_at,
                c.target_content_owner, p.display_name AS author_name, r.title
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
          ORDER BY created_at ASC",
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
        title: r.title,
        author_id: PrincipalId::from_uuid(r.target_content_owner),
        author_name: r.author_name,
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
// 4. Case Mutations and Conclusions
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

pub async fn claim_case(
    conn: &mut PgConnection,
    actor: PrincipalId,
    case_id: CaseId,
    expected_version: CaseVersion,
    now: OffsetDateTime,
) -> Result<ReviewCase, ModerationActionError> {
    let directory = load_moderation_directory(conn, now).await?;
    let case = load_case_locked(conn, case_id)
        .await?
        .ok_or(ModerationActionError::NotFound)?;
    let transition = case
        .claim(&directory, actor, expected_version, now)
        .map_err(case_refusal_to_action_error)?;

    let next_case = transition.next;
    let (basis_kind, grant_id) = match next_case.facts().state {
        CaseState::Claimed { basis, .. } => match basis {
            AuthorityBasis::Admin => ("admin", None),
            AuthorityBasis::Grant(g) => ("grant", Some(g.as_uuid())),
        },
        _ => return Err(ModerationActionError::Internal),
    };

    let updated = query(
        "UPDATE moderation_cases
            SET version = $1, state = 'claimed', claimed_by = $2,
                claimed_basis_kind = $3, claimed_basis_grant_id = $4,
                updated_at = now()
          WHERE id = $5 AND version = $6",
    )
    .bind(next_case.facts().version.get())
    .bind(actor.as_uuid())
    .bind(basis_kind)
    .bind(grant_id)
    .bind(case_id.as_uuid())
    .bind(expected_version.get())
    .execute(&mut *conn)
    .await
    .map_err(classify)?;

    if updated.rows_affected() == 0 {
        return Err(ModerationActionError::Conflict("case_version_stale"));
    }

    Ok(next_case)
}

pub async fn release_case(
    conn: &mut PgConnection,
    actor: PrincipalId,
    case_id: CaseId,
    expected_version: CaseVersion,
) -> Result<ReviewCase, ModerationActionError> {
    let case = load_case_locked(conn, case_id)
        .await?
        .ok_or(ModerationActionError::NotFound)?;
    let transition = case
        .release(actor, expected_version)
        .map_err(case_refusal_to_action_error)?;

    let next_case = transition.next;

    let updated = query(
        "UPDATE moderation_cases
            SET version = $1, state = 'queued', claimed_by = NULL,
                claimed_basis_kind = NULL, claimed_basis_grant_id = NULL,
                updated_at = now()
          WHERE id = $2 AND version = $3",
    )
    .bind(next_case.facts().version.get())
    .bind(case_id.as_uuid())
    .bind(expected_version.get())
    .execute(&mut *conn)
    .await
    .map_err(classify)?;

    if updated.rows_affected() == 0 {
        return Err(ModerationActionError::Conflict("case_version_stale"));
    }

    Ok(next_case)
}

pub async fn add_case_note(
    conn: &mut PgConnection,
    actor: PrincipalId,
    case_id: CaseId,
    expected_version: CaseVersion,
    note: InternalNote,
    now: OffsetDateTime,
) -> Result<ReviewCase, ModerationActionError> {
    let directory = load_moderation_directory(conn, now).await?;
    let case = load_case_locked(conn, case_id)
        .await?
        .ok_or(ModerationActionError::NotFound)?;
    let transition = case
        .note(&directory, actor, expected_version, note.clone(), now)
        .map_err(case_refusal_to_action_error)?;

    let next_case = transition.next;

    query(
        "INSERT INTO moderation_case_notes (id, case_id, author_id, note, created_at)
         VALUES ($1, $2, $3, $4, now())",
    )
    .bind(Uuid::now_v7())
    .bind(case_id.as_uuid())
    .bind(actor.as_uuid())
    .bind(note.as_str())
    .execute(&mut *conn)
    .await
    .map_err(classify)?;

    let updated = query(
        "UPDATE moderation_cases
            SET version = $1, updated_at = now()
          WHERE id = $2 AND version = $3",
    )
    .bind(next_case.facts().version.get())
    .bind(case_id.as_uuid())
    .bind(expected_version.get())
    .execute(&mut *conn)
    .await
    .map_err(classify)?;

    if updated.rows_affected() == 0 {
        return Err(ModerationActionError::Conflict("case_version_stale"));
    }

    Ok(next_case)
}

pub async fn escalate_case(
    conn: &mut PgConnection,
    actor: PrincipalId,
    case_id: CaseId,
    expected_version: CaseVersion,
) -> Result<ReviewCase, ModerationActionError> {
    let case = load_case_locked(conn, case_id)
        .await?
        .ok_or(ModerationActionError::NotFound)?;
    let transition = case
        .escalate(actor, expected_version)
        .map_err(case_refusal_to_action_error)?;

    let next_case = transition.next;

    let updated = query(
        "UPDATE moderation_cases
            SET version = $1, state = 'escalated', claimed_by = NULL,
                claimed_basis_kind = NULL, claimed_basis_grant_id = NULL,
                updated_at = now()
          WHERE id = $2 AND version = $3",
    )
    .bind(next_case.facts().version.get())
    .bind(case_id.as_uuid())
    .bind(expected_version.get())
    .execute(&mut *conn)
    .await
    .map_err(classify)?;

    if updated.rows_affected() == 0 {
        return Err(ModerationActionError::Conflict("case_version_stale"));
    }

    Ok(next_case)
}

pub async fn takeover_case(
    conn: &mut PgConnection,
    actor: PrincipalId,
    case_id: CaseId,
    expected_version: CaseVersion,
    reason: DecisionReason,
    now: OffsetDateTime,
) -> Result<ReviewCase, ModerationActionError> {
    let directory = load_moderation_directory(conn, now).await?;
    let case = load_case_locked(conn, case_id)
        .await?
        .ok_or(ModerationActionError::NotFound)?;
    let transition = case
        .take_over(&directory, actor, expected_version, Some(&reason), now)
        .map_err(case_refusal_to_action_error)?;

    let next_case = transition.next;
    let (basis_kind, grant_id) = match next_case.facts().state {
        CaseState::Claimed { basis, .. } => match basis {
            AuthorityBasis::Admin => ("admin", None),
            AuthorityBasis::Grant(g) => ("grant", Some(g.as_uuid())),
        },
        _ => return Err(ModerationActionError::Internal),
    };

    let updated = query(
        "UPDATE moderation_cases
            SET version = $1, state = 'claimed', claimed_by = $2,
                claimed_basis_kind = $3, claimed_basis_grant_id = $4,
                updated_at = now()
          WHERE id = $5 AND version = $6",
    )
    .bind(next_case.facts().version.get())
    .bind(actor.as_uuid())
    .bind(basis_kind)
    .bind(grant_id)
    .bind(case_id.as_uuid())
    .bind(expected_version.get())
    .execute(&mut *conn)
    .await
    .map_err(classify)?;

    if updated.rows_affected() == 0 {
        return Err(ModerationActionError::Conflict("case_version_stale"));
    }

    Ok(next_case)
}

// ---------------------------------------------------------------------------
// 6. Conclude Case (Single atomic transaction)
// ---------------------------------------------------------------------------

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

    let case = load_case_locked(&mut tx, case_id)
        .await?
        .ok_or(ModerationActionError::NotFound)?;

    let (work_id, rev_id) = match &case.facts().subject {
        ReviewSubject::WorkRevision { work, revision, .. } => (*work, *revision),
        _ => {
            return Err(ModerationActionError::ValidationFailed(
                "only_work_revision_cases_supported",
            ));
        }
    };

    // Lock work_publications FOR UPDATE
    let _: Option<Uuid> =
        query_scalar("SELECT work_id FROM work_publications WHERE work_id = $1 FOR UPDATE")
            .bind(work_id.as_uuid())
            .fetch_optional(&mut *tx)
            .await
            .map_err(classify)?;

    let owner_id: Uuid = query_scalar("SELECT owner_id FROM works WHERE id = $1 FOR SHARE")
        .bind(work_id.as_uuid())
        .fetch_one(&mut *tx)
        .await
        .map_err(classify)?;

    let (pub_state, mod_state, _) =
        super::publication::load_paired_state(&mut tx, PrincipalId::from_uuid(owner_id), work_id)
            .await?;

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

    let (transition, conclusion) = case
        .conclude(
            &directory,
            actor,
            expected_case_version,
            &current,
            verdict,
            reason.clone(),
            false,
            now,
        )
        .map_err(case_refusal_to_action_error)?;

    let next_case = transition.next;
    let next_version = next_case.facts().version.get();

    let admin_trans = mod_state
        .apply_case_conclusion(&pub_state, &conclusion)
        .map_err(|e| match e {
            ModerationError::StaleDecision => ModerationActionError::Conflict("stale_decision"),
            ModerationError::WrongAuthority => ModerationActionError::Forbidden("wrong_authority"),
            ModerationError::NotAdmin => ModerationActionError::Forbidden("not_admin"),
            _ => ModerationActionError::Internal,
        })?;

    let (concluded_basis_kind, concluded_basis_grant_id) = match conclusion.authority.basis() {
        AuthorityBasis::Admin => ("admin", None),
        AuthorityBasis::Grant(g) => ("grant", Some(g.as_uuid())),
    };

    // Update case to concluded
    let updated_case = query(
        "UPDATE moderation_cases
            SET version = $1, state = 'concluded', concluded_by = $2,
                concluded_basis_kind = $3, concluded_basis_grant_id = $4,
                concluded_verdict = $5, updated_at = now()
          WHERE id = $6 AND version = $7",
    )
    .bind(next_version)
    .bind(actor.as_uuid())
    .bind(concluded_basis_kind)
    .bind(concluded_basis_grant_id)
    .bind(match verdict {
        CaseVerdict::Approve => "approve",
        CaseVerdict::Reject => "reject",
    })
    .bind(case_id.as_uuid())
    .bind(expected_case_version.get())
    .execute(&mut *tx)
    .await
    .map_err(classify)?;

    if updated_case.rows_affected() == 0 {
        tx.rollback().await.map_err(classify)?;
        return Err(ModerationActionError::Conflict("case_version_stale"));
    }

    let next_pub = admin_trans.publication.facts();
    let next_mod = admin_trans.moderation.facts();
    let next_dec = next_mod
        .revisions
        .iter()
        .find(|r| r.revision == rev_id)
        .ok_or(ModerationActionError::Internal)?;

    // Update work_revision_moderation
    let (prov_kind, mod_grant_id) = match next_dec.provenance.as_ref() {
        Some(DecisionProvenance::Admin { .. }) => ("admin", None),
        Some(DecisionProvenance::Moderator { grant, .. }) => ("moderator", Some(grant.as_uuid())),
        _ => return Err(ModerationActionError::Internal),
    };

    let mod_status = match verdict {
        CaseVerdict::Approve => "approved",
        CaseVerdict::Reject => "rejected",
    };

    let reason_str = reason
        .as_ref()
        .map(|r| r.as_str())
        .unwrap_or("approved on review");

    query(
        "UPDATE work_revision_moderation
            SET decision_version = $1, moderation = $2, provenance_kind = $3,
                admin_actor = $4, admin_verdict = $5, admin_reason = $6,
                moderator_grant_id = $7, policy_version = $8, updated_at = now()
          WHERE work_id = $9 AND revision_id = $10",
    )
    .bind(next_dec.version.get())
    .bind(mod_status)
    .bind(prov_kind)
    .bind(actor.as_uuid())
    .bind(match verdict {
        CaseVerdict::Approve => "approve",
        CaseVerdict::Reject => "reject",
    })
    .bind(reason_str)
    .bind(mod_grant_id)
    .bind(next_dec.policy.get())
    .bind(work_id.as_uuid())
    .bind(rev_id.as_uuid())
    .execute(&mut *tx)
    .await
    .map_err(classify)?;

    // Update work_publications
    let pointer_rev = next_pub.pointer.map(|p| p.revision.as_uuid());
    let pointer_gen = next_pub.pointer.map(|p| p.intent_generation.get());
    let withdrawn_rev = next_pub.review_withdrawn.map(|p| p.revision.as_uuid());
    let withdrawn_gen = next_pub.review_withdrawn.map(|p| p.intent_generation.get());
    let ever_public_uuids: Vec<Uuid> = next_pub.ever_public.iter().map(|r| r.as_uuid()).collect();

    query(
        "UPDATE work_publications
            SET pointer_revision_id = $1,
                pointer_intent_generation = $2,
                review_withdrawn_revision_id = $3,
                review_withdrawn_intent_generation = $4,
                ever_public = $5,
                public_epoch = $6,
                updated_at = now()
          WHERE work_id = $7",
    )
    .bind(pointer_rev)
    .bind(pointer_gen)
    .bind(withdrawn_rev)
    .bind(withdrawn_gen)
    .bind(&ever_public_uuids)
    .bind(next_pub.public_epoch.get())
    .bind(work_id.as_uuid())
    .execute(&mut *tx)
    .await
    .map_err(classify)?;

    // Pointer effect on publications table
    match admin_trans.effects.pointer {
        PointerEffect::Set(p) => {
            let meta_id: Option<Uuid> = query_scalar(
                "SELECT metadata_snapshot_id FROM revisions WHERE id = $1 AND work_id = $2",
            )
            .bind(p.revision.as_uuid())
            .bind(work_id.as_uuid())
            .fetch_optional(&mut *tx)
            .await
            .map_err(classify)?;

            query(
                "INSERT INTO publications (work_id, revision_id, generation, published_at, metadata_snapshot_id)
                 VALUES ($1, $2, $3, now(), $4)
                 ON CONFLICT (work_id) DO UPDATE SET
                     revision_id = EXCLUDED.revision_id,
                     generation = EXCLUDED.generation,
                     published_at = EXCLUDED.published_at,
                     metadata_snapshot_id = EXCLUDED.metadata_snapshot_id",
            )
            .bind(work_id.as_uuid())
            .bind(p.revision.as_uuid())
            .bind(p.intent_generation.get())
            .bind(meta_id)
            .execute(&mut *tx)
            .await
            .map_err(classify)?;
        }
        PointerEffect::Clear => {
            query("DELETE FROM publications WHERE work_id = $1")
                .bind(work_id.as_uuid())
                .execute(&mut *tx)
                .await
                .map_err(classify)?;
        }
        PointerEffect::Unchanged => {}
    }

    // Mark any open evaluation jobs for this revision as applied
    query(
        "UPDATE work_evaluation_jobs
            SET state = 'applied', updated_at = now()
          WHERE work_id = $1 AND revision_id = $2 AND state IN ('open', 'leased', 'rerouted')",
    )
    .bind(work_id.as_uuid())
    .bind(rev_id.as_uuid())
    .execute(&mut *tx)
    .await
    .map_err(classify)?;

    tx.commit().await.map_err(classify)?;

    Ok(admin_trans)
}

// ---------------------------------------------------------------------------
// 7. Grant Management
// ---------------------------------------------------------------------------

pub async fn create_moderation_grant(
    conn: &mut PgConnection,
    admin: PrincipalId,
    grantee: Grantee,
    capability: ReviewCapability,
    scope: GrantScope,
    expires_at: Option<OffsetDateTime>,
    now: OffsetDateTime,
) -> Result<GrantId, ModerationActionError> {
    let directory = load_moderation_directory(conn, now).await?;
    let seat = directory
        .require_admin(admin, now)
        .map_err(|_| ModerationActionError::Forbidden("not_admin"))?;
    let id = GrantId::from_uuid(Uuid::now_v7());
    let spec = GrantSpec {
        grantee,
        capability,
        scope,
        expires_at,
    };
    let (_, _audit) = directory.grant(seat, id, spec).map_err(|e| match e {
        DirectoryError::UnknownGroup => ModerationActionError::ValidationFailed("unknown_group"),
        DirectoryError::DuplicateGrant => ModerationActionError::Conflict("duplicate_grant"),
        _ => ModerationActionError::Internal,
    })?;
    let (grantee_kind, grantee_id) = match grantee {
        Grantee::Principal(p) => ("principal", p.as_uuid()),
        Grantee::Group(g) => ("group", g.as_uuid()),
    };
    let (scope_kind, scope_work) = match scope {
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
    .bind(match capability {
        ReviewCapability::Works => "works",
        ReviewCapability::Comments => "comments",
    })
    .bind(scope_kind)
    .bind(scope_work)
    .bind(expires_at)
    .bind(admin.as_uuid())
    .execute(&mut *conn)
    .await
    .map_err(classify)?;

    Ok(id)
}

pub async fn revoke_moderation_grant(
    conn: &mut PgConnection,
    admin: PrincipalId,
    grant_id: GrantId,
    now: OffsetDateTime,
) -> Result<(), ModerationActionError> {
    let directory = load_moderation_directory(conn, now).await?;
    let seat = directory
        .require_admin(admin, now)
        .map_err(|_| ModerationActionError::Forbidden("not_admin"))?;

    let (_, _audit) = directory.revoke(seat, grant_id).map_err(|e| match e {
        DirectoryError::UnknownGrant => ModerationActionError::NotFound,
        DirectoryError::Unchanged => ModerationActionError::Conflict("already_revoked"),
        _ => ModerationActionError::Internal,
    })?;

    let updated = query(
        "UPDATE moderation_grants
            SET revoked = true, revoked_at = now(), revoked_by = $1
          WHERE id = $2 AND revoked = false",
    )
    .bind(admin.as_uuid())
    .bind(grant_id.as_uuid())
    .execute(&mut *conn)
    .await
    .map_err(classify)?;

    if updated.rows_affected() == 0 {
        return Err(ModerationActionError::NotFound);
    }

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
