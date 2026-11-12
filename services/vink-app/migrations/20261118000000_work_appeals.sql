-- MOD-10 / ADM-04: one durable appeal per adverse work decision. History is never rewritten.
CREATE TABLE work_appeals (
    id uuid PRIMARY KEY,
    work_id uuid NOT NULL,
    revision_id uuid NOT NULL,
    decision_version bigint NOT NULL CHECK (decision_version >= 1),
    content_digest text NOT NULL CHECK (content_digest ~ '^[a-f0-9]{64}$'),
    appellant_id uuid NOT NULL REFERENCES principals(id),
    decided_by text NOT NULL CHECK (decided_by IN ('evaluator', 'moderator')),
    moderator_id uuid REFERENCES principals(id),
    state text NOT NULL DEFAULT 'open' CHECK (state IN ('open', 'upheld', 'reversed', 'closed')),
    close_reason text CHECK (close_reason IN ('withdrawn', 'target_gone', 'superseded')),
    concluded_by uuid REFERENCES principals(id),
    resolution_reason text,
    resulting_decision_version bigint,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (work_id, revision_id) REFERENCES work_revision_moderation(work_id, revision_id),
    UNIQUE (work_id, revision_id, decision_version),
    CHECK ((decided_by = 'moderator') = (moderator_id IS NOT NULL)),
    CHECK ((state = 'closed') = (close_reason IS NOT NULL)),
    CHECK ((state IN ('upheld', 'reversed')) =
        (concluded_by IS NOT NULL AND resolution_reason IS NOT NULL AND resulting_decision_version IS NOT NULL)),
    CHECK (resolution_reason IS NULL OR char_length(resolution_reason) BETWEEN 1 AND 2000),
    CHECK (resulting_decision_version IS NULL OR resulting_decision_version > decision_version)
);
CREATE INDEX work_appeals_open ON work_appeals(created_at, id) WHERE state = 'open';
ALTER TABLE work_revision_decision_history ADD COLUMN appeal_id uuid REFERENCES work_appeals(id);
