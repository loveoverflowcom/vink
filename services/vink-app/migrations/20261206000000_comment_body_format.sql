-- The old writer and every existing receipt used literal text.
ALTER TABLE comments ADD COLUMN body_format text NOT NULL DEFAULT 'plain'
    CHECK (body_format IN ('plain', 'markdown'));
