-- The Feed can seek directly to a first-public position when the viewer follows
-- many eligible authors. For sparse viewers the planner can start at active edges.
CREATE INDEX work_first_publications_feed_order_idx
    ON work_first_publications (first_published_at DESC, work_id DESC)
    WHERE provenance = 'exact';
