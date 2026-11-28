-- Private provider output belongs to one committed evaluation and decision version.
-- Historical rows retain NULL. The current projection reads this same history row.
ALTER TABLE work_revision_decision_history ADD COLUMN evaluator_message text
    CHECK (evaluator_message IS NULL OR (
        evaluation_id IS NOT NULL AND provenance_kind IS NOT NULL
        AND ((provenance_kind = 'ai_approved' AND moderation = 'approved')
            OR (provenance_kind = 'ai_rejected' AND moderation = 'rejected'))
        AND char_length(evaluator_message) BETWEEN 1 AND 600
        AND btrim(evaluator_message) <> '' AND evaluator_message !~ '[<>[:cntrl:]]'
    ));
