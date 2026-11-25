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
import time
from urllib.request import Request, urlopen

from artifacts import MAX_FILE, PROJECT, encode, record, require, sha, stage, validate
from browser_runner import RemoteBrowser
import maintenance

HERE = Path(__file__).resolve().parent
STATE = Path("/var/lib/vink-production-release")
WRANGLER = HERE.parent / "node_modules/.bin/wrangler"
SPEC_FIELDS = ("environment", "public_origin", "source_sha", "platform", "images")
READER_UUID = re.compile(r"[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}")
EMPTY_CATALOG = "NOT_RUN: verified empty catalog"
PUBLISHED_READER = "PASS: verified published Reader"
PUBLISHED_READER_HTTP = "PASS: verified published Reader API over HTTP"


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


def pages_read(account, suffix=""):
    endpoint = f"https://api.cloudflare.com/client/v4/accounts/{account}/pages/projects/{PROJECT}{suffix}"
    req = Request(endpoint, headers={"Authorization": "Bearer " + os.environ["CLOUDFLARE_API_TOKEN"]})
    with urlopen(req, timeout=30) as response:
        payload = json.load(response)
    require(payload["success"], "Pages read failed")
    return payload["result"]


def project(account):
    p = pages_read(account)
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


def preview_branch(rev):
    # Keep the release branch itself within Pages' observed 28-character alias limit.
    return "r-" + rev[:26]


def preview_receipt(account, rev, marker):
    branch = preview_branch(rev)
    alias = f"https://{branch}.{PROJECT}.pages.dev"
    for _ in range(12):
        matches = []
        for item in pages_read(account, "/deployments?env=preview"):
            meta = item.get("deployment_trigger", {}).get("metadata", {})
            if (item.get("environment") == "preview" and meta.get("branch") == branch
                    and meta.get("commit_hash") == rev and meta.get("commit_message") == marker):
                matches.append(item)
        require(len(matches) <= 1, "matching Pages preview is ambiguous")
        if matches and matches[0]["latest_stage"]["status"] == "success":
            preview = matches[0]
            ident, url = preview["id"], preview["url"]
            require(re.fullmatch(r"[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}", ident)
                    and url == f"https://{ident[:8]}.{PROJECT}.pages.dev", "invalid immutable Pages preview URL")
            require(alias in (preview.get("aliases") or []), "provider did not attach the Worker fallback alias")
            return {"preview_deployment": ident, "preview_url": url, "preview_alias": alias}
        time.sleep(1)
    raise ValueError("matching successful Pages preview not observed; reconcile with provider before resuming")


def export(spec, archive, *, retained=False):
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
        manifest = record(package / "pkg", spec, retained=retained)
        dest = archive / rev
        if dest.exists():
            require(json.loads((dest / "manifest.json").read_bytes()) == manifest, "conflicting release for SHA")
        else:
            store(package / "manifest.json", manifest)
            package.rename(dest)
        return manifest


def read_asset(url):
    output = subprocess.check_output([
        "curl", "-q", "--proto", "=https", "--silent", "--show-error", "--max-time", "90",
        "--max-filesize", str(MAX_FILE), "--header", "Accept-Encoding: identity",
        "--write-out", "\n%{http_code}\t%{content_type}\t%header{x-vink-asset-revision}\t%header{x-vink-asset-route}", url,
    ])
    body, _, metadata = output.rpartition(b"\n")
    status, mime, revision, route = metadata.decode("ascii").split("\t")
    return status, mime.split(";", 1)[0].lower(), revision, route, body


def verify_asset(host, rev, name, entry, *, serving_edge=False):
    url = f"{host}/_v/{rev}/pkg/{name}"
    status, mime, revision, route, body = read_asset(url)
    require(status == "200" and mime == entry["type"] and revision == rev and sha(body) == entry["sha256"],
            f"Pages asset mismatch: {url} (HTTP {status})")
    if serving_edge:
        require(route in {"inventory/r26", "fallback/r26"},
                f"serving Worker has no verified short-alias route for {url}")


def verify_assets(host, rev, manifest, *, serving_edge=False):
    for name, entry in manifest["assets"].items():
        verify_asset(host, rev, name, entry, serving_edge=serving_edge)


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
    owners = ["vink-app", "vink-identity"]
    if any(maintenance.has_notifications(releases / rev) for rev in (old, new)):
        owners.append("vink-notifications")
    for owner in owners:
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


def assert_empty_catalog():
    status, mime, _, _, body = read_asset("https://vink.forum/api/v1/works")
    require(status == "200" and mime == "application/json", "public catalog unreachable")
    data = json.loads(body)
    require(isinstance(data, dict) and data.get("works") == []
            and "next_cursor" in data and data["next_cursor"] is None,
            "empty-catalog smoke requires an empty public catalog")


def verify_catalog_smoke(reader=None):
    if reader is None:
        assert_empty_catalog()
        return
    status, mime, _, _, body = read_asset(f"https://vink.forum/api/v1/works/{reader}/reader")
    require(status == "200" and mime == "application/json", "public Reader unreachable")
    data = json.loads(body)
    require(isinstance(data, dict) and data.get("status") == "published" and data.get("work_id") == reader,
            "resume requires the selected Reader to be published")


def rollback_baseline(a, spec, archive, active, canonical, pending, interrupted):
    """Bind a reverse rollout to the immediate predecessor of this controller's rollout."""
    active_raw = active.read_bytes()
    saved = pending.get("rollback_from") if pending else None
    if saved and saved["interrupted"]:
        require(re.fullmatch(r"[0-9a-f]{64}", saved["sha256"]), "invalid rollback journal fingerprint")
        interrupted = (STATE / "incidents" / (saved["sha256"] + ".http-rollback.json")).read_bytes()
    raw = interrupted if interrupted is not None else active_raw
    source = json.loads(raw)
    binding = {"sha256": sha(raw), "interrupted": interrupted is not None}
    require(saved is None or binding == saved, "rollback source changed")
    prior = spec["source_sha"]
    controller = a.previous_origin
    require(prior != controller and source["account"] == a.account and source["project"] == PROJECT
            and source["edge"] == a.previous_edge and source["previous_origin"] == prior
            and source["release"]["source_sha"] == controller and source["schema_action"] == "same"
            and source.get("browser_coverage", {}).get("status") == "NOT_RUN"
            and "rollback_from" not in source, "rollback requires the immediate predecessor of an HTTP-only rollout")
    if interrupted is not None:
        active_receipt = json.loads(active_raw)
        require(source["phase"] in {"origin_switch_started", "origin_ready", "http_verified"}
                and source.get("origin_release", controller) == controller
                and active_receipt["account"] == a.account and active_receipt["edge"] == source["previous_edge"]
                and active_receipt["origin_release"] == prior and active_receipt["release"] == spec
                and active_receipt["retained"][prior] == source["retained"][prior],
                "interrupted rollout must agree with the active predecessor receipt")
    else:
        require(source["phase"] == "http_only_verified" and source["origin_release"] == controller,
                "rollback requires an observed HTTP-only rollout")
    if not pending:
        require(matching(canonical, controller, "vink-release-" + source["worker_digest"]),
                "Pages does not match the rollout being reversed")
    manifests = {}
    for expected in (spec, source["release"]):
        rev = expected["source_sha"]
        manifest = (archive / rev / "manifest.json").read_bytes()
        require(sha(manifest) == source["retained"][rev], "rollback archive differs from rollout receipt")
        manifests[rev] = json.loads(manifest)
        require({key: manifests[rev][key] for key in SPEC_FIELDS} == expected,
                "rollback receipt differs from archived release or images")
    return source, binding, manifests[controller]


def preserve_rollback_journal(journal, raw, binding):
    require(journal.read_bytes() == raw and sha(raw) == binding["sha256"], "rollback source journal changed")
    incidents = STATE / "incidents"
    if not incidents.exists():
        incidents.mkdir()
        maintenance.sync_dir(STATE)
    archived = incidents / (binding["sha256"] + ".http-rollback.json")
    if not archived.exists():
        maintenance.durable(archived, raw)
    require(archived.read_bytes() == raw, "rollback archive differs from source journal")


def browser_env(repo, home):
    return {"PATH": os.defpath + ":/usr/local/bin", "HOME": home, "GIT_OPTIONAL_LOCKS": "0",
            "GIT_CONFIG_COUNT": "1", "GIT_CONFIG_KEY_0": "safe.directory",
            "GIT_CONFIG_VALUE_0": str(repo)}


def browser_preflight(chrome):
    user = pwd.getpwnam("vink-release-check")
    require(user.pw_uid > 0, "dedicated browser user required")
    repo = HERE.parents[2]
    with tempfile.TemporaryDirectory(prefix="vink-browser-") as tmp:
        os.chown(tmp, user.pw_uid, user.pw_gid)
        args = {"user": user.pw_uid, "group": user.pw_gid, "extra_groups": [],
                "env": browser_env(repo, tmp), "cwd": repo, "stdout": subprocess.DEVNULL}
        subprocess.run(["git", "status", "--porcelain"], check=True, **args)
        subprocess.run([chrome, "--version"], check=True, **args)
        subprocess.run([os.environ.get("VINK_RELEASE_NODE", "node"), "--version"], check=True, **args)


def browser_check(chrome, reader, rev, *, empty_catalog=False):
    # The browser processes public content with no deployment credentials or root privileges.
    user = pwd.getpwnam("vink-release-check")
    require(user.pw_uid > 0, "dedicated browser account required")
    repo = HERE.parents[2]
    with tempfile.TemporaryDirectory(prefix="vink-browser-") as tmp:
        os.chown(tmp, user.pw_uid, user.pw_gid)
        case = ["--empty-catalog", "true"] if empty_catalog else ["--reader", reader]
        subprocess.run([os.environ.get("VINK_RELEASE_NODE", "node"), repo / "apps/vink-web/e2e/browser-smoke.mjs",
                        "--phase", "production-edge", "--chrome", chrome, "--state", Path(tmp) / "browser.json",
                        "--revision", rev, *case, "--base", "https://vink.forum"], check=True, cwd=repo,
                       user=user.pw_uid, group=user.pw_gid, extra_groups=[],
                       env=browser_env(repo, tmp))


def matching(active, rev, marker):
    if not active:
        return False
    metadata = active["deployment_trigger"]["metadata"]
    return (active["environment"] == "production" and active["latest_stage"]["status"] == "success"
            and metadata["commit_hash"] == rev and metadata["commit_message"] == marker)


def fallback_proof(path, result, manifest):
    require(path is not None, "supply the two-PoP fallback receipt; use --preview-only to stage")
    raw = path.read_bytes()
    evidence = json.loads(raw)
    require(evidence["predecessor"] == result["previous_edge"]
            and evidence["preview_deployment"] == result["preview_deployment"]
            and evidence["revision"] == result["release"]["source_sha"],
            "fallback receipt does not match this rollout")
    require(0 <= time.time() - evidence["measured_at"] <= 900, "fallback receipt is older than 15 minutes")
    asset = manifest["assets"][evidence["asset"]]
    pops = set()
    for observed in evidence["observations"]:
        ray = re.fullmatch(r"[0-9a-f]{16}-([A-Z]{3})", observed["cf_ray"])
        require(ray and observed["status"] == 200 and observed["route"] == "fallback/r26"
                and observed["sha256"] == asset["sha256"] and observed["mime"] == asset["type"]
                and observed["revision"] == evidence["revision"], "fallback observation differs from manifest")
        pops.add(ray[1])
    require(len(pops) >= 2, "fallback requires observations from two distinct Cloudflare PoPs")
    return {"sha256": sha(raw), "asset": evidence["asset"], "pops": sorted(pops)}


def publish(public, branch, rev, marker):
    subprocess.run([WRANGLER, "pages", "deploy", public, "--project-name", PROJECT, "--branch", branch,
                    "--commit-hash", rev, "--commit-message", marker], cwd=public.parent, check=True)


def bridge(a, spec, remote, journal):
    """Publish a Pages-only predecessor, retaining the original rollback as incident evidence."""
    activefile = STATE / "active.json"
    archive = STATE / "archives"
    incidents = STATE / "incidents"
    active = remote["canonical_deployment"]
    if not journal.exists():
        done = json.loads(activefile.read_bytes())
        require(done["phase"] == "edge_bridge_verified" and done["account"] == a.account
                and done["aborted_release"] == spec["source_sha"] and done["failed_edge"] == a.previous_edge
                and done["origin_release"] == a.previous_origin and deployment(remote) == done["edge"]
                and matching(active, a.previous_origin, "vink-bridge-" + done["worker_digest"])
                and sha((incidents / done["rollback_archive"]).read_bytes()) == done["rollback_sha256"],
                "completed bridge differs from provider or rollback archive")
        return done
    raw = journal.read_bytes()
    pending = json.loads(raw)
    old = a.previous_origin
    require(pending["phase"] == "origin_rolled_back" and pending["account"] == a.account
            and pending["project"] == PROJECT and pending["release"] == spec
            and pending["origin_release"] == pending["previous_origin"] == old
            and pending["edge"] == a.previous_edge and not maintenance.GUARD.exists(),
            "bridge requires a matching, completed origin rollback")
    require(WRANGLER.is_file() and not run("git", "-C", HERE, "status", "--porcelain"),
            "bridge requires the clean controller and locked Wrangler")
    controller = run("git", "-C", HERE, "rev-parse", "HEAD")
    require(re.fullmatch(r"[0-9a-f]{40}", controller), "invalid bridge controller revision")
    retained = {d.name: sha((d / "manifest.json").read_bytes()) for d in archive.iterdir()}
    require(retained == pending["retained"] and {old, spec["source_sha"]} <= retained.keys(),
            "retained archives differ from rollback journal")
    old_manifest = json.loads((archive / old / "manifest.json").read_bytes())
    previous = {name: old_manifest[name] for name in SPEC_FIELDS}
    validate(previous)
    require(previous["source_sha"] == old and previous["platform"] == spec["platform"], "wrong origin archive")
    require(a.schema_review and sha(a.schema_review.read_bytes()) == pending["schema_review"],
            "bridge requires the original schema review")
    evidence = json.loads(a.schema_review.read_bytes())
    origin = maintenance.Origin(old, spec, previous["images"])
    rolled_back = origin.read_state(origin.state_key(evidence))
    require(rolled_back["phase"] == "rolled_back", "origin journal has not completed rollback")
    origin.check_ledgers(origin.before)
    containers = {name: origin.container(name, running=True) for name in maintenance.running_services(previous["images"])}
    run(HERE / "origin.sh", "observe", old, old, *[previous["images"][k] for k in ("web", "worker", "identity")],
        previous["platform"], "same", previous["images"].get("notifications", ""))
    digest = sha(raw)
    incidents.mkdir(exist_ok=True)
    archived = incidents / (digest + ".json")
    intent = incidents / (digest + ".bridge.json")
    with tempfile.TemporaryDirectory(dir=STATE) as temp:
        public = Path(temp) / "public"
        worker_digest = stage(archive, old, public, pending["legacy_assets"])
        marker = "vink-bridge-" + worker_digest
        request = {"account": a.account, "failed_edge": pending["edge"], "controller": controller,
                   "worker_digest": worker_digest, "rollback_sha256": digest}
        if intent.exists():
            require(json.loads(intent.read_bytes()) == request and matching(active, old, marker),
                    "bridge upload outcome uncertain; read provider before any retry")
        else:
            require(deployment(remote) == pending["edge"]
                    and matching(active, spec["source_sha"], "vink-release-" + pending["worker_digest"]),
                    "Pages differs from rolled-back release")
            require(not activefile.exists() or json.loads(activefile.read_bytes())["edge"] == pending["previous_edge"],
                    "existing active receipt differs from rollback predecessor")
            store(intent, request)
            publish(public, "main", old, marker)
            active = project(a.account)["canonical_deployment"]
            require(matching(active, old, marker), "bridge upload has not been observed")
        result = {**request, "phase": "edge_bridge_verified", "project": PROJECT, "edge": active["id"],
                  "release": previous, "origin_release": old, "retained": retained, "legacy_assets": pending["legacy_assets"],
                  "aborted_release": spec["source_sha"], "rollback_archive": archived.name}
        host = active["url"]
        require(re.fullmatch(r"https://[a-z0-9-]+\.vink-production\.pages\.dev", host), "wrong bridge provider host")
        for rev in retained:
            manifest = json.loads((archive / rev / "manifest.json").read_bytes())
            verify_assets(host, rev, manifest, serving_edge=True)
            verify_assets(spec["public_origin"], rev, manifest, serving_edge=True)
        subprocess.run([HERE.parent.parent / "production/smoke-edge.sh", "edge", "--release", old], check=True)
        require(deployment(project(a.account)) == active["id"] and not maintenance.GUARD.exists(),
                "bridge or origin changed during verification")
        require(all(origin.container(name, running=True) == cid for name, cid in containers.items()),
                "origin containers changed during the Pages-only release")
        origin.check_ledgers(origin.before)
        require(journal.read_bytes() == raw, "rollback journal changed")
        if not archived.exists():
            os.link(journal, archived)
            maintenance.sync_dir(incidents)
        require(archived.read_bytes() == raw, "rollback archive differs")
        store(activefile, result)
        journal.unlink()
        maintenance.sync_dir(STATE)
        return result


def advance_origin(origin, journal, result, mode, evidence):
    """Return whether the origin can enter ordinary public verification."""
    if mode == "rollback-origin":
        require(result["phase"] in {"origin_switch_started", "origin_prepared",
                                    "origin_rollback_started", "origin_rolled_back"},
                "origin may have accepted writes; rollback is unavailable")
        if result["phase"] in {"origin_switch_started", "origin_prepared"}:
            started = origin.rollback_started(evidence, prepared=result["phase"] == "origin_prepared")
            if not started:
                # The predecessor is still serving; acknowledge rollback without inventing origin state.
                result.update(phase="origin_rolled_back", origin_release=result["previous_origin"])
                store(journal, result)
                return False
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
    p.add_argument("mode", choices=["plan", "apply", "resume", "open", "rollback-origin", "bridge", "status"])
    p.add_argument("release", type=Path, help="public JSON copied from qualified GHCR publish receipt")
    p.add_argument("--maintenance", action="store_true", help="prepare origin offline and require an explicit open")
    p.add_argument("--bootstrap-receipt", type=Path, help="reconcile a manually verified Pages predecessor")
    p.add_argument("--preview-only", action="store_true", help="stage candidate assets and stop before production promotion")
    p.add_argument("--fallback-proof", type=Path, help="two-PoP receipt for the staged maintenance release")
    p.add_argument("--schema-review", type=Path, help="rehearsal evidence when migrations differ")
    p.add_argument("--account", required=True)
    p.add_argument("--reader", help="public production work UUID for browser qualification")
    p.add_argument("--empty-catalog", action="store_true", help="verify empty catalog, report published Reader NOT_RUN")
    p.add_argument("--resume-with-reader", metavar="UUID",
                   help="verify a published Reader for one resume attempt of a rolling empty-catalog release")
    p.add_argument("--browser-host", help="pinned SSH alias for an external rolling release browser")
    p.add_argument("--http-only", metavar="REASON", help="use HTTP smoke only for a same-schema rolling release")
    p.add_argument("--rollback-predecessor", action="store_true",
                   help="with --http-only, use the current release controller to restore its recorded predecessor")
    p.add_argument("--previous-origin", required=True)
    p.add_argument("--previous-edge", required=True, help="observed Pages deployment id or 'none' on bootstrap")
    a = p.parse_args()
    browser_coverage = None
    if a.http_only is not None:
        require(a.mode in {"plan", "apply", "resume"} and not a.maintenance and a.schema_review is None
                and a.browser_host is None and a.previous_edge != "none",
                "HTTP-only requires an existing predecessor and a rolling plan/apply/resume without a schema review or browser host")
        require(1 <= len(a.http_only) <= 240 and a.http_only.isprintable() and a.http_only == a.http_only.strip(),
                "HTTP-only requires a single-line reason of 1 to 240 characters")
        browser_coverage = {"status": "NOT_RUN", "reason": a.http_only}
    require(not a.rollback_predecessor or browser_coverage, "rolling predecessor rollback requires --http-only")
    final_phase = "http_only_verified" if browser_coverage else "browser_verified"
    if a.browser_host is not None:
        require(a.mode in {"plan", "apply", "resume"} and not a.maintenance,
                "external browser only supports rolling plan/apply/resume")
    if a.resume_with_reader is not None:
        require(a.mode == "resume" and a.empty_catalog and not a.maintenance and not a.preview_only
                and READER_UUID.fullmatch(a.resume_with_reader),
                "--resume-with-reader needs a published UUID and a rolling --empty-catalog resume")
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
        if a.mode == "bridge":
            print(json.dumps(bridge(a, spec, remote, journal)))
            return
        require(not a.preview_only or a.mode in {"apply", "resume"}, "preview-only requires apply or resume")
        if a.empty_catalog:
            require(a.reader is None, "empty-catalog smoke cannot use a reader UUID")
        else:
            require(a.reader and READER_UUID.fullmatch(a.reader),
                    "supply an existing public reader UUID")
        external = None
        chrome = None
        if a.browser_host is not None:
            external = RemoteBrowser(a.browser_host, Path(os.environ["VINK_RELEASE_BROWSER_SSH_CONFIG"]),
                                     Path(os.environ["VINK_RELEASE_BROWSER_KNOWN_HOSTS"]))
        resume = a.mode in {"resume", "open", "rollback-origin"}
        require(not (a.bootstrap_receipt or a.mode in {"open", "rollback-origin"}) or a.maintenance,
                "manual origin commands require maintenance opt-in")
        journal_raw = journal.read_bytes() if journal.exists() else None
        interrupted = journal_raw if a.rollback_predecessor and not resume else None
        require(interrupted is not None or resume == (journal_raw is not None),
                "pending release requires status/resume; new release requires apply")
        pending = json.loads(journal_raw) if resume else None
        predecessor = a.previous_edge if a.previous_edge != "none" else None
        if resume:
            require(a.rollback_predecessor == ("rollback_from" in pending), "resume must use the original rollback mode")
            require(pending.get("browser_coverage") == browser_coverage,
                    "resume must use the original browser coverage and reason")
            if browser_coverage:
                require(pending["http_reader"] == a.reader, "resume must use the original HTTP Reader")
            require(pending.get("browser_runner") == (external.binding if external else None),
                    "resume must use the original browser runner")
            require(pending["release"] == spec and pending["account"] == a.account and
                    pending["previous_origin"] == a.previous_origin and pending["previous_edge"] == predecessor,
                    "resume must use original inputs")
            if pending["schema_action"] != "maintenance":
                require(("reader_coverage" in pending) == a.empty_catalog,
                        "resume must use the original catalog smoke mode")
        else:
            require(current == predecessor, "Pages predecessor changed")
        if not external and not browser_coverage:
            chrome = Path(os.environ["VINK_RELEASE_CHROME"])
            require(chrome.is_absolute() and os.access(chrome, os.X_OK), "read-only guest browser required")
            require(pwd.getpwnam("vink-release-check").pw_uid > 0, "dedicated browser user required")
        controller = a.previous_origin if a.rollback_predecessor else rev
        require(run("git", "-C", HERE, "rev-parse", "HEAD") == controller, "release checkout differs from input")
        require(run("git", "-C", HERE, "status", "--porcelain") == "", "release checkout is dirty")
        require(WRANGLER.is_file(), "install the locked Wrangler in infra/cloudflare")
        if not resume:
            if external:
                external.check("preflight", rev)
            elif not browser_coverage:
                browser_preflight(chrome)
            if a.empty_catalog:
                assert_empty_catalog()
            elif browser_coverage:
                verify_catalog_smoke(a.reader)
        archive = STATE / "archives"
        archive.mkdir(exist_ok=True)
        # A missing archive after a host loss must not discard assets from the serving deployment.
        previous = STATE / "active.json"
        legacy = rev
        if resume and previous.exists() and pending.get("edge") == current:
            completed = json.loads(previous.read_bytes())
            if completed.get("phase") == final_phase and completed == pending:
                journal.unlink()
                print(json.dumps(completed))
                return
        old_images = None
        rollback_from = None
        rollback_assets = None
        if a.bootstrap_receipt:
            require(predecessor and not previous.exists(), "bootstrap receipt is only for the first controller release")
            active_predecessor = remote["canonical_deployment"] if current == predecessor else None
            old_images = bootstrap_baseline(a.bootstrap_receipt, predecessor, a.previous_origin, spec["platform"],
                                            archive, active_predecessor)
            legacy = a.previous_origin
        elif predecessor:
            require(previous.exists(), "restore the previous release receipt and archives")
            if a.rollback_predecessor:
                last, rollback_from, rollback_assets = rollback_baseline(
                    a, spec, archive, previous, remote["canonical_deployment"], pending, interrupted)
            else:
                last = json.loads(previous.read_bytes())
                require(last["edge"] == predecessor and last["account"] == a.account
                        and last["origin_release"] == a.previous_origin, "active edge differs from receipt")
            legacy = last["legacy_assets"]
            old_images = last["release"]["images"]
            for release, digest in last["retained"].items():
                require(sha((archive / release / "manifest.json").read_bytes()) == digest, "lost retained release")
        schema = schema_action(a.previous_origin, rev, a.schema_review, window=a.maintenance)
        require(not a.empty_catalog or a.maintenance or schema == "same",
                "rolling empty-catalog smoke requires unchanged migrations")
        require(not external or schema == "same", "external rolling browser requires unchanged migrations")
        manifest = export(spec, archive, retained=True) if rollback_from else export(spec, archive)
        images = spec["images"]
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
        if "notifications" in images:
            origin_args.append(images["notifications"])
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
            if external:
                result["browser_runner"] = external.binding
            if browser_coverage:
                result.update(browser_coverage=browser_coverage, http_reader=a.reader)
            if rollback_from:
                result["rollback_from"] = rollback_from
            if a.bootstrap_receipt:
                result["bootstrap_receipt"] = sha(a.bootstrap_receipt.read_bytes())
            if a.empty_catalog:
                result["reader_coverage"] = EMPTY_CATALOG
            if a.mode == "plan":
                print(json.dumps(result))
                return
            if resume:
                require(all(pending.get(k) == v for k, v in result.items()
                            if k != "reader_coverage" or a.maintenance), "release bytes or target changed")
                result = pending
                if a.resume_with_reader:
                    verify_catalog_smoke(a.resume_with_reader)
                    result["reader_resume_work"] = a.resume_with_reader
                    store(journal, result)
                if a.preview_only:
                    require(result["phase"] in {"preview_upload_started", "preview_verified"},
                            "preview-only cannot resume production phases")
                if a.mode == "open":
                    require(result["phase"] in {"origin_prepared", "origin_opening"}, "origin is not prepared")
                if a.mode == "rollback-origin":
                    require(result["phase"] in {"origin_switch_started", "origin_prepared",
                                                "origin_rollback_started", "origin_rolled_back"},
                            "origin may have accepted writes; rollback is unavailable")
            else:
                if a.empty_catalog:
                    assert_empty_catalog()
                require(deployment(project(a.account)) == predecessor, "Pages changed during staging")
                if interrupted is not None:
                    preserve_rollback_journal(journal, interrupted, rollback_from)
                result["phase"] = "preview_upload_started"
                store(journal, result)
            marker = "vink-release-" + worker_digest

            if result["phase"] == "preview_upload_started":
                if not resume:
                    publish(public, preview_branch(rev), rev, marker)
                # Recover the provider receipt if the upload ACK was lost.
                result.update(preview_receipt(a.account, rev, marker))
                verify_assets(result["preview_url"], rev, manifest)
                if rollback_assets:
                    verify_assets(result["preview_url"], controller, rollback_assets)
                result["phase"] = "preview_verified"
                store(journal, result)
            if result["phase"] == "preview_verified":
                remote = project(a.account)
                require(deployment(remote) == predecessor, "Pages predecessor changed")
                verify_assets(result["preview_alias"], rev, manifest)
                if rollback_assets:
                    verify_assets(result["preview_alias"], controller, rollback_assets)
                if a.preview_only:
                    print(json.dumps(result))
                    return
                if external:
                    external.check("preflight", rev)
                elif browser_coverage and resume:
                    verify_assets(result["preview_url"], rev, manifest)
                    run(HERE / "origin.sh", "plan", *origin_args)
                if predecessor:
                    host = remote["canonical_deployment"]["url"]
                    require(re.fullmatch(r"https://[a-z0-9-]+\.vink-production\.pages\.dev", host),
                            "wrong predecessor provider host")
                    previous_manifest = json.loads((archive / a.previous_origin / "manifest.json").read_bytes())
                    verify_asset(host, a.previous_origin, "vink.js", previous_manifest["assets"]["vink.js"],
                                 serving_edge=True)
                    verify_assets(spec["public_origin"], rev, manifest, serving_edge=True)
                    if a.maintenance:
                        result["fallback_proof"] = fallback_proof(a.fallback_proof, result, manifest)
                if a.empty_catalog or browser_coverage:
                    verify_catalog_smoke(a.resume_with_reader or a.reader)
                require(deployment(project(a.account)) == predecessor, "Pages predecessor changed during fallback verification")
                result["phase"] = "production_upload_started"
                store(journal, result)
                publish(public, "main", rev, marker)
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
                verify_assets(result["preview_alias"], rev, manifest)
                if predecessor:
                    verify_assets(spec["public_origin"], rev, manifest, serving_edge=True)
                if rollback_assets:
                    verify_assets(host, controller, rollback_assets)
                    verify_assets(spec["public_origin"], controller, rollback_assets, serving_edge=True)
                run(HERE / "origin.sh", "plan", *origin_args)
                if a.empty_catalog or browser_coverage:
                    verify_catalog_smoke(a.resume_with_reader or a.reader)
                require(deployment(project(a.account)) == result["edge"], "Pages changed before origin switch")
                result["phase"] = "origin_switch_started"
                store(journal, result)
                if not origin:
                    run(HERE / "origin.sh", "apply", *origin_args)
            elif a.mode != "rollback-origin" and result["phase"] in {"origin_switch_started", "origin_prepared"}:
                # Asset access may have changed while maintenance was paused. Keep rollback available.
                verify_assets(result["preview_alias"], rev, manifest)
                if predecessor:
                    verify_assets(spec["public_origin"], rev, manifest, serving_edge=True)
                require(deployment(project(a.account)) == result["edge"], "Pages changed during maintenance")
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
            reader = a.resume_with_reader or a.reader
            if external or browser_coverage:
                verify_assets(result["preview_url"], rev, manifest)
                verify_assets(result["preview_alias"], rev, manifest)
                verify_assets(spec["public_origin"], rev, manifest, serving_edge=True)
                if rollback_assets:
                    verify_assets(spec["public_origin"], controller, rollback_assets, serving_edge=True)
            if external or browser_coverage or a.resume_with_reader:
                verify_catalog_smoke(reader)
            if external:
                result["browser_receipt_sha256"] = external.check(
                    "smoke", rev, reader=reader, preview=result["preview_deployment"], edge=result["edge"])
            elif not browser_coverage:
                if a.empty_catalog and not reader:
                    browser_check(chrome, None, rev, empty_catalog=True)
                else:
                    browser_check(chrome, reader, rev)
            if external or browser_coverage:
                observed = project(a.account)["canonical_deployment"]
                require(matching(observed, rev, marker) and observed["id"] == result["edge"],
                        "Pages changed during release smoke")
                run(HERE / "origin.sh", "observe", rev, rev, *origin_args[2:])
            if external or browser_coverage or a.resume_with_reader:
                verify_catalog_smoke(reader)
            if a.resume_with_reader:
                result["reader_coverage"] = PUBLISHED_READER_HTTP if browser_coverage else PUBLISHED_READER
            elif a.empty_catalog:
                result["reader_coverage"] = EMPTY_CATALOG
                if browser_coverage:
                    result.pop("reader_resume_work", None)
            result["phase"] = final_phase
            store(journal, result)
            store(STATE / (active["id"] + ".json"), result)
            store(previous, result)
            journal.unlink()
            maintenance.sync_dir(journal.parent)
            print(json.dumps(result))


if __name__ == "__main__":
    main()
