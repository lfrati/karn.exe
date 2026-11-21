# karn.exe build entry points.

# Native rules engine builds, correctness checks and performance measurements.
.DEFAULT_GOAL := help
PYTHON ?= uv run python
MGN_TESTS := $(patsubst scripts/tests/%.py,%,$(wildcard scripts/tests/test_mgn_*.py))

.PHONY: help check test test-rules test-mgn
help:
	@echo "make test-rules         Focused rules suite"
	@echo "make test-mgn           MGN integration records and their tooling"
	@echo "make check              Rust workspace, tooling and MGN integration records"
	@echo "make coverage           Opt-in engine coverage (set COVERAGE_ARGS with durable --out and --native-trace)"
	@echo "make progress          Full validation and detailed progress report"
	@echo "make readme-progress   Refresh README admission and test inventory (no test run)"

test: check

# Isolated instrumented builds; never use these binaries for performance evidence.
COVERAGE_ARGS ?=
.PHONY: coverage
coverage:
	$(PYTHON) scripts/code_coverage.py $(COVERAGE_ARGS)

test-rules:
	cargo test --locked -p game --test rules

test-mgn:
	$(PYTHON) scripts/rust_tools.py build
	$(PYTHON) scripts/tooling_tests.py --durations 0 $(MGN_TESTS)

# Use the test profile: its debug assertions and overflow checks are intentional.
# Run these sequentially even under make -j: tooling fixtures temporarily edit sources.
check:
	$(PYTHON) -c 'import sys; assert sys.version_info >= (3, 12), "Python 3.12+ is required"'
	$(PYTHON) scripts/rules.py --check
	cargo test --locked --workspace
	cargo build --locked --profile fast -p game --bin game-gate --bin game-serve -p grammar --bin grammar-gate
	$(PYTHON) scripts/tooling_tests.py --durations 0

# Detailed rules, admission and evidence measurements.
PROGRESS_ARGS ?=
.PHONY: progress progress-check
progress:
	@$(PYTHON) scripts/progress.py $(PROGRESS_ARGS)

progress-check: progress

.PHONY: readme-progress
readme-progress:
	@$(PYTHON) scripts/readme_progress.py

# Recorded decisions; opt-in Rust timings and counters, compiled out of ordinary builds.
PROFILE_TRACE ?= tests/fixtures/performance/bant-ordered-mgn.jsonl
PROFILE_ARGS ?=
.PHONY: profile
profile:
	cargo build --release --target-dir target/profile -p game --bin game-serve --features profile
	$(PYTHON) scripts/perf_replay.py $(PROFILE_TRACE) --engine target/profile/release/game-serve \
		--profile-dir .cache/performance/profile --output .cache/performance/profile/report.json $(PROFILE_ARGS)

# Engine-only supplied-action throughput and typed observations; discovery diagnostics are skipped.
# All recorded complete-game workloads by default; no player, JSON or pipe time in the loop.
CORE_ARGS ?=
.PHONY: perf-core
perf-core:
	cargo build --release -p game --bin game-serve
	$(PYTHON) scripts/perf_core.py $(CORE_ARGS)


