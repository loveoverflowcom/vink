-- The projection accompanies works/comments without rewriting immutable content. Keeping
-- the retirement test here makes the compatibility label and the search key safe too.
CREATE VIEW public_author_attribution AS
SELECT p.id,
       CASE WHEN profile.principal_id IS NOT NULL AND profile.current_handle IS NULL
            THEN 'Tài khoản đã xóa' ELSE p.display_name END AS display_name,
       CASE WHEN (SELECT ready FROM author_handle_rollout WHERE singleton)
            THEN profile.current_handle END AS handle,
       profile.principal_id IS NOT NULL AND profile.current_handle IS NULL AS retired,
       CASE WHEN profile.principal_id IS NULL OR profile.current_handle IS NOT NULL
            THEN p.display_name_search_key END AS display_name_search_key
  FROM principals p
  LEFT JOIN author_profiles profile ON profile.principal_id = p.id;
