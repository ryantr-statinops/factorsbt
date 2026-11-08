# Local quickstart

**Status:** Implemented local synthetic workflow. This guide uses the existing smoke programs to start and stop their own backend/model fixtures. Verification of these instructions is recorded separately; earlier environment evidence is in [Phase E](phase-e-local.md).

## Environment

Use a checkout of the repository and run the commands below from its root in **Developer PowerShell for Visual Studio**, configured for x64 host and target. Install Visual Studio C++ Build Tools and the Windows SDK, Rust/Cargo, and Python first. The recorded host uses Python 3.13.13, Rust/Cargo 1.99.1, Visual Studio Build Tools 17.14.44, MSVC 14.44.35207 and Windows SDK 10.0.26100.0. These are observed versions, not a supported-platform matrix. The Python package declares Python >=3.13; other operating systems/toolchains have not been verified by these records.

The demo needs HTTP loopback sockets. Initial Python build/test dependencies and Cargo dependencies may require network access. Cargo smoke builds run offline, so prepare their cache before starting. No provider account or credentials are required.

## Install and prepare dependencies

From the repository root in that PowerShell session:

```powershell
$demoEnv = Join-Path $env:TEMP 'factorsbt-docs-completeness-venv'
python -m venv $demoEnv
$demoPython = Join-Path $demoEnv 'Scripts\python.exe'
& $demoPython -m pip install --disable-pip-version-check ./python-sdk
& $demoPython -m pip install --disable-pip-version-check -r python-sdk/requirements-test.txt
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'factorsbt-docs-completeness-target'
$env:CARGO_PROFILE_DEV_DEBUG = '0'
cargo fetch --locked --manifest-path rust/Cargo.toml
```

Use a new environment directory if that name already contains unrelated work. Package installation exposes `factorsbt.exe` in the environment's `Scripts` directory and needs no runtime dependency beyond the standard library. `jsonschema` is test-only. A first offline build without the dependency cache may fail; `--offline` does not install missing dependencies from the network. Retain Cargo.lock and do not run `cargo update` as setup.

The smoke build above disables dev debug symbols through a temporary Cargo environment override; it does not edit a build manifest or SDK timeout. Default-symbol local runs encountered 5-second HTTP timeouts during this documentation review; the installed CLI smoke subsequently passed with this override. This observation is not proof of a single timeout cause or a latency guarantee. Rebuilding changes the executable digest: create fresh source runs and keep the same binary for their replay.

## Run the self-contained demo

```powershell
& $demoPython -B python-sdk/tests/smoke_backtest.py
& $demoPython -B python-sdk/tests/smoke_sdk_failures.py
& $demoPython -B python-sdk/tests/smoke_cli.py
```

Check each command's exit status (`$LASTEXITCODE` should be `0`) before continuing. The scripts build the Rust backend, use free loopback ports and synthetic fixtures, and stop child servers in cleanup. They create temporary evidence and reports; those are not durable user research outputs.

The backtest smoke covers scores/targets/order-intents runs, independent NAV/evidence checks, sequential comparison and recorded replay after backend restart and after stopping the model fixture. The failure smoke covers uncertain submission acknowledgment, idempotency conflict, model failure replay and a wait timeout that does not cancel backend work. The CLI smoke installs a separate clean package and invokes its command from outside the repository with PYTHONPATH empty.

This is the runnable first demo. There is no standalone model-fixture launcher in the package. The [CLI](cli.md) and [SDK](python-sdk.md) guides describe use against an already running backend and a compatible model service; they do not start those services. Do not treat helper classes inside the smoke tests as a public API.

## Run unit and contract checks

```powershell
$env:PYTHONPATH = Join-Path (Get-Location) 'python-sdk/src'
& $demoPython -B -m unittest discover -s python-sdk/tests -v
```

Set PYTHONPATH explicitly when testing a source checkout; do not rely on test import order. The suite validates shared contracts/fixtures and SDK/CLI behavior. It does not by itself run the Rust engine or prove every acceptance scenario end to end.

For retained evidence, backend configuration, restart limits and troubleshooting, read [local operations](local-operations.md). For independent experiments, read [comparison](comparison.md); bounded research has an existing [runnable example](../../python-sdk/examples/research_workflows.py) and [interface guide](../interfaces/research-jobs.md).
