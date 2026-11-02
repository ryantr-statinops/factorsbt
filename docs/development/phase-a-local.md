# Phase A local development

**Status:** Working instructions. Rust backend execution remains blocked on the current Windows host.

## Scope

The executable slice in Phase A is limited to `GET /healthz` and the synchronous Python `Client.health()` call. Run submission, simulation, pricing, persistence, and the other API paths remain specifications. The Python SDK has no runtime third-party dependencies. JSON Schema validation is a test-only dependency in `python-sdk/requirements-test.txt`.

## Python SDK and contract examples

From the repository root in PowerShell:

```powershell
python -m venv .venv
& .\.venv\Scripts\python.exe -m pip install -r python-sdk/requirements-test.txt
& .\.venv\Scripts\python.exe -m unittest discover -s python-sdk/tests -v
```

The unit suite checks decimal and UTC wire primitives, `Client.health()` transport behavior, Draft 2020-12 schemas, and shared contract examples. The schema tests distinguish schema-invalid examples from examples that are schema-valid but semantically invalid. The Rust shared-example tests are in the contracts crate and require a buildable Rust host.

## Rust workspace checks

From the repository root:

```powershell
cargo fmt --manifest-path rust/Cargo.toml --all -- --check
cargo metadata --offline --manifest-path rust/Cargo.toml --no-deps --format-version 1
cargo check --offline --manifest-path rust/Cargo.toml --workspace --all-targets
```

On the implementation host, formatting and Cargo metadata checks succeeded. `cargo check` was attempted after dependencies were fetched and failed before project crates compiled because the host lacks `link.exe`. A compatible MSVC C++ linker/toolset and Windows SDK are prerequisites for compile, test, backend launch, and the integration smoke check. Do not treat `cargo metadata` as a code compile.

After the Rust prerequisites are available, run:

```powershell
cargo test --offline --manifest-path rust/Cargo.toml --workspace --all-targets
python python-sdk/tests/smoke_health.py
```

The smoke script builds into a temporary Cargo target directory, selects an available loopback port, calls the Rust service through `Client.health()`, and terminates the service in a `finally` block. It was attempted on the current host and stopped at the same missing-linker error before the service could start.

## Environment evidence

- Python 3.13.13, Rust/Cargo 1.99.1, target `x86_64-pc-windows-msvc`.
- `cargo fmt` and offline Cargo metadata validation succeeded.
- Python unittest suite, including JSON Schema validation, passed in an isolated virtual environment.
- Rust compile/test and Python-to-Rust smoke are unverified because MSVC linker/Windows SDK are absent.
- No CI workflow or engine/backtest behavior was added by Phase A foundation work.
