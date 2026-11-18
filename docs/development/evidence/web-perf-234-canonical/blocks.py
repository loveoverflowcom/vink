"""Print SQL to replace the one public lab work in a disposable copy of the #234 DB."""
import gzip
import json
from pathlib import Path
import zipfile

archive = Path("docs/development/evidence/web-perf-237/canonical-20261118/raw.zip")
with zipfile.ZipFile(archive) as bundle:
    corpus = json.loads(gzip.decompress(bundle.read("canonical-inputs.json.gz")))
source = next(row["document"]["source"] for row in corpus if row["case"] == "tiny_blocks-200000")
assert len(source.encode()) == 599998 and len(source.split("\n\n")) == 200000
# Hex is unambiguous even when future synthetic corpora contain SQL metacharacters.
print("""
BEGIN;
DO $$ BEGIN
  IF current_database() NOT IN ('vink_perf_blocks', 'vink_perf_blocks_repro') THEN
    RAISE EXCEPTION 'requires disposable vink_perf_blocks scratch copy';
  END IF;
  IF NOT EXISTS (SELECT 1 FROM publications WHERE work_id = md5('vink-100-work-1')::uuid
                 AND revision_id = md5('vink-100-revision-1')::uuid) THEN
    RAISE EXCEPTION 'requires original #100 public pointer';
  END IF;
END $$;
WITH fixture AS (
  SELECT r.work_id, r.title, r.created_by, convert_from(decode('%s','hex'),'UTF8') AS body
  FROM revisions r WHERE r.id = md5('vink-100-revision-1')::uuid
), added AS (
  INSERT INTO revisions (id, work_id, title, document, content_hash, created_by)
  SELECT 'b48fe416-6ebf-bd1c-a08e-9f2dec3d0375', work_id, title,
         jsonb_build_object('format','default','source',body),
         encode(sha256(convert_to('vink-document-content','UTF8') ||
           int8send(7) || convert_to('default','UTF8') ||
           int8send(octet_length(title)) || convert_to(title,'UTF8') ||
           int8send(octet_length(body)) || convert_to(body,'UTF8')), 'hex'), created_by
  FROM fixture RETURNING id, work_id
)
UPDATE publications SET revision_id = added.id, generation = generation + 1
FROM added WHERE publications.work_id = added.work_id;
COMMIT;
""" % source.encode().hex())
