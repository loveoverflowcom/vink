-- Synthetic persisted rows at 20261121000000. Copy the immutable document and digest
-- from the v1 sample when creating each fixture revision, before applying the policy DDL.
DO $$
DECLARE
    sample revisions;
    rev uuid;
    decision text;
    version bigint;
    provenance text;
    reason text;
    cause text;
    job uuid;
BEGIN
    SELECT * INTO STRICT sample FROM revisions WHERE id='44444444-4444-4444-8444-444444444444';
    FOR decision, version, provenance, reason, cause IN VALUES
        ('pending', 1, NULL, NULL, NULL),
        ('approved', 2, 'ai_approved', NULL, NULL),
        ('rejected', 2, 'ai_rejected', 'spam_or_abuse', NULL),
        ('needs_admin', 2, 'ai_needs_admin', NULL, 'uncertain')
    LOOP
        rev := gen_random_uuid();
        job := gen_random_uuid();
        INSERT INTO revisions(id,work_id,title,document,content_hash,created_by)
        VALUES(rev,sample.work_id,sample.title,sample.document,sample.content_hash,sample.created_by);
        INSERT INTO work_revision_moderation(work_id,revision_id,decision_version,moderation,
                policy_version,provenance_kind,policy_reason_code,needs_admin_cause)
        VALUES(sample.work_id,rev,version,decision,1,provenance,reason,cause);
        INSERT INTO work_evaluation_jobs(id,work_id,revision_id,content_digest,policy_version,state)
        VALUES(job,sample.work_id,rev,sample.content_hash,1,
               CASE WHEN decision='pending' THEN 'open' ELSE 'applied' END);
        INSERT INTO work_revision_decision_history(id,work_id,revision_id,decision_version,
                moderation,policy_version,provenance_kind,policy_reason_code,needs_admin_cause,evaluation_id)
        VALUES(gen_random_uuid(),sample.work_id,rev,version,decision,1,provenance,reason,cause,
               CASE WHEN decision<>'pending' THEN job END);
        IF decision='needs_admin' THEN
            INSERT INTO moderation_cases(id,subject_kind,work_id,revision_id,content_digest,
                    decision_version,target_content_owner,target_work_owner,origin,version,state)
            VALUES(gen_random_uuid(),'work_revision',sample.work_id,rev,sample.content_hash,
                   version,sample.created_by,sample.created_by,'evaluator_uncertain',1,'queued');
        END IF;
        INSERT INTO work_publication_command_receipts(actor_id,work_id,operation,command_key,
                payload_digest,outcome,revision_id,generation)
        VALUES(sample.created_by,sample.work_id,'request_public','migration-'||decision,
               sha256(convert_to('request_public:'||rev||':1','UTF8')),'submitted',rev,2);
    END LOOP;
END $$;
