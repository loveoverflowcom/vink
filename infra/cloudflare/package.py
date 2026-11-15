#!/usr/bin/env python3
"""Stage an image-exported site without credentials or provider calls. Output must be new."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import tomllib
from urllib.parse import urlsplit

def digest(data):
    return hashlib.sha256(data).hexdigest()

def canonical(data):
    return (json.dumps(data, ensure_ascii=False, sort_keys=True, indent=2) + "\n").encode()

def main():
    p = argparse.ArgumentParser(description=__doc__)
    for arg in ("site", "inventory", "output", "source-sha", "origin", "account", "project", "branch"):
        p.add_argument("--" + arg, required=True)
    p.add_argument("--environment", choices=("preview", "production"), default="preview")
    a = p.parse_args()
    assert re.fullmatch(r"[a-f0-9]{40}", a.source_sha), "full source SHA required"
    assert re.fullmatch(r"[a-f0-9]{32}", a.account), "account ID required"
    assert re.fullmatch(r"vink-[a-z0-9-]+", a.project), "Vink project required"
    assert (a.branch == "main") == (a.environment == "production"), "branch/environment mismatch"
    origin = urlsplit(a.origin)
    assert origin.scheme == "https" and origin.hostname and not origin.username and not origin.password
    assert origin.path in ("", "/") and not origin.query and not origin.fragment
    inventory = tomllib.loads(Path(a.inventory).read_text())
    if a.environment == "production":
        assert origin.hostname != urlsplit(inventory["public_origin"]).hostname, "cutover requires a separate origin hostname"
    expected = {"schema", "public_origin", "platform", "app_image", "worker_image", "identity_image", "host_port", "uid", "socket_dir", "secrets_dir", "dns_target", "dns_ttl"}
    assert set(inventory) == expected, "only public inventory fields accepted"
    for role, package in (("app", "web"), ("worker", "worker"), ("identity", "identity")):
        assert re.fullmatch(r"ghcr.io/loveoverflowcom/vink-" + package + r"@sha256:[a-f0-9]{64}", inventory[role + "_image"])
    assert inventory["schema"] == 1 and inventory["platform"] in ("linux/amd64", "linux/arm64")
    site, output = Path(a.site).resolve(), Path(a.output)
    assert not output.exists(), "do not mutate an existing release"
    entries = []
    for file in sorted(site.rglob("*")):
        assert not file.is_symlink(), "symlink refused"
        if file.is_dir():
            continue
        rel = file.relative_to(site).as_posix()
        assert rel.startswith("pkg/") and all(not x.startswith(".") for x in Path(rel).parts)
        assert file.suffix in (".js", ".css", ".wasm", ".woff2", ".svg", ".png", ".webmanifest", ".json", ".txt") or rel == "pkg/expressions/LICENSE-ANIMATED.md", rel
        data = file.read_bytes()
        delivery = "pages" if len(data) <= 25 * 1024 * 1024 else "origin"
        assert delivery == "pages" or rel == "pkg/vink_bg.wasm", "unexpected oversized asset"
        entries.append({"path": "/" + rel, "bytes": len(data), "sha256": digest(data), "delivery": delivery})
    assert {"/pkg/vink.js", "/pkg/vink.css", "/pkg/vink_bg.wasm"} <= {x["path"] for x in entries}
    asset_digest = digest(canonical(entries))
    release = {"origin": a.origin.rstrip("/"), "asset_digest": asset_digest,
               "assets": [x["path"] for x in entries if x["delivery"] == "pages"],
               "origin_assets": [x["path"] for x in entries if x["delivery"] == "origin"]}
    template = Path(__file__).with_name("worker.mjs").read_text()
    assert template.count("/*VINK_RELEASE*/ null") == 1
    worker = template.replace("/*VINK_RELEASE*/ null", json.dumps(release)).encode()
    public = output / "public"
    public.mkdir(parents=True)
    for entry in entries:
        if entry["delivery"] == "pages":
            rel = entry["path"].lstrip("/")
            target = public / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(site / rel, target)
    (public / "_worker.js").write_bytes(worker)
    manifest = {"schema": "vink-test-release/v1", "environment": "test", "source_sha": a.source_sha,
                "inventory": inventory, "auth_sign_in": "disabled", "asset_digest": asset_digest,
                "assets": entries, "edge_worker_sha256": digest(worker), "origin": release["origin"],
                "cloudflare": {"account_id": a.account, "project": a.project, "branch": a.branch,
                               "environment": a.environment, "production_branch": "main", "deployment": None},
                "secrets": [], "google_cloud": {"project_id": "vink-d0846", "bucket": "vink-d0846-test-config"}}
    data = canonical(manifest)
    (output / "manifest.json").write_bytes(data)
    (output / "manifest.sha256").write_text(digest(data) + "  manifest.json\n")
    print(json.dumps({"manifest_sha256": digest(data), "asset_digest": asset_digest,
                      "pages_files": len(release["assets"]), "origin_files": len(release["origin_assets"])}))

if __name__ == "__main__":
    main()
