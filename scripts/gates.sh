#!/usr/bin/env bash
# Build the grammar gate (optimized, incremental) and run every gate; fails if any gate reports a
# mismatch or an ambiguity. Usage: scripts/gates.sh [gate ...]   (default: all)
set -euo pipefail
cd "$(dirname "$0")/.."
# the target directory is this checkout's (scripts/rust_tools.py claim_target), never another's
target="${CARGO_TARGET_DIR:-target}"
marker="$target/karn-checkout"
here="$(pwd -P)"
if [ -f "$marker" ] && [ "$(cat "$marker")" != "$here" ]; then
  echo "$target was built from $(cat "$marker"), not this checkout $here: build each checkout into its own target directory"
  exit 1
fi
mkdir -p "$target"
[ -f "$marker" ] || printf '%s\n' "$here" > "$marker"
# a failed build must not run the previous binary
if ! log=$(cargo build --profile fast -p grammar --bin grammar-gate 2>&1); then
  grep -E "^error" -A12 <<<"$log"
  echo "build failed"
  exit 1
fi
grep -E "^warning" -A6 <<<"$log" || true
gates=("$@")
[ ${#gates[@]} -eq 0 ] && gates=(selectors amounts triggers conditions costs effects cards)
# The sub-corpora four gates read are generated from the pinned corpus and rules by two
# scripts (a fresh checkout has none). They are valid for the inputs they were made from: a
# stamp with those inputs' digest is written after a whole generation and removed before
# one, so a changed input, a failed or partial generation, or a missing file regenerates,
# without rewriting the committed research report.
dumps=(noun_phrases amounts conditions triggers)
inputs=(resources/oracle/cards.jsonl resources/rules/MagicCompRules-*.txt scripts/inventory.py scripts/corpus_structure.py)
stamp=.cache/oracle/dumps.sha256
want=$(cat "${inputs[@]}" | sha256sum | cut -d' ' -f1)
why=""
for d in "${dumps[@]}"; do [ -s ".cache/oracle/$d.jsonl" ] || { why="$d.jsonl missing"; break; }; done
if [ -z "$why" ] && [ "$(cat "$stamp" 2>/dev/null)" != "$want" ]; then why="inputs changed since they were generated"; fi
if [ -n "$why" ]; then
  echo "generating the corpus dumps in .cache/oracle ($why)"
  mkdir -p .cache/oracle
  rm -f "$stamp"
  read -ra python <<<"${PYTHON:-python3}"  # a command line, as the Makefile's PYTHON is
  if ! log=$("${python[@]}" scripts/corpus_structure.py --no-report --dump-nps --dump-amounts --dump-conditions --dump-triggers 2>&1); then
    tail -n 12 <<<"$log"
    echo "corpus dumps failed"
    exit 1
  fi
  for d in "${dumps[@]}"; do
    [ -s ".cache/oracle/$d.jsonl" ] || { echo "corpus dumps failed: $d.jsonl not written"; exit 1; }
  done
  printf '%s\n' "$want" > "$stamp"
fi
# Use the artifact directory of the build above, including isolated worktree builds.
gate="${CARGO_TARGET_DIR:-target}/fast/grammar-gate"
# one gate at a time: each already uses every core
status=0
for g in "${gates[@]}"; do
  out=$("$gate" "$g" 2>&1) || status=1
  result=$(grep -E 'RoundTrip:|all rows round-trip|GATE' <<<"$out" | sed 's/^ *//' | tr '\n' ' ' || true)
  # a gate that reports nothing failed: say why, and fail even when it exits 0
  if [ -z "$result" ]; then result="NO RESULT: $(head -n1 <<<"$out")"; status=1; fi
  printf '%-11s %s\n' "$g" "$result"
done
exit $status
