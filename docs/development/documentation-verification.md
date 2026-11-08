# Documentation refresh verification

**Status:** Local verification record for the documentation completeness update on 2026-11-08. This update changes Markdown only; it introduces no runtime, wire schema, package dependency, license grant or model-fixture launcher.

## Environment and boundaries

Verification used Windows x64, Python 3.13.13, Rust/Cargo 1.99.1, Visual Studio 2022 Developer PowerShell 17.14.44 with x64 MSVC and the Windows SDK. These observations do not define a supported-platform/version matrix. The Python package was installed into a new temporary virtual environment; `jsonschema==4.26.0` was installed as a test dependency. Cargo artifacts and smoke evidence used temporary directories.

The final smoke build used `CARGO_PROFILE_DEV_DEBUG=0`, with no runtime source or timeout changes. A rebuilt backend has a new executable digest; each verification source/replay pair used the same binary. Historical Phase A–E records remain evidence of their own runs, not results rerun wholesale by this update.

## Executed checks

From the repository root in x64 Developer PowerShell, using the environment prepared in the [quickstart](quickstart.md):

```powershell
$env:PYTHONPATH = Join-Path (Get-Location) 'python-sdk/src'
& $demoPython -B -m unittest discover -s python-sdk/tests -v
& $demoPython -B python-sdk/tests/smoke_backtest.py
& $demoPython -B python-sdk/tests/smoke_sdk_failures.py
& $demoPython -B python-sdk/tests/smoke_cli.py
cargo test --locked --offline --manifest-path rust/Cargo.toml -p factorsbt-data
```

| Check | Observed result and limit |
| --- | --- |
| Python unit/contracts suite | 68 tests passed. Includes SDK/CLI transport, schemas, shared examples and fixture digests; not a full engine acceptance run. |
| Backtest smoke | All three modes completed with 32 artifact references per run and matched recorded replay. Sequential comparison reported 28 shared timeline points with zero unmatched points for baseline/price-only. Replay also matched after backend restart with the model fixture offline. |
| Failure smoke | Passed uncertain acknowledgment lookup, idempotency conflict, model-failure replay and non-cancelling wait timeout. |
| Installed CLI smoke | Passed package installation in a separate clean environment, health, submit/lookup/wait/result and matched replay after stopping the model fixture. Its command ran outside the repository with PYTHONPATH empty. |
| Research example | The unchanged [research_workflows.py](../../python-sdk/examples/research_workflows.py) ran against a temporary loopback backend: reranking outcome `equal`, episode rewards `-1.02` and `1499.98` USD, replay matched. No model/provider service was required. |
| Rust data suite | 16 existing data-crate tests passed; doc tests reported zero cases. This is targeted fixture/snapshot verification, not a full workspace regression. |
| SDK documentation snippets | The configuration block executed offline and exactly matched the shared minimal spot example; all documented Python blocks compiled. Standalone network snippets still require their stated services. |

The research example was invoked with `FACTORSBT_BASE_URL` pointing at a separately started built backend, absolute fixture/evidence paths and an isolated temporary evidence root. Verification checked health before invocation and terminated the owned process afterward. The operations guide's `cargo run` command is a manifest-based launch recipe; the automated checks launched the built executable. No full Rust workspace regression was run for these Markdown changes; the fixture guide additionally targets the existing data-crate suite.

## Failures encountered and interpretation

- The first source-checkout unit run omitted PYTHONPATH and could not import CLI tests correctly. After explicitly setting the source path, all 68 tests passed; no test code was changed.
- With default dev debug symbols, installed CLI smoke failed at submission lookup on two attempts with the SDK's 5-second request timeout. The research example also encountered a polling timeout. A diagnostic run with four Tokio workers completed reranking/episode but timed out on research replay submission.
- The installed CLI smoke and unchanged research example subsequently passed on the build with dev debug symbols disabled; the successful research run used default Tokio worker selection and default SDK timeouts. This limited observation does not prove the timeout's root cause or guarantee that every host meets the default deadline. The [quickstart](quickstart.md) records the selected build configuration and the [operations guide](local-operations.md) explains uncertain acknowledgment recovery.

Timeout failures are not financial outcomes. No automatic retry, fallback decision, SDK timeout increase or hidden runtime fix was added to obtain these results.

## Documentation and Git acceptance

The review checked relative file targets, heading anchors and balanced fenced blocks across 62 repository Markdown files; reconciled status vocabulary and ownership links; and compared documented operations/SDK methods with OpenAPI and source. The [acceptance table](../examples/mvp/acceptance-cases.md#verification-traceability) maps all 13 case IDs to concrete tests or explicitly limited related coverage; each referenced Rust test function was found in its linked source. It is a source inspection, not a claim that every exact case ran end to end during this refresh.

Each commit stages one explicit Markdown path, reviews the staged diff, runs `git diff --cached --check` and verifies the committed file list. The pre-existing edit in `plan/implementation/README.md` remains outside the commit series and is checked for unchanged bytes. One additional quickstart-only correction records the build configuration discovered during verification; no history is amended or squashed.

Phases are pushed to `codex/docs-completeness` only after their checks. No pull request or merge is included. No GitHub Actions workflow is declared in the repository tree; attempts to inspect the remote Actions API returned HTTP 504, so remote CI status could not be verified. Local checks are not represented as CI results.

## Remaining decisions

License selection, real providers, hosted deployment, persistent metadata/recovery, retention, general RL/training and public API stability remain outside this update. Standalone model-fixture launching is still absent. Preserve the explicit case-coverage gaps rather than interpreting documentation completeness as complete simulation verification.
