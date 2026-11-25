SELECT s.work_id, s.generation, s.saved_at, s.cursor_key, target.kind,
       target.revision_id, r.title, a.display_name, a.handle, t.label AS form
FROM saved_works s
JOIN saved_work_targets target ON target.work_id = s.work_id
LEFT JOIN revisions r ON r.work_id = target.work_id AND r.id = target.revision_id
LEFT JOIN public_author_attribution a ON target.kind = 'readable' AND a.id = target.owner_id
LEFT JOIN work_metadata_snapshot_tags slot ON slot.snapshot_id = target.metadata_snapshot_id AND slot.slot = 0
LEFT JOIN tags t ON t.id = slot.tag_id AND t.status = 'active'
WHERE s.principal_id = $1 AND s.saved_at IS NOT NULL
  AND ($2::timestamptz IS NULL OR
       (s.saved_at <= $2 AND (s.saved_at < $2 OR s.work_id > $3)))
ORDER BY s.saved_at DESC, s.work_id
LIMIT $4
