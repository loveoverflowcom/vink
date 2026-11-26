"""Run with python3 -B -m unittest discover -s tools -p test_local_budget.py."""

from importlib.machinery import SourceFileLoader
from importlib.util import module_from_spec, spec_from_loader
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
from tempfile import TemporaryDirectory
import time
import unittest
from unittest.mock import Mock, patch

LAUNCHER = Path(__file__).with_name("local-budget").resolve()
SPEC = spec_from_loader("local_budget", SourceFileLoader("local_budget", str(LAUNCHER)))
budget = module_from_spec(SPEC)
SPEC.loader.exec_module(budget)

# Only tests replace the private slot. Production launchers cannot opt out of admission
# by selecting another checkout, target, temp directory or Cargo home.
ENTRY = """
import os, runpy, sys
from pathlib import Path
launcher, slot, *args = sys.argv[1:]
main = runpy.run_path(launcher)["main"]
main.__globals__["SLOT"] = Path(slot)
if "FIXTURE_PRESSURE" in os.environ:
    main.__globals__["pressure"] = lambda: os.environ["FIXTURE_PRESSURE"]
sys.argv = [launcher, *args]
try:
    sys.exit(main())
except main.__globals__["Refusal"] as error:
    print(error, file=sys.stderr)
    sys.exit(error.code)
"""

FAKE_CARGO = """
import json, os, pathlib, subprocess, sys
operation = sys.argv[2]
if operation == "nested":
    nested = subprocess.run(["tools/local-budget", "run", "--", "cargo", "test", "ok"])
    if nested.returncode != 2:
        sys.exit(9)
if operation in ("linger", "orphan", "stubborn"):
    command = ["/bin/sleep", "5"]
    if operation == "stubborn":
        command = ["/bin/sh", "-c", "trap '' TERM; exec /bin/sleep 20"]
    descendant = subprocess.Popen(command)
    pathlib.Path("child").write_text(str(descendant.pid))
    if operation != "orphan":
        descendant.wait()
pathlib.Path("profiles.json").write_text(json.dumps(
    {k: v for k, v in os.environ.items() if k.startswith("CARGO_PROFILE_")}))
pathlib.Path("cargo-dirs.json").write_text(json.dumps([
    os.environ["CARGO_TARGET_DIR"], os.environ["CARGO_BUILD_TARGET_DIR"],
    os.environ["CARGO_BUILD_BUILD_DIR"]]))
pathlib.Path("invocation.json").write_text(json.dumps(
    [sys.argv[1:], os.environ["CARGO_BUILD_JOBS"], os.environ["RUST_TEST_THREADS"]]))
sys.exit(7 if operation == "failure" else 0)
"""


class Profile(unittest.TestCase):
    def test_source_precedence_and_invalid_caps(self):
        with TemporaryDirectory() as temp:
            temp = Path(temp)
            root = temp / "checkout"
            global_config = temp / "cargo-home"
            global_config.mkdir()
            (root / ".cargo").mkdir(parents=True)
            (global_config / "config.toml").write_text("[build]\njobs = 1\n")
            local = root / ".cargo" / "config"
            local.write_text("[build]\njobs = 4\n")
            (root / ".cargo" / "config.toml").write_text("[build]\njobs = 1\n")
            (root / ".env").write_text('RUST_TEST_THREADS=8\n RUST_TEST_THREADS="1"\n')
            env = {"CARGO_HOME": str(global_config)}
            self.assertEqual(budget.settings(env, root), (2, 1))
            local.unlink()
            self.assertEqual(budget.settings(env, root), (1, 1))
            self.assertEqual(budget.settings(dict(env, RUST_TEST_THREADS="8"), root), (1, 2))
            self.assertEqual(budget.settings(dict(env, CARGO_BUILD_JOBS="1"), root), (1, 1))
            for key in ["CARGO_BUILD_JOBS", "RUST_TEST_THREADS"]:
                for value in ["", "0", "-1", "x", "1.5"]:
                    with self.subTest(key=key, value=value), self.assertRaises(budget.Refusal):
                        budget.settings(dict(env, **{key: value}), root)
            local.write_text('[env]\nRUST_TEST_THREADS = { value = "4", force = true }\n')
            with self.assertRaisesRegex(budget.Refusal, "Cargo \\[env\\]"):
                budget.settings(env, root)
            local.unlink()
            with self.assertRaisesRegex(budget.Refusal, "inherited jobserver"):
                budget.settings(dict(env, MAKEFLAGS="--jobserver-auth=3,4"), root)
            with self.assertRaisesRegex(budget.Refusal, "rustc wrapper"):
                budget.settings(dict(env, RUSTC_WRAPPER="sccache"), root)
            # A configured wrapper is refused, but Cargo treats an empty variable as disabling it.
            local.write_text('[build]\nrustc-wrapper = "sccache"\n')
            with self.assertRaises(budget.Refusal) as error:
                budget.settings(env, root)
            self.assertEqual(error.exception.code, 69)
            self.assertEqual(budget.settings(dict(env, RUSTC_WRAPPER=""), root), (1, 1))
            for key in budget.CARGO_DIRECTORIES:
                local.write_text(f'[env]\n{key} = {{ value = "/shared", force = true }}\n')
                with self.subTest(key=key), self.assertRaisesRegex(budget.Refusal, key):
                    budget.settings(env, root)
            local.unlink()
            with self.assertRaisesRegex(budget.Refusal, "CARGO_ALIAS_XTASK"):
                budget.settings(dict(env, CARGO_ALIAS_XTASK="run"), root)

    def test_low_disk_detects_global_incremental_override(self):
        with TemporaryDirectory() as temp:
            root = Path(temp)
            local = root / ".cargo"
            local.mkdir()
            config = local / "config.toml"
            config.write_text("[build]\nincremental = true\n")
            env = {"CARGO_HOME": temp}
            with self.assertRaisesRegex(budget.Refusal, "build-wide incremental"):
                budget.settings(env, root, low_disk=True)
            budget.settings(env, root)
            budget.settings(dict(env, CARGO_INCREMENTAL="0"), root, low_disk=True)
            config.write_text('[env]\nCARGO_PROFILE_DEV_DEBUG = { value = "2", force = true }\n')
            with self.assertRaisesRegex(budget.Refusal, "CARGO_PROFILE_DEV_DEBUG"):
                budget.settings(env, root, low_disk=True)

    def test_low_disk_checks_xtask_file_defaults_as_well_as_bootstrap(self):
        with TemporaryDirectory() as temp:
            root = Path(temp)
            (root / ".env").write_text("CARGO_INCREMENTAL=1\n")
            env = {"CARGO_HOME": temp}
            with self.assertRaisesRegex(budget.Refusal, "xtask .env"):
                budget.settings(env, root, low_disk=True)
            budget.settings(env, root)
            budget.settings(dict(env, CARGO_INCREMENTAL="0"), root, low_disk=True)
            # A file override applies to nested commands only. Bootstrap still sees build=true.
            (root / ".env").write_text("CARGO_INCREMENTAL=0\n")
            (root / ".cargo").mkdir()
            (root / ".cargo/config.toml").write_text("[build]\nincremental = true\n")
            with self.assertRaisesRegex(budget.Refusal, "bootstrap"):
                budget.settings(env, root, low_disk=True)

    def test_argv_preserves_narrower_libtest_flag_and_refuses_overrides(self):
        argv = ["cargo", "test", "-p", "xtask", "--", "--test-threads=1"]
        self.assertEqual(budget.validate_command(["--", *argv], 2), argv)
        for command in (["ui-preview", "cmp", "verify"], ["ui-preview", "ios", "build"],
                        ["verify", "web"], ["verify", "preview"], ["verify", "all"],
                        ["infra", "build-images"]):
            with self.subTest(command=command), self.assertRaises(budget.Refusal) as error:
                budget.validate_command(["--", "cargo", "xtask", *command], 2)
            self.assertEqual(error.exception.code, 69)
        for args in [
            ["--target-dir", "../shared"],
            ["--target-dir=/shared"],
            ["--target-dir"],
            ["--jobs", "8"],
            ["-j8"],
            ["-qj8"],
            ["--config=build.jobs=8"],
            ["--", "--test-threads", "8"],
        ]:
            with self.subTest(args=args), self.assertRaises(budget.Refusal):
                budget.validate_command(["--", "cargo", "test", *args], 2)

    def test_explicit_local_target_directory(self):
        with TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            (root / "target").mkdir()
            (root / "alias").symlink_to(root / "target", target_is_directory=True)
            with patch.object(budget, "ROOT", root):
                for value in ("target", str(root / "target"), "alias", "missing/../target"):
                    for args, expected in (
                        (["--target-dir", value], ["--target-dir", str(root / "target")]),
                        ([f"--target-dir={value}"], [f"--target-dir={root / 'target'}"]),
                    ):
                        with self.subTest(args=args):
                            cmd = ["--", "cargo", "check", *args]
                            self.assertEqual(budget.validate_command(cmd, 2),
                                             ["cargo", "check", *expected])

    def test_argv_plan_follows_xtask_last_argument_rule(self):
        # xtask's cli::parse inspects only when `--plan` is last; the launcher mirrors that.
        for command in (["verify", "web", "--scenario", "main", "--plan"],
                        ["verify", "preview", "--plan"], ["verify", "unit", "store"],
                        ["build", "api", "--plan"]):
            argv = ["cargo", "xtask", *command]
            with self.subTest(command=command):
                self.assertEqual(budget.validate_command(["--", *argv], 2), argv)
        for command in (["verify", "web", "--plan", "--scenario", "main"],
                        ["ui-preview", "leptos", "verify"], ["ui-preview", "cmp", "build"],
                        ["ui-preview", "android", "run"]):
            with self.subTest(command=command), self.assertRaises(budget.Refusal) as error:
                budget.validate_command(["--", "cargo", "xtask", *command], 2)
            self.assertEqual(error.exception.code, 69)
        for argv in (["cargo", "test"], ["--", "cargo", "run", "-p", "xtask"],
                     ["--", "cargo", "+nightly", "test"], ["--", "cargo"], ["--", "rustc", "test"]):
            with self.subTest(argv=argv), self.assertRaises(budget.Refusal) as error:
                budget.validate_command(argv, 2)
            self.assertEqual(error.exception.code, 2)
        argv = ["cargo", "test", "-p", "xtask", "--", "--test-threads", "2"]
        self.assertEqual(budget.validate_command(["--", *argv], 2), argv)

    def test_light_plan_and_strict_do_not_bootstrap(self):
        with TemporaryDirectory() as temp:
            env = dict(os.environ, PATH=temp, CARGO_HOME=temp)
            env.pop(budget.MARKER, None)
            plan = subprocess.run([sys.executable, LAUNCHER, "plan"], env=env,
                                  capture_output=True, text=True, timeout=5)
            self.assertEqual(plan.returncode, 0 if sys.platform == "darwin" else 69, plan.stderr)
            strict = subprocess.run(
                [sys.executable, LAUNCHER, "run", "--mode", "strict", "--", "cargo", "test"],
                env=env, capture_output=True, text=True, timeout=5,
            )
            self.assertEqual(strict.returncode, 69, strict.stderr)
            self.assertIn("UNSUPPORTED", strict.stderr)
            if sys.platform == "darwin":
                for args, status, message in (
                    (["xtask", "verify", "unit", "store"], 0, "command passes resource policy"),
                    (["xtask", "verify", "web", "--scenario", "discovery"], 69, "UNSUPPORTED"),
                    (["test", "--jobs=8"], 2, "CARGO_BUILD_JOBS"),
                ):
                    checked = subprocess.run(
                        [sys.executable, LAUNCHER, "plan", "--", "cargo", *args],
                        env=env, capture_output=True, text=True, timeout=5)
                    self.assertEqual(checked.returncode, status, checked.stderr)
                    self.assertIn(message, checked.stderr)
                    self.assertIn("hard_cap=none", checked.stderr)
            self.assertEqual(list(Path(temp).iterdir()), [])

    def test_no_live_sample_has_unknown_peak(self):
        child = Mock(pid=12)
        child.wait.return_value = 0
        with (patch.object(budget, "exit_seen", side_effect=[False, True]),
              patch.object(budget, "session_members", return_value={}),
              patch.object(budget, "signal_group", return_value=False),
              patch.object(budget, "log") as log):
            result = budget.supervise(child, time.monotonic(), None, [None], True)
        self.assertEqual(result, 0)
        messages = [call.args[0] for call in log.call_args_list]
        self.assertTrue(any("members=0 rss_sum=UNKNOWN" in line for line in messages))
        self.assertTrue(any("sampled_peak_rss_sum=UNKNOWN" in line for line in messages))


@unittest.skipUnless(sys.platform == "darwin", "needs macOS session and pressure probes")
class Lifecycle(unittest.TestCase):
    def setUp(self):
        temp = TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        self.slot = self.root / "slot"
        self.launchers = []
        for name in ("checkout-a", "checkout-b"):
            dest = self.root / name / "tools" / "local-budget"
            dest.parent.mkdir(parents=True)
            shutil.copy2(LAUNCHER, dest)
            self.launchers.append(dest)
        fake = self.root / "cargo"
        fake.write_text(f"#!{sys.executable}\n{FAKE_CARGO}")
        fake.chmod(0o700)
        self.env = dict(os.environ, PATH=f"{temp.name}:{os.environ['PATH']}",
                        CARGO_HOME=temp.name, CARGO_BUILD_JOBS="1", RUST_TEST_THREADS="7")
        self.env.pop(budget.MARKER, None)

    def launch(self, operation, *cargo_args, checkout=0, timeout=None, samples=False, low_disk=False):
        options = ["--timeout", str(timeout)] if timeout is not None else []
        if low_disk:
            options.append("--low-disk")
        if samples:
            options.append("--samples")
        child = subprocess.Popen(
            [sys.executable, "-c", ENTRY, self.launchers[checkout], self.slot, "run",
             *options, "--", "cargo", "test", operation, *cargo_args],
            env=self.env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        )
        self.addCleanup(self.reap, child)
        return child

    @staticmethod
    def reap(child):
        if child.poll() is None:
            child.terminate()
        child.communicate(timeout=16)

    def result(self, child, code):
        _, stderr = child.communicate(timeout=16)
        self.assertEqual(child.returncode, code, stderr)
        return stderr

    def ready(self):
        child_file = self.launchers[0].parents[1] / "child"
        deadline = time.monotonic() + 4
        while time.monotonic() < deadline:
            if child_file.exists() and (pid := child_file.read_text().strip()):
                return int(pid)
            time.sleep(0.03)
        self.fail("fixture child failed to report readiness")

    def test_bootstrap_uses_each_checkout_target(self):
        self.env.update(dict.fromkeys(budget.CARGO_DIRECTORIES, str(self.root / "shared")))
        for index, launcher in enumerate(self.launchers):
            root = launcher.parents[1]
            (root / "alias").symlink_to("target", target_is_directory=True)
            output = self.result(self.launch("ok", "--target-dir=alias", checkout=index), 0)
            self.assertNotIn("sample elapsed=", output)
            expected = str(root.resolve() / "target")
            invocation = json.loads((root / "invocation.json").read_text())
            self.assertEqual(invocation[0], ["test", "ok", f"--target-dir={expected}"])
            self.assertIn(f"cargo_target_dir={expected} cargo_build_dir={expected}", output)
            self.assertEqual(json.loads((root / "cargo-dirs.json").read_text()), [expected] * 3)
        self.assertFalse((self.root / "shared").exists())

    def test_low_disk_only_sets_dev_test_on_opt_in(self):
        root = self.launchers[0].parents[1]
        self.env = {k: v for k, v in self.env.items() if not k.startswith("CARGO_PROFILE_")}
        self.env.update(CARGO_PROFILE_DEV_DEBUG="2", CARGO_PROFILE_RELEASE_DEBUG="1")
        expected = {"CARGO_PROFILE_DEV_DEBUG": "2", "CARGO_PROFILE_RELEASE_DEBUG": "1"}
        compact = {"CARGO_PROFILE_DEV_DEBUG": "0", "CARGO_PROFILE_TEST_DEBUG": "0",
                   "CARGO_PROFILE_DEV_INCREMENTAL": "false", "CARGO_PROFILE_TEST_INCREMENTAL": "false",
                   "CARGO_PROFILE_RELEASE_DEBUG": "1"}
        for low_disk in (True, False):
            with self.subTest(low_disk=low_disk):
                output = self.result(self.launch("ok", low_disk=low_disk), 0)
                profiles = json.loads((root / "profiles.json").read_text())
                self.assertEqual(profiles, compact if low_disk else expected)
                if low_disk:
                    for k, v in budget.LOW_DISK.items():
                        self.assertIn(f"{k}={v}", output)

    def test_symlinked_shared_target_refused_before_admission(self):
        checkout = self.launchers[0].parents[1]
        target = checkout / "target"
        for destination in (self.root, self.root / "absent", "target"):
            with self.subTest(destination=destination):
                target.symlink_to(destination, target_is_directory=True)
                self.assertIn("symlink", self.result(self.launch("ok"), 2))
                self.assertFalse(self.slot.exists())
                self.assertFalse((checkout / "invocation.json").exists())
                target.unlink()

    def test_cross_checkout_cancel_and_nested(self):
        owner = self.launch("linger")
        pid = self.ready()
        sid = int((self.slot / "session").read_text().split()[1])
        self.assertIn(pid, budget.session_members(sid))
        contender = self.launch("ok", checkout=1)
        unrelated = subprocess.Popen(["/bin/sleep", "10"], start_new_session=True)
        self.addCleanup(self.reap, unrelated)
        self.assertIn("heavy slot busy", self.result(contender, 75))
        launch_dir = self.launchers[1].parents[1]
        self.assertFalse((launch_dir / "invocation.json").exists())
        owner.send_signal(signal.SIGINT)
        self.assertIn("stopping owned process group", self.result(owner, 130))
        self.assertIsNone(unrelated.poll())
        self.assertFalse(budget.session_members(sid))
        self.assertIn("nested launcher refused", self.result(self.launch("nested", checkout=1), 0))
        invocation = json.loads((launch_dir / "invocation.json").read_text())
        self.assertEqual(invocation, [["test", "nested"], "1", "2"])

    def test_crash_fences_orphan_even_after_leader_exits(self):
        owner = self.launch("orphan")
        self.ready()
        sid = int((self.slot / "session").read_text().split()[1])
        owner.kill()
        self.assertEqual(owner.wait(timeout=5), -signal.SIGKILL)
        # The orphan inherits the dead supervisor's pipes, so do not wait on those yet.
        self.assertTrue(budget.session_members(sid))
        self.assertIn("still has live processes", self.result(self.launch("ok", checkout=1), 75))
        owner.communicate(timeout=8)
        self.result(self.launch("ok", checkout=1), 0)
        self.assertFalse((self.slot / "session").exists())

    def test_timeout_and_command_failure_propagate(self):
        stubborn = self.launch("stubborn", timeout=1, samples=True)
        self.ready()
        sid = int((self.slot / "session").read_text().split()[1])
        output = self.result(stubborn, 124)
        self.assertIn("timeout", output)
        self.assertRegex(output, r"sample elapsed=[0-9.]+s members=[1-9][0-9]* rss_sum=[0-9]+ KiB")
        self.assertFalse(budget.session_members(sid))
        self.result(self.launch("failure"), 7)

    def test_pressure_refuses_before_cargo(self):
        self.env["FIXTURE_PRESSURE"] = "warning"
        self.assertIn("pressure must be normal", self.result(self.launch("ok"), 75))
        self.assertFalse((self.launchers[0].parents[1] / "invocation.json").exists())
        self.assertFalse((self.slot / "session").exists())

    def test_success_with_persistent_child_is_a_failure(self):
        self.assertIn("child exited with live session members", self.result(self.launch("orphan"), 1))
        self.assertFalse((self.slot / "session").exists())


if __name__ == "__main__":
    unittest.main()
