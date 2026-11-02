# MVP 1 model requests and responses

**Status:** Accepted MVP 1 correlated exchange design for a stateless local HTTP fixture. It does not implement inference, external credentials, automatic retries, or model fallback.

The canonical schemas are [inference request](../../contracts/schemas/model-request.schema.json) and [model response](../../contracts/schemas/model-response.schema.json). Both use the shared [common definitions](../../contracts/schemas/common.schema.json), [instrument configuration](../../contracts/schemas/instrument.schema.json), and [snapshot](../../contracts/schemas/snapshot.schema.json).

## Request

Each request carries `contract_version`, a unique `inference_id`, `run_id`, `scenario_id`, `step_id`, `snapshot_id`, configured `model_id` and `model_version`, `requested_at`, and `simulation_time`. It includes the selected decision policy, IDs eligible for a new decision, the run's instrument catalog, and the complete snapshot inline.

The request's correlation IDs and simulation time must exactly match the nested snapshot. `requested_at` is wall-clock processing time; `simulation_time` controls data visibility and is not advanced by HTTP latency. The model fixture is stateless: it receives the snapshot and policy on every call and does not carry a portfolio or prior request state between steps.

## Response variants

A response echoes the inference/run/scenario/step/snapshot IDs and configured model identity. `responded_at` records actual processing time. Its `response_type` must match the run's single configured output mode.

| Type | Output shape | Semantic checks |
|---|---|---|
| `scores` | One `{instrument_id, score}` per eligible instrument. Scores are finite decimal strings. | No duplicate/unknown/ineligible IDs. Positive scores rank descending; ties use instrument ID. Nonpositive scores are not selected. Top K and derivative sizing follow the submitted policy. |
| `targets` | `spot_weights` and `derivative_contracts` arrays. | IDs are unique and resolve to the matching instrument type. Omitted configured instruments target zero. Spot weights are nonnegative and total no more than one. Derivative quantities are signed integer contracts. |
| `order_intents` | `orders` array with one market intent per instrument and signed decimal quantity. | No duplicate/unknown IDs, zero quantities, short spot, or unsupported type. Portfolio admission still evaluates the entire group against one snapshot. |

The schema checks payload shape. It cannot establish eligible IDs, response mode equality, score coverage, sum of weights, position limits, contract semantics, or shared-capital feasibility. Those are run validation and engine responsibilities. An accepted model response is a proposal; it is not itself an admitted order or fill.

## Correlation and failures

The backend applies a reply only to the pending inference with the exact inference/run/scenario/step/snapshot identity and expected model/version. A missing field is `MODEL_RESPONSE_SCHEMA_INVALID`. A reply with a different identity is `MODEL_RESPONSE_CORRELATION_MISMATCH`; a response type that differs from the configured mode is `MODEL_RESPONSE_MODE_MISMATCH`. A duplicate or late reply is recorded as `MODEL_RESPONSE_DUPLICATE` or `MODEL_RESPONSE_STALE` and never applied to another step.

There is one request per decision step and no automatic inference retry. A timeout, transport error, malformed response, or invalid output fails the run with available correlation IDs. The backend does not substitute an empty output, invoke another model, or convert service failure into an engine decision. Processing timeouts do not create an implicit simulated delay.

## Examples and expected errors

- [Scores request with inline snapshot](../../contracts/examples/model/scores-request.inline-snapshot.json) and [matching scores response](../../contracts/examples/model/scores-response.valid.json) form a valid correlated exchange.
- [Targets response shape](../../contracts/examples/model/targets-response.valid-shape.json) and [market order-intents response shape](../../contracts/examples/model/order-intents-response.valid-shape.json) exercise the other two schema variants. A live exchange must pair each with a request selecting that same output mode.
- [Missing snapshot identity](../../contracts/examples/model/response-missing-snapshot.schema-invalid.json) fails with run error `MODEL_RESPONSE_SCHEMA_INVALID` at `$.snapshot_id`.
- [Mismatched step](../../contracts/examples/model/response-mismatched-step.semantic-invalid.json) is schema-valid but fails with `MODEL_RESPONSE_CORRELATION_MISMATCH`; it cannot be applied to `step-0001`.

These errors fail the run; they are not HTTP error responses to the SDK. Submission errors are defined in [submission identity and failures](mvp-submissions.md). Producer/consumer verification is still pending.
