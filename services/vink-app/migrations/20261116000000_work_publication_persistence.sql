-- Work publication persistence, paired moderation facts and evaluation jobs (PUB-RUNTIME-01, #158).
-- The pure cores of #150 and #151 decide state transitions; this schema enforces integrity and
-- consistent paired facts on disk.
--
-- Tables:
--   - work_publications: Author audience intent, generation fence, public pointer and lifecycle.
--   - work_revision_moderation: Per-revision moderation facts, decision versions and provenances.
--   - work_evaluation_jobs: Durable evaluation jobs (EvaluationTicket).
--   - work_publication_command_receipts: Author idempotency receipts scoped to (actor, work, operation, key).

-- 1. Frozen metadata binding on revisions (AC-02)
ALTER TABLE revisions
    ADD COLUMN metadata_snapshot_id uuid,
    ADD CONSTRAINT revisions_metadata_snapshot_fkey
        FOREIGN KEY (work_id, metadata_snapshot_id)
        REFERENCES work_metadata_snapshots (work_id, id);

COMMENT ON COLUMN revisions.metadata_snapshot_id IS
    'Immutable metadata snapshot bound at snapshot capture time; NULL for legacy unmigrated revisions.';

-- 2. Paired work publications table (CHOICE-01 A)
CREATE TABLE work_publications (
    work_id uuid PRIMARY KEY REFERENCES works (id) ON DELETE CASCADE,
    audience text NOT NULL CHECK (audience IN ('private', 'public')),
    lifecycle text NOT NULL CHECK (lifecycle IN ('active', 'author_deleted', 'removed')),
    deletion_provenance text CHECK (deletion_provenance IN ('tombstone', 'hidden')),
    tombstone_revision_id uuid,
    tombstone_intent_generation bigint,
    generation bigint NOT NULL CHECK (generation >= 1),
    intent_revision_id uuid,
    intent_generation bigint,
    pointer_revision_id uuid,
    pointer_intent_generation bigint,
    policy_version bigint NOT NULL DEFAULT 1 CHECK (policy_version >= 1),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT work_publications_tombstone_fkey
        FOREIGN KEY (work_id, tombstone_revision_id) REFERENCES revisions (work_id, id),
    CONSTRAINT work_publications_intent_fkey
        FOREIGN KEY (work_id, intent_revision_id) REFERENCES revisions (work_id, id),
    CONSTRAINT work_publications_pointer_fkey
        FOREIGN KEY (work_id, pointer_revision_id) REFERENCES revisions (work_id, id),
    CONSTRAINT work_publications_active_check CHECK (
        lifecycle <> 'active' OR (
            deletion_provenance IS NULL
            AND tombstone_revision_id IS NULL
            AND tombstone_intent_generation IS NULL
        )
    ),
    CONSTRAINT work_publications_author_deleted_check CHECK (
        lifecycle <> 'author_deleted' OR (
            audience = 'private'
            AND intent_revision_id IS NULL
            AND intent_generation IS NULL
            AND pointer_revision_id IS NULL
            AND pointer_intent_generation IS NULL
            AND deletion_provenance IS NOT NULL
            AND (
                (deletion_provenance = 'tombstone'
                 AND tombstone_revision_id IS NOT NULL
                 AND tombstone_intent_generation IS NOT NULL
                 AND tombstone_intent_generation > 1
                 AND tombstone_intent_generation < generation)
                OR
                (deletion_provenance = 'hidden'
                 AND tombstone_revision_id IS NULL
                 AND tombstone_intent_generation IS NULL)
            )
        )
    ),
    CONSTRAINT work_publications_removed_check CHECK (
        lifecycle <> 'removed' OR (
            intent_revision_id IS NULL
            AND intent_generation IS NULL
            AND pointer_revision_id IS NULL
            AND pointer_intent_generation IS NULL
            AND deletion_provenance IS NULL
            AND tombstone_revision_id IS NULL
            AND tombstone_intent_generation IS NULL
        )
    ),
    CONSTRAINT work_publications_private_check CHECK (
        audience <> 'private' OR (
            intent_revision_id IS NULL
            AND intent_generation IS NULL
        )
    ),
    CONSTRAINT work_publications_public_check CHECK (
        audience <> 'public' OR lifecycle <> 'active' OR (
            intent_revision_id IS NOT NULL
            AND intent_generation IS NOT NULL
            AND intent_generation = generation
            AND intent_generation > 1
        )
    ),
    CONSTRAINT work_publications_pointer_check CHECK (
        (pointer_revision_id IS NULL AND pointer_intent_generation IS NULL)
        OR (
            pointer_revision_id IS NOT NULL
            AND pointer_intent_generation IS NOT NULL
            AND pointer_intent_generation > 1
            AND pointer_intent_generation <= generation
            AND audience = 'public'
            AND lifecycle = 'active'
            AND (pointer_intent_generation <> generation OR intent_revision_id = pointer_revision_id)
        )
    )
);

COMMENT ON TABLE work_publications IS
    'Author audience intent, generation fence and public pointer state paired with publications projection.';

-- 3. Work revision moderation facts
CREATE TABLE work_revision_moderation (
    work_id uuid NOT NULL,
    revision_id uuid NOT NULL,
    decision_version bigint NOT NULL CHECK (decision_version >= 1),
    moderation text NOT NULL CHECK (moderation IN ('pending', 'approved', 'rejected', 'needs_admin')),
    provenance_kind text CHECK (provenance_kind IN ('ai_approved', 'ai_rejected', 'ai_needs_admin', 'admin', 'legacy_bootstrap')),
    policy_reason_code text CHECK (policy_reason_code IN ('safety_policy', 'harassment', 'hate_speech', 'sexual_content', 'illegal_content', 'spam_or_abuse')),
    needs_admin_cause text CHECK (needs_admin_cause IN ('uncertain', 'invalid_output')),
    admin_actor uuid REFERENCES principals (id),
    admin_verdict text CHECK (admin_verdict IN ('approve', 'reject')),
    admin_reason text CHECK (admin_reason IS NULL OR (char_length(admin_reason) BETWEEN 1 AND 240 AND admin_reason !~ '[[:cntrl:]]')),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT work_revision_moderation_pkey PRIMARY KEY (work_id, revision_id),
    CONSTRAINT work_revision_moderation_revision_fkey
        FOREIGN KEY (work_id, revision_id) REFERENCES revisions (work_id, id) ON DELETE CASCADE,
    CONSTRAINT work_revision_moderation_pending_check CHECK (
        moderation <> 'pending' OR (
            decision_version = 1
            AND provenance_kind IS NULL
            AND policy_reason_code IS NULL
            AND needs_admin_cause IS NULL
            AND admin_actor IS NULL
            AND admin_verdict IS NULL
            AND admin_reason IS NULL
        )
    ),
    CONSTRAINT work_revision_moderation_approved_check CHECK (
        moderation <> 'approved' OR (
            decision_version > 1
            AND provenance_kind IN ('ai_approved', 'admin', 'legacy_bootstrap')
            AND (
                (provenance_kind = 'ai_approved' AND admin_actor IS NULL AND admin_verdict IS NULL AND admin_reason IS NULL AND policy_reason_code IS NULL AND needs_admin_cause IS NULL)
                OR
                (provenance_kind = 'admin' AND admin_verdict = 'approve' AND admin_actor IS NOT NULL AND admin_reason IS NOT NULL AND policy_reason_code IS NULL AND needs_admin_cause IS NULL)
                OR
                (provenance_kind = 'legacy_bootstrap' AND admin_actor IS NULL AND admin_verdict IS NULL AND admin_reason IS NULL AND policy_reason_code IS NULL AND needs_admin_cause IS NULL)
            )
        )
    ),
    CONSTRAINT work_revision_moderation_rejected_check CHECK (
        moderation <> 'rejected' OR (
            decision_version > 1
            AND provenance_kind IN ('ai_rejected', 'admin')
            AND (
                (provenance_kind = 'ai_rejected' AND policy_reason_code IS NOT NULL AND admin_actor IS NULL AND admin_verdict IS NULL AND admin_reason IS NULL AND needs_admin_cause IS NULL)
                OR
                (provenance_kind = 'admin' AND admin_verdict = 'reject' AND admin_actor IS NOT NULL AND admin_reason IS NOT NULL AND policy_reason_code IS NULL AND needs_admin_cause IS NULL)
            )
        )
    ),
    CONSTRAINT work_revision_moderation_needs_admin_check CHECK (
        moderation <> 'needs_admin' OR (
            decision_version > 1
            AND provenance_kind = 'ai_needs_admin'
            AND needs_admin_cause IS NOT NULL
            AND policy_reason_code IS NULL
            AND admin_actor IS NULL
            AND admin_verdict IS NULL
            AND admin_reason IS NULL
        )
    )
);

COMMENT ON TABLE work_revision_moderation IS
    'Per-revision decision versions and verified provenances for moderation.';

-- 4. Durable evaluation jobs / tickets (CHOICE-02 A)
CREATE TABLE work_evaluation_jobs (
    id uuid PRIMARY KEY,
    work_id uuid NOT NULL,
    revision_id uuid NOT NULL,
    content_digest text NOT NULL CHECK (content_digest ~ '^[0-9a-f]{64}$'),
    policy_version bigint NOT NULL CHECK (policy_version >= 1),
    state text NOT NULL CHECK (state IN ('open', 'applied', 'failed')),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT work_evaluation_jobs_revision_fkey
        FOREIGN KEY (work_id, revision_id) REFERENCES revisions (work_id, id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX work_evaluation_jobs_open_unique
    ON work_evaluation_jobs (work_id, revision_id, policy_version)
    WHERE (state = 'open');

COMMENT ON TABLE work_evaluation_jobs IS
    'Durable evaluation ticket requests produced by submit commands; MOD runtime attaches workers here.';

-- 5. Command receipts for idempotency
CREATE TABLE work_publication_command_receipts (
    actor_id uuid NOT NULL CONSTRAINT work_publication_command_receipts_actor_id_fkey
        REFERENCES principals (id),
    work_id uuid NOT NULL CONSTRAINT work_publication_command_receipts_work_id_fkey
        REFERENCES works (id) ON DELETE CASCADE,
    operation text NOT NULL CHECK (operation IN ('capture_snapshot', 'request_public', 'set_private')),
    command_key text NOT NULL CHECK (command_key ~ '^[A-Za-z0-9_-]{8,128}$'),
    payload_digest bytea NOT NULL CHECK (octet_length(payload_digest) = 32),
    outcome text NOT NULL,
    revision_id uuid,
    generation bigint CHECK (generation >= 1),
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT work_publication_command_receipts_pkey
        PRIMARY KEY (actor_id, work_id, operation, command_key)
);

COMMENT ON TABLE work_publication_command_receipts IS
    'Author command idempotency receipts scoped to (actor, work, operation, command_key).';

-- 6. Upgrades for existing legacy rows (CHOICE-03 A, AC-07)
-- Advance publications generation to >= 2 for legacy rows that were stored as 1
UPDATE publications SET generation = 2 WHERE generation = 1;

-- Backfill revisions.metadata_snapshot_id from publications if previously bound
UPDATE revisions r
   SET metadata_snapshot_id = p.metadata_snapshot_id
  FROM publications p
 WHERE p.work_id = r.work_id AND p.revision_id = r.id AND p.metadata_snapshot_id IS NOT NULL
   AND r.metadata_snapshot_id IS NULL;

-- Ensure moderation approved row with honest legacy_bootstrap provenance for pointed public revisions
INSERT INTO work_revision_moderation (
    work_id, revision_id, decision_version, moderation, provenance_kind, created_at, updated_at
)
SELECT p.work_id, p.revision_id, 2, 'approved', 'legacy_bootstrap', p.published_at, p.published_at
  FROM publications p
ON CONFLICT (work_id, revision_id) DO NOTHING;

-- Populate work_publications for public works
INSERT INTO work_publications (
    work_id, audience, lifecycle, generation,
    intent_revision_id, intent_generation,
    pointer_revision_id, pointer_intent_generation,
    policy_version, updated_at
)
SELECT
    p.work_id,
    'public',
    'active',
    GREATEST(p.generation, 2),
    p.revision_id,
    GREATEST(p.generation, 2),
    p.revision_id,
    GREATEST(p.generation, 2),
    1,
    p.published_at
FROM publications p
ON CONFLICT (work_id) DO NOTHING;

-- Initialize private active state for existing works not in publications
INSERT INTO work_publications (work_id, audience, lifecycle, generation, policy_version)
SELECT w.id, 'private', 'active', 1, 1
FROM works w
WHERE NOT EXISTS (SELECT 1 FROM work_publications wp WHERE wp.work_id = w.id)
ON CONFLICT (work_id) DO NOTHING;
