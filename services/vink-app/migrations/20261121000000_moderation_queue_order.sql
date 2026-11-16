-- #205 measured global queue: avoid scanning/sorting every open work case for its first page.
-- Sparse work grants already use moderation_cases_work_revision_open_unique.
CREATE INDEX moderation_cases_work_queue_order
    ON moderation_cases (created_at, id)
    WHERE state IN ('queued', 'claimed', 'escalated') AND subject_kind = 'work_revision';
