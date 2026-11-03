# ADR 0002: Phase A local tooling and module layout

**Status:** Accepted implementation direction; Phase A foundation verified on the local Windows host.

## Context

Stage 00 and Stage 01 selected a synthetic MVP, shared JSON Schema/OpenAPI contracts, a pure-Python SDK, and a Rust backend/engine. Stage 02 needs a small local foundation for the Phase A health endpoint and cross-language wire types. It must not introduce simulation behavior, a queue, a database, CI, or third-party Python runtime dependencies.

The environment preflight ran before source-code changes. It found Python 3.13.13, Rust/Cargo 1.99.1, and the `x86_64-pc-windows-msvc` Rust target. It did not find the MSVC C++ linker/toolset or Windows SDK needed to link and run a Windows backend executable.

## Decision

- Use a Cargo workspace at `rust/` with `contracts`, `engine`, and `backend` crates. Keep wire types in `contracts`; the engine owns simulation types and rules; the backend owns HTTP and run coordination. The Stage 02 engine crate contains no backtest implementation.
- Build the local HTTP health server with Axum and Tokio, using Serde for JSON. Phase A implements `GET /healthz` only. The other API paths remain the accepted contract specification.
- Keep the Python source package under `python-sdk/src/factorsbt/` with standard-library `urllib.request`, `dataclasses`, `decimal`, `json`, and `unittest`. It communicates with the Rust backend over HTTP/JSON and does not embed or link the Rust engine.
- Keep contract JSON Schemas, OpenAPI, and shared examples at their existing repository paths. Rust and Python consume the same field names and decimal-string convention. Exact dependency versions must be recorded from the actual manifests/resolution; this ADR does not claim a supported version range.
- Use local in-memory run/submission metadata and a configured local evidence directory. Do not add a database, queue, authentication layer, remote deployment, CI, or restart recovery in Phase A.
- Use Rust's built-in Cargo formatting/check/test tooling and Python's built-in `unittest` only after manifests exist. Record a command as verified only after it has run successfully in an identified environment.

## Environment preflight

| Check | Observed result |
|---|---|
| Python | 3.13.13 |
| Rust/Cargo | 1.99.1; host and installed target are `x86_64-pc-windows-msvc` |
| Visual Studio C++ toolchain | No matching installation with the MSVC x64/x86 build tools was found by `vswhere`; `cl.exe`, `link.exe`, and `msbuild.exe` are not available. |
| Windows SDK | Windows Kits library directories were not found. |
| Python JSON Schema validator | No `jsonschema` module was present in the base environment. For the test-only shared-example check, `jsonschema==4.26.0` was installed in an isolated temporary virtualenv; all Python schema/example tests passed. The SDK runtime remains standard-library-only. |

The preflight findings above describe the environment before toolchain provisioning. The prerequisite was subsequently resolved; see the verification record below.

## Follow-up verification 2026-11-03

| Check | Verified result |
|---|---|
| Visual Studio Build Tools | Visual Studio 2022 Build Tools 17.14.44; MSVC x64/x86 toolset 14.44.35207. `VsDevCmd.bat` exposes `cl.exe` and `link.exe`. |
| Windows SDK | Windows SDK 10.0.26100.0; `rc.exe`, `kernel32.lib`, and `ucrt.lib` are available. |
| Rust | Rust/Cargo 1.99.1, target `x86_64-pc-windows-msvc`; `cargo fmt`, `cargo clippy`, `cargo check`, and `cargo test` passed offline. Clippy is warning-free. |
| Rust tests | 11 contract crate unit tests and 3 shared-example tests passed; backend and engine crates have no unit tests yet. |
| Python/contracts | 8 Python tests passed, including Draft 2020-12 schemas and shared examples with test-only `jsonschema==4.26.0`. |
| Integration | `python-sdk/tests/smoke_health.py` built and launched the backend, reached `GET /healthz` through the Python SDK, then cleaned up the process. |
| CI | No CI workflow exists; CI was not run. |

## Alternatives considered

- Native Python bindings or an embedded Rust engine would add packaging and native-linking requirements to the SDK; they are outside the selected HTTP boundary.
- A database or queue would add operational state not needed for the local health slice and is not selected.
- A third-party Python runtime dependency would contradict the accepted SDK boundary. The Draft 2020-12 validator is limited to test setup and is not imported by the SDK runtime.

## Consequences

The first executable path stays small and language boundaries remain explicit. Cargo.lock records the resolved Rust dependencies. The health-only backend/SDK path, contracts, Rust tests, Python tests, formatting, and Clippy were verified. This does not establish backtest execution, simulation correctness, or run API behavior. No CI workflow exists.

## Related documents

- [ADR 0001: Python SDK and Rust backend/engine](0001-python-sdk-rust-backend.md)
- [Phase A project foundation plan](../../plan/implementation/02-project-foundation.md)
- [MVP contract compatibility](../interfaces/mvp-contract-compatibility.md)
- [Development workflow](../development/workflow.md)
