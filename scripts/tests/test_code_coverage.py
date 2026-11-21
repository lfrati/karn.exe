"""Coverage tooling contracts: source denominator and missing-process evidence."""
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from code_coverage import inventory, test_lines as excluded_lines, verify_processes


class CoverageTests(unittest.TestCase):
    def test_inline_test_items_ignore_literal_braces_and_array_semicolons(self):
        source = '''fn production() {}
#[cfg(test)]
fn helper() -> [usize; 1] {
    let literal = "}";
    [1]
}
fn after() {}
#[cfg(test)]
mod tests {
    /* } */ fn check() { let raw = r#"{"#; }
}
fn last() {}
'''
        self.assertEqual(excluded_lines(source), set(range(2, 7)) | set(range(8, 12)))

    def test_counts_only_production_regions_and_preserves_partial_line_gap(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            src = root / "crates/game/src/core.rs"
            src.parent.mkdir(parents=True)
            src.write_text('fn used() { if true {} else {} }\nfn missed() {}\n#[cfg(test)]\nfn check() {}\n')
            filename = str(src)
            def function(name, line, count, regions=None):
                return {"name": name, "count": count, "filenames": [filename],
                        "regions": regions or [[line, 1, line, 16, count, 0, 0, 0]]}
            export = {"data": [{"functions": [
                function("used", 1, 2, [[1, 1, 1, 15, 2, 0, 0, 0], [1, 22, 1, 29, 0, 0, 0, 0]]),
                function("missed", 2, 0), function("check", 4, 9),
                {"name": "dependency", "count": 0, "filenames": ["/dependency.rs"], "regions": [[1, 1, 1, 3, 0, 0, 0, 0]]}],
                "files": [{"filename": filename, "segments": [[1, 1, 2, True, True, False], [1, 22, 0, True, True, False], [2, 1, 0, True, True, False], [3, 1, 0, False, False, False]]}]}]}
            report = inventory(export, root)
            data = report["files"]["crates/game/src/core.rs"]
            self.assertEqual(data["functions"]["executed"], 1)
            self.assertEqual([f["name"] for f in data["functions"]["missed"]], ["missed"])
            self.assertEqual(data["lines"], {"executed": 1, "missed": [2]})
            self.assertEqual(len(data["regions"]["missed"]), 2)
            self.assertEqual(data["regions"]["missed"][0]["start"], [1, 22])

    def test_every_engine_pid_needs_normal_exit_and_nonempty_profile(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            trace = root / "run.process.42"
            trace.write_text('execve("/engine", ["/engine"], []) = 0\n+++ exited with 0 +++\n')
            with self.assertRaisesRegex(RuntimeError, "no profile"):
                verify_processes(root, "run", {"/engine"}, root)
            (root / "signature-42.profraw").write_bytes(b"profile")
            self.assertEqual(verify_processes(root, "run", {"/engine"}, root)[0]["pid"], 42)
            trace.write_text('execve("/engine", ["/engine"], []) = 0\n+++ killed by SIGKILL +++\n')
            with self.assertRaisesRegex(RuntimeError, "failed/killed"):
                verify_processes(root, "run", {"/engine"}, root)
            with self.assertRaisesRegex(RuntimeError, "no selected engine"):
                verify_processes(root, "run", {"/other"}, root)


if __name__ == "__main__":
    unittest.main()
