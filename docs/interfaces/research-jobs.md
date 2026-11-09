# Local research job interface

**Status:** Implemented for the controlled Phase D reranking and fixed-action episode flows.

This local interface runs two bounded research jobs through the existing Rust backend and engine. It is separate from `/v1/runs` and the Phase C `/v1/replays` resource. Contract version is `1.0.0`; the executable OpenAPI description and JSON Schemas are in [`contracts/openapi.json`](../../contracts/openapi.json) and [`contracts/schemas`](../../contracts/schemas).

## Operations

| Operation | Purpose |
| --- | --- |
| `POST /v1/research/jobs` | Submit reranking, episode, or recorded research replay. `Idempotency-Key` is a lowercase UUID and is also the job ID. |
| `GET /v1/research/jobs/{job_id}` | Read `queued`, `running`, `succeeded`, or `failed` lifecycle and failure details. |
| `GET /v1/research/jobs/{job_id}/result` | Read a terminal result when the worker produced and sealed a report. A running job returns `409`; an unknown job returns `404`. |

The backend validates the complete request and registered fixture reference before queue admission. Equal key and payload return the original acknowledgement; an equal key with different payload returns `409`; a full shared FIFO queue returns `503` without creating a record. Backend metadata is process-local. Research evidence is written under a job-specific local directory and is not overwritten by replay.

## Job types

- **Reranking:** `reranking-dev@1.0.0` and `reranking-eval@1.0.0` use the same three point-in-time candidates for two isolated portfolios. Baseline ranks raw score; reranker subtracts the fixed 0.25 turnover penalty. The report includes branch NAV/P&L, total fill value, candidate selections, requested and admitted target quantities, common admission scale, fills, P&L delta, and whether the reranker ended higher, lower, or equal to baseline. A branch error does not discard the other branch.
- **Episode:** `episode-dev@1.0.0` and `episode-eval@1.0.0` accept exactly two pre-supplied target weights from `0`, `0.5`, and `1`. Each action passes through engine admission and next-open execution. The result carries close observations, actions, admitted quantities, fills and completed-step rewards. A failure after a completed step preserves that trace and last valid observation, marks the job failed, and exposes the sealed report.
- **Replay:** uses the source job's sealed request and fixture, checks artifact hashes, contract and executable digest, and recomputes its report without model or provider calls. A matching source episode failure is a successful verification with `source_outcome: failed`; divergence returns a failed replay with the first report mismatch. Replay evidence is stored separately from source evidence.

## Ranking and reward interpretation

Reranking uses the same point-in-time cash, 50% SPOT_A, and 50% SPOT_B candidates for two independent branches. Baseline selects by raw score; reranking selects by `raw_score - 0.25 * turnover`, where `turnover = abs(target_A - current_weight_A) + abs(target_B - current_weight_B)`. Current weights come from each branch's own close holdings/NAV; targets still pass grouped engine admission and next-open execution. The coefficient is fixed synthetic policy, not a calibrated financial objective. Candidate availability is checked before selection.

Episode observations contain simulation time, mark, cash, quantity and NAV. Each reward is `observation_end.nav - observation_start.nav`, in USD after the completed transition's fees. It is not a percentage return, annualized score or training objective. Actions are target portfolio weights, not orders or guaranteed fills. Three close points provide exactly two transitions; the last close does not force a flattening trade. An incomplete failed step has no fabricated reward.

The `*-dev` and `*-eval` fixture IDs select separate registered synthetic datasets at version `1.0.0`. Episode evaluation includes fees; development uses zero fees. These fixed datasets are not user-uploadable market data or a general train/evaluation split protocol. Definitions live in [research fixtures](../../rust/crates/backend/src/research/fixtures.rs); the [Phase D record](../development/phase-d-local.md) records controlled expected outcomes.

Lifecycle failure and report availability are separate: a failed episode with a sealed partial report can be read with `get_research_result`, whereas an unsealed/missing report returns `409 RESEARCH_RESULT_UNAVAILABLE`. A validation or manifest-write failure records `RESEARCH_EVIDENCE_FAILED` and does not publish the result. `wait_research_job` raises on failed lifecycle; use its job ID to inspect status and retrieve an available partial report explicitly. A matched replay of such a failed source succeeds as verification and retains `source_outcome: failed`.

## Python SDK usage

`Client` exposes `submit_reranking`, `submit_episode`, `submit_research_replay`, `get_research_job`, `wait_research_job`, and `get_research_result`. If a submit acknowledgement is uncertain, the raised exception retains the job ID for lookup; the SDK does not repeat the POST. Each research status request is bounded by the remaining local wait deadline; `ResearchWaitTimeout` retains the last observed status, including a terminal reply that arrived too late. A local polling timeout does not cancel the backend job. See [`research_workflows.py`](../../python-sdk/examples/research_workflows.py).

This interface does not provide online `step()` interaction, policy training, cancellation, random-seed control, hosted persistence, or public API stability. Verification details and compatibility limits are recorded in the [Phase D local verification guide](../development/phase-d-local.md).
