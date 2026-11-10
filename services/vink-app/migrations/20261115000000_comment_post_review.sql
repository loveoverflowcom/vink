-- Explicit operator re-review, independent of admission. Existing admission provenance and
-- comments_admission_premoderation_only are unchanged. No automatic producer on human approval.
CREATE TABLE comment_post_reviews (
    id uuid PRIMARY KEY,
    request_key uuid NOT NULL UNIQUE,
    comment_id uuid NOT NULL REFERENCES comments(id),
    expected_version bigint NOT NULL CHECK (expected_version >= 1),
    payload_digest bytea NOT NULL CHECK (octet_length(payload_digest) = 32),
    origin text NOT NULL CHECK (origin = 'operator_rereview'),
    policy text NOT NULL CHECK (policy = 'operator-rereview.v1'),
    note text NOT NULL CHECK (char_length(note) BETWEEN 1 AND 200),
    database_role text NOT NULL DEFAULT session_user,
    created_at timestamptz NOT NULL DEFAULT now(),
    deadline_at timestamptz NOT NULL,
    CHECK (deadline_at = created_at + interval '15 minutes'),
    UNIQUE(comment_id, expected_version),
    UNIQUE(id, comment_id, expected_version, payload_digest, origin, policy)
);

CREATE TABLE comment_post_review_results (
    ticket_id uuid PRIMARY KEY,
    command_key uuid NOT NULL UNIQUE,
    comment_id uuid NOT NULL,
    expected_version bigint NOT NULL,
    payload_digest bytea NOT NULL,
    origin text NOT NULL,
    policy text NOT NULL,
    finding text NOT NULL CHECK (finding IN ('clear', 'flagged', 'unresolved')),
    note text NOT NULL CHECK (char_length(note) BETWEEN 1 AND 200),
    database_role text NOT NULL DEFAULT session_user,
    created_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY(ticket_id, comment_id, expected_version, payload_digest, origin, policy)
        REFERENCES comment_post_reviews(id, comment_id, expected_version, payload_digest, origin, policy)
);

-- Service audit, never a fabricated moderator principal/grant. One immutable receipt is the
-- ONLY completion of the content obligation; an outbox dead letter is not a completion.
CREATE TABLE comment_post_review_receipts (
    ticket_id uuid PRIMARY KEY REFERENCES comment_post_reviews(id),
    outcome text NOT NULL CHECK (outcome IN ('kept', 'held', 'superseded', 'removed')),
    observed_version bigint NOT NULL CHECK (observed_version >= 1),
    resulting_version bigint NOT NULL CHECK (resulting_version >= observed_version),
    finding text CHECK (finding IN ('clear', 'flagged', 'unresolved')),
    actor text NOT NULL DEFAULT 'post_review_service.v1' CHECK (actor = 'post_review_service.v1'),
    completed_at timestamptz NOT NULL DEFAULT now()
);

-- Poison is visible unresolved debt. No comment is guessed from corrupt event metadata.
CREATE TABLE comment_post_review_quarantines (
    event_id uuid PRIMARY KEY,
    reason text NOT NULL CHECK (reason IN ('invalid_binding', 'unsupported_event', 'missing_ticket')),
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE FUNCTION reject_post_review_change() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'post-review provenance and audit are immutable'
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

CREATE TRIGGER comment_post_reviews_immutable BEFORE UPDATE OR DELETE ON comment_post_reviews
    FOR EACH ROW EXECUTE FUNCTION reject_post_review_change();
CREATE TRIGGER comment_post_review_results_immutable BEFORE UPDATE OR DELETE ON comment_post_review_results
    FOR EACH ROW EXECUTE FUNCTION reject_post_review_change();
CREATE TRIGGER comment_post_review_receipts_immutable BEFORE UPDATE OR DELETE ON comment_post_review_receipts
    FOR EACH ROW EXECUTE FUNCTION reject_post_review_change();
CREATE TRIGGER comment_post_review_quarantines_immutable BEFORE UPDATE OR DELETE ON comment_post_review_quarantines
    FOR EACH ROW EXECUTE FUNCTION reject_post_review_change();

CREATE INDEX comment_post_reviews_age_idx ON comment_post_reviews(created_at);

COMMENT ON TABLE comment_post_reviews IS 'Explicit operator authority, immutable 15-minute business deadline. Never admission_approved. Event id = ticket id; audit notes identify the operator and reason, as for outbox requeue.';
COMMENT ON TABLE comment_post_review_results IS 'Structured operator assessment of the exact ticket; not an AI adapter. No public route or DTO.';
COMMENT ON TABLE comment_post_review_receipts IS 'Service decision audit; commits with comment effect and delivery outcome. Retained indefinitely; no moderator grant is forged.';
