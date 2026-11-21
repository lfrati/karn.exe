"""A snapshot of the working tree for `make progress`: what a run tests, apart from the tree.

The live checkout keeps being edited while a run takes minutes, and a build directory shared
between checkouts reuses the other one's code. A run therefore captures the tree it measures:
every tracked and untracked input (sources, configuration, pinned resources, fixtures; not
build outputs or caches) is copied to a fixed path with its own `target`,
and the suites run there. The copy is a git worktree detached at the live HEAD so that the
scripts' `git` calls report the live commit and the same uncommitted changes; the live
repository itself is never modified (no stash, no checkout, no index refresh). The fixed path
keeps cargo's incremental artifacts valid from one run to the next, so a run rebuilds only
what changed since the previous snapshot, as a checkout would.

A capture that races an edit (the tree differs before and after the copy) is retried, then
refused: a mixed snapshot measures no checkout. Its manifest names the live root, commit,
status and every file's content hash, so a finished run stays attributed to the snapshot
when the live tree changes afterwards.
"""
from __future__ import annotations

import datetime as dt
import hashlib
import json
import os
import subprocess
from pathlib import Path

# top-level entries which are outputs or caches, never inputs; the
# snapshot's own `.git`, `target` and `.cache` live under these names and are kept
EXCLUDED_ROOTS = (".git", "target", ".cache", ".venv")
EXCLUDED_PATHS = ()
EXCLUDED_NAMES = ("__pycache__",)
EXCLUDED_SUFFIXES = (".pyc",)
RETRIES = 3


class CaptureConflict(RuntimeError):
    """The live tree changed while it was being copied."""


def git(root: Path, *args: str) -> str:
    return subprocess.run(["git", *args], cwd=root, text=True, capture_output=True, check=True).stdout.rstrip("\n")


def excluded(relative: str, name: str) -> bool:
    top = relative.split("/", 1)[0]
    return (top in EXCLUDED_ROOTS or relative in EXCLUDED_PATHS or name in EXCLUDED_NAMES
            or name.endswith(EXCLUDED_SUFFIXES))


def scan(root: Path, *, hashes: bool) -> dict[str, list]:
    """Every input file under `root`: size and mtime, and its content hash when asked."""
    files = {}
    for directory, names, basenames in os.walk(root):
        relative = Path(directory).relative_to(root).as_posix()
        prefix = "" if relative == "." else relative + "/"
        names[:] = sorted(n for n in names if not excluded(prefix + n, n))
        for name in sorted(basenames):
            path = Path(directory) / name
            if excluded(prefix + name, name):
                continue
            if path.is_symlink():
                files[prefix + name] = ["link", os.readlink(path)]
                continue
            stat = path.stat()
            entry = [stat.st_size, stat.st_mtime_ns]
            if hashes:
                entry.append(hashlib.sha256(path.read_bytes()).hexdigest())
            files[prefix + name] = entry
    return files


def digest(files: dict[str, list]) -> str:
    """One identity of the snapshot's contents (paths and content hashes, not mtimes)."""
    h = hashlib.sha256()
    for path, entry in sorted(files.items()):
        h.update(path.encode() + b"\0" + str(entry[-1]).encode() + b"\0")
    return h.hexdigest()


def rsync(root: Path, dest: Path) -> None:
    # mtimes compared to the nanosecond: a checkout in the snapshot and a live edit of the
    # same size within one second would otherwise pass for the same file
    args = ["rsync", "-a", "--delete", "--quiet", "--modify-window=-1"]
    args += [f"--exclude=/{name}" for name in EXCLUDED_ROOTS + EXCLUDED_PATHS]
    args += [f"--exclude={name}" for name in EXCLUDED_NAMES]
    args += [f"--exclude=*{suffix}" for suffix in EXCLUDED_SUFFIXES]
    subprocess.run([*args, f"{root}/", f"{dest}/"], check=True)


def link(target: Path, path: Path) -> None:
    """`path` is a symlink to `target`; anything else there is an error, never replaced."""
    if path.is_symlink():
        if os.readlink(path) == str(target):
            return
        raise RuntimeError(f"{path} links to {os.readlink(path)}, not {target}")
    if path.is_dir() and not any(path.iterdir()):
        path.rmdir()  # the checkout's placeholder of an uninitialized submodule
    if path.exists():
        raise RuntimeError(f"{path} exists and is not a link to {target}")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.symlink_to(target)


def worktree(root: Path, dest: Path, head: str) -> None:
    """`dest` is a git worktree of `root`'s repository, detached at `head`. Tracked files
    that are not inputs are left out of its checkout (sparse), so that its status lists the
    same changes as the live tree rather than every such file as deleted."""
    if not (dest / ".git").exists():
        if dest.exists() and any(dest.iterdir()):
            raise RuntimeError(f"{dest} exists and is not a snapshot worktree; remove it")
        dest.parent.mkdir(parents=True, exist_ok=True)
        git(root, "worktree", "prune")
        git(root, "worktree", "add", "--detach", "-q", "--no-checkout", str(dest), head)
        git(dest, "sparse-checkout", "set", "--no-cone", "/*",
            *(f"!/{name}/" for name in EXCLUDED_ROOTS + EXCLUDED_PATHS))
        git(dest, "checkout", "-q", "--detach", head)
    elif git(dest, "rev-parse", "HEAD") != head:
        git(dest, "checkout", "-q", "--detach", "--force", head)


def capture(root: Path, dest: Path, manifest_path: Path, cache: Path | None = None) -> dict:
    """Copy `root`'s inputs to `dest` and record what was copied at `manifest_path`; the
    snapshot's `.cache` links to `cache` (default: `root`'s)."""
    root, dest = root.resolve(), dest.resolve()
    if dest == root or root in dest.parents and dest.relative_to(root).parts[0] != ".cache":
        raise RuntimeError(f"snapshot {dest} must be outside the inputs of {root}")
    head = git(root, "rev-parse", "HEAD")
    status = git(root, "status", "--porcelain", "--ignore-submodules")
    worktree(root, dest, head)
    for attempt in range(RETRIES):
        before = scan(root, hashes=True)
        rsync(root, dest)
        if scan(root, hashes=False) == {k: v[:2] for k, v in before.items()}:
            break
    else:
        raise CaptureConflict(f"{root} kept changing while it was being copied ({RETRIES} attempts); "
                              "pause edits of this checkout to measure it")
    copied, expected = scan(dest, hashes=False), {k: v[:2] for k, v in before.items()}
    if copied != expected:
        differing = sorted(k for k in copied.keys() | expected.keys() if copied.get(k) != expected.get(k))
        raise CaptureConflict(f"{dest} differs from {root} after the copy: {', '.join(differing[:5])}"
                              f"{' ...' if len(differing) > 5 else ''}")
    link((cache or root / ".cache").resolve(), dest / ".cache")
    manifest = {"root": str(root), "snapshot": str(dest), "revision": head, "status": status,
                "captured_at": dt.datetime.now(dt.timezone.utc).isoformat(),
                "digest": digest(before), "files": before}
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    manifest_path.write_text(json.dumps(manifest, indent=1) + "\n")
    return manifest


def live_matches(manifest: dict) -> bool:
    """Whether the live tree still has the snapshot's contents."""
    return digest(scan(Path(manifest["root"]), hashes=True)) == manifest["digest"]
