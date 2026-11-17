# Data Phase D1a local verification

**Status:** D1a intraday clock, coverage and execution prerequisites passed local verification on Windows x64 with Python 3.13.13 and Rust/Cargo 1.99.1. The acceptance uses synthetic schedules and bars. It does not verify an authoritative exchange calendar catalog, provider ingestion, or SDK/API end-to-end intraday runs.

## Checks run

| Check | Result |
| --- | --- |
| `cargo fmt --manifest-path rust/Cargo.toml --all --check` | Passed. |
| `cargo test --locked --offline --manifest-path rust/Cargo.toml --workspace -- --test-threads=1` | Passed: 194 tests across the workspace and 0 doc tests. Existing local loopback service tests ran with loopback access enabled. |
| `cargo clippy --locked --offline --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings` | Passed. Two strict-Clippy findings were corrected before the final gate rerun. |
| `cargo test --locked --offline --manifest-path rust/Cargo.toml -p factorsbt-backend --test intraday_acceptance -- --nocapture` | Passed: 5 acceptance tests. |
| `$env:PYTHONPATH = Join-Path (Get-Location) 'python-sdk/src'; python -B -m unittest discover -s python-sdk/tests -p 'test_*.py' -q` | Passed: 77 tests, including the versioned session schedule examples. |

## Verified behavior and limits

- The intraday clock consumes a versioned offline `America/New_York` schedule and emits UTC minute open/close events. Synthetic checks cover daylight and standard offsets, a 210-minute early close, an omitted holiday date, and same-instant event ordering. The existing daily UTC clock retains its close/open/close sequence.
- Coverage reports missing minute intervals only when the source retrieval is marked complete. An incomplete source request is reported separately; no bar is synthesized or forward-filled. A missing final close remains unavailable and the engine flatten primitive fails without mutating the portfolio or canceling pending orders.
- Backend order scheduling skips a bar open equal to the decision timestamp and never selects a later session's open. Session-close flattening uses the final bar close with configured spot fees/slippage, cancels pending orders only after successful fills, and requires an unambiguous valid final close for each open spot position.
- The check runs against synthetic schedules/bars; it does not establish an authoritative maintained calendar dataset. D1a does not connect the primitives to HTTP `/v1`, change the v1 run schema, submit complete intraday runs through Python/CLI, or ingest provider data.

No provider request, credential, or private dataset was used. D2 online ingestion remains gated by account entitlement and data-use rights.
