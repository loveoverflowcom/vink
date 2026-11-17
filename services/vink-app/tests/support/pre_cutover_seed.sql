-- The dev-seed sample as binaries before the canonical document cutover (#255) wrote it: retired
-- {schema_version, blocks} documents and the retired v1 content hash. Test fixture of historical
-- data only; it runs on schemas older than migration 20261122000000 and nowhere else.
BEGIN;
INSERT INTO principals (id, display_name) VALUES
    ('11111111-1111-4111-8111-111111111111', 'Tác giả mẫu (dữ liệu phát triển)'),
    ('22222222-2222-4222-8222-222222222222', 'Người viết khác (dữ liệu phát triển)')
ON CONFLICT (id) DO NOTHING;
INSERT INTO works (id, owner_id) VALUES
    ('33333333-3333-4333-8333-333333333333', '11111111-1111-4111-8111-111111111111')
ON CONFLICT (id) DO NOTHING;
CREATE TEMP TABLE pre_cutover_stanzas ON COMMIT DROP AS
SELECT * FROM (VALUES
    (1, 1, 'Phố cũ mưa giăng sợi mỏng,'), (1, 2, 'mái ngói rêu xanh lặng thầm.'),
    (1, 3, 'Ai để quên chiếc ô vàng'), (1, 4, 'bên hiên quán nước ven sông.'),
    (2, 1, 'Mực loang trên trang giấy trắng,'), (2, 2, 'chữ nghiêng như gió qua cầu.'),
    (2, 3, 'Câu thơ chưa kịp đặt tên'), (2, 4, '    đã theo mưa về rất lâu.')
) AS s(stanza, n, line);
INSERT INTO revisions (id, work_id, title, document, content_hash, created_by)
SELECT '44444444-4444-4444-8444-444444444444', '33333333-3333-4333-8333-333333333333',
       '[Mẫu] Mưa trên phố cũ',
       jsonb_build_object('schema_version', 1, 'blocks', (
           SELECT jsonb_agg(jsonb_build_object('lines', l.lines) ORDER BY l.stanza)
             FROM (SELECT stanza, jsonb_agg(line ORDER BY n) AS lines
                     FROM pre_cutover_stanzas GROUP BY stanza) l)),
       encode(sha256(convert_to('vink-revision-content-v1', 'UTF8')
           || int8send(octet_length('[Mẫu] Mưa trên phố cũ')::bigint)
           || convert_to('[Mẫu] Mưa trên phố cũ', 'UTF8')
           || int8send(2)
           || (SELECT string_agg(int8send(4) || (SELECT string_agg(
                       int8send(octet_length(t.line)::bigint) || convert_to(t.line, 'UTF8'), ''::bytea
                       ORDER BY t.n) FROM pre_cutover_stanzas t WHERE t.stanza = g.stanza),
                   ''::bytea ORDER BY g.stanza)
                 FROM (SELECT DISTINCT stanza FROM pre_cutover_stanzas) g)), 'hex'),
       '11111111-1111-4111-8111-111111111111'
ON CONFLICT (id) DO NOTHING;
DO $$
DECLARE
    seed_pointer boolean := true;
    inserted bigint;
BEGIN
    -- As dev_seed: the paired publication state (#158) only on a schema that has it, once.
    IF to_regclass('work_publications') IS NOT NULL THEN
        INSERT INTO work_publications (work_id, audience, lifecycle, generation,
                                       intent_revision_id, intent_generation,
                                       pointer_revision_id, pointer_intent_generation,
                                       ever_public, public_epoch, policy_version, updated_at)
        VALUES ('33333333-3333-4333-8333-333333333333', 'public', 'active', 2,
                '44444444-4444-4444-8444-444444444444', 2,
                '44444444-4444-4444-8444-444444444444', 2,
                ARRAY['44444444-4444-4444-8444-444444444444'::uuid], 1, 1, now())
        ON CONFLICT (work_id) DO NOTHING;
        GET DIAGNOSTICS inserted = ROW_COUNT;
        seed_pointer := inserted = 1;
        IF seed_pointer THEN
            INSERT INTO work_revision_moderation (work_id, revision_id, decision_version,
                                                  moderation, provenance_kind)
            VALUES ('33333333-3333-4333-8333-333333333333',
                    '44444444-4444-4444-8444-444444444444', 2, 'approved', 'legacy_bootstrap')
            ON CONFLICT (work_id, revision_id) DO NOTHING;
        END IF;
    END IF;
    IF seed_pointer THEN
        INSERT INTO publications (work_id, revision_id, generation, published_at)
        VALUES ('33333333-3333-4333-8333-333333333333',
                '44444444-4444-4444-8444-444444444444', 2, now())
        ON CONFLICT (work_id) DO NOTHING;
    END IF;
END $$;
INSERT INTO working_drafts (work_id, version, title, document)
SELECT '33333333-3333-4333-8333-333333333333', 1, '[Mẫu] Mưa trên phố cũ',
       jsonb_build_object('schema_version', 1, 'blocks', (
           SELECT jsonb_agg(jsonb_build_object('lines', l.lines) ORDER BY l.stanza)
             FROM (SELECT stanza, jsonb_agg(line ORDER BY n) AS lines FROM pre_cutover_stanzas
                   GROUP BY stanza
                   UNION ALL SELECT 3, '["Khổ đang viết dở — chưa công khai."]'::jsonb) l))
ON CONFLICT (work_id) DO NOTHING;
COMMIT;
