"""Reports must launch and hash the binaries Cargo actually rebuilt."""
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import rust_tools


class RustToolPaths(unittest.TestCase):
    def test_target_directory_matches_cargo_environment(self):
        with patch.object(rust_tools, "ROOT", Path("/checkout")), patch.dict(os.environ, {}, clear=True):
            self.assertEqual(rust_tools.fast_binary("game-gate"), Path("/checkout/target/fast/game-gate"))
            os.environ["CARGO_TARGET_DIR"] = "build-output"
            self.assertEqual(rust_tools.fast_binary("game-serve"), Path("/checkout/build-output/fast/game-serve"))
            os.environ["CARGO_TARGET_DIR"] = "/shared/target"
            self.assertEqual(rust_tools.fast_binary("game-serve"), Path("/shared/target/fast/game-serve"))


class TargetOwnership(unittest.TestCase):
    """A target directory belongs to the checkout that first built it: another checkout's
    build into it is refused, as cargo would reuse the first one's compiled crates there."""

    def test_a_target_is_claimed_once_and_refused_to_another_checkout(self):
        with tempfile.TemporaryDirectory() as temp:
            a, b = Path(temp) / "a", Path(temp) / "b"
            a.mkdir(), b.mkdir()
            target = Path(temp) / "target"
            self.assertEqual(rust_tools.claim_target(target, a), target.resolve())
            self.assertEqual((target / rust_tools.MARKER).read_text().strip(), str(a.resolve()))
            rust_tools.claim_target(target, a)  # again, by the same checkout
            with self.assertRaisesRegex(RuntimeError, "was built from .*a, not this checkout .*b"):
                rust_tools.claim_target(target, b)
            self.assertEqual((target / rust_tools.MARKER).read_text().strip(), str(a.resolve()), "the owner stays")

    def test_the_default_is_this_checkout_and_its_cargo_target(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "checkout"
            root.mkdir()
            with patch.object(rust_tools, "ROOT", root), patch.dict(os.environ, {}, clear=True):
                self.assertEqual(rust_tools.claim_target(), (root / "target").resolve())
                os.environ["CARGO_TARGET_DIR"] = str(Path(temp) / "elsewhere")
                self.assertEqual(rust_tools.claim_target(), (Path(temp) / "elsewhere").resolve())
                self.assertEqual((Path(temp) / "elsewhere" / rust_tools.MARKER).read_text().strip(), str(root.resolve()))


if __name__ == "__main__":
    unittest.main()
