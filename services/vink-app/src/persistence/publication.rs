//! PostgreSQL persistence adapter for author publication commands and paired moderation facts (PUB-RUNTIME-01, #158).

use std::future::Future;

use sha2::{Digest, Sha256};
use sqlx::types::Json;
use sqlx::{PgConnection, query, query_as, query_scalar};
use time::OffsetDateTime;
use uuid::Uuid;

use super::postgres::{PgStore, classify, integrity};
use super::principal::locked_standing;
use super::taxonomy::{SnapshotCapture, capture_metadata_snapshot};
use crate::application::moderation::{ModerationPlanError, plan_author_publication};
use crate::application::ports::{
    AuthorPrivateOutcome, AuthorPublicRequestOutcome, AuthorPublicationStateFacts,
    AuthorPublicationStore, AuthorSnapshotCapture, BoundPrincipal, CaptureSnapshotOutcome,
    RequestPublicOutcome, SetPrivateOutcome, StoreError,
};
use crate::domain::document::{Document, DocumentData, Title};
use crate::domain::draft::DraftVersion;
use crate::domain::ids::{EvaluationId, PrincipalId, RevisionId, WorkId};
use crate::domain::moderation::{
    AdminVerdict, DecisionProvenance, DecisionReason, DecisionVersion, EvaluationJobFacts,
    EvaluationTarget, JobState, ModerationError, NeedsAdminCause, PolicyReasonCode, PolicyVersion,
    RevisionDecisionFacts, WorkModeration, WorkModerationFacts,
};
use crate::domain::principal::Standing;
use crate::domain::publication::{
    Audience, AuthorCommand, AuthorOutcome, DeletionProvenance, IntentGeneration, Lifecycle,
    Moderation, PointerEffect, PublicPointer, Publication, PublicationError, PublicationFacts,
    PublishIntent, RevisionFacts, Snapshot,
};
use crate::domain::revision::{ContentHash, RevisionContent};
use crate::domain::taxonomy::MetadataVersion;

/// A stored receipt outcome; anything else is corruption, never a guessed answer.
fn parse_outcome(raw: &str) -> Result<AuthorOutcome, StoreError> {
    [
        AuthorOutcome::Saved,
        AuthorOutcome::Submitted,
        AuthorOutcome::Waiting,
        AuthorOutcome::Published,
        AuthorOutcome::MadePrivate,
        AuthorOutcome::Reaffirmed,
        AuthorOutcome::Deleted,
    ]
    .into_iter()
    .find(|outcome| outcome.as_str() == raw)
    .ok_or_else(|| integrity("unknown receipt outcome", raw))
}

/// The stored generation of an accepted command's receipt (written for every accepted command).
fn receipt_generation(raw: Option<i64>) -> Result<IntentGeneration, StoreError> {
    raw.and_then(IntentGeneration::new)
        .ok_or_else(|| integrity("receipt generation", raw.unwrap_or_default()))
}

pub(super) async fn load_paired_state(
    conn: &mut PgConnection,
    actor: PrincipalId,
    work_id: WorkId,
) -> Result<
    (
        Publication,
        WorkModeration,
        Vec<(RevisionId, String, Option<Uuid>, OffsetDateTime)>,
    ),
    StoreError,
> {
    #[derive(sqlx::FromRow)]
    struct PubRow {
        audience: String,
        lifecycle: String,
        deletion_provenance: Option<String>,
        tombstone_revision_id: Option<Uuid>,
        tombstone_intent_generation: Option<i64>,
        generation: i64,
        intent_revision_id: Option<Uuid>,
        intent_generation: Option<i64>,
        pointer_revision_id: Option<Uuid>,
        pointer_intent_generation: Option<i64>,
        policy_version: i64,
    }

    let pub_row: Option<PubRow> = query_as(
        "SELECT audience, lifecycle, deletion_provenance,
                tombstone_revision_id, tombstone_intent_generation,
                generation, intent_revision_id, intent_generation,
                pointer_revision_id, pointer_intent_generation, policy_version
           FROM work_publications
          WHERE work_id = $1",
    )
    .bind(work_id.as_uuid())
    .fetch_optional(&mut *conn)
    .await
    .map_err(classify)?;

    let pub_row = pub_row.unwrap_or(PubRow {
        audience: "private".to_string(),
        lifecycle: "active".to_string(),
        deletion_provenance: None,
        tombstone_revision_id: None,
        tombstone_intent_generation: None,
        generation: 1,
        intent_revision_id: None,
        intent_generation: None,
        pointer_revision_id: None,
        pointer_intent_generation: None,
        policy_version: 1,
    });

    #[derive(sqlx::FromRow)]
    struct RevRow {
        id: Uuid,
        title: String,
        document: Json<DocumentData>,
        content_hash: String,
        metadata_snapshot_id: Option<Uuid>,
        created_at: OffsetDateTime,
    }

    let rev_rows: Vec<RevRow> = query_as(
        "SELECT id, title, document, content_hash, metadata_snapshot_id, created_at
           FROM revisions
          WHERE work_id = $1
          ORDER BY created_at ASC, id ASC",
    )
    .bind(work_id.as_uuid())
    .fetch_all(&mut *conn)
    .await
    .map_err(classify)?;

    #[derive(sqlx::FromRow)]
    struct ModRow {
        revision_id: Uuid,
        decision_version: i64,
        moderation: String,
        provenance_kind: Option<String>,
        policy_reason_code: Option<String>,
        needs_admin_cause: Option<String>,
        admin_actor: Option<Uuid>,
        admin_verdict: Option<String>,
        admin_reason: Option<String>,
    }

    let mod_rows: Vec<ModRow> = query_as(
        "SELECT revision_id, decision_version, moderation,
                provenance_kind, policy_reason_code, needs_admin_cause,
                admin_actor, admin_verdict, admin_reason
           FROM work_revision_moderation
          WHERE work_id = $1",
    )
    .bind(work_id.as_uuid())
    .fetch_all(&mut *conn)
    .await
    .map_err(classify)?;

    #[derive(sqlx::FromRow)]
    struct JobRow {
        id: Uuid,
        revision_id: Uuid,
        content_digest: String,
        policy_version: i64,
        state: String,
    }

    let job_rows: Vec<JobRow> = query_as(
        "SELECT id, revision_id, content_digest, policy_version, state
           FROM work_evaluation_jobs
          WHERE work_id = $1
          ORDER BY created_at ASC, id ASC",
    )
    .bind(work_id.as_uuid())
    .fetch_all(&mut *conn)
    .await
    .map_err(classify)?;

    let mut pub_revisions = Vec::with_capacity(rev_rows.len());
    let mut mod_revisions = Vec::new();
    let mut metadata_list = Vec::with_capacity(rev_rows.len());

    for rev in rev_rows {
        let rev_id = RevisionId::from_uuid(rev.id);
        let title = Title::parse(&rev.title).map_err(|e| integrity("revision title", e))?;
        let doc = Document::parse(rev.document.0).map_err(|e| integrity("revision document", e))?;
        let content =
            RevisionContent::new(title, doc).map_err(|e| integrity("revision content", e))?;
        if content.hash().as_str() != rev.content_hash {
            return Err(integrity(
                "revision content hash mismatch",
                content.hash().as_str(),
            ));
        }

        let mod_row = mod_rows.iter().find(|m| m.revision_id == rev.id);
        let (moderation, decision_facts) = match mod_row {
            None => (Moderation::Unsubmitted, None),
            Some(m) => {
                let mod_status = match m.moderation.as_str() {
                    "pending" => Moderation::Pending,
                    "approved" => Moderation::Approved,
                    "rejected" => Moderation::Rejected,
                    "needs_admin" => Moderation::NeedsAdmin,
                    other => return Err(integrity("unknown moderation status", other)),
                };

                let provenance = match m.provenance_kind.as_deref() {
                    None => None,
                    Some("ai_approved") => Some(DecisionProvenance::AiApproved),
                    Some("ai_rejected") => {
                        let code_str = m.policy_reason_code.as_deref().unwrap_or_default();
                        let code = PolicyReasonCode::parse(code_str)
                            .ok_or_else(|| integrity("unknown policy reason code", code_str))?;
                        Some(DecisionProvenance::AiRejected(code))
                    }
                    Some("ai_needs_admin") => {
                        let cause = match m.needs_admin_cause.as_deref() {
                            Some("uncertain") => NeedsAdminCause::Uncertain,
                            Some("invalid_output") => NeedsAdminCause::InvalidOutput,
                            other => {
                                return Err(integrity(
                                    "unknown needs admin cause",
                                    other.unwrap_or("none"),
                                ));
                            }
                        };
                        Some(DecisionProvenance::AiNeedsAdmin(cause))
                    }
                    Some("admin") => {
                        let actor = PrincipalId::from_uuid(
                            m.admin_actor
                                .ok_or_else(|| integrity("admin actor missing", rev.id))?,
                        );
                        let verdict = match m.admin_verdict.as_deref() {
                            Some("approve") => AdminVerdict::Approve,
                            Some("reject") => AdminVerdict::Reject,
                            other => {
                                return Err(integrity(
                                    "admin verdict unknown",
                                    other.unwrap_or("none"),
                                ));
                            }
                        };
                        let reason =
                            DecisionReason::parse(m.admin_reason.as_deref().unwrap_or_default())
                                .map_err(|e| integrity("admin reason invalid", e))?;
                        Some(DecisionProvenance::Admin {
                            actor,
                            verdict,
                            reason,
                        })
                    }
                    Some("legacy_bootstrap") => Some(DecisionProvenance::LegacyBootstrap),
                    Some(other) => return Err(integrity("unknown provenance kind", other)),
                };

                let dec_version = DecisionVersion::new(m.decision_version)
                    .ok_or_else(|| integrity("decision version invalid", m.decision_version))?;

                (
                    mod_status,
                    Some(RevisionDecisionFacts {
                        revision: rev_id,
                        digest: content.hash().clone(),
                        version: dec_version,
                        provenance,
                    }),
                )
            }
        };

        pub_revisions.push(RevisionFacts {
            snapshot: Snapshot {
                work: work_id,
                revision: rev_id,
                content,
            },
            moderation,
        });

        if let Some(dec) = decision_facts {
            mod_revisions.push(dec);
        }

        metadata_list.push((rev_id, rev.title, rev.metadata_snapshot_id, rev.created_at));
    }

    let mut mod_jobs = Vec::with_capacity(job_rows.len());
    for job in job_rows {
        let job_rev = RevisionId::from_uuid(job.revision_id);
        let digest = ContentHash::parse(&job.content_digest)
            .ok_or_else(|| integrity("job content digest invalid", &job.content_digest))?;
        let pol = PolicyVersion::new(job.policy_version)
            .ok_or_else(|| integrity("job policy version invalid", job.policy_version))?;
        let state = match job.state.as_str() {
            "open" => JobState::Open,
            "applied" => JobState::Applied,
            "failed" => JobState::Failed,
            other => return Err(integrity("unknown job state", other)),
        };

        mod_jobs.push(EvaluationJobFacts {
            id: EvaluationId::from_uuid(job.id),
            target: EvaluationTarget::WorkRevision {
                work: work_id,
                revision: job_rev,
                digest,
            },
            policy: pol,
            state,
        });
    }

    let audience = match pub_row.audience.as_str() {
        "private" => Audience::Private,
        "public" => Audience::Public,
        other => return Err(integrity("unknown audience", other)),
    };

    let lifecycle = match pub_row.lifecycle.as_str() {
        "active" => Lifecycle::Active,
        "author_deleted" => match pub_row.deletion_provenance.as_deref() {
            Some("tombstone") => {
                let rev = pub_row
                    .tombstone_revision_id
                    .map(RevisionId::from_uuid)
                    .ok_or_else(|| integrity("tombstone revision missing", work_id.as_uuid()))?;
                let tombstone_gen = pub_row
                    .tombstone_intent_generation
                    .and_then(IntentGeneration::new)
                    .ok_or_else(|| integrity("tombstone generation missing", work_id.as_uuid()))?;
                Lifecycle::AuthorDeleted(DeletionProvenance::PublicTombstone(PublicPointer {
                    revision: rev,
                    intent_generation: tombstone_gen,
                }))
            }
            Some("hidden") => Lifecycle::AuthorDeleted(DeletionProvenance::Hidden),
            other => {
                return Err(integrity(
                    "unknown deletion provenance",
                    other.unwrap_or("none"),
                ));
            }
        },
        "removed" => Lifecycle::Removed,
        other => return Err(integrity("unknown lifecycle", other)),
    };

    let generation = IntentGeneration::new(pub_row.generation)
        .ok_or_else(|| integrity("generation invalid", pub_row.generation))?;

    let intent = match (pub_row.intent_revision_id, pub_row.intent_generation) {
        (Some(rev), Some(intent_gen)) => Some(PublishIntent {
            revision: RevisionId::from_uuid(rev),
            generation: IntentGeneration::new(intent_gen)
                .ok_or_else(|| integrity("intent generation invalid", intent_gen))?,
        }),
        _ => None,
    };

    let pointer = match (
        pub_row.pointer_revision_id,
        pub_row.pointer_intent_generation,
    ) {
        (Some(rev), Some(pointer_gen)) => Some(PublicPointer {
            revision: RevisionId::from_uuid(rev),
            intent_generation: IntentGeneration::new(pointer_gen)
                .ok_or_else(|| integrity("pointer intent generation invalid", pointer_gen))?,
        }),
        _ => None,
    };

    let publication_facts = PublicationFacts {
        work: work_id,
        owner: actor,
        audience,
        lifecycle,
        generation,
        intent,
        pointer,
        revisions: pub_revisions,
    };

    let publication = Publication::restore(publication_facts)
        .map_err(|e| integrity("publication facts restore failed", format!("{e:?}")))?;

    let mod_policy = PolicyVersion::new(pub_row.policy_version)
        .ok_or_else(|| integrity("moderation policy version invalid", pub_row.policy_version))?;

    let mod_facts = WorkModerationFacts {
        work: work_id,
        policy: mod_policy,
        revisions: mod_revisions,
        jobs: mod_jobs,
    };

    let moderation = WorkModeration::restore(&publication, mod_facts)
        .map_err(|e| integrity("work moderation facts restore failed", format!("{e:?}")))?;

    Ok((publication, moderation, metadata_list))
}

impl AuthorPublicationStore for PgStore {
    async fn load_author_publication_state(
        &self,
        actor: PrincipalId,
        work_id: WorkId,
    ) -> Result<Option<AuthorPublicationStateFacts>, StoreError> {
        let mut tx = self.pool().begin().await.map_err(classify)?;
        let owner: Option<Uuid> = query_scalar("SELECT owner_id FROM works WHERE id = $1")
            .bind(work_id.as_uuid())
            .fetch_optional(&mut *tx)
            .await
            .map_err(classify)?;

        let Some(owner) = owner else {
            tx.commit().await.map_err(classify)?;
            return Ok(None);
        };

        if owner != actor.as_uuid() {
            tx.commit().await.map_err(classify)?;
            return Ok(None);
        }

        let (publication, moderation, revision_metadata) =
            load_paired_state(&mut tx, actor, work_id).await?;
        tx.commit().await.map_err(classify)?;

        Ok(Some(AuthorPublicationStateFacts {
            publication,
            moderation,
            revision_metadata,
        }))
    }

    fn capture_snapshot(
        &self,
        actor: PrincipalId,
        work_id: WorkId,
        command_key: &str,
        expected_draft_version: DraftVersion,
        expected_metadata_version: MetadataVersion,
    ) -> impl Future<Output = Result<CaptureSnapshotOutcome, StoreError>> + Send {
        let command_key = command_key.to_string();
        async move {
            let payload_bytes = format!(
                "capture_snapshot:{}:{}",
                expected_draft_version.get(),
                expected_metadata_version.get()
            );
            let digest = Sha256::digest(payload_bytes.as_bytes());

            let mut tx = self.pool().begin().await.map_err(classify)?;

            // 2. Lock graph: pointer FOR UPDATE -> work_publications FOR UPDATE -> works FOR SHARE -> principals FOR UPDATE
            let _: Option<Uuid> =
                query_scalar("SELECT revision_id FROM publications WHERE work_id = $1 FOR UPDATE")
                    .bind(work_id.as_uuid())
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(classify)?;

            let _: Option<Uuid> =
                query_scalar("SELECT work_id FROM work_publications WHERE work_id = $1 FOR UPDATE")
                    .bind(work_id.as_uuid())
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(classify)?;

            let owner: Option<Uuid> =
                query_scalar("SELECT owner_id FROM works WHERE id = $1 FOR SHARE")
                    .bind(work_id.as_uuid())
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(classify)?;

            let Some(owner) = owner else {
                tx.rollback().await.map_err(classify)?;
                return Ok(CaptureSnapshotOutcome::NotFound);
            };

            if owner != actor.as_uuid() {
                tx.rollback().await.map_err(classify)?;
                return Ok(CaptureSnapshotOutcome::NotFound);
            }

            let standing = locked_standing(&mut tx, actor).await?;
            if standing != Standing::Active {
                return Err(StoreError::PrincipalSuspended);
            }

            // Receipt after the standing fence: the actor's principal row FOR UPDATE serializes
            // every command of this actor, so a concurrent retry with the same key waits for the
            // first and replays its committed answer instead of racing it (as endorsement does).
            #[derive(sqlx::FromRow)]
            struct ReceiptRow {
                payload_digest: Vec<u8>,
                revision_id: Option<Uuid>,
            }

            let stored: Option<ReceiptRow> = query_as(
                "SELECT payload_digest, revision_id
                   FROM work_publication_command_receipts
                  WHERE actor_id = $1 AND work_id = $2 AND operation = 'capture_snapshot' AND command_key = $3",
            )
            .bind(actor.as_uuid())
            .bind(work_id.as_uuid())
            .bind(&command_key)
            .fetch_optional(&mut *tx)
            .await
            .map_err(classify)?;

            if let Some(receipt) = stored {
                if receipt.payload_digest == digest.as_slice() {
                    let rev_id = receipt.revision_id.ok_or_else(|| {
                        integrity("missing revision id in capture receipt", work_id.as_uuid())
                    })?;
                    let (content_hash, meta_id): (String, Option<Uuid>) = query_as(
                        "SELECT content_hash, metadata_snapshot_id FROM revisions WHERE id = $1",
                    )
                    .bind(rev_id)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(classify)?;

                    tx.commit().await.map_err(classify)?;
                    return Ok(CaptureSnapshotOutcome::Saved(AuthorSnapshotCapture {
                        revision_id: RevisionId::from_uuid(rev_id),
                        content_hash: ContentHash::parse(&content_hash)
                            .ok_or_else(|| integrity("content hash invalid", &content_hash))?,
                        metadata_snapshot_id: meta_id,
                        draft_version: expected_draft_version,
                        metadata_version: expected_metadata_version,
                        replayed: true,
                    }));
                } else {
                    tx.rollback().await.map_err(classify)?;
                    return Ok(CaptureSnapshotOutcome::KeyReused);
                }
            }

            // 3. Working draft check
            #[derive(sqlx::FromRow)]
            struct DraftRow {
                version: i64,
                title: String,
                document: Json<DocumentData>,
            }

            let draft: Option<DraftRow> = query_as(
                "SELECT version, title, document FROM working_drafts WHERE work_id = $1 FOR SHARE",
            )
            .bind(work_id.as_uuid())
            .fetch_optional(&mut *tx)
            .await
            .map_err(classify)?;

            let Some(draft) = draft else {
                tx.rollback().await.map_err(classify)?;
                return Ok(CaptureSnapshotOutcome::NotFound);
            };

            if draft.version != expected_draft_version.get() {
                tx.rollback().await.map_err(classify)?;
                return Ok(CaptureSnapshotOutcome::Conflict {
                    current_draft_version: DraftVersion::new(draft.version),
                    current_metadata_version: None,
                });
            }

            // 4. Capture metadata snapshot
            let metadata_capture =
                capture_metadata_snapshot(&mut tx, work_id, expected_metadata_version).await?;
            let metadata_snapshot_id = match metadata_capture {
                SnapshotCapture::Captured { id, .. } => id,
                SnapshotCapture::Conflict { current } => {
                    // Only the metadata moved: the draft matched, so it is not reported as stale.
                    tx.rollback().await.map_err(classify)?;
                    return Ok(CaptureSnapshotOutcome::Conflict {
                        current_draft_version: None,
                        current_metadata_version: Some(current),
                    });
                }
            };

            // 5. Create immutable revision
            let rev_id = Uuid::now_v7();
            let title = Title::parse(&draft.title).map_err(|e| integrity("draft title", e))?;
            let doc = Document::parse(draft.document.0).map_err(|e| integrity("draft doc", e))?;
            let content =
                RevisionContent::new(title, doc).map_err(|e| integrity("revision content", e))?;
            let hash = content.hash().clone();

            query(
                "INSERT INTO revisions (id, work_id, title, document, content_hash, created_by, metadata_snapshot_id)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(rev_id)
            .bind(work_id.as_uuid())
            .bind(content.title().as_str())
            .bind(Json(content.document().to_data()))
            .bind(hash.as_str())
            .bind(actor.as_uuid())
            .bind(metadata_snapshot_id)
            .execute(&mut *tx)
            .await
            .map_err(classify)?;

            // 6. Ensure work_publications exists
            query(
                "INSERT INTO work_publications (work_id, audience, lifecycle, generation, policy_version)
                 VALUES ($1, 'private', 'active', 1, 1)
                 ON CONFLICT (work_id) DO NOTHING",
            )
            .bind(work_id.as_uuid())
            .execute(&mut *tx)
            .await
            .map_err(classify)?;

            // 7. Write receipt
            query(
                "INSERT INTO work_publication_command_receipts (actor_id, work_id, operation, command_key, payload_digest, outcome, revision_id, generation)
                 VALUES ($1, $2, 'capture_snapshot', $3, $4, 'saved', $5, 1)",
            )
            .bind(actor.as_uuid())
            .bind(work_id.as_uuid())
            .bind(&command_key)
            .bind(digest.as_slice())
            .bind(rev_id)
            .execute(&mut *tx)
            .await
            .map_err(classify)?;

            tx.commit().await.map_err(classify)?;

            Ok(CaptureSnapshotOutcome::Saved(AuthorSnapshotCapture {
                revision_id: RevisionId::from_uuid(rev_id),
                content_hash: hash,
                metadata_snapshot_id: Some(metadata_snapshot_id),
                draft_version: expected_draft_version,
                metadata_version: expected_metadata_version,
                replayed: false,
            }))
        }
    }

    fn request_public(
        &self,
        actor: PrincipalId,
        work_id: WorkId,
        command_key: &str,
        revision_id: RevisionId,
        expected_generation: IntentGeneration,
    ) -> impl Future<Output = Result<RequestPublicOutcome, StoreError>> + Send {
        let command_key = command_key.to_string();
        async move {
            let payload_bytes = format!(
                "request_public:{}:{}",
                revision_id.as_uuid(),
                expected_generation.get()
            );
            let digest = Sha256::digest(payload_bytes.as_bytes());

            let mut tx = self.pool().begin().await.map_err(classify)?;

            // 2. Lock graph: publications FOR UPDATE -> work_publications FOR UPDATE -> works FOR SHARE -> principals FOR UPDATE
            let _: Option<Uuid> =
                query_scalar("SELECT revision_id FROM publications WHERE work_id = $1 FOR UPDATE")
                    .bind(work_id.as_uuid())
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(classify)?;

            let _: Option<Uuid> =
                query_scalar("SELECT work_id FROM work_publications WHERE work_id = $1 FOR UPDATE")
                    .bind(work_id.as_uuid())
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(classify)?;

            let owner: Option<Uuid> =
                query_scalar("SELECT owner_id FROM works WHERE id = $1 FOR SHARE")
                    .bind(work_id.as_uuid())
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(classify)?;

            let Some(owner) = owner else {
                tx.rollback().await.map_err(classify)?;
                return Ok(RequestPublicOutcome::NotFound);
            };

            if owner != actor.as_uuid() {
                tx.rollback().await.map_err(classify)?;
                return Ok(RequestPublicOutcome::NotFound);
            }

            let standing = locked_standing(&mut tx, actor).await?;
            if standing != Standing::Active {
                return Err(StoreError::PrincipalSuspended);
            }

            // Receipt after the standing fence: the actor's principal row FOR UPDATE serializes
            // every command of this actor, so a concurrent retry with the same key waits for the
            // first and replays its committed answer instead of racing it (as endorsement does).
            #[derive(sqlx::FromRow)]
            struct ReceiptRow {
                payload_digest: Vec<u8>,
                outcome: String,
                generation: Option<i64>,
            }

            let stored: Option<ReceiptRow> = query_as(
                "SELECT payload_digest, outcome, generation
                   FROM work_publication_command_receipts
                  WHERE actor_id = $1 AND work_id = $2 AND operation = 'request_public' AND command_key = $3",
            )
            .bind(actor.as_uuid())
            .bind(work_id.as_uuid())
            .bind(&command_key)
            .fetch_optional(&mut *tx)
            .await
            .map_err(classify)?;

            if let Some(receipt) = stored {
                if receipt.payload_digest == digest.as_slice() {
                    let outcome = parse_outcome(&receipt.outcome)?;
                    let receipt_gen = receipt_generation(receipt.generation)?;
                    tx.commit().await.map_err(classify)?;
                    return Ok(RequestPublicOutcome::Accepted(AuthorPublicRequestOutcome {
                        outcome,
                        generation: receipt_gen,
                        ticket: None,
                        replayed: true,
                    }));
                } else {
                    tx.rollback().await.map_err(classify)?;
                    return Ok(RequestPublicOutcome::KeyReused);
                }
            }

            // 3. Load paired state
            let (pub_state, mod_state, _) = load_paired_state(&mut tx, actor, work_id).await?;

            if pub_state.facts().generation != expected_generation {
                tx.rollback().await.map_err(classify)?;
                return Ok(RequestPublicOutcome::Stale {
                    current_generation: pub_state.facts().generation,
                });
            }

            if pub_state
                .facts()
                .revisions
                .iter()
                .all(|r| r.snapshot.revision != revision_id)
            {
                tx.rollback().await.map_err(classify)?;
                return Ok(RequestPublicOutcome::UnknownRevision);
            }

            let new_eval_id = EvaluationId::generate();
            let transition = match plan_author_publication(
                Some(BoundPrincipal {
                    principal: actor,
                    standing,
                }),
                &pub_state,
                &mod_state,
                AuthorCommand::RequestPublic {
                    revision: revision_id,
                    expected: expected_generation,
                },
                Some(new_eval_id),
            ) {
                Ok(t) => t,
                Err(ModerationPlanError::Rejected(ModerationError::Publication(
                    PublicationError::Stale,
                ))) => {
                    tx.rollback().await.map_err(classify)?;
                    return Ok(RequestPublicOutcome::Stale {
                        current_generation: pub_state.facts().generation,
                    });
                }
                Err(ModerationPlanError::Rejected(ModerationError::Publication(
                    PublicationError::RevisionRejected,
                ))) => {
                    tx.rollback().await.map_err(classify)?;
                    return Ok(RequestPublicOutcome::RevisionRejected);
                }
                Err(ModerationPlanError::Rejected(ModerationError::Publication(
                    PublicationError::UnknownRevision,
                ))) => {
                    tx.rollback().await.map_err(classify)?;
                    return Ok(RequestPublicOutcome::UnknownRevision);
                }
                Err(ModerationPlanError::Rejected(ModerationError::Publication(
                    PublicationError::WorkUnavailable,
                ))) => {
                    tx.rollback().await.map_err(classify)?;
                    return Ok(RequestPublicOutcome::WorkUnavailable);
                }
                Err(e) => {
                    tx.rollback().await.map_err(classify)?;
                    return Err(integrity(
                        "plan_author_publication failed",
                        format!("{e:?}"),
                    ));
                }
            };

            let next_pub = transition.publication.facts();
            let pointer_rev = next_pub.pointer.map(|p| p.revision.as_uuid());
            let pointer_gen = next_pub.pointer.map(|p| p.intent_generation.get());

            // 4. Update work_publications
            query(
                "INSERT INTO work_publications (
                     work_id, audience, lifecycle, generation,
                     intent_revision_id, intent_generation,
                     pointer_revision_id, pointer_intent_generation,
                     policy_version, updated_at
                 )
                 VALUES ($1, 'public', 'active', $2, $3, $2, $4, $5, $6, now())
                 ON CONFLICT (work_id) DO UPDATE SET
                     audience = 'public',
                     generation = EXCLUDED.generation,
                     intent_revision_id = EXCLUDED.intent_revision_id,
                     intent_generation = EXCLUDED.intent_generation,
                     pointer_revision_id = EXCLUDED.pointer_revision_id,
                     pointer_intent_generation = EXCLUDED.pointer_intent_generation,
                     updated_at = now()",
            )
            .bind(work_id.as_uuid())
            .bind(next_pub.generation.get())
            .bind(revision_id.as_uuid())
            .bind(pointer_rev)
            .bind(pointer_gen)
            .bind(mod_state.facts().policy.get())
            .execute(&mut *tx)
            .await
            .map_err(classify)?;

            // 5. Pointer effect on publications table
            match transition.effects.pointer {
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

            // 6. If ticket produced, write work_revision_moderation and work_evaluation_jobs
            if let Some(ticket) = &transition.ticket {
                // The domain opens a ticket only for an unsubmitted revision, which has no row:
                // a conflict here is a desync to refuse, never a decision to reset.
                query(
                    "INSERT INTO work_revision_moderation (work_id, revision_id, decision_version, moderation, created_at, updated_at)
                     VALUES ($1, $2, 1, 'pending', now(), now())",
                )
                .bind(work_id.as_uuid())
                .bind(revision_id.as_uuid())
                .execute(&mut *tx)
                .await
                .map_err(classify)?;

                let EvaluationTarget::WorkRevision { digest, .. } = &ticket.target else {
                    return Err(integrity(
                        "ticket target must be work revision",
                        work_id.as_uuid(),
                    ));
                };

                query(
                    "INSERT INTO work_evaluation_jobs (id, work_id, revision_id, content_digest, policy_version, state, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, 'open', now(), now())",
                )
                .bind(ticket.id.as_uuid())
                .bind(work_id.as_uuid())
                .bind(revision_id.as_uuid())
                .bind(digest.as_str())
                .bind(ticket.policy.get())
                .execute(&mut *tx)
                .await
                .map_err(classify)?;
            }

            // 7. Write receipt
            query(
                "INSERT INTO work_publication_command_receipts (actor_id, work_id, operation, command_key, payload_digest, outcome, revision_id, generation)
                 VALUES ($1, $2, 'request_public', $3, $4, $5, $6, $7)",
            )
            .bind(actor.as_uuid())
            .bind(work_id.as_uuid())
            .bind(&command_key)
            .bind(digest.as_slice())
            .bind(transition.outcome.as_str())
            .bind(revision_id.as_uuid())
            .bind(next_pub.generation.get())
            .execute(&mut *tx)
            .await
            .map_err(classify)?;

            tx.commit().await.map_err(classify)?;

            Ok(RequestPublicOutcome::Accepted(AuthorPublicRequestOutcome {
                outcome: transition.outcome,
                generation: next_pub.generation,
                ticket: transition.ticket,
                replayed: false,
            }))
        }
    }

    fn set_private(
        &self,
        actor: PrincipalId,
        work_id: WorkId,
        command_key: &str,
        expected_generation: IntentGeneration,
    ) -> impl Future<Output = Result<SetPrivateOutcome, StoreError>> + Send {
        let command_key = command_key.to_string();
        async move {
            let payload_bytes = format!("set_private:{}", expected_generation.get());
            let digest = Sha256::digest(payload_bytes.as_bytes());

            let mut tx = self.pool().begin().await.map_err(classify)?;

            // 2. Lock graph: publications FOR UPDATE -> work_publications FOR UPDATE -> works FOR SHARE -> principals FOR UPDATE
            let _: Option<Uuid> =
                query_scalar("SELECT revision_id FROM publications WHERE work_id = $1 FOR UPDATE")
                    .bind(work_id.as_uuid())
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(classify)?;

            let _: Option<Uuid> =
                query_scalar("SELECT work_id FROM work_publications WHERE work_id = $1 FOR UPDATE")
                    .bind(work_id.as_uuid())
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(classify)?;

            let owner: Option<Uuid> =
                query_scalar("SELECT owner_id FROM works WHERE id = $1 FOR SHARE")
                    .bind(work_id.as_uuid())
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(classify)?;

            let Some(owner) = owner else {
                tx.rollback().await.map_err(classify)?;
                return Ok(SetPrivateOutcome::NotFound);
            };

            if owner != actor.as_uuid() {
                tx.rollback().await.map_err(classify)?;
                return Ok(SetPrivateOutcome::NotFound);
            }

            let standing = locked_standing(&mut tx, actor).await?;
            if standing != Standing::Active {
                return Err(StoreError::PrincipalSuspended);
            }

            // Receipt after the standing fence: the actor's principal row FOR UPDATE serializes
            // every command of this actor, so a concurrent retry with the same key waits for the
            // first and replays its committed answer instead of racing it (as endorsement does).
            #[derive(sqlx::FromRow)]
            struct ReceiptRow {
                payload_digest: Vec<u8>,
                outcome: String,
                generation: Option<i64>,
            }

            let stored: Option<ReceiptRow> = query_as(
                "SELECT payload_digest, outcome, generation
                   FROM work_publication_command_receipts
                  WHERE actor_id = $1 AND work_id = $2 AND operation = 'set_private' AND command_key = $3",
            )
            .bind(actor.as_uuid())
            .bind(work_id.as_uuid())
            .bind(&command_key)
            .fetch_optional(&mut *tx)
            .await
            .map_err(classify)?;

            if let Some(receipt) = stored {
                if receipt.payload_digest == digest.as_slice() {
                    let outcome = parse_outcome(&receipt.outcome)?;
                    let receipt_gen = receipt_generation(receipt.generation)?;
                    tx.commit().await.map_err(classify)?;
                    return Ok(SetPrivateOutcome::Accepted(AuthorPrivateOutcome {
                        outcome,
                        generation: receipt_gen,
                        replayed: true,
                    }));
                } else {
                    tx.rollback().await.map_err(classify)?;
                    return Ok(SetPrivateOutcome::KeyReused);
                }
            }

            // 3. Load paired state
            let (pub_state, mod_state, _) = load_paired_state(&mut tx, actor, work_id).await?;

            if pub_state.facts().generation != expected_generation {
                tx.rollback().await.map_err(classify)?;
                return Ok(SetPrivateOutcome::Stale {
                    current_generation: pub_state.facts().generation,
                });
            }

            let transition = match plan_author_publication(
                Some(BoundPrincipal {
                    principal: actor,
                    standing,
                }),
                &pub_state,
                &mod_state,
                AuthorCommand::SetPrivate {
                    expected: expected_generation,
                },
                None,
            ) {
                Ok(t) => t,
                Err(ModerationPlanError::Rejected(ModerationError::Publication(
                    PublicationError::Stale,
                ))) => {
                    tx.rollback().await.map_err(classify)?;
                    return Ok(SetPrivateOutcome::Stale {
                        current_generation: pub_state.facts().generation,
                    });
                }
                Err(ModerationPlanError::Rejected(ModerationError::Publication(
                    PublicationError::WorkUnavailable,
                ))) => {
                    tx.rollback().await.map_err(classify)?;
                    return Ok(SetPrivateOutcome::WorkUnavailable);
                }
                Err(e) => {
                    tx.rollback().await.map_err(classify)?;
                    return Err(integrity(
                        "plan_author_publication failed",
                        format!("{e:?}"),
                    ));
                }
            };

            let next_pub = transition.publication.facts();

            // 4. Update work_publications to private
            query(
                "INSERT INTO work_publications (
                     work_id, audience, lifecycle, generation,
                     intent_revision_id, intent_generation,
                     pointer_revision_id, pointer_intent_generation,
                     policy_version, updated_at
                 )
                 VALUES ($1, 'private', 'active', $2, NULL, NULL, NULL, NULL, $3, now())
                 ON CONFLICT (work_id) DO UPDATE SET
                     audience = 'private',
                     generation = EXCLUDED.generation,
                     intent_revision_id = NULL,
                     intent_generation = NULL,
                     pointer_revision_id = NULL,
                     pointer_intent_generation = NULL,
                     updated_at = now()",
            )
            .bind(work_id.as_uuid())
            .bind(next_pub.generation.get())
            .bind(mod_state.facts().policy.get())
            .execute(&mut *tx)
            .await
            .map_err(classify)?;

            // 5. Delete pointer from publications
            query("DELETE FROM publications WHERE work_id = $1")
                .bind(work_id.as_uuid())
                .execute(&mut *tx)
                .await
                .map_err(classify)?;

            // 6. Write receipt
            query(
                "INSERT INTO work_publication_command_receipts (actor_id, work_id, operation, command_key, payload_digest, outcome, generation)
                 VALUES ($1, $2, 'set_private', $3, $4, $5, $6)",
            )
            .bind(actor.as_uuid())
            .bind(work_id.as_uuid())
            .bind(&command_key)
            .bind(digest.as_slice())
            .bind(transition.outcome.as_str())
            .bind(next_pub.generation.get())
            .execute(&mut *tx)
            .await
            .map_err(classify)?;

            tx.commit().await.map_err(classify)?;

            Ok(SetPrivateOutcome::Accepted(AuthorPrivateOutcome {
                outcome: transition.outcome,
                generation: next_pub.generation,
                replayed: false,
            }))
        }
    }
}
