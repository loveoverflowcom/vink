-- One irreversible Social fence per bound principal. Account owns verification and retention.
CREATE TABLE social_retirements (
    principal_id uuid PRIMARY KEY REFERENCES principal_bindings (principal_id),
    operation_id uuid NOT NULL UNIQUE CHECK (operation_id <> '00000000-0000-0000-0000-000000000000'),
    edges_complete boolean NOT NULL DEFAULT false
);

CREATE FUNCTION keep_social_retirement_fence() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'UPDATE' THEN
        IF NEW.principal_id = OLD.principal_id AND NEW.operation_id = OLD.operation_id
           AND (NEW.edges_complete OR NOT OLD.edges_complete) THEN
            RETURN NEW;
        END IF;
    END IF;
    RAISE EXCEPTION 'social retirement fence and completed checkpoint are permanent'
        USING ERRCODE = 'integrity_constraint_violation';
END;
$$;
CREATE TRIGGER keep_social_retirement_fence BEFORE UPDATE OR DELETE ON social_retirements
    FOR EACH ROW EXECUTE FUNCTION keep_social_retirement_fence();

CREATE FUNCTION prevent_retired_profile_provision() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF EXISTS (SELECT 1 FROM social_retirements WHERE principal_id = NEW.principal_id) THEN
        RAISE EXCEPTION 'retired principal cannot receive a profile'
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER prevent_retired_profile_provision BEFORE INSERT ON author_profiles
    FOR EACH ROW EXECUTE FUNCTION prevent_retired_profile_provision();

-- A bound legacy author without a profile still loses public attribution at the fence.
-- Keep compatibility for the profile tombstones already produced by the existing owner.
CREATE OR REPLACE VIEW public_author_attribution AS
SELECT p.id,
       CASE WHEN visibility.retired THEN 'Tài khoản đã xóa' ELSE p.display_name END AS display_name,
       CASE WHEN NOT visibility.retired AND (SELECT ready FROM author_handle_rollout WHERE singleton)
            THEN profile.current_handle END AS handle,
       visibility.retired,
       CASE WHEN NOT visibility.retired THEN p.display_name_search_key END AS display_name_search_key
  FROM principals p
  LEFT JOIN author_profiles profile ON profile.principal_id = p.id
  LEFT JOIN social_retirements retirement ON retirement.principal_id = p.id
 CROSS JOIN LATERAL (SELECT retirement.principal_id IS NOT NULL OR
                            (profile.principal_id IS NOT NULL AND profile.current_handle IS NULL)
                            AS retired) visibility;
