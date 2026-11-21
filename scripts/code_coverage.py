#!/usr/bin/env python3
"""Opt-in instrumented engine inventory; raw evidence stays outside the checkout."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys

from rule_evidence import mask, closing
from rust_tools import ROOT, claim_target
from mgn.runner import inventory_records

CARGO_LLVM_COV = "0.8.4"
TOOLCHAIN = "1.94.0"
NATIVE_REPLAY = """import json,sys
from pathlib import Path
sys.path.insert(0, 'scripts')
from perf_replay import replay
trace,engine,output=map(Path,sys.argv[1:])
requests=[json.loads(line) for line in trace.read_text().splitlines() if line.strip()]
report=replay(engine,requests,180)
output.write_text(json.dumps(report,indent=2)+'\\n')
"""


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_processes(out, prefix, executables, profiles):
    """Every traced selected executable must exit normally and flush its own profile."""
    processes = []
    for trace in sorted(out.glob(prefix + ".process.*")):
        text = trace.read_text()
        selected = [exe for exe in executables if f'execve({json.dumps(exe)},' in text]
        if not selected:
            continue
        pid = int(trace.suffix[1:])
        if not text.rstrip().endswith("+++ exited with 0 +++"):
            raise RuntimeError(f"selected engine process {pid} failed/killed: {trace}")
        raw = list(profiles.glob(f"*-{pid}.profraw"))
        if not raw or any(p.stat().st_size == 0 for p in raw):
            raise RuntimeError(f"selected engine process {pid} contributed no profile: {trace}")
        processes.append({"pid": pid, "executable": selected[0], "trace": str(trace), "profiles": [str(p) for p in raw]})
    if not processes:
        raise RuntimeError(f"no selected engine execution in {prefix}")
    return processes


def test_lines(source):
    """Exclude complete #[cfg(test)] items, including helpers outside test modules."""
    text = mask(source)
    excluded = set()
    for match in re.finditer(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]", text):
        start = match.start()
        at = match.end()
        while at < len(text):
            if text[at] in "([":
                at = closing(text, at)
                continue
            if text[at] == "{":
                end = closing(text, at)
                break
            if text[at] == ";":
                end = at + 1
                break
            at += 1
        else:
            raise ValueError("unterminated cfg(test) item")
        excluded.update(range(source.count("\n", 0, start) + 1,
                              source.count("\n", 0, end) + 2))
    return excluded


def inventory(export, root=ROOT):
    """Deduplicate monomorphizations; preserve zero-count region coordinates."""
    production = root / "crates/game/src"
    sources = {str(p.resolve()): p.read_text() for p in production.rglob("*.rs")}
    exclusions = {p: test_lines(text) for p, text in sources.items()}
    source_lines = {p: text.splitlines() for p, text in sources.items()}
    files = {}
    def eligible(filename, line):
        return filename in exclusions and line not in exclusions[filename]
    for data in export["data"]:
        for function in data.get("functions", []):
            regions = function.get("regions", [])
            if not regions:
                continue
            first = regions[0]
            filename = function["filenames"][first[5]]
            if not eligible(filename, first[0]):
                continue
            key = str(Path(filename).relative_to(root))
            item = files.setdefault(key, {"functions": {}, "regions": {}, "lines": {}})
            identity = tuple(first[:4])
            row = {"name": function["name"], "line": first[0], "column": first[1], "count": function["count"],
                   "source": source_lines[filename][first[0] - 1].strip()}
            prior = item["functions"].get(identity)
            if prior:
                row["count"] += prior["count"]
            item["functions"][identity] = row
            for region in regions:
                # LLVM kind 0 is a counted code region; expansion/skipped/gap regions
                # are not independently executable source obligations.
                if region[7] != 0:
                    continue
                region_file = function["filenames"][region[5]]
                if region_file != filename or not eligible(filename, region[0]):
                    continue
                coords = tuple(region[:4])
                item["regions"][coords] = item["regions"].get(coords, 0) + region[4]
        for file in data.get("files", []):
            filename = file["filename"]
            if filename not in exclusions:
                continue
            key = str(Path(filename).relative_to(root))
            item = files.setdefault(key, {"functions": {}, "regions": {}, "lines": {}})
            segments = file.get("segments", [])
            # Each segment's count applies until the next coordinate. Retain a line
            # if any counted non-gap interval intersects it; any executed interval
            # makes that line executed (regions retain the finer missed intervals).
            for a, b in zip(segments, segments[1:]):
                if not a[3] or (len(a) > 5 and a[5]):
                    continue
                end = b[0] if b[1] > 1 else b[0] - 1
                for line in range(a[0], end + 1):
                    if eligible(filename, line):
                        item["lines"][line] = max(item["lines"].get(line, 0), a[2])
    result = {}
    for filename, item in sorted(files.items()):
        functions = sorted(item["functions"].values(), key=lambda x: (x["line"], x["name"]))
        regions = [{"start": list(k[:2]), "end": list(k[2:]), "count": v} for k, v in sorted(item["regions"].items())]
        result[filename] = {
            "scope": "binary" if "/bin/" in filename else "library",
            "functions": {"executed": sum(f["count"] > 0 for f in functions), "missed": [f for f in functions if not f["count"]]},
            "regions": {"executed": sum(r["count"] > 0 for r in regions), "missed": [r for r in regions if not r["count"]]},
            "lines": {"executed": sum(v > 0 for v in item["lines"].values()), "missed": [k for k, v in sorted(item["lines"].items()) if not v]},
        }
    return {"files": result, "source_files_without_mapping": sorted(str(Path(p).relative_to(root)) for p in exclusions if str(Path(p).relative_to(root)) not in files)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", required=True, type=Path, help="new durable evidence directory outside the checkout")
    parser.add_argument("--cargo-llvm-cov", type=Path, default=Path("cargo-llvm-cov"))
    parser.add_argument("--target-dir", type=Path, help="reuse this checkout's isolated instrumented build directory")
    parser.add_argument("--native-trace", type=Path, action="append", required=True,
                        help="explicit compatible native JSONL trace; replay through existing perf_replay API (repeatable)")
    args = parser.parse_args()
    out = args.out.resolve()
    if out.is_relative_to(ROOT) or out.is_relative_to(Path("/tmp")):
        parser.error("--out must be durable storage outside the checkout and /tmp")
    out.mkdir(parents=True, exist_ok=False)
    env = dict(os.environ)
    env.pop("RUSTFLAGS", None)
    env.pop("CARGO_ENCODED_RUSTFLAGS", None)
    target = claim_target(args.target_dir or out / "target")
    env["CARGO_TARGET_DIR"] = str(target)
    env["RUSTUP_TOOLCHAIN"] = TOOLCHAIN
    commands = []
    status = {"format": "engine-code-coverage/1", "status": "collecting", "commands": commands,
              "toolchain": TOOLCHAIN, "cargo_llvm_cov": CARGO_LLVM_COV, "categories": {}}
    def save():
        (out / "report.json").write_text(json.dumps(status, indent=2) + "\n")
    def run(command, name, *, capture=False, run_env=None, trace=False):
        log = out / (name + ".log")
        commands.append({"argv": [str(x) for x in command], "log": str(log)})
        if trace:
            command = ["strace", "-ff", "-e", "trace=execve,exit_group", "-o", str(out / (name + ".process")), *command]
        with log.open("w") as stream:
            completed = subprocess.run(command, cwd=ROOT, env=run_env or env, text=True, stdout=stream, stderr=subprocess.STDOUT)
        commands[-1]["returncode"] = completed.returncode
        save()
        if completed.returncode:
            raise RuntimeError(f"{name} failed ({completed.returncode}); see {log}")
        return log.read_text() if capture else None
    def source():
        paths = [ROOT / "Cargo.toml", ROOT / "Cargo.lock", *sorted((ROOT / "crates").rglob("*.rs")), *sorted((ROOT / "crates").rglob("Cargo.toml")), *sorted((ROOT / "crates").rglob("*.json")), *sorted((ROOT / "resources").rglob("*"))]
        paths = [p for p in paths if p.is_file()]
        return {str(p.relative_to(ROOT)): digest(p) for p in paths}
    try:
        status["revision"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        status["source_sha256"] = source()
        version = run([str(args.cargo_llvm_cov), "llvm-cov", "--version"], "coverage-version", capture=True)
        if f" {CARGO_LLVM_COV}" not in version:
            raise RuntimeError(f"install cargo-llvm-cov {CARGO_LLVM_COV}; found {version.strip()}")
        settings = run([str(args.cargo_llvm_cov), "llvm-cov", "show-env", "--sh"], "coverage-env", capture=True)
        for line in settings.splitlines():
            if line.startswith("export "):
                key, value = line[7:].rstrip(";").split("=", 1)
                env[key] = shlex.split(value)[0]
        env["CARGO_TARGET_DIR"] = str(target)
        env.update(GAME_SOURCE_REVISION=status["revision"], GAME_SOURCE_DIRTY="true")
        toolroot = Path(subprocess.check_output(["rustc", "--print", "sysroot"], env=env, text=True).strip())
        host = re.search(r"host: (.+)", subprocess.check_output(["rustc", "-vV"], env=env, text=True))[1]
        llvm = toolroot / "lib/rustlib" / host / "bin"
        status["rustc"] = run(["rustc", "-vV"], "rustc", capture=True)
        status["branch_coverage"] = {"available": False, "reason": "Rust 1.94 stable has no supported -Z coverage-options=branch; no nightly toolchain substituted."}
        status["scope"] = {"primary": "production crates/game/src, library and binaries identified separately", "excluded": "dependencies, other workspace crates, tests/, complete #[cfg(test)] items", "limitations": "LLVM mapped source only; macros/generated functions may map to invocation sites. Unmapped files listed; no inferred coverage. Functions merge monomorphizations by source region coordinates, retaining closures. Doctests excluded; --workspace --lib --tests retains unit and integration tests.", "profile": "test: opt-level=1, debug assertions and overflow checks enabled"}
        objects = set()
        def artifacts(text):
            for line in text.splitlines():
                if line.startswith("{"):
                    event = json.loads(line)
                    if event.get("executable"):
                        objects.add(event["executable"])
        profiles = {}
        for category in ("rust", "mgn", "native"):
            folder = out / "profiles" / category
            folder.mkdir(parents=True)
            profiles[category] = folder
        env["LLVM_PROFILE_FILE"] = str(profiles["rust"] / "%m-%p.profraw")
        artifacts(run(["cargo", "test", "--locked", "--workspace", "--lib", "--tests", "--no-run", "--message-format=json"], "build-tests", capture=True))
        run(["cargo", "test", "--locked", "--workspace", "--lib", "--tests"], "rust-tests", trace=True)
        artifacts(run(["cargo", "build", "--locked", "--profile", "test", "-p", "game", "--bin", "game-serve", "--message-format=json"], "build-engine", capture=True))
        engine = target / "debug/game-serve"
        status["engine"] = {"path": str(engine), "sha256": digest(engine)}
        workload_profiles = []
        for index, (name, record) in enumerate(inventory_records(active=True)):
            folder = profiles["mgn"] / str(index)
            folder.mkdir()
            env["LLVM_PROFILE_FILE"] = str(folder / "%m-%p.profraw")
            run([sys.executable, "scripts/mgn/runner.py", str(record), "--engine", str(engine), "--out", str(out / "mgn" / str(index))], f"mgn-{index}", trace=True)
            report = json.loads(next((out / "mgn" / str(index)).rglob("report.json")).read_text())
            if any(value not in ("pass", "not_applicable") for value in report["levels"].values()):
                raise RuntimeError(f"incomplete MGN replay: {name}")
            workload_profiles.append({"category": "mgn", "name": name, "trace_prefix": f"mgn-{index}", "folder": str(folder), "input_sha256": digest(record)})
        for index, trace in enumerate(args.native_trace):
            trace = trace.resolve()
            name = f"native-{index}"
            folder = profiles["native"] / str(index)
            folder.mkdir()
            env["LLVM_PROFILE_FILE"] = str(folder / "%m-%p.profraw")
            run([sys.executable, "-c", NATIVE_REPLAY, str(trace), str(engine), str(out / (name + ".json"))], name, trace=True)
            workload_profiles.append({"category": "native", "name": str(trace), "trace_prefix": name, "folder": str(folder), "input_sha256": digest(trace), "scope": "supplied native actions/observations; replay success, not semantic vetting or performance evidence"})
        status["objects"] = {p: digest(Path(p)) for p in sorted(objects)}
        for category, folder in profiles.items():
            raw = sorted(folder.rglob("*.profraw"))
            if not raw or any(p.stat().st_size == 0 for p in raw):
                raise RuntimeError(f"missing/empty {category} profiles")
            status["categories"][category] = {"profiles": {str(p): digest(p) for p in raw}}
        # Every replay profile must share the selected engine's LLVM binary ID.
        engine_ids = run([str(llvm / "llvm-readobj"), "--notes", str(engine)], "engine-notes", capture=True)
        status["engine"]["elf_notes"] = engine_ids
        build_id = re.search(r"Build ID: ([a-fA-F0-9]+)", engine_ids)
        if not build_id:
            raise RuntimeError("selected engine has no ELF build ID")
        status["workloads"] = workload_profiles
        for index, workload in enumerate(workload_profiles):
            raw = sorted(Path(workload["folder"]).glob("*.profraw"))
            if not raw or any(p.stat().st_size == 0 for p in raw):
                raise RuntimeError(f"missing/empty profiles for {workload['name']}")
            # Validate every individual process profile, not only a merged category:
            # a profile from an ordinary binary cannot stand in for this engine.
            for number, profile in enumerate(raw):
                ids = run([str(llvm / "llvm-profdata"), "show", "--binary-ids", str(profile)], f"profile-id-{index}-{number}", capture=True)
                if build_id[1].lower() not in ids.lower():
                    raise RuntimeError(f"profile did not identify selected engine: {profile}")
            workload["process_profiles"] = len(raw)
            workload["engine_processes"] = verify_processes(out, workload["trace_prefix"], {str(engine)}, Path(workload["folder"]))
        game_tests = {p for p in objects if re.search(r"/(game|rules|game_gate|game_serve|native_selection_inventory|perf_baseline)-[0-9a-f]+$", p)}
        status["categories"]["rust"]["engine_test_processes"] = verify_processes(out, "rust-tests", game_tests, profiles["rust"])
        groups = {k: sorted(v.rglob("*.profraw")) for k, v in profiles.items()}
        groups["combined"] = sum(groups.values(), [])
        names = {}
        status["demangler"] = run(["c++filt", "--version"], "demangler-version", capture=True)
        for category, raw in groups.items():
            profdata = out / (category + ".profdata")
            run([str(llvm / "llvm-profdata"), "merge", "-sparse", *map(str, raw), "-o", str(profdata)], "merge-" + category)
            exported = run([str(llvm / "llvm-cov"), "export", "--instr-profile=" + str(profdata), *["--object=" + p for p in sorted(objects)]], "llvm-" + category, capture=True)
            data = json.loads(exported)
            unknown = sorted({f["name"] for d in data["data"] for f in d.get("functions", [])} - names.keys())
            if unknown:
                demangled = subprocess.run(["c++filt", "-s", "rust"], input="\n".join(unknown) + "\n", text=True, capture_output=True, check=True).stdout.splitlines()
                if len(demangled) != len(unknown):
                    raise RuntimeError("symbol demangler output length mismatch")
                names.update(zip(unknown, demangled))
            for dataset in data["data"]:
                for function in dataset.get("functions", []):
                    function["name"] = names[function["name"]]
            status["categories"].setdefault(category, {})["inventory"] = inventory(data)
        if source() != status["source_sha256"] or digest(engine) != status["engine"]["sha256"]:
            raise RuntimeError("source or engine changed during collection")
        status["status"] = "complete"
        save()
        for category, data in status["categories"].items():
            files = data["inventory"]["files"]
            print(f"{category}: {len(files)} mapped engine files")
            ranked = sorted(files.items(), key=lambda x: len(x[1]["regions"]["missed"]), reverse=True)
            for filename, item in ranked[:10]:
                print(f"  {filename}: {item['functions']['executed']} executed functions, {len(item['functions']['missed'])} missed; {len(item['regions']['missed'])} missed regions; {len(item['lines']['missed'])} missed lines")
                for function in item["functions"]["missed"][:2]:
                    print(f"    {filename}:{function['line']}: {function['source']}")
        print(f"Branch outcomes unavailable on stable {TOOLCHAIN}. Complete file/line gap inventory: {out / 'report.json'}")
    except Exception as error:
        status.update(status="incomplete", error=str(error))
        save()
        raise


if __name__ == "__main__":
    main()
