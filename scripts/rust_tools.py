"""Paths for Rust tools built with this repository's `fast` profile."""
import os
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def target_dir() -> Path:
    # Cargo resolves a relative CARGO_TARGET_DIR against its working directory; our
    # reporting/build commands run at ROOT. Keep launch and provenance hashing aligned.
    target = Path(os.environ.get("CARGO_TARGET_DIR", "target"))
    return target if target.is_absolute() else ROOT / target


def fast_binary(name: str) -> Path:
    return target_dir() / "fast" / name


# Names the checkout a target directory was first built from. Cargo's fingerprints ignore
# where sources live, so a target shared between checkouts reuses the other one's compiled
# code whenever its own sources are not newer (crates/oracle/build.rs makes the same check
# when its identity changes; crates it is not a dependency of never run it).
MARKER = "karn-checkout"


def claim_target(target: Path | None = None, checkout: Path | None = None) -> Path:
    """The target directory this checkout builds into; refused when another checkout built
    it first, rather than overwriting that checkout's artifacts or reusing them here."""
    target = (target or target_dir()).resolve()
    checkout = (checkout or ROOT).resolve()
    marker = target / MARKER
    if marker.is_file():
        owner = marker.read_text().strip()
        if Path(owner) != checkout:
            raise RuntimeError(f"{target} was built from {owner}, not this checkout {checkout}: "
                               "build each checkout into its own target directory")
        return target
    target.mkdir(parents=True, exist_ok=True)
    marker.write_text(f"{checkout}\n")
    return target


# What the engine's source is, for a build to state (`game-serve` `version`): the commit
# and whether anything compiled into it differs from that commit. Only a build made here
# can say this; a report never infers a supplied executable's source from a checkout.
SOURCES = ["crates", "Cargo.toml", "Cargo.lock"]


def source_identity(root: Path = ROOT, paths: list[str] = SOURCES) -> dict:
    """The commit checked out at `root`, and whether anything under `paths` differs from it
    (tracked changes or untracked files)."""
    def git(*args: str) -> str:
        return subprocess.run(["git", *args], cwd=root, text=True, capture_output=True, check=True).stdout
    revision = git("rev-parse", "HEAD").strip()
    dirty = bool(git("status", "--porcelain", "--untracked-files=all", "--", *paths).strip())
    return {"revision": revision, "dirty": dirty}


def build(name: str = "game-serve", profile: str = "fast") -> Path:
    """Build a binary that states its source (`GAME_SOURCE_REVISION`/`GAME_SOURCE_DIRTY`)."""
    if profile != "fast":
        raise ValueError("only the fast profile has a known path here")
    claim_target()
    source = source_identity()
    env = {**os.environ, "GAME_SOURCE_REVISION": source["revision"],
           "GAME_SOURCE_DIRTY": "true" if source["dirty"] else "false"}
    subprocess.run(["cargo", "build", "--locked", "--profile", profile, "-p", "game", "--bin", name],
                   cwd=ROOT, env=env, check=True)
    return fast_binary(name)


if __name__ == "__main__":
    import sys
    if sys.argv[1:2] != ["build"]:
        raise SystemExit("usage: rust_tools.py build [NAME]   (a fast build that states its source)")
    print(build(*sys.argv[2:3]))
