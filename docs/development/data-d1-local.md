# Data Phase D1 local verification

**Status:** D1 offline historical data contracts and normalization verified locally on Windows x64 with Python 3.13.13, Rust/Cargo 1.99.1. This does not verify an intraday simulation, a provider adapter, online ingestion, or hosted behavior.

## Checks run

| Check | Result |
| --- | --- |
| `cargo fmt --manifest-path rust/Cargo.toml --all --check` | Passed. |
| `cargo test --locked --offline --manifest-path rust/Cargo.toml --workspace -- --test-threads=1` | Passed across the workspace with local loopback access enabled. |
| `cargo clippy --locked --offline --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings` | Passed. |
| `cargo test --locked --offline --manifest-path rust/Cargo.toml -p factorsbt-data` | 24 data unit tests and 3 historical acceptance tests passed. |
| `$env:PYTHONPATH = Join-Path (Get-Location) 'python-sdk/src'; python -B -m unittest discover -s python-sdk/tests -p 'test_*.py' -q` | 76 tests passed, including Draft 2020-12 contract examples. |

The workspace regression initially ran in the restricted network sandbox; tests using local loopback failed there. Re-running the same suite with loopback access and one test thread passed. Python schema tests use the repository-pinned `jsonschema==4.26.0` test dependency.

## Verified behavior and limits

- Historical manifest, bar, revision, and dataset-reference examples validate against the new offline schema. Secret-like request parameters and missing volume are rejected.
- Rust normalizes provider Unix milliseconds to minute-aligned UTC and validates exact-decimal OHLCV ordering, interval duration, snapshot availability, instrument identity mapping, and revision selection by availability time. Diagnostics distinguish missing intervals, duplicates, and rejected rows without including raw provider payloads.
- The schema-to-normalization acceptance test reads existing v1 fixtures successfully. No fixture bytes, run v1 schema, or HTTP route changed.
- D1 does not validate exchange-calendar membership, generate intraday events, store JSONL partitions, fetch provider data, or submit historical datasets through the SDK/backend. Those remain later phases.

No provider request or credential was used. D2 online ingestion remains gated by account entitlement and data-use rights.
