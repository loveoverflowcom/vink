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
            ["--jobs", "8"],
            ["-j8"],
            ["-qj8"],
            ["--config=build.jobs=8"],
            ["--", "--test-threads", "8"],
        ]:
            with self.subTest(args=args), self.assertRaises(budget.Refusal):
                budget.validate_command(["--", "cargo", "test", *args], 2)

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
            self.assertEqual(list(Path(temp).iterdir()), [])


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

    def launch(self, operation, checkout=0, timeout=None):
        options = ["--timeout", str(timeout)] if timeout is not None else []
        child = subprocess.Popen(
            [sys.executable, "-c", ENTRY, self.launchers[checkout], self.slot, "run",
             *options, "--", "cargo", "test", operation],
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
        stubborn = self.launch("stubborn", timeout=1)
        self.ready()
        sid = int((self.slot / "session").read_text().split()[1])
        self.assertIn("timeout", self.result(stubborn, 124))
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
