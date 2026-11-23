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


class Maintenance(unittest.TestCase):
    def test_incompatible_schema_requires_explicit_opt_in_and_restore(self):
        with TemporaryDirectory() as tmp:
            root = Path(tmp)
            evidence = {"from": A, "to": B, "action": "maintenance"}
            for owner in ("vink-app", "vink-identity"):
                evidence[owner] = {"old_on_result": "PASS", "new_on_result": "PASS",
                                   "old_on_restored": "PASS", "rehearsal": "scratch"}
                for rev in (A, B):
                    path = root / rev / "services" / owner / "migrations"
                    path.mkdir(parents=True)
                    (path / "1_initial.sql").write_text("first")
            app = root / B / "services/vink-app/migrations"
            (app / "2_add.sql").write_text("add")
            evidence["vink-app"]["old_on_result"] = "FAIL"
            review = root / "review.json"
            review.write_bytes(art.encode(evidence))
            self.assertEqual(release.schema_action(A, B, review, root, window=True), "maintenance")
            with self.assertRaisesRegex(ValueError, "opt-in"):
                release.schema_action(A, B, review, root)
            review.write_bytes(art.encode(evidence | {"action": "expand"}))
            with self.assertRaisesRegex(ValueError, "both binaries"):
                release.schema_action(A, B, review, root)
            evidence["vink-app"]["old_on_restored"] = "FAIL"
            review.write_bytes(art.encode(evidence))
            with self.assertRaisesRegex(ValueError, "restored predecessor"):
                release.schema_action(A, B, review, root, window=True)

    def test_bootstrap_is_observed_and_bound_to_existing_assets(self):
        with TemporaryDirectory() as tmp:
            root = Path(tmp)
            manifest = package(root, A)
            receipt = root / "bootstrap.json"
            receipt.write_bytes(art.encode({"pages_deployment": "existing-id", "source_sha": A,
                "project": art.PROJECT, "public_origin": manifest["public_origin"], "images": manifest["images"],
                "asset_digest": manifest["asset_digest"]}))
            active = {"id": "existing-id", "environment": "production", "latest_stage": {"status": "success"},
                      "deployment_trigger": {"metadata": {"commit_hash": A}},
                      "url": "https://abcd.vink-production.pages.dev"}
            with patch.object(release, "verify_assets") as verify:
                self.assertEqual(release.bootstrap_baseline(receipt, active["id"], A, "linux/amd64", root, active),
                                 manifest["images"])
                self.assertEqual([call.args[0] for call in verify.call_args_list],
                                 [active["url"], "https://vink.forum"])
            (root / A / "pkg/vink.js").write_text("drift")
            with self.assertRaisesRegex(ValueError, "retained bytes"):
                release.bootstrap_baseline(receipt, active["id"], A, "linux/amd64", root, active)

    def test_open_intent_is_durable_and_can_only_resume_forward(self):
        with TemporaryDirectory() as tmp:
            journal = Path(tmp) / "pending.json"
            result = {"phase": "origin_switch_started", "previous_origin": A}
            ops = []

            class Stub:
                def operate(self, command, _):
                    ops.append(command)
                    if command == "open" and ops.count("open") == 1:
                        self.assert_durable_open()
                        raise OSError("lost open ACK")

                def assert_durable_open(self):
                    assert json.loads(journal.read_bytes())["phase"] == "origin_opening"

            origin = Stub()
            self.assertFalse(release.advance_origin(origin, journal, result, "apply", {}))
            self.assertEqual(ops, ["prepare"])
            self.assertEqual(json.loads(journal.read_bytes())["phase"], "origin_prepared")
            self.assertFalse(release.advance_origin(origin, journal, result, "resume", {}))
            with self.assertRaisesRegex(OSError, "lost open ACK"):
                release.advance_origin(origin, journal, result, "open", {})
            with self.assertRaisesRegex(ValueError, "may have accepted writes"):
                release.advance_origin(origin, journal, result, "rollback-origin", {})
            self.assertTrue(release.advance_origin(origin, journal, result, "resume", {}))
            self.assertEqual(ops, ["prepare", "open", "open"])

    def unstarted_origin(self, root, stack):
        origin = release.maintenance.Origin.__new__(release.maintenance.Origin)
        origin.old, origin.new = A, B
        origin.spec, origin.old_images = spec(B), spec(A)["images"]
        origin.work = root / "maintenance"
        origin.state_path = origin.work / "state.json"
        origin.before = {"vink": {"1": "old-app"}, "vink_identity": {"1": "old-identity"}}
        env = root / "release.env"
        env.write_text("".join(
            f"VINK_PROD_{var}_IMAGE={origin.old_images[name]}\n"
            for var, name in release.maintenance.SERVICES.values()))
        stack.enter_context(patch.object(release.maintenance, "ENV", env))
        stack.enter_context(patch.object(release.maintenance, "GUARD", root / "fence"))
        stack.enter_context(patch.object(release.maintenance, "BACKUPS", root))
        stack.enter_context(patch.object(release.maintenance, "boot_fence"))
        origin.db_oids = stack.enter_context(patch.object(origin, "db_oids", return_value={"vink": 1, "vink_identity": 2}))
        origin.check_ledgers = stack.enter_context(patch.object(origin, "check_ledgers"))
        origin.observe = stack.enter_context(patch.object(origin, "observe", return_value=True))
        return origin, env

    def test_rollback_before_origin_state_observes_predecessor_and_resumes(self):
        for saved in (False, True):
            with self.subTest(saved_env_recorded=saved), TemporaryDirectory() as tmp, contextlib.ExitStack() as stack:
                root = Path(tmp)
                origin, env = self.unstarted_origin(root, stack)
                if saved:
                    origin.work.mkdir()
                    (origin.work / "release-before.env").write_bytes(env.read_bytes())
                journal = root / "pending.json"
                result = {"phase": "origin_switch_started", "edge": "new-asset-bundle", "previous_origin": A}
                release.store(journal, result)
                operate = stack.enter_context(patch.object(origin, "operate", wraps=origin.operate))

                self.assertFalse(release.advance_origin(origin, journal, result, "rollback-origin", {}))
                durable_state = json.loads(journal.read_bytes())
                self.assertEqual(durable_state, {"phase": "origin_rolled_back", "edge": "new-asset-bundle",
                                                 "previous_origin": A, "origin_release": A})
                self.assertFalse(release.advance_origin(origin, journal, durable_state, "resume", {}))
                self.assertFalse(release.advance_origin(origin, journal, durable_state, "rollback-origin", {}))
                origin.observe.assert_called_once_with(A, origin.old_images)
                origin.check_ledgers.assert_called_once_with(origin.before)
                operate.assert_not_called()
                self.assertFalse(origin.state_path.exists())
                self.assertFalse((root / "fence").exists())

    def test_rollback_conflict_leaves_controller_phase_resumable(self):
        cases = ("guard", "env", "snapshot", "copy", "ledger", "readiness", "missing-prepared", "already-opening")
        for cause in cases:
            with self.subTest(cause=cause), TemporaryDirectory() as tmp, contextlib.ExitStack() as stack:
                root = Path(tmp)
                origin, env = self.unstarted_origin(root, stack)
                journal = root / "pending.json"
                result = {"phase": "origin_prepared" if cause == "missing-prepared" else "origin_switch_started",
                          "previous_origin": A}
                release.store(journal, result)
                if cause == "guard":
                    (root / "fence").write_text(B)
                elif cause == "env":
                    env.write_text(env.read_text().replace("VINK_PROD_APP_IMAGE=", "VINK_PROD_APP_IMAGE=drift-"))
                elif cause == "snapshot":
                    origin.work.mkdir()
                    (origin.work / "backup").mkdir()
                elif cause == "copy":
                    origin.db_oids.return_value |= {"vink_saved": 3}
                elif cause == "ledger":
                    origin.check_ledgers.side_effect = ValueError("vink migration history differs")
                elif cause == "readiness":
                    origin.observe.return_value = False
                elif cause == "already-opening":
                    origin.work.mkdir()
                    release.store(origin.state_path, {"from": A, "release": spec(B), "old_images": spec(A)["images"],
                                                      "review": {}, "phase": "opening"})
                before = journal.read_bytes()
                operate = stack.enter_context(patch.object(origin, "operate", wraps=origin.operate))
                with self.assertRaises(ValueError):
                    release.advance_origin(origin, journal, result, "rollback-origin", {})
                self.assertEqual(journal.read_bytes(), before)
                operate.assert_not_called()
                if cause == "guard":
                    # Once the conflicting marker is reconciled, resume can still prepare.
                    (root / "fence").unlink()
                    operate.side_effect = lambda command, _: self.assertEqual(command, "prepare")
                    self.assertFalse(release.advance_origin(origin, journal, result, "resume", {}))
                    self.assertEqual(json.loads(journal.read_bytes())["phase"], "origin_prepared")

    def test_rollback_intent_is_durable_and_preserves_combined_edge(self):
        with TemporaryDirectory() as tmp:
            journal = Path(tmp) / "pending.json"
            result = {"phase": "origin_prepared", "edge": "new-asset-bundle", "previous_origin": A}
            calls = []

            class Stub:
                def rollback_started(self, _, *, prepared):
                    assert prepared
                    return True

                def operate(self, command, _):
                    assert command == "rollback"
                    assert json.loads(journal.read_bytes())["phase"] == "origin_rollback_started"
                    calls.append(command)
                    if len(calls) == 1:
                        raise OSError("lost rollback ACK")

            origin = Stub()
            with self.assertRaisesRegex(OSError, "lost rollback ACK"):
                release.advance_origin(origin, journal, result, "rollback-origin", {})
            durable_state = json.loads(journal.read_bytes())
            self.assertFalse(release.advance_origin(origin, journal, durable_state, "resume", {}))
            self.assertEqual(json.loads(journal.read_bytes()), {"phase": "origin_rolled_back",
                                  "edge": "new-asset-bundle", "origin_release": A, "previous_origin": A})
            self.assertFalse(release.advance_origin(origin, journal, durable_state, "resume", {}))
            self.assertEqual(calls, ["rollback", "rollback"])

    def test_unit_overlay_preserves_mixed_live_compose_labels(self):
        from maintenance import CURRENT, observed_overlay

        paths = [str(CURRENT / value) for value in ("services/postgres/compose.yaml",
                 "infra/production/compose.yaml", "infra/production/auth-resend.compose.yaml")]
        unit = "podman compose " + " ".join("-f " + path for path in paths)
        base = ",".join(paths[:2])
        labels = {"app": base, "worker": base, "identity": ",".join(paths)}
        flags = {"app": {"VINK_AUTH_SIGN_IN": "enabled"}, "worker": {"VINK_AUTH_SIGN_IN": "disabled"},
                 "identity": {"VINK_IDENTITY_AUTHORITY": "native", "VINK_IDENTITY_MAIL_TRANSPORT": "resend-api"}}
        env = b"VINK_PROD_AUTH_SIGN_IN=enabled\n"
        self.assertTrue(observed_overlay(unit, labels, flags, env))
        with self.assertRaisesRegex(ValueError, "Compose labels"):
            observed_overlay(unit, labels | {"identity": base}, flags, env)
        with self.assertRaisesRegex(ValueError, "startup Compose"):
            observed_overlay(unit + " -f /unexpected.yaml", labels, flags, env)
