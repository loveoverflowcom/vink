"""Focused Git metadata tests. All Git changes and remotes live in TemporaryDirectory."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

SCRIPTS = Path(__file__).resolve().parent
SKILL_SCRIPTS = Path(".agents/skills/vink-git-workflow/scripts")


class GitWorkflow(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="vink-git-lint-")
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        # Do not inherit hooks, Git directory/index, identity, signing or network remotes.
        self.env = dict(PATH=os.environ["PATH"], HOME=str(self.root),
                        LC_ALL="C", GIT_CONFIG_NOSYSTEM="1", GIT_TERMINAL_PROMPT="0")

    def call(self, args, *, cwd=None, stdin="", code=0):
        result = subprocess.run(args, cwd=cwd or self.root, env=self.env, input=stdin,
                                capture_output=True, encoding="utf-8")
        self.assertEqual(result.returncode, code,
                         f"{args!r}\nstdout: {result.stdout}\nstderr: {result.stderr}")
        return result

    def test_cli(self):
        msg = self.root / "message"
        zero = "0" * 40
        sha = "a" * 40
        def row(name, local=sha, remote=zero):
            return f"HEAD {local} refs/heads/{name} {remote}\n"
        cases = [
            ([], 2), (["unknown"], 2), (["--help"], 0), (["--help", "x"], 2),
            (["branch", "fix/333-kiem-git"], 0), (["branch", "codex/old"], 1),
            (["branch", "hotfix/v1"], 0), (["branch", "develop"], 1),
            (["branch", "fix/á"], 1), (["branch", "fix/-a"], 1),
            (["branch", "fix/" + "x" * 46], 0), (["branch", "fix/" + "x" * 47], 1),
            (["branch", "fix/a\nother"], 1), (["branch", "fix/a", "ignored"], 2),
            (["branch", ""], 1), (["message"], 2), (["msg", msg, "x"], 2),
            (["message", self.root / "absent"], 1), (["title"], 2),
            (["title", "fix: sửa", "extra"], 2), (["title", "Merge abc"], 1),
            (["title", "fixup! x"], 1), (["title", "fix: thay\nđổi"], 1),
            (["title", "fix: \t"], 1), (["title", "fix(reader): sửa."], 1),
            (["title", "fix: " + "ế" * 58 + "🌿"], 0),
            (["title", "fix: " + "ế" * 59 + "🌿"], 1), (["pre-push", "extra"], 2),
        ]
        messages = [
            ("", 1), ("fix: sửa lỗi", 0), ("fix: sửa\nbody thiếu cách dòng", 1),
            ("fix: " + "ế" * 66 + "🌿", 0), ("fix: " + "ế" * 67 + "🌿", 1),
            ("fixup! x", 0), ('Revert "old title"', 0), ("Merge branch 'other'", 0),
            ("docs: nêu ví dụ\n\nDạng <issue> trong cú pháp.", 0),
            ("docs: xem nguồn\n\nRefs: #22\nDecision: O-17@r2", 0),
            ("docs: xem nguồn\n\nEvidence: https://github.com/o/r/compare/a...b", 0),
            ("docs: xem nguồn\n\nEvidence: https://drive.google.com/...", 1),
            ("docs: xem nguồn\n\nRefs:\nEvidence: <url>", 1),
            ("docs: xem nguồn\n\nRefs: \t", 1), ("docs: xem nguồn\n\nRefs:#22", 1),
            ("docs: xem nguồn\n\nSpec: <link>", 1), ("docs: xem nguồn\n\nRefs: #TBD", 1),
            ("docs: xem nguồn\n\nTài liệu [gốc](https://example.org)", 1),
            ("docs: xem nguồn\n\n# ------------------------ >8 ------------------------\n#TBD", 0),
        ]
        pushes = [
            (row("fix/333-check"), 0), (row("codex/new"), 1),
            (row("codex/old", remote=sha), 0), (row("codex/old", local=zero, remote=sha), 0),
            (row("fix/a") + row("bad/b") + row("fix/b"), 1), (row("develop"), 0),
            (f"refs/tags/v1 {sha} refs/tags/v1 {zero}\n", 0),
        ]
        shells = list(dict.fromkeys(s for s in ("/bin/sh", shutil.which("dash")) if s))
        for shell in shells:
            lint = [shell, SCRIPTS / "git-lint.sh"]
            for args, code in cases:
                with self.subTest(shell=shell, args=args):
                    self.call(lint + args, code=code)
            for message, code in messages:
                with self.subTest(shell=shell, message=message):
                    msg.write_text(message + "\n", encoding="utf-8")
                    self.call(lint + ["message", msg], code=code)
            for data, code in pushes:
                with self.subTest(shell=shell, data=data):
                    self.call(lint + ["pre-push"], stdin=data, code=code)

    def test_relative_hooks_in_linked_worktree(self):
        repo = self.root / "repo"
        linked = self.root / "linked checkout"
        remote = self.root / "remote.git"
        self.call(["git", "init", "--bare", remote])
        self.call(["git", "init", "-b", "develop", repo])

        def git(*args, cwd=repo, code=0):
            return self.call(["git", *args], cwd=cwd, code=code)

        git("config", "user.name", "Hook test")
        git("config", "user.email", "hooks@example.invalid")
        scripts = repo / SKILL_SCRIPTS
        scripts.mkdir(parents=True)
        shutil.copy2(SCRIPTS / "git-lint.sh", scripts)
        shutil.copytree(SCRIPTS / "hooks", scripts / "hooks")
        git("add", str(SKILL_SCRIPTS))
        git("commit", "-m", "legacy initial message")  # opt-out default
        git("remote", "add", "origin", str(remote))
        git("push", "origin", "HEAD:refs/heads/codex/old")
        git("worktree", "add", "--no-track", "-b", "fix/333-check", linked, "HEAD")
        git("config", "--local", "core.hooksPath", str(SKILL_SCRIPTS / "hooks"))
        self.call([linked / SKILL_SCRIPTS / "git-lint.sh", "branch"], cwd=linked)
        git("commit", "--allow-empty", "-m", "invalid", cwd=linked, code=1)
        git("commit", "--allow-empty", "-m", "fix: sửa lỗi", "-m", "Refs: #333", cwd=linked)
        git("push", "origin", "HEAD:refs/heads/codex/new", cwd=linked, code=1)
        git("ls-remote", "--exit-code", "--heads", "origin", "refs/heads/codex/new", cwd=linked, code=2)
        git("push", "origin", "HEAD:refs/heads/codex/old", cwd=linked)
        git("push", "origin", "HEAD:refs/heads/fix/333-check", cwd=linked)
        for ref in ("codex/old", "fix/333-check"):
            self.assertEqual(git("rev-parse", "HEAD", cwd=linked).stdout.strip(),
                             git("ls-remote", "--heads", "origin", f"refs/heads/{ref}").stdout.split()[0])
        git("push", "origin", ":refs/heads/codex/old", cwd=linked)


if __name__ == "__main__":
    unittest.main(verbosity=2)
