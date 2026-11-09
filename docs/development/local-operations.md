# Local backend operations

**Status:** Implemented loopback service with process-local metadata and filesystem evidence. These are local developer/research operations, not a hosted deployment runbook.

## Start and configure

Use x64 Developer PowerShell with the prerequisites in the [quickstart](quickstart.md). From the repository root, start a backend in a dedicated terminal:

```powershell
$env:FACTORSBT_BIND_ADDR = '127.0.0.1:8080'
$env:FACTORSBT_FIXTURE_DIR = Join-Path (Get-Location) 'contracts/fixtures/data'
$env:FACTORSBT_EVIDENCE_DIR = Join-Path $env:TEMP 'factorsbt-local-evidence'
cargo run --locked --offline --manifest-path rust/Cargo.toml -p factorsbt-backend --bin factorsbt-backend
```

Set `CARGO_TARGET_DIR` consistently if reusing a built binary for replay. A successful health response means fixtures, evidence path and workers initialized, not that a model is reachable or a job completed. The backend does not launch a model service. Run submissions require a compatible stateless HTTP service at the configured endpoint; the package has no standalone model-fixture command. The self-contained smoke scripts own their fixture processes.

| Setting | Default and effect |
| --- | --- |
| `FACTORSBT_BIND_ADDR` | `127.0.0.1:8080`; numeric loopback socket address only. Nonloopback binding is rejected. |
| `FACTORSBT_MAX_ACTIVE_RUNS` | `2`; positive worker capacity shared by fresh runs, replays and research jobs. |
| `FACTORSBT_MAX_QUEUED_RUNS` | `16`; nonnegative waiting capacity; `0` disables waiting, not active work. |
| `FACTORSBT_FIXTURE_DIR` | Checked-in data fixture directory derived from the backend crate's compile-time manifest directory. Set an absolute path when relocating the binary. |
| `FACTORSBT_EVIDENCE_DIR` | `.factorsbt/evidence` relative to process working directory. Prefer an explicit absolute directory. |
| `FACTORSBT_BASE_URL` | CLI/example client setting, not backend binding. CLI precedence is flag, environment, then `http://127.0.0.1:8080`. |

Queue-full/shutdown admission returns `503 BACKEND_UNAVAILABLE` without new work. Existing matching idempotency keys can return their original acknowledgment even if the queue is full. Metadata and idempotency maps are confined to one backend process; do not start multiple processes on a shared evidence root expecting coordinated ownership or deduplication.

## Evidence and restart

Fresh run evidence is under `<root>/runs/<run-id>/evidence/`. Replay/research evidence uses separate job directories. Keep the complete sealed manifest and referenced bytes together. Reading the run evidence index verifies every referenced artifact; downloading a single artifact validates its manifest references and checks the requested file's containment, size and SHA-256 at read time. Replay admission verifies the full source bundle, and the worker rechecks each artifact when it reads it. These checks detect accidental or post-seal changes; they are not authentication or a retention policy. Never edit sealed source files to repair a replay incompatibility.

| Operation after restart | Behavior |
| --- | --- |
| Run/submission/replay/research status lookup | Prior process metadata is not restored. Save IDs and configuration before stopping. |
| Sealed run evidence/artifact reads | Can resolve source disk evidence under the same configured root. |
| New replay from a sealed source | Validates source integrity, evidence/contract format and exact backend executable digest before enqueue. |
| Resume queued/running work | Unsupported. Evidence on disk is not job recovery. |

Use Ctrl+C for backend shutdown. The service stops accepting new work and drains accepted work before joining workers; this is not a per-job cancellation API or a durable checkpoint. Wait for process exit before restart. An abrupt termination can leave incomplete evidence that cannot be replayed. Do not remove evidence while jobs are active. Retention/cleanup automation is not implemented; any manual archival must preserve sealed source contents and the compatible binary if replay is needed.

## Diagnose common failures

| Symptom | Check or next action |
| --- | --- |
| Windows linker/SDK missing | Use x64 Developer PowerShell with C++ Build Tools and Windows SDK, not an ordinary shell without toolchain variables. |
| Offline dependency error | Run the quickstart's locked fetch with network access before offline build. |
| Cannot bind/listen, or loopback socket denied | Check chosen port, existing listener, local policy and socket permission; do not bind to a public address as a workaround. |
| Model timeout/HTTP/schema/correlation error | Inspect run failure and available sealed evidence; validate endpoint, selected output mode and echoed request identities. No inference retry/fallback is performed. |
| Lost submit acknowledgment | Use the original submission/replay/job lookup ID; never infer rejection from a timeout. |
| Client wait expired | Inspect the retained ID later; local wait timeout did not cancel backend work. |
| Evidence integrity error | Preserve the files for inspection and compare references/hashes; do not fabricate missing bytes. |
| Replay source incompatible | Use the original executable and contract with complete sealed inputs. A rebuilt binary may have a different digest even if package/wire versions match. |
| Old ID returns not found after restart | Process-local metadata is gone; use sealed-source evidence/replay where supported, not automatic resubmission under a new key. |

Keep backend `request_id` and job IDs when diagnosing failures. Error payloads/logs must not expose private data, credentials or unrestricted local paths. For API error meanings see [submission failures](../interfaces/mvp-submissions.md), [run evidence](../interfaces/results-evidence.md), [replay](../interfaces/recorded-replay.md) and [research jobs](../interfaces/research-jobs.md).
