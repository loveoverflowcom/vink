#!/usr/bin/env python3
"""Forced command for the release-only SSH key. Host inputs stay on the release machine."""
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

# Installed in a root-owned directory beside artifacts.py. Use sudo -n for this fixed
# receiver only; sshd's forced command has no PTY or forwarding and cannot supply argv.
from artifacts import SHA, require, validate


def prepare(rev):
    base = Path("/opt/vink-prod/releases")
    tree = base / rev
    require(base.resolve() == base and base.stat().st_uid == 0 and not base.stat().st_mode & 0o022,
            "release directory must be owned and writable only by root")
    if not tree.exists():
        with tempfile.TemporaryDirectory(dir=base) as tmp:
            candidate = Path(tmp) / "tree"
            subprocess.run(["git", "clone", "--depth", "1", "--single-branch", "--branch", "develop",
                            "--no-tags", "https://github.com/loveoverflowcom/vink.git", candidate], check=True,
                           umask=0o022)
            head = subprocess.check_output(["git", "-C", candidate, "rev-parse", "HEAD"], text=True).strip()
            require(head == rev, "develop moved; qualify a new release instead of deploying another checkout")
            subprocess.run(["npm", "ci", "--prefix", "infra/cloudflare", "--ignore-scripts",
                            "--no-audit", "--no-fund"], cwd=candidate, check=True, umask=0o022)
            candidate.rename(tree)
    return tree


def main():
    data = sys.stdin.buffer.read(8193)
    require(len(data) <= 8192, "unexpected release request size")
    request = json.loads(data)
    require(set(request) == {"release", "previous_origin", "previous_edge", "reader"}, "unexpected fields")
    spec = request["release"]
    validate(spec)
    require(SHA.fullmatch(request["previous_origin"]) and
            (request["previous_edge"] == "none" or re.fullmatch("[a-f0-9-]{36}", request["previous_edge"])),
            "invalid predecessor")
    require(re.fullmatch("[a-f0-9-]{36}", request["reader"]), "invalid reader")
    tree = prepare(spec["source_sha"])
    # Validate retained trees before reusing them (including an explicitly selected rollback).
    require(tree.resolve() == tree and tree.stat().st_uid == 0 and
            subprocess.check_output(["git", "-C", tree, "rev-parse", "HEAD"], text=True).strip() == spec["source_sha"] and
            not subprocess.check_output(["git", "-C", tree, "status", "--porcelain"]), "unprepared release tree")
    with tempfile.TemporaryDirectory() as tmp:
        receipt = Path(tmp) / "release.json"
        receipt.write_text(json.dumps(spec))
        args = [receipt, "--account", os.environ["CLOUDFLARE_ACCOUNT_ID"],
                "--previous-origin", request["previous_origin"], "--previous-edge", request["previous_edge"],
                "--reader", request["reader"]]
        review = tree.parent.parent / "schema-reviews" / (request["previous_origin"] + "-" + spec["source_sha"] + ".json")
        if review.exists():
            args.extend(["--schema-review", review])
        command = tree / "infra/cloudflare/production/release.py"
        for mode in ("plan", "apply"):
            subprocess.run([sys.executable, command, mode, *args], cwd=tree, check=True)


if __name__ == "__main__":
    main()
