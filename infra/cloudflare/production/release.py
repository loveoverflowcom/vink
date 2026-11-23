#!/usr/bin/env python3
"""Run on the release host. All provider mutations require apply and an observed predecessor."""
import argparse
import fcntl
import json
import os
import pwd
from pathlib import Path
import re
import subprocess
import tempfile
from urllib.request import Request, urlopen

from artifacts import PROJECT, encode, record, require, sha, stage, validate
import maintenance

HERE = Path(__file__).resolve().parent
STATE = Path("/var/lib/vink-production-release")
WRANGLER = HERE.parent / "node_modules/.bin/wrangler"


def run(*args):
    return subprocess.check_output(args, text=True).strip()


def store(path, value):
    tmp = path.with_suffix(".new")
    with tmp.open("wb") as f:
        f.write(encode(value))
        f.flush()
        os.fsync(f.fileno())
    os.replace(tmp, path)
    maintenance.sync_dir(path.parent)


def project(account):
    endpoint = f"https://api.cloudflare.com/client/v4/accounts/{account}/pages/projects/{PROJECT}"
    req = Request(endpoint, headers={"Authorization": "Bearer " + os.environ["CLOUDFLARE_API_TOKEN"]})
    with urlopen(req, timeout=30) as response:
        payload = json.load(response)
    require(payload["success"], "Pages project read failed")
    p = payload["result"]
    require(p["name"] == PROJECT and p["production_branch"] == "main" and not p.get("source")
            and p["subdomain"] == PROJECT + ".pages.dev",
            "expected existing main-branch Direct Upload project")
    for scope in ("production", "preview"):
        config = p["deployment_configs"][scope]
        require(config.get("fail_open") is not True and config.get("compatibility_date", "") >= "2026-10-01",
                "verify Pages fail-closed settings and compatibility date")
    bindings = p["deployment_configs"]["production"]["env_vars"]
    require(bindings.get("ORIGIN_KEY", {}).get("type") == "secret_text", "missing protected origin binding")
    return p


def deployment(p):
    return (p.get("canonical_deployment") or {}).get("id")


def export(spec, archive):
    rev = spec["source_sha"]
    for image in spec["images"].values():
        run("podman", "pull", "--platform", spec["platform"], image)
        meta, = json.loads(run("podman", "image", "inspect", image))
        require(image in meta["RepoDigests"] and meta["Labels"]["org.opencontainers.image.revision"] == rev
                and meta["Os"] + "/" + meta["Architecture"] == spec["platform"], "image provenance mismatch")
    with tempfile.TemporaryDirectory(dir=archive.parent) as work:
        package = Path(work) / "release"
        package.mkdir()
        container = run("podman", "create", spec["images"]["web"])
        try:
            marker = Path(work) / "site-release"
            run("podman", "cp", container + ":/opt/vink/site-release", str(marker))
            require(marker.read_text().strip() == rev, "Web package was not built for the declared release")
            run("podman", "cp", container + ":/opt/vink/site/pkg", str(package / "pkg"))
        finally:
            run("podman", "rm", container)
        manifest = record(package / "pkg", spec)
        dest = archive / rev
        if dest.exists():
            require(json.loads((dest / "manifest.json").read_bytes()) == manifest, "conflicting release for SHA")
        else:
            store(package / "manifest.json", manifest)
            package.rename(dest)
        return manifest


def verify_assets(host, rev, manifest):
    for name, entry in manifest["assets"].items():
        url = f"{host}/_v/{rev}/pkg/{name}"
        with urlopen(url, timeout=90) as body:
            require(body.url == url and body.status == 200 and body.headers.get_content_type() == entry["type"] and
                    sha(body.read()) == entry["sha256"], "Pages asset mismatch: " + name)


def schema_action(old, new, review, releases=Path("/opt/vink-prod/releases"), *, window=False):
    if not review:
        require(not window, "maintenance requires an explicit schema review")
        return "same"
    evidence = json.loads(review.read_bytes())
    action = evidence["action"]
    require(evidence["from"] == old and evidence["to"] == new and action in ("expand", "keep", "maintenance"),
            "schema review must match source, destination and migration direction")
    require(window == (action == "maintenance"), "maintenance requires explicit opt-in and matching review")
    incompatible = False
    adds_migrations = False
    for owner in ("vink-app", "vink-identity"):
        report = evidence[owner]
        require(report["new_on_result"] == "PASS" and report["rehearsal"],
                "candidate requires rehearsal on the resulting schema")
        if window:
            require(report["old_on_result"] in ("PASS", "FAIL") and report["old_on_restored"] == "PASS",
                    "maintenance requires the actual old outcome and a rehearsal of the restored predecessor")
            incompatible |= report["old_on_result"] == "FAIL"
        else:
            require(report["old_on_result"] == "PASS", "both binaries require rehearsal on the resulting schema")
        before = maintenance.ledger_files(releases, old, owner)
        after = maintenance.ledger_files(releases, new, owner)
        older, newer = (after, before) if action == "keep" else (before, after)
        require(list(older.items()) == list(newer.items())[:len(older)], "edited or divergent applied migration")
        adds_migrations |= len(newer) > len(older)
    require(not window or (incompatible and adds_migrations), "maintenance requires an incompatible forward migration")
    return action


def bootstrap_baseline(path, predecessor, old, platform, archive, active):
    data = json.loads(path.read_bytes())
    require(data["pages_deployment"] == predecessor and data["source_sha"] == old and
            data["project"] == PROJECT and data["public_origin"] == "https://vink.forum",
            "bootstrap receipt differs from predecessor")
    manifest = json.loads((archive / old / "manifest.json").read_bytes())
    spec = {"environment": "production", "public_origin": data["public_origin"], "source_sha": old,
            "platform": platform, "images": data["images"]}
    require(record(archive / old / "pkg", spec) == manifest and manifest["asset_digest"] == data["asset_digest"],
            "bootstrap receipt differs from retained bytes")
    if active:
        metadata = active["deployment_trigger"]["metadata"]
        require(active["id"] == predecessor and active["environment"] == "production"
                and active["latest_stage"]["status"] == "success" and metadata["commit_hash"] == old,
                "provider does not confirm the bootstrap deployment")
        host = active["url"]
        require(re.fullmatch(r"https://[a-z0-9-]+\.vink-production\.pages\.dev", host), "wrong provider host")
        verify_assets(host, old, manifest)
        verify_assets("https://vink.forum", old, manifest)
    return data["images"]


def browser_check(chrome, reader, rev):
    # The browser processes public content with no deployment credentials or root privileges.
    user = pwd.getpwnam("vink-release-check")
    require(user.pw_uid > 0, "dedicated browser account required")
    repo = HERE.parents[2]
    with tempfile.TemporaryDirectory(prefix="vink-browser-") as tmp:
        os.chown(tmp, user.pw_uid, user.pw_gid)
        subprocess.run(["node", repo / "apps/vink-web/e2e/browser-smoke.mjs", "--phase", "production-edge",
                        "--chrome", chrome, "--state", Path(tmp) / "browser.json", "--revision", rev,
                        "--reader", reader, "--base", "https://vink.forum"], check=True, cwd=repo,
                       user=user.pw_uid, group=user.pw_gid, extra_groups=[],
                       env={"PATH": os.defpath + ":/usr/local/bin", "HOME": tmp,
                            "GIT_CONFIG_COUNT": "1", "GIT_CONFIG_KEY_0": "safe.directory",
                            "GIT_CONFIG_VALUE_0": str(repo)})


def matching(active, rev, marker):
    if not active:
        return False
    metadata = active["deployment_trigger"]["metadata"]
    return (active["environment"] == "production" and active["latest_stage"]["status"] == "success"
            and metadata["commit_hash"] == rev and metadata["commit_message"] == marker)


def advance_origin(origin, journal, result, mode, evidence):
    """Return whether the origin can enter ordinary public verification."""
    if mode == "rollback-origin":
        require(result["phase"] in {"origin_switch_started", "origin_prepared",
                                    "origin_rollback_started", "origin_rolled_back"},
                "origin may have accepted writes; rollback is unavailable")
        if result["phase"] != "origin_rolled_back":
            result["phase"] = "origin_rollback_started"
            store(journal, result)
    if result["phase"] == "origin_rollback_started":
        origin.operate("rollback", evidence)
        result.update(phase="origin_rolled_back", origin_release=result["previous_origin"])
        store(journal, result)
    if result["phase"] == "origin_rolled_back":
        return False
    if result["phase"] == "origin_switch_started":
        origin.operate("prepare", evidence)
        result["phase"] = "origin_prepared"
        store(journal, result)
    if result["phase"] == "origin_prepared":
        if mode != "open":
            return False
        result["phase"] = "origin_opening"
        store(journal, result)
    if result["phase"] == "origin_opening":
        origin.operate("open", evidence)
    return True


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("mode", choices=["plan", "apply", "resume", "open", "rollback-origin", "status"])
    p.add_argument("release", type=Path, help="public JSON copied from qualified GHCR publish receipt")
    p.add_argument("--maintenance", action="store_true", help="prepare origin offline and require an explicit open")
    p.add_argument("--bootstrap-receipt", type=Path, help="reconcile a manually verified Pages predecessor")
    p.add_argument("--schema-review", type=Path, help="rehearsal evidence when migrations differ")
    p.add_argument("--account", required=True)
    p.add_argument("--reader", help="public production work UUID for browser qualification")
    p.add_argument("--previous-origin", required=True)
    p.add_argument("--previous-edge", required=True, help="observed Pages deployment id or 'none' on bootstrap")
    a = p.parse_args()
    spec = json.loads(a.release.read_bytes())
    validate(spec)
    rev = spec["source_sha"]
    require(re.fullmatch("[0-9a-f]{40}", a.previous_origin), "invalid predecessor SHA")
    require(re.fullmatch("[0-9a-f]{32}", a.account) and a.account == os.environ.get("CLOUDFLARE_ACCOUNT_ID"),
            "account differs from operator target")
    os.umask(0o077)
    require(STATE.is_dir(), f"prepare root-owned {STATE} before release")
    journal = STATE / "pending.json"
    with (STATE / "lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        remote = project(a.account)
        current = deployment(remote)
        if a.mode == "status":
            print(json.dumps({"edge": current, "origin_tree": str(Path("/opt/vink-prod/current").resolve()),
                              "pending": json.loads(journal.read_bytes()) if journal.exists() else None,
                              "origin_maintenance": maintenance.GUARD.exists()}))
            return
        require(a.reader and re.fullmatch(r"[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}", a.reader),
                "supply an existing public reader UUID")
        chrome = Path(os.environ["VINK_RELEASE_CHROME"])
        require(chrome.is_absolute() and os.access(chrome, os.X_OK), "read-only guest browser required")
        require(pwd.getpwnam("vink-release-check").pw_uid > 0, "dedicated browser user required")
        resume = a.mode in {"resume", "open", "rollback-origin"}
        require(not (a.bootstrap_receipt or a.mode in {"open", "rollback-origin"}) or a.maintenance,
                "manual origin commands require maintenance opt-in")
        require(resume == journal.exists(), "pending release requires status/resume; new release requires apply")
        pending = json.loads(journal.read_bytes()) if resume else None
        predecessor = a.previous_edge if a.previous_edge != "none" else None
        if resume:
            require(pending["release"] == spec and pending["account"] == a.account and
                    pending["previous_origin"] == a.previous_origin and pending["previous_edge"] == predecessor,
                    "resume must use original inputs")
        else:
            require(current == predecessor, "Pages predecessor changed")
        require(run("git", "-C", HERE, "rev-parse", "HEAD") == rev, "release checkout differs from input")
        require(run("git", "-C", HERE, "status", "--porcelain") == "", "release checkout is dirty")
        require(WRANGLER.is_file(), "install the locked Wrangler in infra/cloudflare")
        archive = STATE / "archives"
        archive.mkdir(exist_ok=True)
        # A missing archive after a host loss must not discard assets from the serving deployment.
        previous = STATE / "active.json"
        legacy = rev
        if resume and previous.exists() and pending.get("edge") == current:
            completed = json.loads(previous.read_bytes())
            if completed.get("phase") == "browser_verified" and completed == pending:
                journal.unlink()
                print(json.dumps(completed))
                return
        old_images = None
        if a.bootstrap_receipt:
            require(predecessor and not previous.exists(), "bootstrap receipt is only for the first controller release")
            active_predecessor = remote["canonical_deployment"] if current == predecessor else None
            old_images = bootstrap_baseline(a.bootstrap_receipt, predecessor, a.previous_origin, spec["platform"],
                                            archive, active_predecessor)
            legacy = a.previous_origin
        elif predecessor:
            require(previous.exists(), "restore the previous release receipt and archives")
            last = json.loads(previous.read_bytes())
            require(last["edge"] == predecessor and last["account"] == a.account
                    and last["origin_release"] == a.previous_origin, "active edge differs from receipt")
            legacy = last["legacy_assets"]
            old_images = last["release"]["images"]
            for release, digest in last["retained"].items():
                require(sha((archive / release / "manifest.json").read_bytes()) == digest, "lost retained release")
        manifest = export(spec, archive)
        images = spec["images"]
        schema = schema_action(a.previous_origin, rev, a.schema_review, window=a.maintenance)
        evidence = None
        origin = None
        if a.maintenance:
            evidence = json.loads(a.schema_review.read_bytes())
            require(old_images and evidence["images"] == {"from": old_images, "to": spec["images"]},
                    "schema rehearsal does not match the two image sets")
            origin = maintenance.Origin(a.previous_origin, spec, old_images)
            if not resume:
                origin.preflight(evidence)
        origin_args = [a.previous_origin, rev, images["web"], images["worker"], images["identity"], spec["platform"], schema]
        if not resume:
            run(HERE / "origin.sh", "plan", *origin_args)
        with tempfile.TemporaryDirectory(dir=STATE) as temp:
            public = Path(temp) / "public"
            worker_digest = stage(archive, rev, public, legacy)
            retained = {d.name: sha((d / "manifest.json").read_bytes()) for d in archive.iterdir()}
            result = {"release": spec, "account": a.account, "project": PROJECT, "previous_edge": predecessor,
                      "previous_origin": a.previous_origin, "asset_digest": manifest["asset_digest"],
                      "legacy_assets": legacy, "worker_digest": worker_digest, "retained": retained,
                      "schema_action": schema, "schema_review": sha(a.schema_review.read_bytes()) if a.schema_review else None}
            if a.bootstrap_receipt:
                result["bootstrap_receipt"] = sha(a.bootstrap_receipt.read_bytes())
            if a.mode == "plan":
                print(json.dumps(result))
                return
            if resume:
                require(all(pending.get(k) == v for k, v in result.items()), "release bytes or target changed")
                result = pending
                if a.mode == "open":
                    require(result["phase"] in {"origin_prepared", "origin_opening"}, "origin is not prepared")
                if a.mode == "rollback-origin":
                    require(result["phase"] in {"origin_switch_started", "origin_prepared",
                                                "origin_rollback_started", "origin_rolled_back"},
                            "origin may have accepted writes; rollback is unavailable")
            else:
                require(deployment(project(a.account)) == predecessor, "Pages changed during staging")
                result["phase"] = "preview_upload_started"
                store(journal, result)
            marker = "vink-release-" + worker_digest

            def publish(branch):
                subprocess.run([WRANGLER, "pages", "deploy", public, "--project-name", PROJECT, "--branch", branch,
                                "--commit-hash", rev, "--commit-message", marker], cwd=temp, check=True)

            if result["phase"] == "preview_upload_started":
                if not resume:
                    publish("r-" + rev)
                # An uncertain upload is read back. Never repeat a deploy from a missing ACK.
                verify_assets(f"https://r-{rev}.{PROJECT}.pages.dev", rev, manifest)
                result["phase"] = "preview_verified"
                store(journal, result)
            if result["phase"] == "preview_verified":
                require(deployment(project(a.account)) == predecessor, "Pages predecessor changed")
                result["phase"] = "production_upload_started"
                store(journal, result)
                publish("main")
            active = project(a.account)["canonical_deployment"]
            require(matching(active, rev, marker), "expected Pages release not active; reconcile with provider")
            if result["phase"] == "production_upload_started":
                result.update(edge=active["id"], phase="edge_promoted")
                store(journal, result)
            require(result["edge"] == active["id"], "canonical deployment identity changed")
            if result["phase"] == "edge_promoted":
                host = active["url"]
                require(re.fullmatch(r"https://[a-z0-9-]+\.vink-production\.pages\.dev", host), "wrong provider host")
                verify_assets(host, rev, manifest)
                run(HERE / "origin.sh", "plan", *origin_args)
                result["phase"] = "origin_switch_started"
                store(journal, result)
                if not origin:
                    run(HERE / "origin.sh", "apply", *origin_args)
            if origin and not advance_origin(origin, journal, result, a.mode, evidence):
                print(json.dumps(result))
                return
            # Read the actual container IDs, revision, digests and readiness before accepting a lost ACK.
            run(HERE / "origin.sh", "observe", rev, rev, *origin_args[2:])
            result.update(origin_release=rev, phase="origin_ready")
            store(journal, result)
            subprocess.run([HERE.parent.parent / "production/smoke-edge.sh", "edge", "--release", rev], check=True)
            result["phase"] = "http_verified"
            store(journal, result)
            browser_check(chrome, a.reader, rev)
            result["phase"] = "browser_verified"
            store(journal, result)
            store(STATE / (active["id"] + ".json"), result)
            store(previous, result)
            journal.unlink()
            maintenance.sync_dir(journal.parent)
            print(json.dumps(result))


if __name__ == "__main__":
    main()
