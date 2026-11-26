use super::*;

/// Account called this after locking the work and recording the verified principal fence.
/// Publication still chooses the old URL's access from the current paired facts.
pub(in crate::persistence) async fn retire_owned_work(
    conn: &mut PgConnection,
    owner: PrincipalId,
    work: WorkId,
) -> Result<(), StoreError> {
    let (publication, moderation, _) = load_paired_state(conn, owner, work).await?;
    let current = publication.facts();
    let terminal = if current.lifecycle.is_active() {
        if current.review_withdrawn.is_some() {
            // Preserve the stronger moderation disposition. Account deletion cannot make a
            // withdrawn revision or its thread discoverable under an author tombstone.
            publication.admin_remove().map(|(next, _)| next)
        } else {
            publication
                .decide_author(owner, AuthorCommand::Delete { expected: None })
                .map(|transition| transition.next)
        }
        .map_err(|error| integrity("account deletion publication", error))?
    } else {
        publication
    };

    let next = terminal.facts();
    let (lifecycle, provenance, pointer) = match next.lifecycle {
        Lifecycle::AuthorDeleted(DeletionProvenance::PublicTombstone(pointer)) => {
            ("author_deleted", Some("tombstone"), Some(pointer))
        }
        Lifecycle::AuthorDeleted(DeletionProvenance::Hidden) => {
            ("author_deleted", Some("hidden"), None)
        }
        Lifecycle::Removed => ("removed", None, None),
        Lifecycle::Active => return Err(integrity("account deletion left active work", work)),
    };

    query(
        "INSERT INTO work_publications
             (work_id, audience, lifecycle, deletion_provenance, tombstone_revision_id,
              tombstone_intent_generation, generation, policy_version)
         VALUES ($1, 'private', $2, $3, $4, $5, $6, $7)
         ON CONFLICT (work_id) DO UPDATE SET audience = 'private', lifecycle = EXCLUDED.lifecycle,
             deletion_provenance = EXCLUDED.deletion_provenance,
             tombstone_revision_id = EXCLUDED.tombstone_revision_id,
             tombstone_intent_generation = EXCLUDED.tombstone_intent_generation,
             generation = EXCLUDED.generation, intent_revision_id = NULL, intent_generation = NULL,
             pointer_revision_id = NULL, pointer_intent_generation = NULL,
             hidden_revision_id = NULL, hidden_intent_generation = NULL,
             review_withdrawn_revision_id = NULL, review_withdrawn_intent_generation = NULL,
             updated_at = clock_timestamp()",
    )
    .bind(work.as_uuid())
    .bind(lifecycle)
    .bind(provenance)
    .bind(pointer.map(|p| p.revision.as_uuid()))
    .bind(pointer.map(|p| p.intent_generation.get()))
    .bind(next.generation.get())
    .bind(moderation.facts().policy.get())
    .execute(&mut *conn)
    .await
    .map_err(classify)?;

    // The WorkId survives for links/Saved Works. Its FK cascades do not run.
    super::super::work_view::erase_work(conn, work).await?;
    super::super::moderation_write::apply_pointer_effect(
        conn,
        work,
        crate::domain::publication::PointerEffect::Clear,
    )
    .await?;
    close_withdrawn_moderation(conn, work).await?;
    Ok(())
}

/// The replaced work is terminal and inaccessible. Destructive payload updates cannot roll
/// back its already committed visibility fence.
pub(in crate::persistence) async fn erase_owned_work_content(
    conn: &mut PgConnection,
    work: WorkId,
) -> Result<(), StoreError> {
    for statement in [
        "DELETE FROM working_drafts WHERE work_id = $1",
        "DELETE FROM work_draft_tags WHERE work_id = $1",
        "DELETE FROM work_draft_metadata WHERE work_id = $1",
        "DELETE FROM work_metadata_snapshot_tags WHERE snapshot_id IN
           (SELECT id FROM work_metadata_snapshots WHERE work_id = $1)",
        "UPDATE revisions SET title = NULL, document = NULL WHERE work_id = $1 AND title IS NOT NULL",
    ] {
        query(statement)
            .bind(work.as_uuid())
            .execute(&mut *conn)
            .await
            .map_err(classify)?;
    }
    Ok(())
}
