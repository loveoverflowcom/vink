-- Work moderation runtime persistence, admin seats, grants, cases and automation (#159).

-- 1. Expand work_publications with hidden pointer, review_withdrawn pointer, ever_public and public_epoch
ALTER TABLE work_publications
    ADD COLUMN hidden_revision_id uuid,
    ADD COLUMN hidden_intent_generation bigint,
    ADD COLUMN review_withdrawn_revision_id uuid,
    ADD COLUMN review_withdrawn_intent_generation bigint,
    ADD COLUMN ever_public uuid[] NOT NULL DEFAULT '{}',
    ADD COLUMN public_epoch bigint NOT NULL DEFAULT 0,
    ADD CONSTRAINT work_publications_hidden_fkey
        FOREIGN KEY (work_id, hidden_revision_id) REFERENCES revisions (work_id, id),
    ADD CONSTRAINT work_publications_review_withdrawn_fkey
        FOREIGN KEY (work_id, review_withdrawn_revision_id) REFERENCES revisions (work_id, id);

-- Backfill ever_public and public_epoch from existing pointer
UPDATE work_publications
   SET ever_public = ARRAY[pointer_revision_id],
       public_epoch = 1
 WHERE pointer_revision_id IS NOT NULL;

-- 2. Expand work_evaluation_jobs with execution columns and new states
ALTER TABLE work_evaluation_jobs
    DROP CONSTRAINT work_evaluation_jobs_state_check;

ALTER TABLE work_evaluation_jobs
    ADD CONSTRAINT work_evaluation_jobs_state_check
        CHECK (state IN ('open', 'leased', 'applied', 'failed', 'withdrawn', 'rerouted', 'quarantined')),
    ADD COLUMN claim_generation bigint NOT NULL DEFAULT 0,
    ADD COLUMN claimed_by text,
    ADD COLUMN lease_expires_at timestamptz,
    ADD COLUMN available_at timestamptz NOT NULL DEFAULT now(),
    ADD COLUMN attempts int NOT NULL DEFAULT 0,
    ADD COLUMN max_attempts int NOT NULL DEFAULT 3,
    ADD COLUMN scenario text,
    ADD COLUMN last_error text;

CREATE INDEX work_evaluation_jobs_claim_idx
    ON work_evaluation_jobs (available_at, state)
    WHERE state IN ('open', 'leased');

-- 3. Admin seats and audit (CLI provisioned)
CREATE TABLE moderation_admin_seats (
    principal_id uuid PRIMARY KEY REFERENCES principals (id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE moderation_admin_audit (
    id uuid PRIMARY KEY,
    operator text NOT NULL,
    action text NOT NULL CHECK (action IN ('add', 'remove')),
    principal_id uuid NOT NULL REFERENCES principals (id),
    reason text,
    created_at timestamptz NOT NULL DEFAULT now()
);

-- 4. Moderation groups and members
CREATE TABLE moderation_groups (
    id uuid PRIMARY KEY,
    name text NOT NULL UNIQUE CHECK (char_length(name) BETWEEN 1 AND 80),
    created_by uuid NOT NULL REFERENCES principals (id),
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE moderation_group_members (
    group_id uuid NOT NULL REFERENCES moderation_groups (id) ON DELETE CASCADE,
    principal_id uuid NOT NULL REFERENCES principals (id) ON DELETE CASCADE,
    added_by uuid NOT NULL REFERENCES principals (id),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (group_id, principal_id)
);

-- 5. Admin-issued moderation grants
CREATE TABLE moderation_grants (
    id uuid PRIMARY KEY,
    grantee_kind text NOT NULL CHECK (grantee_kind IN ('principal', 'group')),
    grantee_id uuid NOT NULL,
    capability text NOT NULL CHECK (capability IN ('works', 'comments')),
    scope_kind text NOT NULL CHECK (scope_kind IN ('global', 'work')),
    scope_work_id uuid REFERENCES works (id) ON DELETE CASCADE,
    expires_at timestamptz,
    revoked boolean NOT NULL DEFAULT false,
    revoked_at timestamptz,
    revoked_by uuid REFERENCES principals (id),
    origin text NOT NULL CHECK (origin IN ('admin', 'inherited_comment')),
    created_by uuid REFERENCES principals (id),
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT moderation_grants_scope_check CHECK (scope_kind <> 'work' OR scope_work_id IS NOT NULL)
);

-- 6. Expand work_revision_moderation to reference moderation_grants and support moderator / policy_rereview provenances
ALTER TABLE work_revision_moderation
    ADD COLUMN policy_version bigint NOT NULL DEFAULT 1 CHECK (policy_version >= 1),
    ADD COLUMN moderator_grant_id uuid REFERENCES moderation_grants (id);

ALTER TABLE work_revision_moderation
    DROP CONSTRAINT work_revision_moderation_provenance_kind_check,
    DROP CONSTRAINT work_revision_moderation_approved_check,
    DROP CONSTRAINT work_revision_moderation_rejected_check;

ALTER TABLE work_revision_moderation
    ADD CONSTRAINT work_revision_moderation_provenance_kind_check
        CHECK (provenance_kind IN ('ai_approved', 'ai_rejected', 'ai_needs_admin', 'admin', 'moderator', 'policy_rereview', 'legacy_bootstrap')),
    ADD CONSTRAINT work_revision_moderation_approved_check CHECK (
        moderation <> 'approved' OR (
            decision_version > 1
            AND provenance_kind IN ('ai_approved', 'admin', 'moderator', 'legacy_bootstrap')
            AND (
                (provenance_kind = 'ai_approved' AND admin_actor IS NULL AND admin_verdict IS NULL AND admin_reason IS NULL AND policy_reason_code IS NULL AND needs_admin_cause IS NULL AND moderator_grant_id IS NULL)
                OR
                (provenance_kind = 'admin' AND admin_verdict = 'approve' AND admin_actor IS NOT NULL AND admin_reason IS NOT NULL AND policy_reason_code IS NULL AND needs_admin_cause IS NULL AND moderator_grant_id IS NULL)
                OR
                (provenance_kind = 'moderator' AND admin_verdict = 'approve' AND admin_actor IS NOT NULL AND admin_reason IS NOT NULL AND moderator_grant_id IS NOT NULL AND policy_reason_code IS NULL AND needs_admin_cause IS NULL)
                OR
                (provenance_kind = 'legacy_bootstrap' AND admin_actor IS NULL AND admin_verdict IS NULL AND admin_reason IS NULL AND policy_reason_code IS NULL AND needs_admin_cause IS NULL AND moderator_grant_id IS NULL)
            )
        )
    ),
    ADD CONSTRAINT work_revision_moderation_rejected_check CHECK (
        moderation <> 'rejected' OR (
            decision_version > 1
            AND provenance_kind IN ('ai_rejected', 'admin', 'moderator')
            AND (
                (provenance_kind = 'ai_rejected' AND policy_reason_code IS NOT NULL AND admin_actor IS NULL AND admin_verdict IS NULL AND admin_reason IS NULL AND needs_admin_cause IS NULL AND moderator_grant_id IS NULL)
                OR
                (provenance_kind = 'admin' AND admin_verdict = 'reject' AND admin_actor IS NOT NULL AND admin_reason IS NOT NULL AND policy_reason_code IS NULL AND needs_admin_cause IS NULL AND moderator_grant_id IS NULL)
                OR
                (provenance_kind = 'moderator' AND admin_verdict = 'reject' AND admin_actor IS NOT NULL AND admin_reason IS NOT NULL AND moderator_grant_id IS NOT NULL AND policy_reason_code IS NULL AND needs_admin_cause IS NULL)
            )
        )
    );

-- 7. Review cases and notes (MOD-12)
CREATE TABLE moderation_cases (
    id uuid PRIMARY KEY,
    subject_kind text NOT NULL CHECK (subject_kind IN ('work_revision', 'comment')),
    work_id uuid NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    revision_id uuid,
    content_digest text,
    comment_id uuid,
    decision_version bigint NOT NULL CHECK (decision_version >= 1),
    target_content_owner uuid NOT NULL REFERENCES principals (id),
    target_work_owner uuid NOT NULL REFERENCES principals (id),
    origin text NOT NULL CHECK (origin IN ('evaluator_uncertain', 'invalid_output', 'retries_exhausted', 'automation_paused', 'policy_rereview', 'reviewer_intervention')),
    version bigint NOT NULL CHECK (version >= 1),
    state text NOT NULL CHECK (state IN ('queued', 'claimed', 'escalated', 'concluded', 'closed')),
    claimed_by uuid REFERENCES principals (id),
    claimed_basis_kind text CHECK (claimed_basis_kind IN ('admin', 'grant')),
    claimed_basis_grant_id uuid REFERENCES moderation_grants (id),
    concluded_by uuid REFERENCES principals (id),
    concluded_basis_kind text CHECK (concluded_basis_kind IN ('admin', 'grant')),
    concluded_basis_grant_id uuid REFERENCES moderation_grants (id),
    concluded_verdict text CHECK (concluded_verdict IN ('approve', 'reject')),
    close_reason text CHECK (close_reason IN ('withdrawn', 'target_gone', 'superseded')),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

-- At most one open case per work revision
CREATE UNIQUE INDEX moderation_cases_work_revision_open_unique
    ON moderation_cases (work_id, revision_id)
    WHERE (state IN ('queued', 'claimed', 'escalated') AND subject_kind = 'work_revision');

CREATE TABLE moderation_case_notes (
    id uuid PRIMARY KEY,
    case_id uuid NOT NULL REFERENCES moderation_cases (id) ON DELETE CASCADE,
    author_id uuid NOT NULL REFERENCES principals (id),
    note text NOT NULL CHECK (char_length(note) BETWEEN 1 AND 2000),
    created_at timestamptz NOT NULL DEFAULT now()
);

-- 8. Automation mode state and audit (MOD-14)
CREATE TABLE moderation_automation_state (
    id int PRIMARY KEY CHECK (id = 1),
    mode text NOT NULL CHECK (mode IN ('enabled', 'paused')),
    version bigint NOT NULL CHECK (version >= 1),
    updated_at timestamptz NOT NULL DEFAULT now()
);

INSERT INTO moderation_automation_state (id, mode, version, updated_at)
VALUES (1, 'enabled', 1, now())
ON CONFLICT (id) DO NOTHING;

CREATE TABLE moderation_automation_audit (
    id uuid PRIMARY KEY,
    admin_id uuid NOT NULL REFERENCES principals (id),
    mode text NOT NULL CHECK (mode IN ('enabled', 'paused')),
    version bigint NOT NULL CHECK (version >= 1),
    reason text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
