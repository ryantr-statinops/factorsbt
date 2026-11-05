# Phase D local verification

**Status:** Verified locally on Windows with MSVC, Rust/Cargo and Python 3.13 on 2026-11-05. This evidence covers the bounded reranking and fixed-action episode jobs; it does not imply general RL training, hosted deployment, cross-build replay, or public API stability.

## Prerequisites and commands

- Visual Studio 2022 Build Tools with x64 linker and Windows SDK for `x86_64-pc-windows-msvc`.
- Python 3.13. Test-only dependency: `jsonschema==4.26.0` from `python-sdk/requirements-test.txt` in a temporary virtual environment.
- Loopback access for the local Rust backend. Evidence is written to temporary directories during smoke runs.

From `rust/`, these workspace checks passed after Phase D changes:

~~~powershell
cargo fmt --all -- --check
cargo check --workspace --all-targets --offline
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo test --workspace --offline
~~~

The full Rust workspace passed 63 backend tests, 19 contracts tests, 3 shared contract examples, 16 data tests, 52 engine tests, 1 controlled engine acceptance test, and doc tests. The engine suite includes checked financial and pricing behavior. Cargo may print non-failing Windows incremental-cache hard-link warnings.

From the repository root, using the temporary Python test environment:

~~~powershell
& (Join-Path $env:TEMP 'factorsbt-phase-d-venv\Scripts\python.exe') -B -m unittest discover -s python-sdk/tests -v
~~~

The Python suite passed 54 tests. It checks research request/status/result schemas and OpenAPI wiring, including invalid action values, source request conflicts, terminal failure shape, and typed report decoding for reranking, episode and replay.

## End-to-end evidence

- `python-sdk/tests/smoke_backtest.py` passed through the Rust backend for the three Phase B output modes, Phase C comparison, and recorded replay after backend restart with the model fixture offline. It verified replay evidence without mutating the source.
- `python-sdk/tests/smoke_sdk_failures.py` passed lost acknowledgement lookup, idempotency conflict, controlled model failure replay, and local wait timeout behavior.
- `python-sdk/examples/research_workflows.py` ran reranking and episode jobs through the Rust HTTP backend, then replayed the sealed episode. The evaluation episode reported rewards `-1.02 USD` and `1499.98 USD`; both include the configured per-notional fees. The replay report was matched with no first mismatch.
- Research acceptance tests verified the evaluation reranker changes a selection, carries engine admission values to fills, and reports equal P&L versus baseline for the controlled fixture. Episode tests matched the fee-bearing reward calculations, separate state across episodes, and preservation of the last completed transition after an invalid next-open price. Replay verification tests covered a matched failed episode source and first-mismatch reporting.
- Research evidence stores request, versioned fixture, result/trace, contract version, executable digest, byte sizes and SHA-256 in a per-job directory. API integration tests verified a sealed source can be loaded after backend restart and replay does not change its source evidence.

## Limits

Research fixtures are controlled and registered in code by ID/version; they are not market data. The engine supplies admission, next-open execution, fees, ledger, marks and NAV for each research step. The episode endpoint accepts a complete fixed action sequence and returns a finished trace; it is not a live interactive RL environment. Replay requires the same executable digest and contract version. There is no CI workflow, so no CI result is available.
