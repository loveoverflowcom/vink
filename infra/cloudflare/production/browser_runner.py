#!/usr/bin/env python3
"""Run the checked-in guest browser smoke on a dedicated machine over pinned SSH."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import shlex
import subprocess
import sys
import tempfile


MAX_MESSAGE = 4096
PUBLIC_ORIGIN = "https://vink.forum"
UUID = r"[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}"
FIELDS = {"protocol", "phase", "revision", "nonce", "reader", "preview", "edge"}
SSH_CONFIG_FIELDS = {"host", "hostname", "user", "port", "identityfile", "hostkeyalias"}


def require(ok, message):
    if not ok:
        raise ValueError(message)


def matches(value, pattern):
    return isinstance(value, str) and re.fullmatch(pattern, value) is not None


def checked_request(request):
    require(isinstance(request, dict) and set(request) == FIELDS and type(request["protocol"]) is int
            and request["protocol"] == 1, "invalid browser request fields")
    require(matches(request["revision"], r"[0-9a-f]{40}") and matches(request["nonce"], r"[0-9a-f]{64}"),
            "invalid browser revision or nonce")
    if request["phase"] == "preflight":
        require(all(request[field] is None for field in ("reader", "preview", "edge")),
                "preflight cannot identify a deployment or Reader")
    else:
        require(request["phase"] == "smoke" and matches(request["preview"], UUID) and matches(request["edge"], UUID)
                and (request["reader"] is None or matches(request["reader"], UUID)), "invalid browser smoke target")
    return request


def message(raw):
    require(len(raw) <= MAX_MESSAGE, "browser message too large")
    return json.loads(raw)


def trusted_path(path):
    require(matches(str(path), r"/[A-Za-z0-9_./-]+") and path.is_file() and path.resolve() == path,
            "browser SSH inputs must be absolute regular files")
    require(all(p.stat().st_uid == 0 and not p.stat().st_mode & 0o022 for p in (path, *path.parents)),
            "browser SSH inputs must be root-owned and protected from writes")


def checked_config(contents, host):
    fields = {}
    for line in contents.decode("ascii").splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split()
        require(len(parts) == 2, "browser SSH config requires one value per directive")
        name, value = parts[0].lower(), parts[1]
        require(name in SSH_CONFIG_FIELDS and name not in fields, "unsupported browser SSH config directive")
        fields[name] = value
    require(fields.get("host") == host and {"hostname", "user", "identityfile"} <= fields.keys(),
            "browser SSH config must name the alias, endpoint, user and dedicated identity")
    require(matches(fields["identityfile"], r"/[A-Za-z0-9_./-]+"), "browser SSH identity must be an absolute path")


class RemoteBrowser:
    def __init__(self, host, config, known_hosts):
        require(matches(host, r"[a-zA-Z0-9][a-zA-Z0-9.-]{0,62}"), "invalid browser SSH alias")
        self.host = host
        self.files = {"ssh_config": config, "known_hosts": known_hosts}
        self.binding = {"host": host, **self.fingerprints()}
        self.command = [
            "/usr/bin/ssh", "-F", str(config), "-T", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10",
            "-o", "StrictHostKeyChecking=yes", "-o", "UpdateHostKeys=no", "-o", "IdentitiesOnly=yes",
            "-o", "VerifyHostKeyDNS=no", "-o", "IdentityAgent=none",
            "-o", "GlobalKnownHostsFile=/dev/null", "-o", f"UserKnownHostsFile={known_hosts}",
            "-o", "ClearAllForwardings=yes", "-o", "ControlMaster=no", "-o", "ControlPath=none",
            "-o", "PermitLocalCommand=no", "-o", "ProxyCommand=none", "-o", "ProxyJump=none",
            "-o", "ForwardAgent=no", "-o", "ForwardX11=no", "--", host, "vink-release-browser",
        ]

    def fingerprints(self):
        result = {}
        for name, path in self.files.items():
            trusted_path(path)
            contents = path.read_bytes()
            if name == "ssh_config":
                checked_config(contents, self.host)
            result[name + "_sha256"] = hashlib.sha256(contents).hexdigest()
        return result

    def check(self, phase, revision, *, reader=None, preview=None, edge=None):
        require(all(self.binding[key] == value for key, value in self.fingerprints().items()),
                "browser SSH trust changed")
        request = checked_request(dict(protocol=1, phase=phase, revision=revision, nonce=secrets.token_hex(32),
                                       reader=reader, preview=preview, edge=edge))
        reply = subprocess.run(self.command, input=json.dumps(request).encode(), stdout=subprocess.PIPE, check=True,
                               timeout=180, env={"PATH": os.defpath})
        receipt = message(reply.stdout)
        require(isinstance(receipt, dict) and receipt.pop("result", None) == "passed"
                and checked_request(receipt) == request, "browser runner reply differs from the current request")
        return hashlib.sha256(reply.stdout).hexdigest()


def browser_executable(chrome, temp):
    if sys.platform != "darwin":
        return chrome
    # A fresh Chrome profile on macOS cannot depend on access to a login keychain.
    launcher = Path(temp) / "chrome"
    launcher.write_text(f'#!/bin/sh\nexec {shlex.quote(str(chrome))} --use-mock-keychain "$@"\n')
    launcher.chmod(0o700)
    return launcher


def run_request(request, repo, chrome, node):
    checked_request(request)
    require(os.geteuid() != 0, "browser runner must be unprivileged")
    require(repo.is_absolute() and repo.is_dir() and all(
        program.is_absolute() and os.access(program, os.X_OK) for program in (node, chrome)),
        "browser runner requires an absolute checkout, Chrome and Node")
    script = repo / "apps/vink-web/e2e/browser-smoke.mjs"
    require(script.is_file() and all(not os.access(p, os.W_OK) for p in (repo, repo / ".git", script)),
            "browser runner checkout must be read-only")
    # Only public traffic and a disposable Chrome profile are needed on the runner.
    with tempfile.TemporaryDirectory(prefix="vink-browser-") as temp:
        env = {"PATH": os.defpath + ":/usr/local/bin", "HOME": temp, "GIT_OPTIONAL_LOCKS": "0",
               "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_COUNT": "1",
               "GIT_CONFIG_KEY_0": "safe.directory", "GIT_CONFIG_VALUE_0": str(repo)}
        options = {"cwd": repo, "env": env, "check": True, "timeout": 150}
        for git_args, expected in ((["rev-parse", "HEAD"], request["revision"]),
                                   (["status", "--porcelain", "--untracked-files=all"], "")):
            result = subprocess.run(["git", *git_args], stdout=subprocess.PIPE, text=True, **options)
            require(result.stdout.strip() == expected, "browser checkout differs from release or is dirty")
        if request["phase"] == "preflight":
            chrome_version = subprocess.run([chrome, "--version"], stdout=subprocess.PIPE,
                                            text=True, **options).stdout.strip()
            node_version = subprocess.run([node, "--version"], stdout=subprocess.PIPE,
                                          text=True, **options).stdout.strip()
            require(re.fullmatch(r"(?:Google Chrome(?: for Testing)?|Chromium|HeadlessChrome) [0-9]+(?:\.[0-9]+){3}.*",
                                 chrome_version), "external runner requires Chrome")
            version = re.fullmatch(r"v([0-9]+)\.[0-9]+\.[0-9]+", node_version)
            require(version and int(version[1]) >= 22, "external runner requires Node 22 or newer")
        else:
            reader = request["reader"]
            args = ["--reader", reader] if reader else ["--empty-catalog", "true"]
            subprocess.run([node, script, "--phase", "production-edge",
                            "--base", PUBLIC_ORIGIN, "--chrome", browser_executable(chrome, temp),
                            "--state", Path(temp) / "browser.json",
                            "--revision", request["revision"], *args], stdout=sys.stderr, **options)
    return {**request, "result": "passed"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--chrome", type=Path, required=True)
    parser.add_argument("--node", type=Path, required=True)
    args = parser.parse_args()
    request = message(sys.stdin.buffer.read(MAX_MESSAGE + 1))
    print(json.dumps(run_request(request, args.repo, args.chrome, args.node)))


if __name__ == "__main__":
    main()
