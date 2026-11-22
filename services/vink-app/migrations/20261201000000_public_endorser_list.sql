-- Public Thấm list (#348, P-08 R3): every endorsement row gets an opaque, stable position, so a
-- public cursor never carries a PrincipalId and a reader who unlikes and likes again keeps the
-- same place in the list (rows are never deleted; `selected` flips). Same pattern as
-- `author_follows.cursor_key` (#294).
ALTER TABLE work_endorsements
    ADD COLUMN cursor_key uuid NOT NULL DEFAULT gen_random_uuid();

ALTER TABLE work_endorsements
    ADD CONSTRAINT work_endorsements_cursor_key_key UNIQUE (cursor_key);

-- One index range per page: the current members of one (work, revision) in cursor order.
CREATE INDEX work_endorsements_public_list_idx
    ON work_endorsements (work_id, revision_id, cursor_key)
    WHERE selected;
