"""scripts/gates.sh keeps the gates' generated corpus dumps valid for the inputs they came from."""
import os
import json
from pathlib import Path
import shutil
import stat
import subprocess
import tempfile
import unittest

from pinned import RULES_EDITION, RULES_NAME

ROOT = Path(__file__).resolve().parents[2]

# stands in for corpus_structure.py: writes the four dumps (or what GENERATE says), counts its runs
GENERATOR = '''import os, sys
from pathlib import Path
from pinned import RULES
Path("generator.runs").open("a").write("run\\n")
Path("generator.rules").write_text(RULES.read_text())
mode = os.environ.get("GENERATE", "all")
if mode == "fail":
    sys.exit("extraction broke")
names = ["noun_phrases", "amounts", "conditions", "triggers"][: 2 if mode == "partial" else 4]
for name in names:
    Path(".cache/oracle", name + ".jsonl").write_text('{"text": "' + name + '"}\\n')
'''
# stands for the built gate: every gate passes
GATE = '#!/bin/sh\necho "$1 RoundTrip: 1 (100.0%) GATE no mismatch, no ambiguity: ok"\n'
CARGO = '#!/bin/sh\necho "    Finished fast profile"\n'


class GatesTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "scripts").mkdir()
        shutil.copy(ROOT / "scripts" / "gates.sh", self.root / "scripts" / "gates.sh")
        shutil.copy(ROOT / "scripts" / "pinned.py", self.root / "scripts" / "pinned.py")
        (self.root / "scripts" / "corpus_structure.py").write_text(GENERATOR)
        (self.root / "scripts" / "inventory.py").write_text("# classification\n")
        (self.root / "resources" / "oracle").mkdir(parents=True)
        (self.root / "resources" / "rules").mkdir()
        self.manifest = self.root / "resources" / "manifest.json"
        self.manifest.write_text(json.dumps({"sources": {"rules": {"edition": RULES_EDITION}}}))
        (self.root / "resources" / "oracle" / "cards.jsonl").write_text('{"name": "Island"}\n')
        (self.root / "resources" / "rules" / RULES_NAME).write_text("100.1 These are the rules.\n")
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.executable(self.bin / "cargo", CARGO)
        self.target = self.root / "target"
        (self.target / "fast").mkdir(parents=True)
        self.executable(self.target / "fast" / "grammar-gate", GATE)
        self.oracle = self.root / ".cache" / "oracle"
        self.stamp = self.oracle / "dumps.sha256"

    @staticmethod
    def executable(path: Path, text: str):
        path.write_text(text)
        path.chmod(path.stat().st_mode | stat.S_IEXEC)

    def gates(self, generate="all", *gates):
        env = {**os.environ, "PATH": f"{self.bin}{os.pathsep}{os.environ['PATH']}", "CARGO_TARGET_DIR": str(self.target),
               "GENERATE": generate, "PYTHON": os.environ.get("PYTHON", "python3")}
        env["PYTHON"] = env["PYTHON"].split()[0] if env["PYTHON"].split() else "python3"
        result = subprocess.run(["bash", "scripts/gates.sh", *gates], cwd=self.root, env=env, text=True, capture_output=True)
        return result.returncode, result.stdout + result.stderr

    def runs(self) -> int:
        try:
            return len((self.root / "generator.runs").read_text().splitlines())
        except FileNotFoundError:
            return 0

    def test_missing_dumps_are_generated_once_and_kept_while_their_inputs_stand(self):
        code, out = self.gates("all", "selectors")
        self.assertEqual(code, 0, out)
        self.assertIn("generating the corpus dumps in .cache/oracle (noun_phrases.jsonl missing)", out)
        self.assertEqual((self.oracle / "triggers.jsonl").read_text(), '{"text": "triggers"}\n')
        self.assertTrue(self.stamp.exists(), "a whole generation is stamped with its inputs' digest")
        self.assertEqual(self.runs(), 1)
        code, out = self.gates("all", "selectors")
        self.assertEqual(code, 0, out)
        self.assertNotIn("generating", out)
        self.assertEqual(self.runs(), 1, "unchanged inputs: the dumps are reused")

    def test_a_changed_input_regenerates_the_dumps(self):
        self.gates("all", "selectors")
        for change in ("scripts/corpus_structure.py", "scripts/inventory.py", "scripts/pinned.py", "resources/oracle/cards.jsonl",
                       f"resources/rules/{RULES_NAME}"):
            with self.subTest(change=change):
                before = self.runs()
                path = self.root / change
                path.write_text(path.read_text() + "# changed\n")
                code, out = self.gates("all", "selectors")
                self.assertEqual(code, 0, out)
                self.assertIn("(inputs changed since they were generated)", out)
                self.assertEqual(self.runs(), before + 1)

    def test_manifest_switch_between_existing_rules_files_regenerates_and_unchanged_inputs_reuse(self):
        other = "20260717" if RULES_EDITION != "20260717" else "20260925"
        selected = self.root / "resources" / "rules" / f"MagicCompRules-{other}.txt"
        selected.write_text("100.1 The other existing rules edition.\n")
        code, out = self.gates("all", "selectors")
        self.assertEqual(code, 0, out)
        self.assertEqual(self.runs(), 1)
        self.assertEqual((self.root / "generator.rules").read_text(), "100.1 These are the rules.\n")
        first_stamp = self.stamp.read_text()
        # Both rule files already exist. Only the manifest's selection changes.
        self.manifest.write_text(json.dumps({"sources": {"rules": {"edition": other}}}))
        code, out = self.gates("all", "selectors")
        self.assertEqual(code, 0, out)
        self.assertEqual(self.runs(), 2)
        self.assertNotEqual(self.stamp.read_text(), first_stamp)
        self.assertEqual((self.root / "generator.rules").read_text(), selected.read_text())
        code, out = self.gates("all", "selectors")
        self.assertEqual(code, 0, out)
        self.assertEqual(self.runs(), 2, "unchanged selected inputs reuse the stamp")

    def test_a_failed_or_partial_generation_fails_the_gates_and_is_not_trusted_next_time(self):
        code, out = self.gates("fail", "selectors")
        self.assertEqual(code, 1)
        self.assertIn("extraction broke", out)
        self.assertIn("corpus dumps failed", out)
        self.assertFalse(self.stamp.exists())
        code, out = self.gates("partial", "selectors")
        self.assertEqual(code, 1)
        self.assertIn("corpus dumps failed: conditions.jsonl not written", out)
        self.assertFalse(self.stamp.exists())
        self.assertTrue((self.oracle / "noun_phrases.jsonl").exists(), "what was written stays, unstamped")
        code, out = self.gates("all", "selectors")
        self.assertEqual(code, 0, out)
        self.assertIn("generating", out)
        self.assertTrue(self.stamp.exists())
        self.assertEqual(self.runs(), 3)

    def test_an_edited_dump_without_a_new_stamp_is_not_a_change_but_a_missing_one_is(self):
        self.gates("all", "selectors")
        (self.oracle / "amounts.jsonl").unlink()
        code, out = self.gates("all", "selectors")
        self.assertEqual(code, 0, out)
        self.assertIn("(amounts.jsonl missing)", out)
        self.assertEqual(self.runs(), 2)


if __name__ == "__main__":
    unittest.main()
