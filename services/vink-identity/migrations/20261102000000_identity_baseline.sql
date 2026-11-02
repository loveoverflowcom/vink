-- Identity baseline. The authentication authority is not selected (D-05, S-03), so Identity
-- stores no credential, session or security state yet. This version marks the empty schema the
-- Identity migrator owns; tables arrive in new migrations together with the authority they
-- serve. Identity never stores or reads App data (principals, works, drafts, publications).
SELECT 1;
