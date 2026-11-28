-- The production statements of persistence/discussion.rs, prepared with parameters as SQLx sends
-- them. Keep in sync with ITEM_COLUMNS / PUBLIC_REPLIES / VISIBLE_TO_VIEWER / OWN_* constants.
\set ON_ERROR_STOP 1
PREPARE roots(uuid, uuid, timestamptz, uuid, bigint) AS
SELECT c.root_id, c.id, c.author_id, a.display_name, c.body, c.review,
     c.removal, c.moderation_version, c.created_at, c.reply_to_id, c.replied_to_reply,
     (SELECT d.reason FROM comment_moderation_decisions d WHERE d.comment_id = c.id
       ORDER BY d.version DESC LIMIT 1) AS last_reason,
     NULL::text AS context_review, NULL::text AS context_removal,
     NULL::bigint AS context_version, NULL::text AS context_author_name,
     (SELECT count(*) FROM comments r WHERE r.root_id = c.id
       AND r.kind = 'reply' AND r.review = 'approved' AND r.removal IS NULL) AS public_replies
  FROM comments c JOIN principals a ON a.id = c.author_id
 WHERE c.thread_id = $1 AND c.kind = 'root'
   AND ($3::timestamptz IS NULL OR (c.created_at, c.id) > ($3, $4))
   AND ((c.removal IS NULL AND (c.review = 'approved' OR c.author_id = $2))
        OR (SELECT count(*) FROM comments r WHERE r.root_id = c.id
             AND r.kind = 'reply' AND r.review = 'approved' AND r.removal IS NULL) > 0)
 ORDER BY c.created_at, c.id
 LIMIT $5;

PREPARE previews(uuid[], uuid, bigint) AS
SELECT c.root_id, c.id, c.author_id, a.display_name, c.body, c.review,
     c.removal, c.moderation_version, c.created_at, c.reply_to_id, c.replied_to_reply,
     (SELECT d.reason FROM comment_moderation_decisions d WHERE d.comment_id = c.id
       ORDER BY d.version DESC LIMIT 1) AS last_reason,
     x.review AS context_review, x.removal AS context_removal,
     x.moderation_version AS context_version, xa.display_name AS context_author_name,
     0::bigint AS public_replies
  FROM unnest($1::uuid[]) AS parent(id)
 CROSS JOIN LATERAL (
       SELECT c.* FROM comments c
        WHERE c.root_id = parent.id AND c.kind = 'reply'
          AND c.removal IS NULL AND (c.review = 'approved' OR c.author_id = $2)
        ORDER BY c.created_at, c.id
        LIMIT $3) c
  JOIN principals a ON a.id = c.author_id
  LEFT JOIN comments x ON x.id = c.reply_to_id
  LEFT JOIN principals xa ON xa.id = x.author_id
 ORDER BY c.root_id, c.created_at, c.id;

PREPARE replies(uuid, uuid, timestamptz, uuid, bigint) AS
SELECT c.root_id, c.id, c.author_id, a.display_name, c.body, c.review,
     c.removal, c.moderation_version, c.created_at, c.reply_to_id, c.replied_to_reply,
     (SELECT d.reason FROM comment_moderation_decisions d WHERE d.comment_id = c.id
       ORDER BY d.version DESC LIMIT 1) AS last_reason,
     x.review AS context_review, x.removal AS context_removal,
     x.moderation_version AS context_version, xa.display_name AS context_author_name,
     0::bigint AS public_replies
  FROM comments c
  JOIN principals a ON a.id = c.author_id
  LEFT JOIN comments x ON x.id = c.reply_to_id
  LEFT JOIN principals xa ON xa.id = x.author_id
 WHERE c.root_id = $1 AND c.kind = 'reply'
   AND c.removal IS NULL AND (c.review = 'approved' OR c.author_id = $2)
   AND ($3::timestamptz IS NULL OR (c.created_at, c.id) > ($3, $4))
 ORDER BY c.created_at, c.id
 LIMIT $5;

PREPARE own_list(uuid, uuid, timestamptz, uuid, bigint) AS
SELECT c.id, c.author_id, c.body, c.review, c.removal, c.moderation_version,
     c.created_at, c.root_id, r.author_id AS root_author_id, r.review AS root_review,
     r.removal AS root_removal, r.moderation_version AS root_version,
     CASE WHEN c.root_id IS NULL THEN 0 ELSE (SELECT count(*) FROM comments pr
       WHERE pr.root_id = c.root_id AND pr.kind = 'reply' AND pr.review = 'approved'
         AND pr.removal IS NULL) END AS root_public_replies,
     c.reply_to_id, c.replied_to_reply, x.review AS context_review,
     x.removal AS context_removal, x.moderation_version AS context_version,
     xa.display_name AS context_author_name,
     (SELECT d.reason FROM comment_moderation_decisions d WHERE d.comment_id = c.id
       ORDER BY d.version DESC LIMIT 1) AS last_reason
  FROM comments c
  LEFT JOIN comments r ON r.id = c.root_id
  LEFT JOIN comments x ON x.id = c.reply_to_id
  LEFT JOIN principals xa ON xa.id = x.author_id
 WHERE c.thread_id = $1 AND c.author_id = $2 AND c.removal IS NULL AND c.review <> 'approved'
   AND ($3::timestamptz IS NULL OR (c.created_at, c.id) > ($3, $4))
 ORDER BY c.created_at, c.id
 LIMIT $5;

PREPARE own_exact(uuid, uuid, uuid[]) AS
SELECT c.id, c.author_id, c.body, c.review, c.removal, c.moderation_version,
     c.created_at, c.root_id, r.author_id AS root_author_id, r.review AS root_review,
     r.removal AS root_removal, r.moderation_version AS root_version,
     CASE WHEN c.root_id IS NULL THEN 0 ELSE (SELECT count(*) FROM comments pr
       WHERE pr.root_id = c.root_id AND pr.kind = 'reply' AND pr.review = 'approved'
         AND pr.removal IS NULL) END AS root_public_replies,
     c.reply_to_id, c.replied_to_reply, x.review AS context_review,
     x.removal AS context_removal, x.moderation_version AS context_version,
     xa.display_name AS context_author_name,
     (SELECT d.reason FROM comment_moderation_decisions d WHERE d.comment_id = c.id
       ORDER BY d.version DESC LIMIT 1) AS last_reason
  FROM comments c
  LEFT JOIN comments r ON r.id = c.root_id
  LEFT JOIN comments x ON x.id = c.reply_to_id
  LEFT JOIN principals xa ON xa.id = x.author_id
 WHERE c.thread_id = $1 AND c.author_id = $2 AND c.id = ANY($3::uuid[])
 ORDER BY c.created_at, c.id;
