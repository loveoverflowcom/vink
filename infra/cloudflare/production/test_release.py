import contextlib
import io
import json
import os
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from types import SimpleNamespace
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
    def test_browser_launcher_selects_correct_smoke_case(self):
        reader = "33333333-3333-4333-8333-333333333333"
        user = SimpleNamespace(pw_uid=501, pw_gid=50)
        with patch.object(release.pwd, "getpwnam", return_value=user), patch.object(release.os, "chown"), \
                patch.object(release.subprocess, "run") as launch, patch.dict(os.environ, {"VINK_RELEASE_NODE": "/node22"}):
            release.browser_check(Path("/chrome"), None, A, empty_catalog=True)
            empty = launch.call_args.args[0]
            self.assertEqual(empty[0], "/node22")
            self.assertEqual(empty[empty.index("--empty-catalog") + 1], "true")
            self.assertNotIn("--reader", empty)
            self.assertNotIn(None, empty)
            self.assertEqual(launch.call_args.kwargs["user"], user.pw_uid)
            release.browser_check(Path("/chrome"), reader, A)
            populated = launch.call_args.args[0]
            self.assertEqual(populated[populated.index("--reader") + 1], reader)
            self.assertNotIn("--empty-catalog", populated)

    def test_podman_probe_joins_only_the_running_postgres_namespace(self):
        origin = release.maintenance.Origin.__new__(release.maintenance.Origin)
        origin.postgres = "9" * 12
        with patch.object(release.maintenance, "readback", return_value="|1|") as read:
            self.assertEqual(origin.container_network(), f"container:{origin.postgres}")
            read.assert_called_once()
        namespace = "/run/netns/netns-25b063fc-4411-337a-220b-8e1aac00a5d5"
        with patch.object(release.maintenance, "readback", return_value=f"{A + A[:24]}|802400|{namespace}"), \
                patch.object(release.maintenance.os, "stat", side_effect=lambda p: {
                    namespace: SimpleNamespace(st_dev=2, st_ino=3),
                    "/proc/802400/ns/net": SimpleNamespace(st_dev=2, st_ino=3),
                }[p]) as observe:
            self.assertEqual(origin.container_network(), f"ns:{namespace}")
            observe.side_effect = lambda _: SimpleNamespace(st_dev=2, st_ino=observe.call_count)
            with self.assertRaisesRegex(ValueError, "does not match"):
                origin.container_network()

    def test_fallback_requires_two_distinct_pops_and_matching_bytes(self):
        with TemporaryDirectory() as tmp, patch.object(release.time, "time", return_value=1000):
            path = Path(tmp) / "proof.json"
            manifest = package(Path(tmp) / "archives", B)
            expected = manifest["assets"]["vink-editor.js"]
            first = {"cf_ray": "0123456789abcdef-SIN", "status": 200, "route": "fallback/r26",
                     "sha256": expected["sha256"], "mime": expected["type"], "revision": B}
            second = first | {"cf_ray": "123456789abcdef0-HKG"}
            evidence = {"predecessor": "previous", "preview_deployment": "preview", "revision": B,
                        "measured_at": 999, "asset": "vink-editor.js", "observations": [first, second]}
            current = {"previous_edge": "previous", "preview_deployment": "preview", "release": spec(B)}
            path.write_bytes(art.encode(evidence))
            self.assertEqual(release.fallback_proof(path, current, manifest)["pops"], ["HKG", "SIN"])
            for change, error in (
                ({"observations": [first, first]}, "two distinct"),
                ({"observations": [first, second | {"sha256": "wrong"}]}, "differs from manifest"),
                ({"observations": [first, second | {"route": "inventory/r26"}]}, "differs from manifest"),
                ({"measured_at": 99}, "older than"),
            ):
                path.write_bytes(art.encode(evidence | change))
                with self.subTest(change=change), self.assertRaisesRegex(ValueError, error):
                    release.fallback_proof(path, current, manifest)

    def test_pages_only_bridge_preserves_rollback_after_lost_ack(self):
        with TemporaryDirectory() as tmp, contextlib.ExitStack() as stack:
            root = Path(tmp)
            archive = root / "archives"
            manifests = {rev: package(archive, rev) for rev in (A, B)}
            retained = {rev: art.sha(art.encode(manifest)) for rev, manifest in manifests.items()}
            receipt = root / "request.json"
            receipt.touch()
            schema = root / "review.json"
            schema.write_bytes(art.encode({"rehearsed": True}))
            journal = root / "pending.json"
            failed_digest = "f" * 64
            original = art.encode({"release": spec(B), "phase": "origin_rolled_back", "project": art.PROJECT,
                                   "previous_origin": A, "origin_release": A, "edge": "failed-edge",
                                   "previous_edge": "previous", "legacy_assets": A, "account": ACCOUNT,
                                   "worker_digest": failed_digest, "retained": retained,
                                   "schema_review": art.sha(schema.read_bytes())})
            journal.write_bytes(original)

            def deployment(edge, rev, marker):
                return {"id": edge, "url": f"https://{edge}.vink-production.pages.dev",
                        "environment": "production", "latest_stage": {"status": "success"},
                        "deployment_trigger": {"metadata": {"commit_hash": rev, "commit_message": marker}}}

            remote = {"canonical_deployment": deployment("failed-edge", B, "vink-release-" + failed_digest)}
            origin = SimpleNamespace(
                before={"verified": True}, state_key=lambda evidence: evidence,
                read_state=lambda _: {"phase": "rolled_back"}, check_ledgers=lambda _: None,
                container=lambda name, running: name,
            )
            calls = []
            verified = []

            def upload(_, branch, rev, marker):
                self.assertEqual((branch, rev), ("main", A))
                calls.append(marker)
                remote["canonical_deployment"] = deployment("bridge-edge", A, marker)
                raise OSError("lost bridge ACK")

            def local(*argv):
                if "rev-parse" in argv:
                    return B
                if "origin.sh" in str(argv[0]):
                    self.assertEqual(argv[1], "observe")
                return ""

            for name, value in {"STATE": root, "WRANGLER": receipt, "project": lambda _: remote, "publish": upload,
                                "run": local, "verify_assets": lambda host, rev, *_, **__: verified.append((host, rev))}.items():
                stack.enter_context(patch.object(release, name, value))
            stack.enter_context(patch.object(release.maintenance, "GUARD", root / "guard"))
            stack.enter_context(patch.object(release.maintenance, "Origin", return_value=origin))
            stack.enter_context(patch.object(release.subprocess, "run"))
            args = SimpleNamespace(account=ACCOUNT, previous_edge="failed-edge", previous_origin=A, schema_review=schema)
            with self.assertRaisesRegex(OSError, "lost bridge ACK"):
                release.bridge(args, spec(B), remote, journal)
            self.assertEqual(journal.read_bytes(), original)
            self.assertFalse((root / "active.json").exists())
            active = remote["canonical_deployment"]
            remote["canonical_deployment"] = deployment("failed-edge", B, "vink-release-" + failed_digest)
            with self.assertRaisesRegex(ValueError, "uncertain"):
                release.bridge(args, spec(B), remote, journal)
            remote["canonical_deployment"] = active
            result = release.bridge(args, spec(B), remote, journal)
            self.assertEqual(result["phase"], "edge_bridge_verified")
            self.assertEqual(result["origin_release"], A)
            self.assertEqual((root / "incidents" / result["rollback_archive"]).read_bytes(), original)
            self.assertEqual(json.loads((root / "active.json").read_bytes()), result)
            self.assertFalse(journal.exists())
            self.assertEqual(release.bridge(args, spec(B), remote, journal), result)
            self.assertEqual(len(calls), 1)
            self.assertEqual(set(verified), {(host, rev) for host in (active["url"], spec(A)["public_origin"]) for rev in (A, B)})

    def test_preview_receipt_requires_exact_revision_and_assigned_alias(self):
        ident = "2243f07a-acfb-4225-bc62-962a976578d7"
        immutable = f"https://{ident[:8]}.vink-production.pages.dev"
        alias = f"https://r-{B[:26]}.vink-production.pages.dev"
        item = {"id": ident, "url": immutable, "environment": "preview",
                "latest_stage": {"status": "success"}, "aliases": [alias],
                "deployment_trigger": {"metadata": {"branch": f"r-{B[:26]}",
                                                    "commit_hash": B, "commit_message": "marker"}}}
        with patch.object(release, "pages_read", return_value=[item]) as read:
            self.assertEqual(release.preview_receipt(ACCOUNT, B, "marker"),
                             {"preview_deployment": ident, "preview_url": immutable, "preview_alias": alias})
            read.assert_called_once_with(ACCOUNT, "/deployments?env=preview")
            item["aliases"] = [immutable]
            with self.assertRaisesRegex(ValueError, "did not attach"):
                release.preview_receipt(ACCOUNT, B, "marker")

    def test_legacy_404_stops_before_promotion_or_origin_switch(self):
        with TemporaryDirectory() as tmp, contextlib.ExitStack() as stack:
            root = Path(tmp)
            receipt = root / "request.json"
            receipt.write_bytes(art.encode(spec(B)))
            archive = root / "archives"
            old = package(archive, A)
            manifest = package(archive, B)
            alias = f"https://r-{B[:26]}.vink-production.pages.dev"
            long_alias = f"https://r-{B}.vink-production.pages.dev"
            immutable = "https://2243f07a.vink-production.pages.dev"
            predecessor = "https://69fff7eb.vink-production.pages.dev"
            prior = {"id": "previous", "url": predecessor}
            (root / "active.json").write_bytes(art.encode(
                {"edge": prior["id"], "account": ACCOUNT, "origin_release": A, "legacy_assets": A,
                 "release": spec(A), "retained": {A: art.sha(art.encode(old))}}
            ))
            calls = []
            approved_old = False
            apex_body = None

            def local(*argv):
                calls.append(list(map(str, argv)))
                return B if "rev-parse" in argv else ""

            def fetch(url):
                host, asset = url.split("/_v/")
                rev, name = asset.split("/pkg/")
                if host == long_alias or (host == spec(B)["public_origin"] and apex_body is None):
                    return "404", "text/plain", "", "", b"not found"
                if host == spec(B)["public_origin"]:
                    return "200", art.TYPES[Path(name).suffix], rev, "fallback/r26", apex_body
                self.assertIn(host, {immutable, alias, predecessor})
                route = "inventory/r26" if approved_old else ""
                return "200", art.TYPES[Path(name).suffix], rev, route, (rev + name).encode()

            for name, value in {
                "STATE": root, "WRANGLER": receipt, "project": lambda _: {"canonical_deployment": prior},
                "export": lambda *_: manifest, "read_asset": fetch, "run": local, "browser_preflight": lambda _: None,
                "preview_receipt": lambda *_: {"preview_deployment": "preview",
                                                "preview_url": immutable, "preview_alias": alias},
            }.items():
                stack.enter_context(patch.object(release, name, value))
            stack.enter_context(patch.object(release.subprocess, "run", side_effect=lambda argv, **_: calls.append(argv)))
            stack.enter_context(patch.object(release.pwd, "getpwnam", return_value=type("User", (), {"pw_uid": 501})()))
            stack.enter_context(patch.dict(os.environ, {"CLOUDFLARE_ACCOUNT_ID": ACCOUNT, "VINK_RELEASE_CHROME": "/bin/sh"}))
            args = [str(receipt), "--account", ACCOUNT, "--previous-origin", A, "--previous-edge", prior["id"],
                    "--reader", "33333333-3333-4333-8333-333333333333"]
            with self.assertRaisesRegex(ValueError, r"HTTP 404"):
                release.verify_assets(long_alias, B, manifest)
            with patch("sys.argv", ["release.py", "apply", *args]), self.assertRaisesRegex(ValueError, "short-alias route"):
                release.main()
            approved_old = True
            with patch("sys.argv", ["release.py", "resume", *args]), self.assertRaisesRegex(ValueError, r"HTTP 404"):
                release.main()
            apex_body = b"wrong bytes"
            with patch("sys.argv", ["release.py", "resume", *args]), self.assertRaisesRegex(ValueError, r"HTTP 200"):
                release.main()
            pending = json.loads((root / "pending.json").read_bytes())
            self.assertEqual(pending["phase"], "preview_verified")
            uploads = [cmd[cmd.index("--branch") + 1] for cmd in calls if "--branch" in cmd]
            self.assertEqual(uploads, [f"r-{B[:26]}"])
            self.assertFalse(any("apply" in cmd for cmd in calls))

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
                                "export":lambda *_:manifest, "verify_assets":lambda *_, **__:None,
                                "preview_receipt":lambda *_: {"preview_deployment": "preview",
                                    "preview_url": "https://2243f07a.vink-production.pages.dev",
                                    "preview_alias": f"https://r-{B[:26]}.vink-production.pages.dev"},
                                "browser_check":lambda *_:None, "browser_preflight":lambda _:None, "run":local}.items():
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
