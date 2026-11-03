# Phase A local development

**Status:** Verified on Windows with the MSVC target. This guide records the Phase A health-only checks; Phase B Stage 06 later added the local run API described in the [MVP HTTP API](../interfaces/mvp-http-api.md).

## Scope

The Phase A verification recorded here covers the local `GET /healthz` path and synchronous Python `Client.health()` call. It did not test run submission or simulation. Those operations now have separate local Rust implementation evidence in [Stage 06](../../plan/implementation/06-backend-api.md); the Python SDK backtest flow, pricing acceptance suite, and persistence beyond process-local metadata/local evidence remain outside this Phase A guide. The Python SDK has no runtime third-party dependencies. JSON Schema validation is a test-only dependency from `python-sdk/requirements-test.txt`.

## Python SDK and contract examples

Create a temporary virtual environment and run the suite from the repository root in PowerShell:

```powershell
$testEnv = Join-Path $env:TEMP 'factorsbt-phase-a-test'
python -m venv $testEnv
& (Join-Path $testEnv 'Scripts\python.exe') -m pip install -r python-sdk/requirements-test.txt
& (Join-Path $testEnv 'Scripts\python.exe') -m unittest discover -s python-sdk/tests -v
```

The suite checks decimal and UTC wire primitives, `Client.health()` transport behavior, Draft 2020-12 schemas, and shared examples. Schema-invalid examples must fail validation; examples with semantic errors must pass schema validation. The Python suite passed: 8 tests.

## Rust workspace

The verified host uses Visual Studio 2022 Build Tools 17.14.44, MSVC 14.44.35207, Windows SDK 10.0.26100.0, and Rust/Cargo 1.99.1 for `x86_64-pc-windows-msvc`. Run from an x64 Developer Command Prompt with the repository root as the working directory:

```powershell
cargo fmt --manifest-path rust/Cargo.toml --all -- --check
cargo metadata --offline --manifest-path rust/Cargo.toml --no-deps --format-version 1
cargo clippy --offline --manifest-path rust/Cargo.toml --workspace --all-targets
cargo check --offline --manifest-path rust/Cargo.toml --workspace --all-targets
cargo test --offline --manifest-path rust/Cargo.toml --workspace --all-targets
```

All commands passed. Rust tests reported 11 unit tests and 3 shared-example tests passed. Clippy completed without warnings.

## Python-to-Rust smoke

Run from the same x64 Developer Command Prompt:

```powershell
python python-sdk/tests/smoke_health.py
```

The script builds into a temporary Cargo target directory, selects a free loopback port, calls the backend through `Client.health()`, and terminates the process in a `finally` block. The smoke passed. The restricted Codex shell returned Windows `WinError 10013` for loopback sockets; the verified smoke ran in a loopback-enabled command context.

## Verification evidence

- `cargo fmt`, `cargo clippy`, `cargo check`, `cargo test`, and offline metadata validation passed.
- Python unittest and Draft 2020-12 validation passed in a temporary virtual environment.
- Python SDK reached the Rust backend at `GET /healthz`; process cleanup completed.
- The repository has no CI workflow. No engine/backtest behavior was added by Phase A.
