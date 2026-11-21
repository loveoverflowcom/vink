-- Work views ("Lượt xem", #341, VDR-WORK-VIEWS@r1): CHOICE-02 A, one transaction per event
-- that checks the work's public pointer under a share lock, claims the reader's 24-hour window,
-- bumps the totals and keeps the event receipt. Only the three kinds of data VIEW-11 allows:
--   1. a total per work and a total per (work, day) — never per reader;
--   2. the keyed digest of (reader, work) for the open window;
--   3. per-event receipts, which hold no reader at all.
-- Windows and receipts are removed by `vink-worker` once older than 24 hours; totals live until
-- their work is deleted (every table cascades from `works`, so a future deletion route can erase
-- them by WorkId: OQ-2). No IP address, fingerprint or reading history is stored. Nothing here
-- is backfilled (VIEW-09): no table is derived from endorsements, comments or access logs.

CREATE TABLE work_view_totals (
    work_id uuid PRIMARY KEY REFERENCES works (id) ON DELETE CASCADE,
    views bigint NOT NULL CHECK (views >= 1),
    updated_at timestamptz NOT NULL
);

-- `day` is the decision instant's date in Asia/Ho_Chi_Minh (UTC+7). For every work the per-day
-- totals add up to its total: both are bumped by the same statement pair.
CREATE TABLE work_view_days (
    work_id uuid NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    day date NOT NULL,
    views bigint NOT NULL CHECK (views >= 1),
    CONSTRAINT work_view_days_pkey PRIMARY KEY (work_id, day)
);

-- One row per (work, reader digest): the instant of that reader's last counted view. It blocks
-- a new count up to and including 24 hours later; an expired row is either replaced by the next
-- counted view or removed by retention.
CREATE TABLE work_view_windows (
    work_id uuid NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    reader_digest bytea NOT NULL CHECK (octet_length(reader_digest) = 32),
    counted_at timestamptz NOT NULL,
    CONSTRAINT work_view_windows_pkey PRIMARY KEY (work_id, reader_digest)
);

CREATE INDEX work_view_windows_counted_at_idx ON work_view_windows (counted_at);

-- The first decision of each event, replayed by every retry. `basis_revision_id` is the public
-- revision that decision saw, absent exactly when the work was not public.
CREATE TABLE work_view_receipts (
    event_digest bytea PRIMARY KEY CHECK (octet_length(event_digest) = 32),
    work_id uuid NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    verdict text NOT NULL
        CHECK (verdict IN ('counted', 'within_window', 'own_work', 'not_public')),
    basis_revision_id uuid,
    decided_at timestamptz NOT NULL,
    CONSTRAINT work_view_receipts_basis_check
        CHECK ((verdict = 'not_public') = (basis_revision_id IS NULL)),
    CONSTRAINT work_view_receipts_basis_fkey FOREIGN KEY (work_id, basis_revision_id)
        REFERENCES revisions (work_id, id) ON DELETE CASCADE
);

CREATE INDEX work_view_receipts_decided_at_idx ON work_view_receipts (decided_at);
CREATE INDEX work_view_receipts_work_id_idx ON work_view_receipts (work_id);

-- When counting started (VIEW-09): the decision instant of the first event ever decided. The
-- read side labels works first public before it with "Tính từ dd/mm/yyyy". No row: nothing has
-- been counted yet.
CREATE TABLE work_view_counting (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    started_at timestamptz NOT NULL
);

-- Totals only ever grow by one view at a time; their identity never changes.
CREATE FUNCTION guard_work_view_total() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.work_id IS DISTINCT FROM OLD.work_id THEN
        RAISE EXCEPTION 'work view total identity is immutable'
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    IF NEW.views <> OLD.views + 1 THEN
        RAISE EXCEPTION 'work view total must grow by one (% -> %)', OLD.views, NEW.views
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER guard_work_view_total BEFORE UPDATE ON work_view_totals
    FOR EACH ROW EXECUTE FUNCTION guard_work_view_total();

CREATE FUNCTION guard_work_view_day() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.work_id IS DISTINCT FROM OLD.work_id OR NEW.day IS DISTINCT FROM OLD.day THEN
        RAISE EXCEPTION 'work view day identity is immutable'
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    IF NEW.views <> OLD.views + 1 THEN
        RAISE EXCEPTION 'work view day total must grow by one (% -> %)', OLD.views, NEW.views
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER guard_work_view_day BEFORE UPDATE ON work_view_days
    FOR EACH ROW EXECUTE FUNCTION guard_work_view_day();

-- A window moves forward only, to a new counted view; its key never changes.
CREATE FUNCTION guard_work_view_window() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.work_id IS DISTINCT FROM OLD.work_id
        OR NEW.reader_digest IS DISTINCT FROM OLD.reader_digest THEN
        RAISE EXCEPTION 'work view window identity is immutable'
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    IF NEW.counted_at <= OLD.counted_at THEN
        RAISE EXCEPTION 'work view window must move forward'
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER guard_work_view_window BEFORE UPDATE ON work_view_windows
    FOR EACH ROW EXECUTE FUNCTION guard_work_view_window();

-- A receipt is the event's first decision: never rewritten (deleted only by retention or with
-- its work). The counting start is written once.
CREATE FUNCTION reject_work_view_rewrite() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION '% rows are never updated', TG_TABLE_NAME
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;

CREATE TRIGGER reject_work_view_receipt_update BEFORE UPDATE ON work_view_receipts
    FOR EACH ROW EXECUTE FUNCTION reject_work_view_rewrite();

CREATE TRIGGER reject_work_view_counting_update BEFORE UPDATE ON work_view_counting
    FOR EACH ROW EXECUTE FUNCTION reject_work_view_rewrite();
