-- Current Handle is the public sort key and enables a bounded browse/keyset scan.
CREATE UNIQUE INDEX author_profiles_search_handle_idx ON author_profiles (current_handle)
    WHERE current_handle IS NOT NULL;

-- "C" lets the planner optimize a literal folded prefix as a B-tree range.
CREATE INDEX principals_search_pen_name_idx ON principals (display_name_search_key COLLATE "C");
