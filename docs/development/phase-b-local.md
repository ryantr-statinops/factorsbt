# Phase B local verification

**Status:** Verified on Windows with MSVC and Python 3.13. This guide records the local commands and controlled results used for Phase B closeout. It does not establish a public API stability guarantee.

## Prerequisites

- Visual Studio 2022 Build Tools 17.14.44, MSVC 14.44.35207, and Windows SDK 10.0.26100.0 for x86_64-pc-windows-msvc.
- Rust and Cargo 1.99.1 on PATH.
- Python 3.13 and the standard library.
- Loopback access for the local Rust backend and the Python HTTP model fixture.

The Rust backend and model service bind only to loopback for this MVP. The smoke scripts build the backend offline, use temporary evidence directories, and stop child processes when complete. The SDK has no runtime third-party dependency; jsonschema is test-only.

## Rust workspace

Open an x64 Visual Studio Developer Command Prompt, change to rust, set a temporary target directory, then run:

~~~powershell
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'factorsbt-phase-b-target'
cargo fmt --all -- --check
cargo check --workspace --all-targets --offline
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo test --workspace --offline
~~~

All four commands passed. The workspace run reported 140 tests across the backend (50), contracts (18), shared Rust examples (3), data (16), engine (52), and controlled engine acceptance (1). Formatting and Clippy completed without findings.

## Python SDK and shared schemas

From the repository root in PowerShell, create a temporary environment, install the declared test dependency, and run the complete suite:

~~~powershell
$testEnv = Join-Path $env:TEMP 'factorsbt-phase-b-python'
python -m venv $testEnv
& (Join-Path $testEnv 'Scripts\python.exe') -m pip install --disable-pip-version-check -r python-sdk/requirements-test.txt
& (Join-Path $testEnv 'Scripts\python.exe') -m unittest discover -s python-sdk/tests -v
~~~

The suite passed 40 tests. It covers SDK primitives and builders, HTTP submission/lookup/wait behavior, Draft 2020-12 schemas, all checked-in shared examples, and versioned fixture digests. The package is used directly from python-sdk/src; no wheel or published distribution is claimed.

## Rust backend and typed SDK integration

Run these from the same x64 Developer Command Prompt at the repository root. Reuse the temporary Cargo target selected above:

~~~powershell
$testPython = Join-Path (Join-Path $env:TEMP 'factorsbt-phase-b-python') 'Scripts\python.exe'
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'factorsbt-phase-b-target'
& $testPython python-sdk/tests/smoke_backtest.py
& $testPython python-sdk/tests/smoke_sdk_failures.py
~~~

The success smoke started the Rust backend and a controlled HTTP model fixture, then submitted one typed run for each of scores, targets, and order_intents. Each run succeeded with fills across the seven-instrument catalog. The script independently recomputed position market values and NAV using Decimal, then read every evidence file and compared its byte size and SHA-256 digest with the returned reference.

The failure smoke dropped one POST acknowledgment after forwarding the request to the backend, resolved that submission by its original key, checked that a changed body under that key returns HTTP 409 with SUBMISSION_ID_REUSED, and verified that a model HTTP 503 fails the run. A short local wait timeout also expired while the backend continued processing; the run later succeeded.

Both scripts call Client.health() before submitting work. The backend reports only the implemented local loopback operations. Metadata and submission identity remain process-local, evidence remains local, and the API has no public stability guarantee.

## Repository automation

The repository has no CI workflow. These checks were run locally; no CI status is available.
