# MVP 1 HTTP API

**Design status:** Accepted MVP 1 contract design.

**Implementation status:** The Rust backend implements the health operation and all four run operations for local loopback use. Stage 06 verification passed against controlled fixtures. This does not establish public API stability or hosted-service support.

This is the loopback HTTP/JSON surface for the Python SDK. The Rust backend coordinates runs; the engine owns simulation state and financial outcomes. `GET /healthz` was verified in Phase A. The four run operations and typed Python backtest flow were implemented and exercised through the Rust service in Phase B Stages 06–07 using controlled fixtures. See the [local verification record](../development/phase-b-local.md).

## Operations

| Method and path | Purpose | Success |
|---|---|---|
| `GET /healthz` | Verify that the local backend accepts requests after fixture, evidence path, and worker initialization. It does not indicate that a submitted run has completed. | `200 OK` with service and contract version. |
| `POST /v1/runs` | Validate and enqueue one run configuration. The caller supplies an `Idempotency-Key` UUID. | `202 Accepted` with submission ID, run ID, and `queued` status. |
| `GET /v1/submissions/{submission_id}` | Resolve an acknowledged or uncertain submission to its run and current status. | `200 OK` with submission and run identity. |
| `GET /v1/runs/{run_id}` | Inspect run lifecycle, progress context, and terminal failure when present. | `200 OK` with current run status. |
| `GET /v1/runs/{run_id}/results` | Retrieve a successful run's summary and evidence references. | `200 OK` after the run reaches `succeeded`. |

The submission body is the versioned `RunConfiguration` contract assembled in the schemas and OpenAPI specification. Its current shape is described in [MVP run and instrument configuration](instrument-configuration.md). Malformed JSON is rejected before enqueue. Schema-invalid or semantically invalid configuration is rejected without creating a run. Semantic checks include resolving instrument references, verifying output constraints, and checking portfolio feasibility where that check belongs to run admission.

### Health response

```json
{
  "status": "ok",
  "service": "factorsbt-backend",
  "contract_version": "1.0.0"
}
```

This response is intentionally small and has no dependency on run persistence. The process loads the local fixture registry, creates the evidence directory, and starts the worker pool before it begins serving. A successful health check does not indicate that a submitted run has completed.

### Submission acknowledgment

```json
{
  "submission_id": "8a2f5ce2-a6b5-4a71-b587-fefdb4a9fbc5",
  "run_id": "c827fcbc-c5fc-48a4-b4aa-21a63f64727e",
  "status": "queued",
  "submitted_at": "2026-01-05T09:00:00Z"
}
```

`submission_id` is the caller-provided idempotency UUID echoed as the lookup key. `run_id` is a backend-generated UUID. The timestamp is backend processing time, in UTC; it is not simulation time.

### Run status

```json
{
  "run_id": "c827fcbc-c5fc-48a4-b4aa-21a63f64727e",
  "status": "running",
  "submitted_at": "2026-01-05T09:00:00Z",
  "started_at": "2026-01-05T09:00:01Z",
  "finished_at": null,
  "progress": {
    "scenario_id": "baseline",
    "step_id": "step-0001",
    "simulation_time": "2026-01-05T16:00:00Z",
    "completed_steps": 1,
    "total_steps": 5
  },
  "failure": null
}
```

The lifecycle is `queued → running → succeeded | failed`. `started_at` and `finished_at` are nullable until set. `progress` is nullable before a step begins. A failed run includes a structured failure object and the identity of its last valid simulation state when available. This is an operational status view; simulation times inside progress remain distinct from processing timestamps.

### Successful result response

`GET /v1/runs/{run_id}/results` returns a `RunResult` document containing the run summary, the configured single output mode, final portfolio state, and evidence references for snapshots, model responses, decisions, orders, fills, settlements, and risk events. The result and evidence shape is specified in [MVP results and evidence references](results-evidence.md). The endpoint does not return fabricated fill data or silently replace unavailable evidence.

## Client behavior and storage boundary

- Submission is asynchronous. A synchronous Python SDK may submit once, then poll the submission or run resource until a terminal state or its local wait deadline.
- There is no cancellation or streaming endpoint in MVP 1. A client wait deadline does not cancel backend work.
- Run metadata and the submission deduplication map are in memory in the backend process. Evidence artifacts are written under a local run directory. Database storage, process-restart recovery, resumption, remote deployment, and cross-process deduplication are outside this contract.
- The first run is local and loopback-only. No authentication mechanism is selected by this API specification.
- The service accepts only loopback bind addresses. `FACTORSBT_BIND_ADDR`, `FACTORSBT_MAX_ACTIVE_RUNS`, `FACTORSBT_MAX_QUEUED_RUNS`, `FACTORSBT_FIXTURE_DIR`, and `FACTORSBT_EVIDENCE_DIR` configure the local process. Defaults are `127.0.0.1:8080`, two active runs, sixteen queued runs, the checked-in fixture directory, and `.factorsbt/evidence`.
- Queue overflow and shutdown rejection return `503 BACKEND_UNAVAILABLE` without creating a new submission. Repeating a recorded key with the same configuration returns the original acknowledgment even when the queue is full.

Error bodies, submission retry rules, and error codes are specified in [submission identity and failures](mvp-submissions.md). The shared schemas, OpenAPI document, and compatibility rules are collected in [contract compatibility](mvp-contract-compatibility.md).

## Status and implementation boundary

These operation definitions remain the Accepted contract for MVP 1. The local Rust health and run operations and Python backtest SDK are Implemented with controlled service checks, but they are not declared stable. Verification does not establish hosted-service behavior or external-provider support.
