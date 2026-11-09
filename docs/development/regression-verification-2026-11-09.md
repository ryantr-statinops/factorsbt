# Code regression verification — 2026-11-09

**Status:** Local regression record for code revision `608ae9a` and the follow-up documentation review. The implementation was verified before these Markdown-only updates; this record does not claim that changing documentation required a new runtime build.

## Environment

Windows x64, Python 3.13.13, Rust/Cargo 1.99.1, Visual Studio 2022 Build Tools with x64 MSVC and Windows SDK. Rust commands ran in x64 Developer PowerShell with `CARGO_TARGET_DIR` under the temporary directory and `CARGO_PROFILE_DEV_DEBUG=0`. The Python suite used a temporary virtual environment. These are observed local versions, not a platform support matrix.

## Checks run

From repository root:

```powershell
cargo fmt --manifest-path rust/Cargo.toml --all --check
cargo test --locked --offline --manifest-path rust/Cargo.toml --workspace
cargo clippy --locked --offline --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings
$env:PYTHONPATH = Join-Path (Get-Location) 'python-sdk/src'
& $demoPython -B -m unittest discover -s python-sdk/tests -p 'test_*.py' -q
& $demoPython -B python-sdk/tests/smoke_backtest.py
& $demoPython -B python-sdk/tests/smoke_sdk_failures.py
& $demoPython -B python-sdk/tests/smoke_cli.py
```

The bounded [research example](../../python-sdk/examples/research_workflows.py) was also run against a temporary loopback backend and evidence directory.

| Check | Result and limit |
| --- | --- |
| Rust format | Workspace format check passed. |
| Rust workspace | 167 unit/integration tests passed across the workspace; doc-test targets had no cases. |
| Strict Clippy | Workspace all-targets check passed with `-D warnings`. |
| Python suite | 74 tests passed. |
| Backtest smoke | Scores, targets and order-intents passed; all produced fills and evidence. Sequential comparison completed. Recorded replay matched after backend restart with the model fixture offline. |
| Failure smoke | Uncertain acknowledgment lookup, idempotency conflict, failed-run replay and non-cancelling local wait timeout passed. |
| Installed CLI smoke | Clean package install, run/result workflow and offline replay passed. |
| Research example | Reranking outcome was equal to baseline; episode rewards were `-1.02` and `1499.98` USD; research replay matched. |

## Evidence I/O measurement

Test-only counters measured file reads and SHA-256 computations, not wall-clock latency:

| Operation | Before | After |
| --- | ---: | ---: |
| Download one artifact from an 11-artifact run bundle | 25 reads / 25 hashes | 2 reads / 2 hashes |
| Read one replay artifact after the source index was validated | 2 reads / 2 hashes | 1 read / 1 hash |

The evidence index still verifies the entire run bundle. Replay admission and worker startup verify the full source bundle; the worker then hashes each artifact again when it reads that artifact. No cross-job cache is used.

## Documentation guide review

Updated the model exchange and decision policy guides for sparse scores, omitted targets, expiry eligibility and zero targets. Updated engine outcome, margin and lifecycle guidance for cost-aware close admission, open-time affordability checks, atomic grouped fills and final-state collateral checks. Updated SDK, HTTP and operations guides for per-request polling deadlines, distinct late-response behavior, result/evidence access and replay verification. Added `GROUP-01`, `COST-01` and `EXP-01` to the acceptance cases with concrete test names and coverage limits.

The verification commands are recipes, while the table above records commands actually run. Acceptance mappings identify related tests and keep remaining end-to-end gaps explicit. This local run is not remote CI; the pushed repository tree has no GitHub Actions workflow, and remote Actions status could not be inspected from this environment.

License selection, real-provider behavior, hosted deployment and durable job recovery remain Open as described by the documentation index.
