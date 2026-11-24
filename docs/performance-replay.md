# Performance

Speed is a primary target. These tools measure fixed workloads so that a change can be
compared with what came before. They say nothing about correctness: that is what the
[tests](testing.md) are for.

## The workloads

Three complete games, each a stream of the actions taken while replaying an MGN record: Bant,
Vivi and Torch. They keep the explicit mana selections and the recorded random outcomes, so
every run executes the same game. The
[manifest](../tests/fixtures/performance/mgn-games.json) pins each trace and the record it
came from.

## Measuring

```sh
make perf-core                 # engine throughput on the three games, one thread
make profile                   # one trace with the engine's internal timers and counters
make perf-scale                # four-player fixtures at 100, 400 and 1,000 objects
uv run scripts/perf_core.py --engine /path/to/engine --engine-source BUILD_COMMIT --build-profile fast --runs 3 --output .cache/performance/core.json
uv run scripts/perf_parallel.py --engine /path/to/engine --engine-source BUILD_COMMIT --build-profile fast --workers 1,2,4,8 --output .cache/performance/parallel.json
```

`perf-core` replays supplied actions through the engine's typed boundary. It excludes player
time, setup, JSON and transport. `profile` builds with internal instrumentation; choose its
trace with `PROFILE_TRACE` and its arguments with `PROFILE_ARGS`. Its report includes
`internal_profiles` per run; instrumented timing is diagnostic and must not be compared
with normal builds. Profiling uses the same typed replay path as `perf-core`, including
current handling of historical quote metadata.

`perf-scale` supplements the complete games with synthetic clone and first-write costs,
cold/warm characteristic queries under four anthems and a same-layer dependency chain,
plus queries after unrelated mana changes and relevant counter/producer changes,
and explicit mana activations among
mostly unrelated trigger subscriptions. Setup and validation are outside timing; every
workload checks its relevant results and branch isolation. The report retains failures,
binary/workload hashes, build provenance and CPU affinity. Rates are per named operation,
not complete-game throughput. Use `SCALE_ARGS='--objects 100,400,1000 --runs 15'` to change
the matrix. Keep the same workload source hash and configuration across compared builds.

`perf_parallel.py` runs each row's workers on as many distinct physical cores, one logical CPU
of each, read from the kernel topology (performance cores before efficient ones on a hybrid
processor). The report records the topology and each row's placement. A row with more workers
than physical cores is refused; `--allow-shared-cores` runs it on SMT siblings and warns.
Rows measure aggregate throughput of independent games; quote the worker count and
placement with each rate.

Without `--workers`, the matrix uses powers of two that fit the process's affinity and adds
the full available physical-core count (12 cores: `1,2,4,8,12`). The report records that
chosen matrix. Explicit requests are validated before any row runs.

## Comparing two builds

- Use the same traces, worker counts, CPU affinity, repetitions and build profile for both
  binaries, on an idle machine. Record them with the result.
- Compare only workloads both builds run with the same meaning. If the earlier build refuses a
  new mechanic, measure the new workload on the candidate alone and mark the difference as not
  comparable: a refusal is not zero throughput, and it is not a speedup.
- Keep every failure in the report. A failed or divergent replay is a failed measurement.
- Repeat a suspected regression without competing work before blaming the change.
- A measured rate belongs to its workload. It is not a guarantee for every board, and never a
  reason to weaken a correctness check.

`engine_source` names the build that was measured; `harness_revision` names the checkout
running the script. An unknown engine source stays unknown.

## History and invalidation investigation

```sh
make perf-scale SCALE_ARGS='--runs 5 --history-events 1,100,1000'
```

The optional history matrix varies committed events independently of objects, with zero or
16 first-occurrence watchers, zero or 16 history-dependent characteristic queries, and
clones every step, every eight steps, or none inside the measured loop. Each step gains
life, activates an explicit mana ability and performs the requested queries. The original
branch, life/mana totals, conditional power and absence of new first-occurrence triggers
are checked. Separate clone rows distinguish an undelivered raw log from delivered turn
facts. Event setup is a bulk fixture with no active watcher or gain replacement; only the
consumed raw log is cleared. Setup, initial clone and validation are excluded from timing.
This measures the raw Game path; hosted complete-game replays remain a separate workload.

For characteristic rows, cache priming and the named mutation happen before timing; the
measured operation is the subsequent query. Profile builds include per-row internal counters
for these rows and history actions. Build `game-scale` with `--features profile` into a
separate target directory for diagnostics; the report flags instrumentation. Never compare
those timings with ordinary release builds. No allocation or memory totals are collected.

The November 22 investigation found that unrelated mana changes already preserve the
cached world: at 1,000 creatures, warm/after-mana queries cost about 45/44 ns under four
anthems and 112/111 ns in the dependency fixture. Diagnostic counters show zero world
builds after mana changes; relevant counter changes or producer departure build one.
**No additional characteristic cache is justified by these fixtures.**

Long-turn history exposed a smaller change: boolean “at least one” conditions counted every
matching event and materialized the whole history. They now iterate the existing retained
facts and undelivered log suffix lazily and stop at one match. Exact counts and ordinals
retain their existing semantics; no history index or second store was added.

Matched release probes (same workload-source hash, CPU 0, five repetitions, eight steps)
with 1,000 prior events and 16 history-dependent queries fell from **109.7 ms to 1.14 ms
per step** without watchers, and **110.1 ms to 1.70 ms** with 16 first-occurrence watchers.
These are synthetic fixtures, not universal game speedups. Clone cost still scales with
retained history (about 17 µs for 1,000 delivered facts in this fixture); exact occurrence
scans remain linear. Neither cost justified importing a persistent event index here.
The first 10,000-event attempt was interrupted before completion and retained as a failed
measurement. Its fixture generated gains iteratively; the corrected matrix uses bulk setup
and stops at 1,000 events. No timing or phase attribution is claimed for the interruption.

The three complete-game typed replays remain within 0.6% of the baseline in balanced
B,C,C,B runs (15 repetitions per phase, CPU 0); Bant JSON throughput is unchanged to within
0.01%, with all 20 response digests identical. No general game speedup is claimed. Baseline
production code is 7f6c277; the candidate adds the history existence path. Raw local evidence
and SHA-256 provenance are retained under `.cache/goal-work/`: `profiling-history-release-v2.json`,
`profiling-history-candidate.json`, `profiling-scale-comparison.json`,
`profiling-history-diagnostic-v2.json`, `profiling-core-diagnostic.json` and
`history-optimization-perf/`. Instrumented profiles are diagnostic only. These reports
were collected without competing builds or tests; their rates apply only to their workloads.

## November 22 keyword-round snapshot and compiler cleanup

The snapshot at `6f32ac1` and its compiler cleanup (`398f0c9`) were compared with
`round-start` (`500f3bf`). These are the three existing complete-game supplied-action
traces; work landed by other builders after the snapshot is outside this comparison.
All binaries used the uninstrumented `fast` profile with incremental compilation disabled.

Measurements used CPU 0 and the order R,B,C,C,B,R: round start, before cleanup,
candidate, candidate, before cleanup, round start. Every phase ran each trace 15 times,
giving 30 samples per trace and binary. Each phase waited for a quiet window; no competing
build or game process was observed during any accepted phase. The table compares the
medians of those 30 samples. Positive time changes are slower.

| Trace | Candidate vs. round start | Candidate vs. before cleanup |
| --- | ---: | ---: |
| Bant | +1.06% | -1.34% |
| Vivi | +0.86% | -2.96% |
| Torch | +1.43% | -0.37% |

These samples show small changes on the three recorded workloads. They establish no
general speedup. Every accepted replay passed its typed final-snapshot checks.
Initial balanced attempts were contaminated by compiler, admission and game processes;
their timings are excluded and their reports retained.

Raw phase results, commands, binary and trace hashes, and competing-process observations
are retained in `.cache/quality/phase-gated-performance/comparison.json`; the initial
contended attempts are in `.cache/quality/round-performance/` and
`.cache/quality/cleanup-performance/`. `.cache/quality/environment.json` records the
build environment. `.cache/quality/run-phase-gated.py` is the local measurement driver.

The cleanup also preserved byte-identical canonical program dumps for all 20,536 admitted
faces and admission results for all 32,872 corpus faces against `6f32ac1`.
`.cache/quality/verification.json` records that comparison and the passing Rust and
Python/tooling checks. Generated targets and the temporary round-start source archive
were removed after validation; rebuilding them is necessary to repeat a measurement.

## November 22 keyword round, measured at its review

The whole round, `round-start` (`500f3bf`) against `f51586d` once every lane had landed,
on the three complete-game supplied-action traces with `perf_core.py` and the `release`
profile. The order was B,C,C,B, five runs of each trace per phase, ten samples per trace and
binary, with no build or game running alongside. The table compares medians; positive time
changes are slower.

| Trace | Round start | Round end | Change |
| --- | ---: | ---: | ---: |
| Bant | 41.66 ms | 42.88 ms | +2.9% |
| Vivi | 20.17 ms | 20.66 ms | +2.4% |
| Torch | 26.00 ms | 27.04 ms | +4.0% |

Runs of one binary spread by about 1%, so the round made these games slower. The cause is
not yet attributed. The mid-round snapshot with its cleanup was within 1.5% of the round
start (above), so most of it came from work that landed after `398f0c9`. Raw results and logs are in
`.cache/round-performance/review-2026-11-24/`.
