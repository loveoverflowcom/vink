-- Stop the #201 application before backout; the old binary keeps Recent/100 semantics.
-- Only a derived key is removed. Draft text, versions and timestamps are untouched.
BEGIN;
ALTER TABLE working_drafts DROP COLUMN desk_title_key;
DROP FUNCTION vink_desk_key(text);
DELETE FROM _sqlx_migrations WHERE version = 20261120000000;
COMMIT;
