import hashlib
import json
import os
from pathlib import Path
import subprocess
from tempfile import TemporaryDirectory
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import browser_runner as runner

REVISION = "a" * 40
PREVIEW = "01234567-1111-4111-8111-111111111111"
EDGE = "01234567-2222-4222-8222-222222222222"
READER = "01234567-3333-4333-8333-333333333333"


def request(phase):
    return dict(protocol=1, phase=phase, revision=REVISION, nonce="b" * 64,
                reader=None, preview=PREVIEW if phase == "smoke" else None,
                edge=EDGE if phase == "smoke" else None)


class BrowserTransport(unittest.TestCase):
    def test_pins_credentials_configuration_and_current_challenge(self):
        with TemporaryDirectory() as temp, patch.object(runner, "trusted_path"):
            config, hosts = [Path(temp) / x for x in ("ssh.conf", "known_hosts")]
            config.write_text("Host mac-smoke\nHostName 127.0.0.1\nPort 2222\nUser smoke\nIdentityFile /secure/key\n")
            hosts.write_text("pinned fixture")
            browser = runner.RemoteBrowser("mac-smoke", config, hosts)
            reply = json.dumps({**request("smoke"), "result": "passed"}).encode()
            with patch.object(runner.secrets, "token_hex", return_value="b" * 64), \
                    patch.object(runner.subprocess, "run", return_value=SimpleNamespace(stdout=reply)) as call:
                self.assertEqual(browser.check("smoke", REVISION, preview=PREVIEW, edge=EDGE),
                                 hashlib.sha256(reply).hexdigest())
                command = call.call_args.args[0]
                self.assertEqual(command[-3:], ["--", "mac-smoke", "vink-release-browser"])
                self.assertIn("StrictHostKeyChecking=yes", command)
                self.assertIn("IdentityAgent=none", command)
                self.assertIn(f"UserKnownHostsFile={hosts}", command)
                self.assertEqual(set(call.call_args.kwargs["env"]), {"PATH"})
                self.assertEqual(json.loads(call.call_args.kwargs["input"]), request("smoke"))
                for change in ({"nonce": "c" * 64}, {"preview": EDGE}, {"edge": PREVIEW},
                               {"revision": "d" * 40}, {"reader": READER}, {"result": "failed"},
                               {"phase": "preflight"}, {"protocol": True}):
                    with self.subTest(change=change), self.assertRaises(ValueError):
                        call.return_value.stdout = json.dumps({**request("smoke"), "result": "passed", **change}).encode()
                        browser.check("smoke", REVISION, preview=PREVIEW, edge=EDGE)
                call.reset_mock()
                hosts.write_text("changed")
                with self.assertRaisesRegex(ValueError, "trust changed"):
                    browser.check("preflight", REVISION)
                call.assert_not_called()

    def test_ssh_config_cannot_load_additional_trust_or_delegate_authentication(self):
        config = b"Host mac-smoke\nHostName 127.0.0.1\nUser smoke\nIdentityFile /secure/key\n"
        runner.checked_config(config, "mac-smoke")
        for directive in (b"Include /other", b"KnownHostsCommand /other", b"IdentityAgent /agent",
                          b"VerifyHostKeyDNS yes", b"Host fallback", b"IdentityFile /another/key"):
            with self.subTest(directive=directive), self.assertRaises(ValueError):
                runner.checked_config(config + directive, "mac-smoke")

    def test_rejects_unscoped_requests_and_untrusted_paths(self):
        for change in ({"public_origin": "https://other.invalid"}, {"revision": "develop"},
                       {"phase": "unknown"}, {"preview": "other"}, {"reader": "bad"}):
            with self.subTest(change=change), self.assertRaises(ValueError):
                runner.checked_request({**request("smoke"), **change})
        with self.assertRaisesRegex(ValueError, "absolute regular"):
            runner.trusted_path(Path("relative"))
        with self.assertRaisesRegex(ValueError, "invalid browser SSH alias"):
            runner.RemoteBrowser("-oProxyCommand", Path("/config"), Path("/known_hosts"))


class BrowserService(unittest.TestCase):
    def test_mac_launches_real_chrome_with_disposable_keychain(self):
        with TemporaryDirectory() as temp, patch.object(runner.sys, "platform", "darwin"):
            binary = Path(temp) / "Chrome fixture"
            binary.write_text('#!/bin/sh\nprintf "%s\\n" "$@"\n')
            binary.chmod(0o700)
            argv = subprocess.check_output([runner.browser_executable(binary, temp), "about:blank"], text=True)
            self.assertEqual(argv.splitlines(), ["--use-mock-keychain", "about:blank"])

    def test_executes_guest_harness_without_secrets_and_checks_checkout(self):
        with TemporaryDirectory() as temp:
            repo = Path(temp)
            script = repo / "apps/vink-web/e2e/browser-smoke.mjs"
            script.parent.mkdir(parents=True)
            script.touch()
            chrome = Path("/chrome")
            node = Path("/node")
            with patch.object(runner.os, "geteuid", return_value=501), \
                    patch.object(runner.os, "access", side_effect=lambda p, mode: mode == os.X_OK), \
                    patch.dict(os.environ, {"CLOUDFLARE_API_TOKEN": "fixture"}), \
                    patch.object(runner.subprocess, "run") as call:
                call.side_effect = [SimpleNamespace(stdout=REVISION + "\n"), SimpleNamespace(stdout=""), None]
                smoke = request("smoke")
                self.assertEqual(runner.run_request(smoke, repo, chrome, node), {**smoke, "result": "passed"})
                args = call.call_args.args[0]
                self.assertEqual(args[:2], [node, script])
                self.assertIn("https://vink.forum", args)
                self.assertEqual(args[-4:], ["--revision", REVISION, "--empty-catalog", "true"])
                self.assertNotIn("CLOUDFLARE_API_TOKEN", call.call_args.kwargs["env"])
                call.side_effect = [SimpleNamespace(stdout=REVISION + "\n"), SimpleNamespace(stdout=""), None]
                runner.run_request({**smoke, "reader": READER}, repo, chrome, node)
                self.assertEqual(call.call_args.args[0][-2:], ["--reader", READER])
                call.side_effect = [SimpleNamespace(stdout=REVISION + "\n"), SimpleNamespace(stdout=" M file")]
                with self.assertRaisesRegex(ValueError, "dirty"):
                    runner.run_request(smoke, repo, chrome, node)
                for chrome_version, node_version, expected in (
                    ("Google Chrome 158.0.8127.89", "v22.23.5", None),
                    ("fixture", "v22.23.5", "requires Chrome"),
                    ("Google Chrome 158.0.8127.89", "v20.0.0", "requires Node 22"),
                ):
                    call.side_effect = [SimpleNamespace(stdout=s) for s in
                                        (REVISION + "\n", "", chrome_version, node_version)]
                    preflight = request("preflight")
                    if expected:
                        with self.assertRaisesRegex(ValueError, expected):
                            runner.run_request(preflight, repo, chrome, node)
                    else:
                        self.assertEqual(runner.run_request(preflight, repo, chrome, node),
                                         {**preflight, "result": "passed"})
                call.side_effect = [SimpleNamespace(stdout="bad")]
                with self.assertRaisesRegex(ValueError, "differs"):
                    runner.run_request(request("preflight"), repo, chrome, node)
                call.side_effect = [SimpleNamespace(stdout=REVISION + "\n"), SimpleNamespace(stdout=""),
                                    subprocess.CalledProcessError(1, "smoke")]
                with self.assertRaises(subprocess.CalledProcessError):
                    runner.run_request(smoke, repo, chrome, node)
