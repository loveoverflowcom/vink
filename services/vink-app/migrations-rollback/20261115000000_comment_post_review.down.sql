-- Only pre-activation backout. Once any request exists, preserve all ticket/result/service
-- receipts by rolling forward. An old binary can run after this down script on unused schema.
BEGIN;
LOCK TABLE discussion_events, discussion_deliveries, comment_post_reviews,
    comment_post_review_results, comment_post_review_receipts,
    comment_post_review_quarantines IN ACCESS EXCLUSIVE MODE;
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM comment_post_reviews)
       OR EXISTS (SELECT 1 FROM comment_post_review_quarantines)
       OR EXISTS (SELECT 1 FROM discussion_deliveries WHERE consumer = 'operator_post_review.v1')
    THEN
        RAISE EXCEPTION 'post-review audit or debt exists; preserve it and roll forward';
    END IF;
END $$;
DROP TABLE comment_post_review_quarantines, comment_post_review_receipts,
    comment_post_review_results, comment_post_reviews;
DROP FUNCTION reject_post_review_change();
DELETE FROM _sqlx_migrations WHERE version = 20261115000000;
COMMIT;
