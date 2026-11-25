import contextlib
import io
import json
import os
from pathlib import Path
import subprocess
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch

import artifacts as art
import release
from test_release import B, ACCOUNT, package, spec


APEX = "https://vink.forum"
A = "e98725569657502e5701afeb3499c8b87ed1d84d"
LEGACY_ASSET = "expressions/pilot.js"
OLD_EDGE = "11111111-1111-4111-8111-111111111111"
NEW_EDGE = "22222222-2222-4222-8222-222222222222"
ROLLBACK_EDGE = "33333333-3333-4333-8333-333333333333"
READER = "44444444-4444-4444-8444-444444444444"
REASON = "Operator approved HTTP smoke for this release"


class HttpOnlyRelease(unittest.TestCase):
    def setUp(self):
        self.root = Path(self.enterContext(TemporaryDirectory()))
        self.active = self.root / "active.json"
        self.journal = self.root / "pending.json"
        self.receipt = self.root / "request.json"
        self.archive = self.root / "archives"
        self.manifests = {rev: package(self.archive, rev) for rev in (A, B)}
        legacy = self.archive / A / "pkg" / LEGACY_ASSET
        legacy.parent.mkdir()
        legacy.write_text(A + LEGACY_ASSET)
        self.manifests[A] = art.record(legacy.parents[1], spec(A), retained=True)
        (self.archive / A / "manifest.json").write_bytes(art.encode(self.manifests[A]))
        self.prior = {"edge": OLD_EDGE, "account": ACCOUNT, "origin_release": A, "legacy_assets": A,
                      "release": spec(A), "retained": {A: art.sha(art.encode(self.manifests[A]))}}
        self.active.write_bytes(art.encode(self.prior))
        self.remote = {"id": OLD_EDGE, "url": "https://11111111.vink-production.pages.dev"}
        self.preview = None
        self.live = A
        self.select_release(B, OLD_EDGE)
        self.calls = []
        self.http_error = None
        self.catalog = {"works": [], "next_cursor": None}
        self.compatible = True
        self.change_edge = False
        for name, value in {
            "STATE": self.root, "WRANGLER": self.receipt,
            "project": lambda _: {"canonical_deployment": self.remote},
            "pages_read": self.pages_read, "read_asset": self.read_asset,
            "export": self.export,
            "run": self.invoke_origin, "publish": self.publish,
        }.items():
            self.enterContext(patch.object(release, name, value))
        self.enterContext(patch.object(release.subprocess, "run", side_effect=self.http_smoke))
        self.enterContext(patch.dict(os.environ, {"CLOUDFLARE_ACCOUNT_ID": ACCOUNT}, clear=True))
        self.browser = self.enterContext(patch.object(release, "browser_check"))
        self.preflight = self.enterContext(patch.object(release, "browser_preflight"))

    def select_release(self, rev, previous_edge, *, controller=None):
        self.candidate, self.predecessor, self.previous_edge = rev, self.live, previous_edge
        self.controller = controller or rev
        self.receipt.write_bytes(art.encode(spec(rev)))

    def execute(self, mode, *extra, reason=REASON, reader=None, previous_edge=None):
        args = ["release.py", mode, str(self.receipt), "--account", ACCOUNT,
                "--previous-origin", self.predecessor, "--previous-edge", previous_edge or self.previous_edge]
        args.extend(("--reader", reader) if reader else ("--empty-catalog",))
        if reason is not None:
            args.extend(("--http-only", reason))
        with patch("sys.argv", [*args, *extra]), contextlib.redirect_stdout(io.StringIO()) as output:
            release.main()
        return json.loads(output.getvalue())

    def pages_read(self, account, suffix):
        self.assertEqual((account, suffix), (ACCOUNT, "/deployments?env=preview"))
        return [self.preview]

    def read_asset(self, url):
        if url == APEX + "/api/v1/works":
            return "200", "application/json", "", "", art.encode(self.catalog)
        if url == f"{APEX}/api/v1/works/{READER}/reader":
            return "200", "application/json", "", "", art.encode({"status": "published", "work_id": READER})
        host, asset = url.split("/_v/")
        rev, name = asset.split("/pkg/")
        self.calls.append(("asset", host, rev))
        return "200", art.TYPES[Path(name).suffix], rev, "inventory/r26", (rev + name).encode()

    def invoke_origin(self, *args):
        if "rev-parse" in args:
            return self.controller
        if str(args[0]).endswith("/origin.sh"):
            mode, old, rev, web, worker, identity, platform, schema = args[1:]
            images = spec(rev)["images"]
            self.assertEqual((web, worker, identity), (images["web"], images["worker"], images["identity"]))
            self.assertEqual((rev, platform, schema), (self.candidate, "linux/amd64", "same"))
            self.assertEqual(old, self.live)
            self.calls.append(("origin", mode))
            if mode == "plan" and not self.compatible:
                raise subprocess.CalledProcessError(1, args, stderr=b"migration tree differs")
            if mode == "apply":
                self.assertEqual(json.loads(self.journal.read_bytes())["phase"], "origin_switch_started")
                self.live = rev
        return ""

    def export(self, target, archive, *, retained=False):
        rev = target["source_sha"]
        manifest = art.record(archive / rev / "pkg", target, retained=retained)
        self.assertEqual(manifest, self.manifests[rev])
        return manifest

    def publish(self, public, branch, rev, marker):
        self.assertEqual(rev, self.candidate)
        for retained in (A, B):
            self.assertEqual((public / "_v" / retained / "pkg/vink.js").read_text(), retained + "vink.js")
        self.assertEqual((public / "_v" / A / "pkg" / LEGACY_ASSET).read_text(), A + LEGACY_ASSET)
        self.calls.append(("publish", branch))
        deployment_id = NEW_EDGE if rev == B else ROLLBACK_EDGE
        is_preview = branch != "main"
        if is_preview:
            deployment_id = deployment_id[:8] + "-7777-4777-8777-777777777777"
        deployed = {"id": deployment_id, "url": f"https://{deployment_id[:8]}.vink-production.pages.dev",
                    "environment": "preview" if is_preview else "production", "latest_stage": {"status": "success"},
                    "deployment_trigger": {"metadata": {"branch": branch, "commit_hash": rev,
                                                       "commit_message": marker}}}
        if is_preview:
            deployed["aliases"] = [f"https://{branch}.vink-production.pages.dev"]
            self.preview = deployed
        else:
            self.remote = deployed

    def http_smoke(self, command, *, check):
        self.assertTrue(check)
        self.assertEqual(command, [release.HERE.parent.parent / "production/smoke-edge.sh", "edge",
                                   "--release", self.candidate])
        self.assertEqual(self.live, self.candidate)
        self.calls.append(("http", self.live))
        if self.http_error:
            raise self.http_error
        if self.change_edge:
            self.remote = {**self.remote, "id": OLD_EDGE}

    def test_http_only_checks_origin_and_http_before_finishing_and_replays_neither_write(self):
        coverage = {"status": "NOT_RUN", "reason": REASON}
        plan = self.execute("plan")
        self.assertEqual(plan["browser_coverage"], coverage)
        self.assertFalse(self.journal.exists())
        self.assertEqual(self.calls, [("origin", "plan")])
        staged = self.execute("apply", "--preview-only")
        self.assertEqual(staged["phase"], "preview_verified")
        self.assertEqual(staged["browser_coverage"], coverage)
        self.assertEqual(self.remote["id"], OLD_EDGE)
        self.assertEqual(self.live, A)
        saved = self.journal.read_bytes()
        for reason in (None, "different reason"):
            with self.subTest(reason=reason), self.assertRaisesRegex(ValueError, "original browser coverage"):
                self.execute("resume", reason=reason)
        self.assertEqual(self.journal.read_bytes(), saved)

        def wrong_preview(url):
            status, mime, rev, route, body = self.read_asset(url)
            if url.startswith(staged["preview_url"] + "/_v/"):
                body += b"changed"
            return status, mime, rev, route, body

        with patch.object(release, "read_asset", side_effect=wrong_preview), \
                self.assertRaisesRegex(ValueError, "Pages asset mismatch"):
            self.execute("resume")
        self.assertEqual(self.journal.read_bytes(), saved)
        self.compatible = False
        with self.assertRaises(subprocess.CalledProcessError):
            self.execute("resume")
        self.assertEqual(self.journal.read_bytes(), saved)
        self.assertNotIn(("publish", "main"), self.calls)
        self.compatible = True

        for error in (subprocess.CalledProcessError(1, "smoke-edge.sh"), FileNotFoundError("smoke-edge.sh")):
            self.http_error = error
            with self.subTest(error=error), self.assertRaises(type(error)):
                self.execute("resume")
            self.assertEqual(json.loads(self.journal.read_bytes())["phase"], "origin_ready")
            self.assertEqual(json.loads(self.active.read_bytes()), self.prior)

        self.http_error = None
        candidate_edge = self.remote
        self.change_edge = True
        with self.assertRaisesRegex(ValueError, "Pages changed during release smoke"):
            self.execute("resume")
        self.assertEqual(json.loads(self.journal.read_bytes())["phase"], "http_verified")
        self.assertEqual(json.loads(self.active.read_bytes()), self.prior)
        self.remote = candidate_edge
        self.change_edge = False

        result = self.execute("resume")
        self.assertEqual(result["phase"], "http_only_verified")
        self.assertEqual(result["browser_coverage"], coverage)
        self.assertEqual(result["reader_coverage"], release.EMPTY_CATALOG)
        self.assertNotIn("browser_receipt_sha256", result)
        self.assertEqual(json.loads(self.active.read_bytes()), result)
        self.assertFalse(self.journal.exists())
        self.assertEqual(self.calls.count(("publish", "main")), 1)
        self.assertEqual(self.calls.count(("origin", "apply")), 1)
        promote = self.calls.index(("publish", "main"))
        switch = self.calls.index(("origin", "apply"))
        http = self.calls.index(("http", B))
        self.assertLess(promote, switch)
        self.assertLess(switch, http)
        self.assertIn(("asset", staged["preview_url"], B), self.calls[:promote])
        self.assertIn(("asset", staged["preview_alias"], B), self.calls[:promote])
        self.assertIn(("asset", APEX, B), self.calls[:promote])
        self.assertIn(("asset", candidate_edge["url"], B), self.calls[promote:switch])
        self.browser.assert_not_called()
        self.preflight.assert_not_called()

    def test_default_requires_browser_and_http_only_rejects_unsupported_schema_or_commands(self):
        with self.assertRaisesRegex(KeyError, "VINK_RELEASE_CHROME"):
            self.execute("apply", "--preview-only", reason=None)
        for mode, extra, edge in (
            ("apply", ("--maintenance",), OLD_EDGE),
            ("apply", ("--schema-review", "review.json"), OLD_EDGE),
            ("apply", ("--browser-host", "runner"), OLD_EDGE),
            ("apply", (), "none"),
            ("rollback-origin", (), OLD_EDGE),
        ):
            with self.subTest(mode=mode, extra=extra, edge=edge), self.assertRaisesRegex(ValueError, "HTTP-only requires"):
                self.execute(mode, *extra, previous_edge=edge)
        for reason in ("", " ", "two\nlines", "x" * 241):
            with self.subTest(reason=reason), self.assertRaisesRegex(ValueError, "single-line reason"):
                self.execute("plan", reason=reason)
        self.compatible = False
        with self.assertRaises(subprocess.CalledProcessError):
            self.execute("apply", "--preview-only")
        self.assertEqual(self.calls, [("origin", "plan")])
        self.assertFalse(self.journal.exists())

    def test_completed_http_only_receipt_survives_lost_ack_without_replaying(self):
        self.execute("apply", "--preview-only")
        self.catalog = {"works": [{"work_id": READER}], "next_cursor": None}
        with self.assertRaisesRegex(ValueError, "empty public catalog"):
            self.execute("resume")
        self.assertEqual(self.remote["id"], OLD_EDGE)
        unlink = Path.unlink

        def lose_ack(path, *args, **kwargs):
            if path == self.journal:
                raise OSError("lost journal cleanup")
            return unlink(path, *args, **kwargs)

        with patch.object(Path, "unlink", lose_ack), self.assertRaisesRegex(OSError, "lost journal cleanup"):
            self.execute("resume", "--resume-with-reader", READER)
        saved = json.loads(self.active.read_bytes())
        self.assertEqual(saved["phase"], "http_only_verified")
        self.assertEqual(saved["reader_resume_work"], READER)
        self.assertEqual(saved["reader_coverage"], release.PUBLISHED_READER_HTTP)
        self.assertEqual(json.loads(self.journal.read_bytes()), saved)
        before = self.calls.copy()
        self.assertEqual(self.execute("resume", "--resume-with-reader", READER), saved)
        self.assertEqual(self.calls, before)
        self.assertFalse(self.journal.exists())

    def test_rolling_rollback_stages_both_releases_and_verifies_the_retained_revision(self):
        self.execute("apply", "--preview-only")
        self.execute("resume")
        self.select_release(A, NEW_EDGE, controller=B)
        rollback = "--rollback-predecessor"
        staged = self.execute("apply", rollback, "--preview-only", reader=READER)
        self.assertEqual(staged["http_reader"], READER)
        self.assertEqual(self.live, B)
        with self.assertRaisesRegex(ValueError, "original rollback mode"):
            self.execute("resume", reader=READER)
        with self.assertRaisesRegex(ValueError, "original HTTP Reader"):
            self.execute("resume", rollback, reader=OLD_EDGE)
        result = self.execute("resume", rollback, reader=READER)
        self.assertEqual(result["phase"], "http_only_verified")
        self.assertEqual((result["edge"], result["origin_release"], result["http_reader"]), (ROLLBACK_EDGE, A, READER))
        self.assertEqual(set(result["retained"]), {A, B})
        self.assertIn(("http", A), self.calls)
        self.assertFalse(self.journal.exists())
        self.browser.assert_not_called()
        self.preflight.assert_not_called()

    def test_current_controller_restores_predecessor_after_http_failure(self):
        self.execute("apply", "--preview-only")
        self.http_error = subprocess.CalledProcessError(1, "smoke-edge.sh")
        with self.assertRaises(subprocess.CalledProcessError):
            self.execute("resume")
        forward = self.journal.read_bytes()
        self.assertEqual(json.loads(forward)["phase"], "origin_ready")
        self.http_error = None
        self.select_release(A, NEW_EDGE, controller=B)
        rollback = "--rollback-predecessor"
        self.execute("plan", rollback)
        self.assertEqual(self.journal.read_bytes(), forward)
        self.controller = A
        with self.assertRaisesRegex(ValueError, "checkout differs"):
            self.execute("apply", rollback, "--preview-only")
        self.controller = B
        wrong = spec(A)
        wrong["images"]["web"] = wrong["images"]["web"].replace("e" * 64, "f" * 64)
        self.receipt.write_bytes(art.encode(wrong))
        with self.assertRaisesRegex(ValueError, "active predecessor receipt"):
            self.execute("apply", rollback, "--preview-only")
        self.receipt.write_bytes(art.encode(spec(A)))
        staged = self.execute("apply", rollback, "--preview-only")
        fingerprint = art.sha(forward)
        self.assertEqual(staged["rollback_from"], {"sha256": fingerprint, "interrupted": True})
        backup = self.root / "incidents" / (fingerprint + ".http-rollback.json")
        self.assertEqual(backup.read_bytes(), forward)
        backup.write_bytes(forward + b" ")
        with self.assertRaisesRegex(ValueError, "rollback source changed"):
            self.execute("resume", rollback)
        backup.write_bytes(forward)
        result = self.execute("resume", rollback)
        self.assertEqual((result["phase"], result["edge"], self.live), ("http_only_verified", ROLLBACK_EDGE, A))
        self.assertEqual(backup.read_bytes(), forward)
        for host in (staged["preview_url"], staged["preview_alias"], self.remote["url"], APEX):
            self.assertIn(("asset", host, A), self.calls)
            self.assertIn(("asset", host, B), self.calls)
        self.assertEqual(self.calls.count(("origin", "apply")), 2)
        self.assertFalse(self.journal.exists())
        self.browser.assert_not_called()
        self.preflight.assert_not_called()
