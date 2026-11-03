-- Vink-owned two-tier discussion (D2T-02a). Owner decision 03/11/2026: the App is the only writer
-- and moderator of comments; there is no Discourse store, mapping or sync table.
--
-- One thread per work. A comment is a root, or a reply in ONE flat list under a root of the same
-- thread; `reply_to_id` is context only. These rules hold without Rust: composite foreign keys
-- bind a reply to a root (kind 'root') of the same thread and a context to a reply of the same
-- root, and a trigger keeps placement immutable. Presence (`removal`) and review are separate
-- columns; public visibility is derived from both by the read paths.

CREATE TABLE discussion_threads (
    id uuid PRIMARY KEY,
    work_id uuid NOT NULL CONSTRAINT discussion_threads_work_id_key UNIQUE REFERENCES works (id),
    comments_open boolean NOT NULL DEFAULT true,
    settings_generation bigint NOT NULL DEFAULT 1 CHECK (settings_generation >= 1),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

COMMENT ON TABLE discussion_threads IS
    'The single discussion thread of a work (COM-01). comments_open is the work author''s per-work switch (COM-07); settings_generation guards stale toggles.';

CREATE TABLE comments (
    id uuid PRIMARY KEY,
    thread_id uuid NOT NULL REFERENCES discussion_threads (id),
    kind text NOT NULL CHECK (kind IN ('root', 'reply')),
    -- For replies: the root, and the constant 'root' that the composite key checks against kind.
    root_id uuid,
    root_kind text,
    -- Context only. NULL for a direct reply to the root; also NULL after a purged context, which
    -- replied_to_reply (fixed at creation) distinguishes from a direct reply.
    reply_to_id uuid,
    replied_to_reply boolean NOT NULL,
    author_id uuid NOT NULL REFERENCES principals (id),
    -- NULL exactly when the writer removed the comment: their text is dropped.
    body text,
    review text NOT NULL CHECK (review IN ('pending', 'approved', 'rejected')),
    removal text CHECK (removal IN ('author', 'moderation')),
    moderation_version bigint NOT NULL CHECK (moderation_version >= 1),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT comments_tier CHECK (
        (kind = 'root' AND root_id IS NULL AND root_kind IS NULL AND reply_to_id IS NULL
            AND NOT replied_to_reply)
        OR (kind = 'reply' AND root_id IS NOT NULL AND root_kind = 'root')
    ),
    CONSTRAINT comments_context_recorded CHECK (reply_to_id IS NULL OR replied_to_reply),
    CONSTRAINT comments_context_not_self CHECK (reply_to_id <> id),
    CONSTRAINT comments_body CHECK (
        (body IS NULL) = (removal IS NOT DISTINCT FROM 'author')
        AND (body IS NULL OR char_length(body) BETWEEN 1 AND 4000)
    ),
    CONSTRAINT comments_id_thread_kind_key UNIQUE (id, thread_id, kind),
    CONSTRAINT comments_id_root_thread_key UNIQUE (id, root_id, thread_id),
    -- A reply's root is a ROOT of the SAME thread (a plain FK to comments(id) would allow either).
    CONSTRAINT comments_root_fkey FOREIGN KEY (root_id, thread_id, root_kind)
        REFERENCES comments (id, thread_id, kind),
    -- A context is a reply of the SAME root and thread. Deleting a context never cascades to its
    -- peers: only this reply's reply_to_id becomes NULL; root/thread keys are untouched.
    CONSTRAINT comments_reply_to_fkey FOREIGN KEY (reply_to_id, root_id, thread_id)
        REFERENCES comments (id, root_id, thread_id) ON DELETE SET NULL (reply_to_id)
);

COMMENT ON TABLE comments IS
    'Roots and flat replies of a work thread. New rows are pending (premoderation, COM-02). Read paths expose only approved, present rows to others; removal is terminal.';

CREATE FUNCTION guard_comment_update() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.id IS DISTINCT FROM OLD.id
        OR NEW.thread_id IS DISTINCT FROM OLD.thread_id
        OR NEW.kind IS DISTINCT FROM OLD.kind
        OR NEW.root_id IS DISTINCT FROM OLD.root_id
        OR NEW.root_kind IS DISTINCT FROM OLD.root_kind
        OR NEW.replied_to_reply IS DISTINCT FROM OLD.replied_to_reply
        OR NEW.author_id IS DISTINCT FROM OLD.author_id
        OR NEW.created_at IS DISTINCT FROM OLD.created_at
        -- The only placement change allowed is a context purge (SET NULL).
        OR (NEW.reply_to_id IS DISTINCT FROM OLD.reply_to_id AND NEW.reply_to_id IS NOT NULL)
    THEN
        RAISE EXCEPTION 'comment % placement is immutable', OLD.id
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    IF OLD.removal IS NOT NULL AND NEW.removal IS DISTINCT FROM OLD.removal THEN
        RAISE EXCEPTION 'comment % removal is terminal', OLD.id
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    IF NEW.moderation_version < OLD.moderation_version THEN
        RAISE EXCEPTION 'comment % moderation version cannot decrease', OLD.id
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER comments_guard_update
    BEFORE UPDATE ON comments
    FOR EACH ROW EXECUTE FUNCTION guard_comment_update();

-- Root page and reply page keysets (created_at ASC, id ASC). No reply_to_id index yet: no path
-- deletes comments, so the context FK's reverse lookup is unused; add it with the purge slice
-- after measuring.
CREATE INDEX comments_thread_roots_idx ON comments (thread_id, created_at, id)
    WHERE kind = 'root';
CREATE INDEX comments_root_replies_idx ON comments (root_id, created_at, id)
    WHERE kind = 'reply';
-- Moderation queue: oldest pending first.
CREATE INDEX comments_pending_idx ON comments (created_at, id)
    WHERE review = 'pending' AND removal IS NULL;

CREATE TABLE discussion_command_receipts (
    actor_id uuid NOT NULL REFERENCES principals (id),
    operation text NOT NULL CHECK (operation = 'submit_comment'),
    command_key text NOT NULL CHECK (command_key ~ '^[A-Za-z0-9_-]{8,128}$'),
    payload_digest bytea NOT NULL CHECK (octet_length(payload_digest) = 32),
    -- Deferred: the receipt is claimed before the comment row exists in the same transaction.
    comment_id uuid NOT NULL CONSTRAINT discussion_command_receipts_comment_fkey
        REFERENCES comments (id) DEFERRABLE INITIALLY DEFERRED,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (actor_id, operation, command_key)
);

COMMENT ON TABLE discussion_command_receipts IS
    'Idempotency receipts (D2T-INV-06): same actor+operation+key and digest replays comment_id; another digest is refused. Retention is not implemented.';

CREATE TABLE comment_moderation_decisions (
    comment_id uuid NOT NULL REFERENCES comments (id),
    -- The comment's moderation_version AFTER this decision.
    version bigint NOT NULL CHECK (version >= 2),
    action text NOT NULL CHECK (action IN ('approve', 'reject', 'remove')),
    moderator_id uuid NOT NULL REFERENCES principals (id),
    decided_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (comment_id, version)
);

COMMENT ON TABLE comment_moderation_decisions IS
    'Append-only audit of moderation decisions (MOD-02/04). The moderator comes from a server-side authority grant, never from a request.';

CREATE FUNCTION reject_moderation_decision_change() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'moderation decisions are append-only'
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

CREATE TRIGGER comment_moderation_decisions_append_only
    BEFORE UPDATE OR DELETE ON comment_moderation_decisions
    FOR EACH ROW EXECUTE FUNCTION reject_moderation_decision_change();
