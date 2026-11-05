# Phase E local verification

**Status:** Verified locally on Windows with Python 3.13.13 and Rust/Cargo 1.99.1 on 2026-11-05. The `factorsbt` command is installed from `python-sdk` and exercises local run and recorded replay operations through the existing Python SDK. This does not claim a public release or PyPI distribution.

## Package and Python checks

The SDK is packaged with setuptools in src layout as `factorsbt` 0.1.0, requires Python 3.13 or later, has no runtime dependencies, and exposes `factorsbt = factorsbt.cli:main`. Test-only `jsonschema==4.26.0` was installed in an isolated virtual environment.

The complete Python test suite passed **68 tests**:

~~~powershell
$testEnv = Join-Path $env:TEMP 'factorsbt-phase-e-venv'
python -m venv $testEnv
& (Join-Path $testEnv 'Scripts\python.exe') -m pip install --disable-pip-version-check -r python-sdk/requirements-test.txt
$env:PYTHONPATH = (Join-Path (Get-Location) 'python-sdk/src')
$testPython = Join-Path $testEnv 'Scripts\python.exe'
& $testPython -B -m unittest discover -s python-sdk/tests -p 'test_*.py' -v
~~~

The CLI acceptance smoke was run with `& $testPython -B python-sdk/tests/smoke_cli.py`. The script created another clean virtual environment, installed the package non-editably with `python -m pip install ./python-sdk`, and invoked the installed `factorsbt.exe` from outside the repository with `PYTHONPATH` empty. It verified health, run submit, submission lookup, wait, and evidence-backed result retrieval. It then shut down the model fixture and verified recorded replay returned `matched: true` without fresh inference. The smoke used submission `00000000-0000-4000-8000-000000000101` and replay `00000000-0000-4000-8000-000000000102`.

## Rust and regression checks

The following commands passed from `rust/` in an x64 Visual Studio Developer environment, with Cargo offline and a temporary target directory:

~~~powershell
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'factorsbt-phase-e-target'
cargo fmt --all -- --check
cargo check --workspace --all-targets --offline
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo test --workspace --offline
~~~

The workspace passed 154 tests: backend 63, contracts 19, shared examples 3, data 16, engine 52, and controlled engine acceptance 1. Integration tests that bind HTTP loopback were run with the required local network permission.

Both established backend smoke scripts passed:

~~~powershell
& $testPython -B python-sdk/tests/smoke_backtest.py
& $testPython -B python-sdk/tests/smoke_sdk_failures.py
~~~

The backtest smoke passed all three output modes, comparison, and recorded replay after backend restart with the model fixture offline. Its replay submission uses a bounded 10 second timeout because Windows source-evidence validation can exceed the former 3 second smoke-only timeout. The failure smoke passed uncertain acknowledgment lookup, idempotency conflict, model failure replay, and non-cancelling wait timeout.

## Limits

The CLI supports local HTTP loopback backends only and does not start the backend or model fixture. It covers backtest and Phase C replay; comparison, artifact downloads, research jobs, interactive console, authentication, and PyPI distribution remain outside this phase. The repository has no CI workflow, so no CI status is available.
