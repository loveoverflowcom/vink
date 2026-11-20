import contextlib
import io
import json
import os
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch

import artifacts as art
import release
import retirement

A, B = "a" * 40, "b" * 40
ACCOUNT = "c" * 32


def spec(rev):
    return {"environment": "production", "public_origin": "https://vink.forum", "source_sha": rev,
            "platform": "linux/amd64", "images": {k: f"ghcr.io/loveoverflowcom/vink-{k}@sha256:" + "e" * 64
                                                 for k in ("web", "worker", "identity")}}


def package(archives, rev):
    pkg = archives / rev / "pkg"
    for name in art.REQUIRED:
        file = pkg / name
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_bytes((rev + name).encode())
    receipt = art.record(pkg, spec(rev))
    (pkg.parent / "manifest.json").write_bytes(art.encode(receipt))
    return receipt


class Packages(unittest.TestCase):
    def test_inventory_and_rollback(self):
        with TemporaryDirectory() as tmp:
            root = Path(tmp)
            archives = root / "archives"
            for rev in (A, B):
                package(archives, rev)
            for rev in (B, A):
                output = root / rev
                art.stage(archives, rev, output, A)
                for keep in (A, B):
                    self.assertEqual((output / "_v" / keep / "pkg/vink.js").read_text(), keep + "vink.js")
            (archives / A / "pkg/vink.css").write_text("drift")
            with self.assertRaisesRegex(ValueError, "drift"):
                art.stage(archives, A, root / "drift")

    def test_rejects_wrong_target_image_and_public_files(self):
        for key, value in (("public_origin", "https://test.vink.forum"), ("environment", "test"),
                           ("source_sha", "develop"), ("images", spec(A)["images"] | {"web": "wrong"})):
            with self.subTest(key=key), self.assertRaises(ValueError):
                art.validate(spec(A) | {key: value})
        with TemporaryDirectory() as tmp:
            root = Path(tmp)
            package(root, A)
            pkg = root / A / "pkg"
            (pkg / "vink.js").unlink()
            with self.assertRaisesRegex(ValueError, "missing"):
                art.inventory(pkg)
            (pkg / "vink.js").symlink_to(pkg / "vink.css")
            with self.assertRaisesRegex(ValueError, "symlink"):
                art.inventory(pkg)
            (pkg / "vink.js").unlink()
            with (pkg / "vink.js").open("wb") as f:
                f.truncate(art.MAX_FILE + 1)
            with self.assertRaisesRegex(ValueError, "limit"):
                art.inventory(pkg)

    def test_schema_review_checks_both_histories_and_direction(self):
        with TemporaryDirectory() as tmp:
            root = Path(tmp)
            evidence = {"from": A, "to": B, "action": "expand"}
            for owner in ("vink-app", "vink-identity"):
                evidence[owner] = {"old_on_result": "PASS", "new_on_result": "PASS", "rehearsal": "scratch read/write"}
                for rev in (A, B):
                    path = root / rev / "services" / owner / "migrations"
                    path.mkdir(parents=True)
                    (path / "1_initial.sql").write_text("first")
                (path / "2_add.sql").write_text("add")
            review = root / "review.json"
            review.write_bytes(art.encode(evidence))
            self.assertEqual(release.schema_action(A, B, review, root), "expand")
            review.write_bytes(art.encode(evidence | {"from": B, "to": A, "action": "keep"}))
            self.assertEqual(release.schema_action(B, A, review, root), "keep")
            (path / "1_initial.sql").write_text("drift")
            with self.assertRaisesRegex(ValueError, "divergent"):
                release.schema_action(B, A, review, root)

    def test_wrong_image_revision_stops_before_export(self):
        web = spec(A)["images"]["web"]
        wrong = {"RepoDigests":[web], "Labels":{"org.opencontainers.image.revision":B},
                 "Os":"linux", "Architecture":"amd64"}
        with patch.object(release, "run", side_effect=["",json.dumps([wrong])]) as commands:
            with self.assertRaisesRegex(ValueError, "provenance"):
                release.export(spec(A), Path("/unused"))
        self.assertEqual(commands.call_count,2)

    def test_retirement_refuses_dependencies_and_shared_resources(self):
        items = [{"kind": k, "id": {"test-host": "test.vink.forum", "test-origin": "origin-test.vink.forum"}.get(k,k),
                  "evidence": "observed", "owner": "test", "callers": [],
                  "action": "restrict" if k in {"test-host","test-origin","pages-project","pages-url","custom-domain"}
                            else "retain"} for k in retirement.KINDS]
        inventory = {"public_origin":"https://vink.forum","production_receipt":"receipt", "resources":items}
        retirement.validate(inventory)
        origin = next(i for i in items if i["kind"] == "test-origin")
        origin["callers"] = ["consumer"]
        with self.assertRaisesRegex(ValueError, "callers"):
            retirement.validate(inventory)
        origin.update(callers=[],owner="shared")
        with self.assertRaisesRegex(ValueError, "preserve"):
            retirement.validate(inventory)


class Recovery(unittest.TestCase):
    def test_lost_production_ack_reads_remote_before_switch(self):
        with TemporaryDirectory() as tmp, contextlib.ExitStack() as stack:
            root = Path(tmp)
            receipt = root / "request.json"
            receipt.write_bytes(art.encode(spec(B)))
            manifest = package(root / "archives", B)
            remote = {"canonical_deployment": None}
            calls = []
            def invoke(argv, **kwargs):
                calls.append(list(map(str, argv)))
                if "--branch" in argv and argv[argv.index("--branch")+1] == "main":
                    remote["canonical_deployment"] = {"id":"actual-id", "environment":"production",
                        "latest_stage":{"status":"success"}, "url":"https://123.vink-production.pages.dev",
                        "deployment_trigger":{"metadata":{"commit_hash":B, "commit_message":argv[-1]}}}
                    raise OSError("lost ACK")
            def local(*argv):
                calls.append(list(map(str,argv)))
                return B if "rev-parse" in argv else ""
            for name, value in {"STATE":root, "WRANGLER":receipt, "project":lambda _:remote,
                                "export":lambda *_:manifest, "verify_assets":lambda *_:None,
                                "browser_check":lambda *_:None, "run":local}.items():
                stack.enter_context(patch.object(release,name,value))
            stack.enter_context(patch.object(release.subprocess,"run", side_effect=invoke))
            stack.enter_context(patch.object(release.pwd,"getpwnam",return_value=type("User",(),{"pw_uid":501})()))
            stack.enter_context(patch.dict(os.environ, {"CLOUDFLARE_ACCOUNT_ID":ACCOUNT,"VINK_RELEASE_CHROME":"/bin/sh"}))
            args = [str(receipt), "--account",ACCOUNT,"--previous-origin",A,"--previous-edge","none",
                    "--reader","33333333-3333-4333-8333-333333333333"]
            def execute(mode):
                with patch("sys.argv",["release.py",mode,*args]),contextlib.redirect_stdout(io.StringIO()):
                    release.main()
            with self.assertRaisesRegex(OSError,"lost ACK"):
                execute("apply")
            pending = json.loads((root / "pending.json").read_bytes())
            self.assertEqual(pending["phase"],"production_upload_started")
            self.assertFalse(any("apply" in c for c in calls))
            with self.assertRaisesRegex(ValueError,"pending"):
                execute("apply")
            active = remote["canonical_deployment"]
            remote["canonical_deployment"] = None
            with self.assertRaisesRegex(ValueError,"not active"):
                execute("resume")
            remote["canonical_deployment"] = active
            execute("resume")
            self.assertEqual(sum("--branch" in c for c in calls),2)
            self.assertEqual(sum("apply" in c for c in calls),1)
            self.assertEqual(json.loads((root/"active.json").read_bytes())["origin_release"],B)
            self.assertFalse((root/"pending.json").exists())
