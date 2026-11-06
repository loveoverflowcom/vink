-- Literary endorsement (LVE, #73): the selection must agree with the mutation version.
--
-- `decide_endorsement` starts every target at OFF/version 1 and advances the version exactly when
-- the selection flips, so every reachable state is selected exactly when its version is even.
-- 20261105000000 bounded the tokens (1 <= version <= order) but accepted rows such as
-- selected = true at version 1, or an UPDATE that flips `selected` without a version step. Public
-- counts are SQL aggregates over `selected` that never pass through `EndorsementState::restore`,
-- so such a row would be counted although no accepted command produced it. This is a structural
-- check like `*_tokens_check`, not a second reducer: the transition itself stays in Rust.
--
-- Names sort after `*_tokens_check` (PostgreSQL tests CHECK constraints alphabetically), so a
-- row with out-of-range tokens still reports the token constraint first.
--
-- Adding a CHECK validates existing rows under the table lock of this migration. Rows written by
-- the application already satisfy it; a database where direct SQL wrote an unreachable row stops
-- here, and the row must be inspected and corrected by hand (there is no safe automatic repair:
-- either its selection or its version is wrong). Backout: drop the three constraints.

ALTER TABLE work_endorsements
    ADD CONSTRAINT work_endorsements_version_parity_check
        CHECK (selected = (version % 2 = 0));

ALTER TABLE comment_endorsements
    ADD CONSTRAINT comment_endorsements_version_parity_check
        CHECK (selected = (version % 2 = 0));

-- A receipt's `selected` is both the intent and the committed selection.
ALTER TABLE endorsement_receipts
    ADD CONSTRAINT endorsement_receipts_version_parity_check
        CHECK (selected = (committed_version % 2 = 0));
