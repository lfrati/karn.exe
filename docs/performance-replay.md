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
Neither case measures per-core scaling, so say so wherever such a row is quoted.

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
