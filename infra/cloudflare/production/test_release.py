import contextlib
import io
import json
import os
import shutil
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


def package(archives, rev, extra=()):
    pkg = archives / rev / "pkg"
    for name in art.REQUIRED.union(extra):
        file = pkg / name
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_bytes((rev + name).encode())
    receipt = art.record(pkg, spec(rev))
    (pkg.parent / "manifest.json").write_bytes(art.encode(receipt))
    return receipt


class Packages(unittest.TestCase):
    def test_standalone_promote_rollback_and_immutable_assets(self):
        with TemporaryDirectory() as tmp:
            root = Path(tmp)
            for rev in (A, B):
                site = root / rev
                package(site / "_v", rev)
                (site / "index.html").write_text(f'<script src="/_v/{rev}/pkg/vink.js"></script>')
            for current, previous in ((B, A), (A, B)):
                output = root / ("pages-" + current)
                art.stage_spa(root / current, [root / previous], output)
                self.assertEqual((output / "index.html").read_bytes(), (root / current / "index.html").read_bytes())
                for rev in (current, previous):
                    self.assertEqual((output / "_v" / rev / "pkg/vink-editor.js").read_bytes(),
                                     (root / rev / "_v" / rev / "pkg/vink-editor.js").read_bytes())
            archive = root / "legacy"
            c = "c" * 40
            package(archive, c)
            art.stage_spa(root / B, [archive / c], root / "transition")
            self.assertTrue((root / "transition" / "_v" / c / "pkg/vink-editor.js").is_file())
            copy = root / "copy"
            shutil.copytree(root / A, copy)
            (copy / "_v" / A / "pkg/vink.js").write_text("different")
            with self.assertRaisesRegex(ValueError, "immutable release"):
                art.stage_spa(root / A, [copy], root / "bad")
            self.assertFalse((root / "bad").exists())

    def test_inventory_and_rollback(self):
        with TemporaryDirectory() as tmp:
            root = Path(tmp)
            archives = root / "archives"
            old_handle_assets = ("author-landscape.webp", "snippets/vink-web-abc/profile/handle.js")
            old = package(archives, A, old_handle_assets)
            new = package(archives, B)
            self.assertTrue(set(old_handle_assets) <= old["assets"].keys())
            self.assertTrue(set(old_handle_assets).isdisjoint(new["assets"]))
            for rev in (B, A):
                output = root / rev
                art.stage(archives, rev, output, A)
                for keep in (A, B):
                    self.assertEqual((output / "_v" / keep / "pkg/vink.js").read_text(), keep + "vink.js")
                for name in old_handle_assets:
                    self.assertEqual((output / "_v" / A / "pkg" / name).read_text(), A + name)
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

    def test_blocks_pilot_in_candidate_but_replays_retained_archive(self):
        with TemporaryDirectory() as tmp:
            root = Path(tmp)
            archives = root / "archives"
            package(archives, A)
            package(archives, B)
            pkg = archives / A / "pkg"
            for name in ("expressions/pilot.js", "snippets/vink-web-ab12/expressions/bridge.js"):
                file = pkg / name
                file.parent.mkdir(parents=True, exist_ok=True)
                file.write_text("historical asset")
                with self.subTest(asset=name), self.assertRaisesRegex(ValueError, "Preview asset"):
                    art.record(pkg, spec(A))
            (pkg.parent / "manifest.json").write_bytes(art.encode(art.record(pkg, spec(A), retained=True)))
            output = root / "staging"
            art.stage(archives, B, output)
            self.assertFalse((output / "_v" / B / "pkg/expressions/pilot.js").exists())
            self.assertTrue((output / "_v" / A / "pkg/expressions/pilot.js").exists())

    def test_first_notification_release_preserves_predecessor_and_binds_fourth_digest(self):
        legacy = spec(A)
        upgraded = spec(B)
        ref = "ghcr.io/loveoverflowcom/vink-notifications@sha256:" + "f" * 64
        upgraded["images"]["notifications"] = ref
        art.validate(legacy)
        art.validate(upgraded)
        origin = release.maintenance.Origin.__new__(release.maintenance.Origin)
        old_env = "".join(f"VINK_PROD_{key}_IMAGE={legacy['images'][image]}\n"
                          for key, image in release.maintenance.SERVICES.values()).encode()
        candidate_env = origin.env(upgraded["images"], old_env)
        self.assertIn(f"VINK_PROD_NOTIFICATIONS_IMAGE={ref}\n".encode(), candidate_env)
        self.assertEqual(origin.env(upgraded["images"], candidate_env), candidate_env)
        self.assertEqual(origin.env(legacy["images"], old_env), old_env)
        self.assertNotIn("notifications", release.maintenance.running_services(legacy["images"]))
        self.assertIn("notifications", release.maintenance.running_services(upgraded["images"]))
        upgraded["images"]["notifications"] = upgraded["images"]["worker"]
        with self.assertRaisesRegex(ValueError, "wrong notifications image"):
            art.validate(upgraded)

    def test_schema_review_checks_all_owner_histories_and_direction(self):
        with TemporaryDirectory() as tmp:
            root = Path(tmp)
            evidence = {"from": A, "to": B, "action": "expand"}
            compose = root / B / "infra/production/compose.yaml"
            compose.parent.mkdir(parents=True)
            compose.write_text("services:\n  notifications:\n")
            for owner in ("vink-app", "vink-identity", "vink-notifications"):
                evidence[owner] = {"old_on_result": "PASS", "new_on_result": "PASS", "rehearsal": "scratch read/write"}
                for rev in (A, B):
                    path = root / rev / "services" / owner / "migrations"
                    path.mkdir(parents=True)
                    (path / "1_initial.sql").write_text("first")
                (path / "2_add.sql").write_text("add")
            review = root / "review.json"
            review.write_bytes(art.encode(evidence))
            self.assertEqual(release.schema_action(A, B, review, root), "expand")
            for rev in (A, B):
                for relative in ("services/postgres/compose.yaml", "infra/production/auth-resend.compose.yaml"):
                    file = root / rev / relative
                    file.parent.mkdir(parents=True, exist_ok=True)
                    file.write_text("existing topology")
            (root / A / "infra/production/compose.yaml").write_text("services:\n")
            additive = evidence | {"changes_existing_data": False}
            review.write_bytes(art.encode(additive))
            with self.assertRaisesRegex(ValueError, "unclassified topology"):
                release.schema_action(A, B, review, root)
            review.write_bytes(art.encode(additive | {"bootstrap_service": "notifications"}))
            self.assertEqual(release.schema_action(A, B, review, root), "expand-additive")
            review.write_bytes(art.encode(evidence | {"changes_existing_data": False,
                                                    "vink-app": {"old_on_result": "FAIL", "new_on_result": "PASS",
                                                                 "rehearsal": "scratch"}}))
            with self.assertRaisesRegex(ValueError, "both binaries"):
                release.schema_action(A, B, review, root)
            review.write_bytes(art.encode(evidence | {"from": B, "to": A, "action": "keep"}))
            self.assertEqual(release.schema_action(B, A, review, root), "keep")
            for owner in ("vink-app", "vink-identity", "vink-notifications"):
                (root / B / "services" / owner / "migrations/2_add.sql").unlink()
            review.write_bytes(art.encode(additive | {"bootstrap_service": "notifications"}))
            with self.assertRaisesRegex(ValueError, "requires a forward migration"):
                release.schema_action(A, B, review, root)
            review.write_bytes(art.encode(evidence | {"from": B, "to": A, "action": "keep"}))
            (path / "1_initial.sql").write_text("drift")
            with self.assertRaisesRegex(ValueError, "divergent"):
                release.schema_action(B, A, review, root)

    def test_timer_observation_reports_age_manifest_and_missing_owner(self):
        with TemporaryDirectory() as tmp, patch.object(release.maintenance, "BACKUPS", Path(tmp)):
            owners = {"vink", "vink_identity"}
            self.assertTrue(release.maintenance.daily_backup(owners)["warnings"])
            stamp = "20260101T000000Z"
            backup = Path(tmp) / stamp
            backup.mkdir()
            manifest = ("f" * 64 + "  vink.dump\n").encode()
            (backup / "SHA256SUMS").write_bytes(manifest)
            (backup / "vink.dump").write_bytes(b"dump")
            with patch.object(release.maintenance.time, "time", return_value=1767225600 + 26 * 3600):
                result = release.maintenance.daily_backup(owners)
                self.assertEqual(result, {"timestamp": stamp, "manifest_sha256": art.sha(manifest),
                                          "warnings": ["backup missing vink_identity"]})
                self.assertFalse(release.maintenance.daily_backup({"vink"})["warnings"])
            with patch.object(release.maintenance.time, "time", return_value=1767225600 + 26 * 3600 + 1):
                self.assertIn("backup outside 26-hour window", release.maintenance.daily_backup(owners)["warnings"])

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

    def test_catalog_requires_proof_of_an_empty_final_page(self):
        for status, mime, body in (
            ("503", "application/json", b'{"works":[],"next_cursor":null}'),
            ("200", "text/html", b'{"works":[],"next_cursor":null}'),
            ("200", "application/json", b'invalid json'),
            ("200", "application/json", b'[]'),
            ("200", "application/json", b'{"works":[]}'),
            ("200", "application/json", b'{"works":[],"next_cursor":"more"}'),
            ("200", "application/json", b'{"works":[{}],"next_cursor":null}'),
        ):
            with self.subTest(status=status, mime=mime, body=body), \
                    patch.object(release, "read_asset", return_value=(status, mime, "", "", body)), \
                    self.assertRaises(ValueError):
                release.assert_empty_catalog()

    def test_same_schema_rolling_empty_catalog_resumes_at_safe_boundaries(self):
        with TemporaryDirectory() as tmp, contextlib.ExitStack() as stack:
            root = Path(tmp)
            archive = root / "archives"
            old, new = package(archive, A), package(archive, B)
            receipt = root / "request.json"
            receipt.write_bytes(art.encode(spec(B)))
            predecessor = {"id": "previous", "url": "https://69fff7eb.vink-production.pages.dev"}
            preview = {"preview_deployment": "preview", "preview_url": "https://2243f07a.vink-production.pages.dev",
                       "preview_alias": f"https://r-{B[:26]}.vink-production.pages.dev"}
            apex, catalog = spec(B)["public_origin"], "https://vink.forum/api/v1/works"
            active = root / "active.json"
            journal = root / "pending.json"
            active.write_bytes(art.encode({"edge": predecessor["id"], "account": ACCOUNT, "origin_release": A,
                "legacy_assets": A, "release": spec(A), "retained": {A: art.sha(art.encode(old))}}))
            remote = {"canonical_deployment": predecessor}
            calls, asset_hosts = [], set()
            empty = ("200", "application/json", "", "", b'{"works":[],"next_cursor":null}')
            outage = ("503", "application/json", "", "", b"{}")
            listing = outage
            same_schema = True
            outage_during_plan = False

            def fetch(url):
                if url == catalog:
                    calls.append("catalog")
                    return listing
                host, asset = url.split("/_v/")
                rev, name = asset.split("/pkg/")
                asset_hosts.add(host)
                return "200", art.TYPES[Path(name).suffix], rev, "inventory/r26", (rev + name).encode()

            def local(*argv):
                nonlocal listing
                if "rev-parse" in argv:
                    return B
                if str(argv[0]).endswith("/origin.sh"):
                    self.assertEqual(argv[-1], "same")
                    calls.append(f"origin {argv[1]}")
                    if not same_schema:
                        raise ValueError("migration trees differ")
                    if outage_during_plan:
                        listing = outage
                return ""

            def publish(public, branch, rev, marker):
                nonlocal listing
                self.assertEqual(rev, B)
                for retained in (A, B):
                    self.assertEqual((public / "_v" / retained / "pkg/vink.js").read_text(), retained + "vink.js")
                calls.append(branch)
                if branch == "main":
                    remote["canonical_deployment"] = {"id": "new-edge", "environment": "production",
                        "url": "https://abcd.vink-production.pages.dev", "latest_stage": {"status": "success"},
                        "deployment_trigger": {"metadata": {"commit_hash": B, "commit_message": marker}}}
                    listing = outage

            for name, value in {"STATE": root, "WRANGLER": receipt, "project": lambda _: remote,
                                "export": lambda *_: new, "read_asset": fetch, "run": local,
                                "publish": publish, "preview_receipt": lambda *_: preview,
                                "browser_preflight": lambda _: None}.items():
                stack.enter_context(patch.object(release, name, value))
            browser = stack.enter_context(patch.object(release, "browser_check"))
            smoke = stack.enter_context(patch.object(release.subprocess, "run"))
            stack.enter_context(patch.object(release.pwd, "getpwnam", return_value=SimpleNamespace(pw_uid=501)))
            stack.enter_context(patch.dict(os.environ, {"CLOUDFLARE_ACCOUNT_ID": ACCOUNT, "VINK_RELEASE_CHROME": "/bin/sh"}))
            args = [str(receipt), "--account", ACCOUNT, "--previous-origin", A, "--previous-edge", predecessor["id"]]

            def execute(mode, *extra):
                with patch("sys.argv", ["release.py", mode, *args, "--empty-catalog", *extra]), \
                        contextlib.redirect_stdout(io.StringIO()) as output:
                    release.main()
                return json.loads(output.getvalue())

            with self.assertRaisesRegex(ValueError, "catalog unreachable"):
                execute("plan")
            self.assertEqual(calls, ["catalog"])
            listing = empty
            with self.assertRaisesRegex(ValueError, "cannot use a reader"):
                execute("plan", "--reader", "unused")
            with patch.object(release, "schema_action", return_value="expand"), \
                    self.assertRaisesRegex(ValueError, "unchanged migrations"):
                execute("apply", "--preview-only")
            same_schema = False
            with self.assertRaisesRegex(ValueError, "migration trees differ"):
                execute("apply", "--preview-only")
            self.assertNotIn(f"r-{B[:26]}", calls)
            self.assertFalse(journal.exists())
            same_schema = True
            self.assertEqual(execute("plan")["schema_action"], "same")
            outage_during_plan = True
            with self.assertRaisesRegex(ValueError, "catalog unreachable"):
                execute("apply", "--preview-only")
            self.assertFalse(journal.exists())
            self.assertNotIn(f"r-{B[:26]}", calls)
            outage_during_plan = False
            listing = empty
            self.assertEqual(execute("apply", "--preview-only")["phase"], "preview_verified")
            staged = journal.read_bytes()
            self.assertEqual(remote["canonical_deployment"], predecessor)

            reader_journal = json.loads(staged)
            del reader_journal["reader_coverage"]
            journal.write_bytes(art.encode(reader_journal))
            with self.assertRaisesRegex(ValueError, "original catalog smoke mode"):
                execute("resume")
            journal.write_bytes(staged)
            listing = ("200", "application/json", "", "", b'{"works":[{}],"next_cursor":null}')
            with self.assertRaisesRegex(ValueError, "requires an empty public catalog"):
                execute("resume")
            self.assertEqual(journal.read_bytes(), staged)
            self.assertEqual(remote["canonical_deployment"], predecessor)
            self.assertNotIn("main", calls)

            listing = empty
            with self.assertRaisesRegex(ValueError, "catalog unreachable"):
                execute("resume")
            self.assertEqual(json.loads(journal.read_bytes())["phase"], "edge_promoted")
            self.assertEqual(calls[-4:], ["catalog", "main", "origin plan", "catalog"])
            self.assertNotIn("origin apply", calls)
            self.assertEqual(json.loads(active.read_bytes())["origin_release"], A)
            listing = empty
            result = execute("resume")
            self.assertEqual(result["phase"], "browser_verified")
            self.assertEqual(result["reader_coverage"], "NOT_RUN: verified empty catalog")
            self.assertEqual(result["origin_release"], B)
            self.assertEqual(calls[-4:], ["origin plan", "catalog", "origin apply", "origin observe"])
            self.assertEqual(calls.count("main"), 1)
            self.assertEqual(asset_hosts, {predecessor["url"], preview["preview_url"], preview["preview_alias"],
                                          apex, remote["canonical_deployment"]["url"]})
            self.assertEqual(json.loads(active.read_bytes()), result)
            self.assertFalse(journal.exists())
            browser.assert_called_once_with(Path("/bin/sh"), None, B, empty_catalog=True)
            smoke.assert_called_once_with([release.HERE.parent.parent / "production/smoke-edge.sh", "edge",
                                          "--release", B], check=True)

    def test_published_reader_recovers_after_origin_switch_without_replaying(self):
        with TemporaryDirectory() as tmp, contextlib.ExitStack() as stack:
            root = Path(tmp)
            archive = root / "archives"
            old, new = package(archive, A), package(archive, B)
            retained = {rev: art.sha(art.encode(manifest)) for rev, manifest in ((A, old), (B, new))}
            worker_digest = art.stage(archive, B, root / "stage", A)
            receipt = root / "request.json"
            receipt.write_bytes(art.encode(spec(B)))
            active = root / "active.json"
            prior = art.encode({"edge": "previous", "account": ACCOUNT, "origin_release": A, "legacy_assets": A,
                                "release": spec(A), "retained": {A: retained[A]}})
            active.write_bytes(prior)
            journal = root / "pending.json"
            pending = {"release": spec(B), "account": ACCOUNT, "project": art.PROJECT,
                       "previous_edge": "previous", "previous_origin": A, "edge": "new-edge", "origin_release": B,
                       "asset_digest": new["asset_digest"], "legacy_assets": A, "worker_digest": worker_digest,
                       "retained": retained, "schema_action": "same", "schema_review": None,
                       "reader_coverage": release.EMPTY_CATALOG, "phase": "http_verified"}
            journal.write_bytes(art.encode(pending))
            remote = {"canonical_deployment": {"id": "new-edge", "environment": "production",
                "latest_stage": {"status": "success"},
                "deployment_trigger": {"metadata": {"commit_hash": B,
                                                     "commit_message": "vink-release-" + worker_digest}}}}
            reader = "33333333-3333-4333-8333-333333333333"
            selected = {"status": "published", "work_id": reader}
            observed = selected
            calls = []

            def fetch(url):
                self.assertEqual(url, f"https://vink.forum/api/v1/works/{reader}/reader")
                return "200", "application/json", "", "", art.encode(observed)

            def local(*argv):
                if "rev-parse" in argv:
                    return B
                if str(argv[0]).endswith("/origin.sh"):
                    calls.append(tuple(argv[1:4]))
                return ""

            def execute(*extra):
                args = [str(receipt), "--account", ACCOUNT, "--previous-origin", A,
                        "--previous-edge", "previous", "--empty-catalog", *extra]
                with patch("sys.argv", ["release.py", "resume", *args]), \
                        contextlib.redirect_stdout(io.StringIO()) as output:
                    release.main()
                return json.loads(output.getvalue())

            for name, value in {"STATE": root, "WRANGLER": receipt, "project": lambda _: remote,
                                "export": lambda *_: new, "read_asset": fetch, "run": local}.items():
                stack.enter_context(patch.object(release, name, value))
            publish = stack.enter_context(patch.object(release, "publish"))
            browser = stack.enter_context(patch.object(release, "browser_check"))
            stack.enter_context(patch.object(release.subprocess, "run"))
            stack.enter_context(patch.object(release.pwd, "getpwnam", return_value=SimpleNamespace(pw_uid=501)))
            stack.enter_context(patch.dict(os.environ, {"CLOUDFLARE_ACCOUNT_ID": ACCOUNT, "VINK_RELEASE_CHROME": "/bin/sh"}))
            override = ("--resume-with-reader", reader)
            for options in (("--resume-with-reader", "not-a-UUID"), (*override, "--maintenance"),
                            (*override, "--preview-only")):
                with self.subTest(options=options), self.assertRaisesRegex(ValueError, "rolling --empty-catalog resume"):
                    execute(*options)
            for observed in ({"status": "hidden", "work_id": reader}, selected | {"work_id": "other-work"}):
                with self.subTest(observed=observed), self.assertRaisesRegex(ValueError, "selected Reader to be published"):
                    execute(*override)
            self.assertEqual(json.loads(journal.read_bytes()), pending)
            self.assertEqual(calls, [])
            browser.assert_not_called()

            observed = selected
            original_store = release.store

            def lose_receipt_ack(path, value):
                if path == active:
                    raise OSError("lost active receipt write")
                original_store(path, value)

            with patch.object(release, "store", side_effect=lose_receipt_ack), \
                    self.assertRaisesRegex(OSError, "lost active receipt write"):
                execute(*override)
            saved = json.loads(journal.read_bytes())
            self.assertEqual(saved["phase"], "browser_verified")
            self.assertEqual(saved["reader_coverage"], release.PUBLISHED_READER)
            self.assertEqual(saved["reader_resume_work"], reader)
            self.assertEqual(active.read_bytes(), prior)

            result = execute(*override)
            self.assertEqual(result, saved)
            self.assertEqual(json.loads(active.read_bytes()), saved)
            self.assertEqual(calls, [("observe", B, B), ("observe", B, B)])
            self.assertFalse(journal.exists())
            publish.assert_not_called()
            self.assertEqual([call.args for call in browser.call_args_list], [(Path("/bin/sh"), reader, B)] * 2)

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
            evidence["vink-app"]["old_on_result"] = "PASS"
            evidence["changes_existing_data"] = True
            review.write_bytes(art.encode(evidence | {"action": "expand"}))
            with self.assertRaisesRegex(ValueError, "existing data require maintenance"):
                release.schema_action(A, B, review, root)
            review.write_bytes(art.encode(evidence))
            self.assertEqual(release.schema_action(A, B, review, root, window=True), "maintenance")
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
        origin.owners = release.maintenance.OWNERS
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
