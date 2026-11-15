#!/usr/bin/env python3
"""Read one GCS generation using a short-lived reader token on stdin; verify before use."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import urllib.parse
import urllib.request

def validate(data):
    m = json.loads(data)
    assert m["schema"] == "vink-test-release/v1" and m["environment"] == "test"
    assert m["auth_sign_in"] == "disabled" and m["secrets"] == []
    assert m["google_cloud"] == {"project_id": "vink-d0846", "bucket": "vink-d0846-test-config"}
    assert re.fullmatch(r"[a-f0-9]{40}", m["source_sha"])
    i = m["inventory"]
    assert set(i) == {"schema", "public_origin", "platform", "app_image", "worker_image", "identity_image", "host_port", "uid", "socket_dir", "secrets_dir", "dns_target", "dns_ttl"}
    assert i["schema"] == 1 and i["platform"] in ("linux/amd64", "linux/arm64")
    assert i["public_origin"] == "https://test.vink.forum" and i["host_port"] == 3300
    assert i["uid"] == 10001 and i["socket_dir"] == "/run/vink-test-identity" and i["secrets_dir"] == "/etc/vink-test"
    assert i["dns_target"] in ("42.96.18.245", "vink-test-frontend.pages.dev") and i["dns_ttl"] == 300
    for role, package in (("app", "web"), ("worker", "worker"), ("identity", "identity")):
        assert re.fullmatch(r"ghcr.io/loveoverflowcom/vink-" + package + r"@sha256:[a-f0-9]{64}", i[role + "_image"])
    return m

def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ("object", "generation", "sha256", "output"):
        p.add_argument("--" + name, required=True)
    p.add_argument("--pull", action="store_true", help="pull/inspect the three public GHCR digests using empty registry credentials")
    a = p.parse_args()
    assert re.fullmatch(r"[1-9][0-9]+", a.generation), "numeric generation required"
    assert re.fullmatch(r"[a-f0-9]{64}", a.sha256), "SHA-256 required"
    assert re.fullmatch(r"releases/[a-f0-9]{64}/deployment-config.json", a.object), "release object required"
    token = sys.stdin.readline().strip()
    assert token and "\n" not in token
    url = "https://storage.googleapis.com/storage/v1/b/vink-d0846-test-config/o/" + urllib.parse.quote(a.object, safe="")
    url += "?alt=media&generation=" + a.generation
    request = urllib.request.Request(url, headers={"Authorization": "Bearer " + token})
    with urllib.request.urlopen(request, timeout=60) as response:
        data = response.read(1024 * 1024 + 1)
    del token, request
    assert len(data) <= 1024 * 1024
    assert hashlib.sha256(data).hexdigest() == a.sha256, "config checksum mismatch"
    m = validate(data)
    output = Path(a.output)
    output.mkdir(mode=0o700, parents=True, exist_ok=False)
    (output / "deployment-config.json").write_bytes(data)
    (output / "test.toml").write_text("\n".join(k + " = " + json.dumps(v) for k, v in m["inventory"].items()) + "\n")
    print("PASS config generation=" + a.generation + " sha256=" + a.sha256)
    if a.pull:
        with tempfile.TemporaryDirectory(prefix="vink-reader-auth-") as temp:
            auth = Path(temp) / "auth.json"
            auth.write_text('{"auths":{}}')
            auth.chmod(0o600)
            for role in ("app", "worker", "identity"):
                image = m["inventory"][role + "_image"]
                subprocess.run(["podman", "pull", "--authfile", str(auth), "--platform", m["inventory"]["platform"], image], check=True)
                inspected = json.loads(subprocess.check_output(["podman", "image", "inspect", image]))[0]
                assert image in inspected["RepoDigests"]
                assert inspected["Os"] + "/" + inspected["Architecture"] == m["inventory"]["platform"]
                assert inspected["Labels"]["org.opencontainers.image.revision"] == m["source_sha"]
                print("PASS " + role + " " + image)

if __name__ == "__main__":
    main()
