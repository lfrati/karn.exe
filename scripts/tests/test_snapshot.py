"""The snapshot a progress run measures: the live tree's inputs, no outputs, no live mutation."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import snapshot


def git(root, *args):
    return subprocess.run(["git", *args], cwd=root, text=True, capture_output=True, check=True).stdout.rstrip("\n")


class SnapshotTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "repo"
        (self.root / "crates").mkdir(parents=True)
        (self.root / "crates" / "lib.rs").write_text("fn main() {}\n")
        (self.root / ".gitignore").write_text("target\n.cache\n__pycache__/\n")
        git(self.root, "init", "-q")
        git(self.root, "-c", "user.email=t@example.com", "-c", "user.name=t", "add", ".")
        git(self.root, "-c", "user.email=t@example.com", "-c", "user.name=t", "commit", "-q", "-m", "start")
        # live edits: a tracked change, an untracked source and build outputs
        (self.root / "crates" / "lib.rs").write_text("fn main() { edited() }\n")
        (self.root / "crates" / "new.rs").write_text("fn new() {}\n")
        (self.root / "target" / "debug").mkdir(parents=True)
        (self.root / "target" / "debug" / "binary").write_text("built\n")
        (self.root / ".cache").mkdir()
        (self.root / ".cache" / "progress.json").write_text("{}\n")
        (self.root / "crates" / "__pycache__").mkdir()
        (self.root / "crates" / "__pycache__" / "x.pyc").write_text("")
        self.dest = self.root / ".cache" / "progress-snapshot" / "tree"
        self.manifest = self.root / ".cache" / "progress-snapshot" / "manifest.json"

    def capture(self):
        return snapshot.capture(self.root, self.dest, self.manifest)

    def test_the_snapshot_has_the_live_inputs_and_shares_the_cache_but_not_the_build(self):
        manifest = self.capture()
        self.assertEqual((self.dest / "crates" / "lib.rs").read_text(), "fn main() { edited() }\n", "the edit, not HEAD")
        self.assertEqual((self.dest / "crates" / "new.rs").read_text(), "fn new() {}\n", "untracked sources are inputs")
        self.assertFalse((self.dest / "target").exists(), "build outputs are not inputs")
        self.assertFalse((self.dest / "crates" / "__pycache__").exists())
        self.assertTrue((self.dest / ".cache").is_symlink())
        self.assertEqual((self.dest / ".cache").resolve(), self.root / ".cache")
        # the snapshot's git reports the live commit and the live changes, the live repository untouched
        self.assertEqual(git(self.dest, "rev-parse", "HEAD"), git(self.root, "rev-parse", "HEAD"))
        self.assertEqual(git(self.dest, "status", "--porcelain", "--ignore-submodules"),
                         git(self.root, "status", "--porcelain", "--ignore-submodules"))
        self.assertEqual(git(self.root, "status", "--porcelain"), " M crates/lib.rs\n?? crates/new.rs")
        self.assertEqual(manifest["revision"], git(self.root, "rev-parse", "HEAD"))
        self.assertEqual(set(manifest["files"]), {".gitignore", "crates/lib.rs", "crates/new.rs"},
                         "the manifest lists what was copied, nothing of the build, cache or board")
        self.assertEqual(json.loads(self.manifest.read_text())["digest"], manifest["digest"])
        self.assertTrue(snapshot.live_matches(manifest))

    def test_a_later_capture_follows_the_live_tree_and_a_new_commit(self):
        first = self.capture()
        (self.root / "crates" / "new.rs").unlink()
        (self.root / "crates" / "lib.rs").write_text("fn main() { again() }\n")
        git(self.root, "-c", "user.email=t@example.com", "-c", "user.name=t", "commit", "-q", "-am", "again")
        self.assertFalse(snapshot.live_matches(first), "the live tree moved on from the first snapshot")
        second = self.capture()
        self.assertNotEqual(first["digest"], second["digest"])
        self.assertFalse((self.dest / "crates" / "new.rs").exists(), "deleted live, deleted in the snapshot")
        self.assertEqual((self.dest / "crates" / "lib.rs").read_text(), "fn main() { again() }\n")
        self.assertEqual(git(self.dest, "rev-parse", "HEAD"), git(self.root, "rev-parse", "HEAD"))
        self.assertEqual(git(self.dest, "status", "--porcelain", "--ignore-submodules"), "")
        self.assertTrue(snapshot.live_matches(second))

    def test_an_edit_during_the_copy_is_retried_then_refused_as_a_conflict(self):
        real, copies = snapshot.rsync, []

        def racing(root, dest):
            real(root, dest)
            copies.append(1)
            (root / "crates" / "lib.rs").write_text(f"fn main() {{ edit{len(copies)}() }}\n")
        with patch.object(snapshot, "rsync", racing):
            with self.assertRaisesRegex(snapshot.CaptureConflict, "kept changing"):
                self.capture()
        self.assertEqual(len(copies), snapshot.RETRIES)
        self.assertFalse(self.manifest.exists(), "a mixed snapshot is no measurement")
        # settled, the next capture takes the final content
        manifest = self.capture()
        self.assertEqual((self.dest / "crates" / "lib.rs").read_text(), f"fn main() {{ edit{snapshot.RETRIES}() }}\n")
        self.assertTrue(snapshot.live_matches(manifest))

    def test_a_snapshot_of_a_worktree_can_keep_its_cache_in_the_main_checkout(self):
        # progress.py --cache can keep an isolated worktree report in a shared cache
        work = Path(self.temp.name) / "work"
        git(self.root, "worktree", "add", "-q", "--detach", str(work))
        cache = self.root / ".cache"
        dest = cache / "progress-snapshot" / "tree"
        manifest = snapshot.capture(work, dest, cache / "progress-snapshot" / "manifest.json", cache=cache)
        self.assertEqual((dest / ".cache").resolve(), cache)
        self.assertEqual(manifest["root"], str(work))
        self.assertEqual((dest / "crates" / "lib.rs").read_text(), "fn main() {}\n", "the worktree's, not the live edit")
        self.assertFalse((work / ".cache").exists(), "nothing written in the measured worktree")
        self.assertEqual(git(self.root, "status", "--porcelain"), " M crates/lib.rs\n?? crates/new.rs")

    def test_the_snapshot_never_replaces_something_else_at_its_path(self):
        self.dest.mkdir(parents=True)
        (self.dest / "notes.txt").write_text("someone's\n")
        with self.assertRaisesRegex(RuntimeError, "not a snapshot worktree"):
            self.capture()
        self.assertEqual((self.dest / "notes.txt").read_text(), "someone's\n")

    def test_exclusions_are_top_level_outputs_and_caches_anywhere(self):
        self.assertTrue(snapshot.excluded("target", "target"))
        self.assertFalse(snapshot.excluded("crates/game/target", "target"), "only the top-level build directory")
        self.assertTrue(snapshot.excluded("scripts/__pycache__", "__pycache__"))
        self.assertTrue(snapshot.excluded("scripts/x.pyc", "x.pyc"))
        self.assertFalse(snapshot.excluded("resources/oracle/cards.jsonl", "cards.jsonl"))


if __name__ == "__main__":
    unittest.main()
