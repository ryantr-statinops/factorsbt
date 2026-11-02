# MVP 1 submission identity and failures

**Status:** Accepted MVP 1 contract behavior for the in-process backend. It does not promise persistence, cross-process deduplication, or exactly-once execution.

## Submission identity

`POST /v1/runs` requires an `Idempotency-Key` header containing a UUID. The value is the `submission_id` used by `GET /v1/submissions/{submission_id}`. The backend creates the run ID and records the submission before returning its acknowledgment.

| Request condition | Behavior |
|---|---|
| First use of a key with a valid configuration | Create one submission and one run; return `202 Accepted`. |
| Existing key with the same configuration | Return the original acknowledgment with the same submission ID and run ID; do not enqueue another run. |
| Existing key with a different configuration | Return `409 Conflict` with `SUBMISSION_ID_REUSED`; preserve the original run. |
| Different key with the same configuration | Create a new, independent submission and run. |
| Invalid JSON, schema-invalid configuration, or semantically invalid configuration | Return an error and create no submission record or run. The caller may correct the request and reuse its key. |

Configuration equality ignores JSON object member ordering. Array ordering and all scalar values, including decimal strings and timestamps, are significant. The backend does not coerce values, insert implicit defaults, or reorder arrays when comparing submissions. `contract_version` is part of the configuration identity. An internal digest may be used, but it must preserve these equality rules.

The deduplication record lives only in the backend process. The implementation does not recover it after restart or share it across backend processes. A run may therefore become unresolvable after a restart even when its local evidence files remain.

## Lost acknowledgment and retry ownership

The SDK must not automatically retry `POST /v1/runs`. If the connection ends before an acknowledgment arrives, the outcome is unknown to the caller:

1. Query `GET /v1/submissions/{submission_id}` using the same idempotency key.
2. If found, use the returned run ID and status; do not submit again.
3. If not found in the still-running backend process, the caller may explicitly repeat the exact same body with the same key. The backend performs key lookup and record creation atomically, so concurrent copies in that process map to the same run. This protection does not survive restart.
4. A different body must use a new key. Reusing a recorded key with changed content returns `SUBMISSION_ID_REUSED`.

Transport retries for safe `GET` requests are separate from run submission. A Python client's local wait timeout only ends that client's wait; it does not cancel or retry the run. Model inference is also not retried automatically in MVP 1. A timeout or invalid model output fails the run with the affected run, scenario, step, and snapshot identities when known.

## Error response

Non-success HTTP responses use this envelope:

```json
{
  "error": {
    "code": "SUBMISSION_ID_REUSED",
    "message": "This submission ID is already bound to a different configuration.",
    "request_id": "2f809a3a-c871-4ad1-8985-4bd8685fc0a4",
    "details": [
      { "path": "$.decision_policy.output_mode", "reason": "differs from the recorded configuration" }
    ]
  }
}
```

`details` is omitted when there is no safe, useful field-level information. A response carries the same `request_id` in an `X-Request-ID` header. Error details must not expose credentials, private payloads, local filesystem paths, or unrestricted artifact locations. Schema errors identify a JSON path and a validation reason; semantic errors use a stable code and identify the affected instrument or reference when safe.

The request's `request_id` identifies one HTTP exchange. It is distinct from submission, run, scenario, step, snapshot, decision, and event IDs. Errors are never converted into trading actions or replacement model output.

## HTTP status and error codes

| HTTP | Code | Meaning |
|---:|---|---|
| 400 | `INVALID_JSON` | Body is not valid JSON. |
| 400 | `INVALID_IDEMPOTENCY_KEY` | Required header is missing or is not a UUID. |
| 400 | `INVALID_RESOURCE_ID` | A path identifier is malformed. |
| 404 | `SUBMISSION_NOT_FOUND` | No retained submission has this ID. |
| 404 | `RUN_NOT_FOUND` | No retained run has this ID. |
| 409 | `SUBMISSION_ID_REUSED` | A recorded key is paired with a different configuration. |
| 409 | `RUN_NOT_SUCCEEDED` | Results were requested before successful completion. |
| 409 | `RUN_FAILED` | Results were requested for a failed run; use the run-status resource for failure and last-valid-state details. |
| 422 | `SCHEMA_VALIDATION_FAILED` | JSON is valid but violates the declared wire schema. |
| 422 | `CONTRACT_VERSION_UNSUPPORTED` | The request names a contract version this backend does not support. |
| 422 | `RUN_CONFIGURATION_INVALID` | Schema-valid configuration violates a cross-field or domain rule. |
| 500 | `INTERNAL_ERROR` | Unexpected request-processing failure; no success is implied. |
| 503 | `BACKEND_UNAVAILABLE` | The local service cannot accept or inspect work at this time. |

Run failures are recorded in the run-status resource rather than returned as a successful submission. Their error code is stable and their context includes `run_id`, `scenario_id`, and `step_id` when those identities exist. A failed result request returns `RUN_FAILED`; its error points callers to the status resource instead of pretending the run succeeded.

## Examples

Two requests that differ only in JSON object member order and use the same key map to one run. Changing `decision_policy.output_mode` while retaining that key returns `409 SUBMISSION_ID_REUSED`. Sending the changed configuration with a new key creates a separate run if it is otherwise valid.

If `targets` contains a negative spot weight, the JSON body is syntactically valid but fails schema validation with `SCHEMA_VALIDATION_FAILED`; it does not create a submission. If a model reply has the wrong `step_id`, the backend fails that run rather than applying the reply to another step.

See the [MVP HTTP API proposal](mvp-http-api.md) for operation paths and the [service contract overview](service-contracts.md) for component responsibilities.
