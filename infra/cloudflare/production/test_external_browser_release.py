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
from test_release import A, B, ACCOUNT, package, spec


PREVIOUS = "11111111-1111-4111-8111-111111111111"
PREVIEW = "22222222-2222-4222-8222-222222222222"
EDGE = "33333333-3333-4333-8333-333333333333"
APEX = "https://vink.forum"
PROOF = "f" * 64


class ExternalBrowserRelease(unittest.TestCase):
    def test_resume_preserves_release_guards_and_does_not_replay_mutations(self):
        self.exercise_release()

    def test_maintenance_preserves_explicit_open_and_resumes_after_browser_failure(self):
        self.exercise_release(maintenance=True)

    def test_maintenance_can_roll_back_while_runner_is_offline(self):
        self.exercise_release(maintenance=True, rollback=True)

    def exercise_release(self, *, maintenance=False, rollback=False):
        with TemporaryDirectory() as tmp, contextlib.ExitStack() as stack:
            state = Path(tmp)
            receipt, journal, active = [state / leaf for leaf in ("request.json", "pending.json", "active.json")]
            receipt.write_bytes(art.encode(spec(B)))
            archive = state / "archives"
            old, new = package(archive, A), package(archive, B)
            prior = {"edge": PREVIOUS, "account": ACCOUNT, "origin_release": A,
                     "legacy_assets": A, "release": spec(A), "retained": {A: art.sha(art.encode(old))}}
            active.write_bytes(art.encode(prior))
            preview = dict(preview_deployment=PREVIEW, preview_url="https://22222222.vink-production.pages.dev",
                           preview_alias=f"https://r-{B[:26]}.vink-production.pages.dev")
            remote = {"canonical_deployment": {"id": PREVIOUS, "url": "https://11111111.vink-production.pages.dev"}}
            external = stack.enter_context(patch.object(release, "RemoteBrowser")).return_value
            binding = {"host": "mac-smoke", "ssh_config_sha256": "a" * 64, "known_hosts_sha256": "b" * 64}
            external.binding = binding
            mode_flags = []
            calls = []
            if maintenance:
                review = state / "schema-review.json"
                proof_file = state / "fallback-proof.json"
                review.write_bytes(art.encode({"images": {"from": spec(A)["images"], "to": spec(B)["images"]}}))
                mode_flags = ["--maintenance", "--schema-review", str(review), "--fallback-proof", str(proof_file)]
                origin = stack.enter_context(patch.object(release.maintenance, "Origin")).return_value
                origin.rollback_started.return_value = True
                fallback = stack.enter_context(patch.object(release, "fallback_proof", return_value=PROOF))
                stack.enter_context(patch.object(release, "schema_action", return_value="maintenance"))

                def operate(phase, _evidence):
                    expected = {"prepare": "origin_switch_started", "open": "origin_opening",
                                "rollback": "origin_rollback_started"}
                    self.assertEqual(json.loads(journal.read_bytes())["phase"], expected[phase])
                    calls.append(("maintenance " + phase,))

                origin.operate.side_effect = operate
            available = False
            accept_smoke = False
            change_edge = False
            published = None

            def remote_check(phase, revision, **kwargs):
                self.assertEqual(revision, B)
                calls.append(("browser " + phase, kwargs))
                if phase == "preflight" and not available:
                    raise OSError("runner offline")
                if phase == "smoke":
                    self.assertEqual(kwargs, dict(reader=None, preview=PREVIEW, edge=EDGE))
                    self.assertEqual(json.loads(journal.read_bytes())["phase"], "http_verified")
                    self.assertEqual(json.loads(active.read_bytes())["origin_release"], A)
                    if not accept_smoke:
                        raise ValueError("browser has not verified")
                    if change_edge:
                        remote["canonical_deployment"] = {**published, "id": PREVIOUS}
                return PROOF

            def invoke_origin(*argv):
                if "rev-parse" in argv:
                    return B
                if str(argv[0]).endswith("/origin.sh"):
                    calls.append(("origin " + argv[1],))
                return ""

            def publish(_public, branch, rev, marker):
                nonlocal published
                self.assertEqual(rev, B)
                calls.append(("publish", branch))
                if branch == "main":
                    published = {"id": EDGE, "url": "https://33333333.vink-production.pages.dev",
                                 "environment": "production", "latest_stage": {"status": "success"},
                                 "deployment_trigger": {"metadata": {"commit_hash": B, "commit_message": marker}}}
                    remote["canonical_deployment"] = published

            for name, value in {
                "STATE": state, "WRANGLER": receipt, "project": lambda _: remote, "export": lambda *_: new,
                "run": invoke_origin, "publish": publish, "preview_receipt": lambda *a: preview,
                "verify_catalog_smoke": lambda reader: calls.append(("catalog", reader)),
                "assert_empty_catalog": lambda: calls.append(("empty catalog",)),
                "verify_assets": lambda host, *_a, **_kw: calls.append(("assets", host)),
                "verify_asset": lambda host, *_a, **_kw: calls.append(("asset", host)),
            }.items():
                stack.enter_context(patch.object(release, name, value))
            local_browser = stack.enter_context(patch.object(release, "browser_check"))
            local_preflight = stack.enter_context(patch.object(release, "browser_preflight"))
            stack.enter_context(patch.object(release.subprocess, "run",
                                             side_effect=lambda *_a, **_k: calls.append(("http smoke",))))
            stack.enter_context(patch.dict(os.environ, {"CLOUDFLARE_ACCOUNT_ID": ACCOUNT,
                "VINK_RELEASE_BROWSER_SSH_CONFIG": "/operator/config",
                "VINK_RELEASE_BROWSER_KNOWN_HOSTS": "/operator/known_hosts"}, clear=True))
            external.check.side_effect = remote_check

            def execute(mode, *flags, external_mode=True):
                args = ["release.py", mode, str(receipt), "--account", ACCOUNT, "--previous-origin", A,
                        "--previous-edge", PREVIOUS, "--empty-catalog", *mode_flags, *flags]
                if external_mode:
                    args += ["--browser-host", "mac-smoke"]
                with patch("sys.argv", args), contextlib.redirect_stdout(io.StringIO()) as output:
                    release.main()
                return json.loads(output.getvalue())

            with self.assertRaisesRegex(OSError, "runner offline"):
                execute("apply", "--preview-only")
            self.assertFalse(journal.exists())
            available = True
            self.assertEqual(execute("plan")["browser_runner"], binding)
            self.assertEqual(execute("apply", "--preview-only")["phase"], "preview_verified")
            staged = journal.read_bytes()
            with self.assertRaisesRegex(ValueError, "original browser runner"):
                execute("resume", external_mode=False)
            external.binding = {**binding, "known_hosts_sha256": "c" * 64}
            with self.assertRaisesRegex(ValueError, "original browser runner"):
                execute("resume")
            external.binding = binding
            available = False
            with self.assertRaisesRegex(OSError, "runner offline"):
                execute("resume")
            self.assertEqual(journal.read_bytes(), staged)
            self.assertNotIn(("publish", "main"), calls)

            available = True
            if maintenance:
                prepared = execute("resume")
                self.assertEqual(prepared["phase"], "origin_prepared")
                fallback.assert_called_once()
                origin.preflight.assert_called()
                prepared_journal = journal.read_bytes()
                available = False
                with self.assertRaisesRegex(OSError, "runner offline"):
                    execute("open")
                self.assertEqual(journal.read_bytes(), prepared_journal)
                self.assertNotIn(("maintenance open",), calls)
                if rollback:
                    restored = execute("rollback-origin")
                    self.assertEqual(restored["phase"], "origin_rolled_back")
                    self.assertEqual(restored["origin_release"], A)
                    self.assertIn(("maintenance rollback",), calls)
                    self.assertEqual(json.loads(active.read_bytes()), prior)
                    return
                available = True
            with self.assertRaisesRegex(ValueError, "browser has not verified"):
                execute("open" if maintenance else "resume")
            self.assertEqual(json.loads(journal.read_bytes())["phase"], "http_verified")
            if maintenance:
                with self.assertRaisesRegex(ValueError, "may have accepted writes"):
                    execute("rollback-origin")
            self.assertEqual(json.loads(active.read_bytes()), prior)
            smoke = next(i for i, entry in enumerate(calls) if entry[0] == "browser smoke")
            self.assertEqual(calls[smoke-4:smoke], [("assets", preview["preview_url"]),
                                                    ("assets", preview["preview_alias"]), ("assets", APEX),
                                                    ("catalog", None)])
            accept_smoke = change_edge = True
            with self.assertRaisesRegex(ValueError, "Pages changed during"):
                execute("resume")
            self.assertEqual(json.loads(active.read_bytes()), prior)
            remote["canonical_deployment"] = published
            change_edge = False
            result = execute("resume")
            self.assertEqual(result["phase"], "browser_verified")
            self.assertEqual(result["browser_receipt_sha256"], PROOF)
            self.assertEqual(result["reader_coverage"], release.EMPTY_CATALOG)
            self.assertEqual(json.loads(active.read_bytes()), result)
            self.assertFalse(journal.exists())
            self.assertEqual(calls.count(("publish", "main")), 1)
            self.assertEqual(calls.count(("origin apply",)), int(not maintenance))
            if maintenance:
                self.assertEqual(calls.count(("maintenance prepare",)), 1)
                self.assertEqual(calls.count(("maintenance open",)), 1)
                self.assertNotIn(("maintenance rollback",), calls)
            self.assertEqual(calls[-2:], [("origin observe",), ("catalog", None)])
            local_browser.assert_not_called()
            local_preflight.assert_not_called()
