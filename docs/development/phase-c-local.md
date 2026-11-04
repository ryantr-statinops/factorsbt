# Phase C local verification

**Status:** Verified on Windows with MSVC and Python 3.13 on 2026-11-04. This record covers the controlled local comparison and recorded-replay path; it does not establish a hosted service or public API stability.

## Prerequisites

- Visual Studio 2022 Build Tools with the x64 MSVC linker and Windows SDK used for `x86_64-pc-windows-msvc`.
- Rust/Cargo available in an x64 Developer Command Prompt.
- Python 3.13 and the temporary test environment with `jsonschema==4.26.0`.
- Loopback access for the Rust backend and controlled Python model fixture.

The backend binds to loopback. Smoke scripts use temporary evidence directories and stop child processes after use. The SDK has no runtime third-party dependency; JSON Schema validation is test-only.

## Rust workspace

From `rust/` in the x64 Developer Command Prompt:

~~~powershell
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'factorsbt-phase-b-target'
cargo fmt --all -- --check
cargo check --workspace --all-targets --offline
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo test --workspace --offline
~~~

All commands passed. The workspace reported 146 tests: backend 56, contracts 18, shared Rust examples 3, data 16, engine 52, and controlled engine acceptance 1. Cargo emitted non-failing Windows incremental-cache hard-link warnings while copying cache files.

## Python SDK and schemas

From the repository root, using the temporary Phase C test environment:

~~~powershell
& (Join-Path $env:TEMP 'factorsbt-phase-c-python\Scripts\python.exe') -B -m unittest discover -s python-sdk/tests -v
~~~

All 48 unit tests passed. The suite checks typed SDK requests/responses, comparison report calculations and partial failures, replay decoders, Draft 2020-12 schemas, shared examples, fixture digests, and local OpenAPI/schema references.

## Comparison and replay integration

Set the temporary Cargo target and run from the repository root:

~~~powershell
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'factorsbt-phase-b-target'
$testPython = Join-Path (Join-Path $env:TEMP 'factorsbt-phase-c-python') 'Scripts\python.exe'
& $testPython -B python-sdk/tests/smoke_backtest.py
& $testPython -B python-sdk/tests/smoke_sdk_failures.py
~~~

The success smoke passed for `scores`, `targets`, and `order_intents`. It restarts the backend before the first replay, then loads the source from sealed local evidence. Each replay matched and returned isolated evidence whose size and SHA-256 were checked. Model fixture request counts did not increase during replay. After comparison, the model fixture was shut down completely; a replay still matched while the source endpoint was offline. Repeated replay submission returned the original acknowledgment; reusing the key for a different source returned the expected conflict. Source evidence was checked again after replay.

The same smoke used `Client.compare()` for controlled baseline, price-only, and delayed-event fixtures. The scenarios completed sequentially, each had metrics and trace/timeline artifact links, and the report was saved locally. Baseline and price-only timelines shared 28 time/instrument points with no unmatched points.

The failure smoke covered a lost run acknowledgment resolved by lookup, changed run configuration under the same key, a controlled model HTTP 503, a matched replay of that failed source, replay evidence integrity, and a local wait timeout that did not cancel the run. The failed-source replay also made no new model HTTP request.

## Scope and limits

- Replay eligibility requires a complete, integrity-checked source manifest, the same executable digest, and contract version `1.0.0`. Incomplete or incompatible source evidence is rejected before enqueue; older Phase B evidence is not upgraded in place.
- Replay evidence is written beneath a replay-specific local directory. Replay metadata and idempotency remain process-local. There is no replay artifact download endpoint.
- Comparison is a Python SDK orchestration of independent v1 runs; it is sequential and writes its report locally. It is not a separate backend comparison job.
- Replay verification compares ordered admission, order, execution, settlement and ledger transitions, then cash, positions, and NAV. Generated IDs and service timestamps are ignored. This does not promise byte-identical artifacts, cross-build/platform replay, or fresh-inference determinism.
- Phase C does not add CI. The repository has no CI workflow, so no CI result is available.
