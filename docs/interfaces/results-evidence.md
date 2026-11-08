# MVP 1 results and evidence references

**Design status:** Accepted MVP 1 result-contract design.

**Implementation status:** The local Rust backend seals run evidence and verifies successful-run artifacts before publishing `succeeded`. Phase C adds verified evidence enumeration and artifact reads for sealed succeeded or failed runs. These operations do not provide persistent job metadata or hosted artifact storage.

The canonical [successful result schema](../../contracts/schemas/run-result.schema.json) is returned by `GET /v1/runs/{run_id}/results` only after the run reaches `succeeded`. A failed or unfinished run is reported by its status resource; an unavailable or failed result is never represented as a successful result with missing fields.

## Result contents

The result gives run/scenario identity, simulation interval, backend finish time, initial cash, fees, event counts, and the final portfolio snapshot with cash, NAV, positions and marks, reserves, margin, and free buying power. Positions still open at the end are reported as-is. A run does not create end-of-run fills to make the portfolio appear flat.

Provenance records fixture identity/version and model ID/version, inference count, and invocation mode. `fresh_inference` means the model service was called for this run. `recorded_response_replay` means prior model responses were replayed without invoking the model; these modes must not be conflated. MVP 1 uses a fresh call to the controlled HTTP fixture.

## Evidence references

Each evidence reference includes artifact ID, kind, media type, path relative to that run's evidence root, SHA-256 digest, byte size, and processing-time creation timestamp. Paths resolve only under the backend's configured run directory, are never absolute, and may not contain `.` or `..` path components. When artifact counts are nonzero, the manifest must reference their evidence; omitted event types must agree with zero counts. The result contains no unrestricted URL, credential-bearing reference, or private payload in an error message.

The result endpoint returns references. The local backend stores artifacts under `<evidence-root>/runs/<run-id>/`; Phase C adds separate operations to fetch their verified bytes. A reference's relative path is not an arbitrary filesystem input or remote URL.

The artifact digest is SHA-256 of the exact stored artifact bytes. This is distinct from submission equality: submission identity compares parsed configuration values under the rules in [submission identity](mvp-submissions.md). A result validator must check that references exist under the run root and that size/digest match the stored file.

## Verified evidence access

| Operation | Response |
| --- | --- |
| `GET /v1/runs/{run_id}/evidence` | An [EvidenceIndex](../../contracts/schemas/evidence-index.schema.json) containing run identity, terminal status and verified artifact references, including the run manifest. |
| `GET /v1/runs/{run_id}/artifacts/{artifact_id}` | Exact stored bytes with the reference's content type, resolved by artifact UUID from that run's manifest. |

The backend checks manifest identity, reference uniqueness, path containment, byte size and SHA-256. It never accepts a client-supplied file path. `Client.get_evidence(run_id)` returns a typed index; `Client.get_artifact(run_id, reference)` returns bytes and independently checks their size/digest. Parse JSON only after this verification.

Sealed evidence can be read after restart from the same evidence root even though run/submission status metadata is lost. A failed run may have sealed inputs, an error and a partial trace; this does not make `/results` available or promise every event type exists. Failure to persist/seal evidence leaves no readable complete index.

| Condition | HTTP status and code |
| --- | --- |
| Queued/running run without sealed evidence | `409 RUN_NOT_SUCCEEDED` |
| Unknown run with no source evidence | `404 RUN_NOT_FOUND` |
| Unknown artifact ID in a valid index | `404 EVIDENCE_ARTIFACT_NOT_FOUND` |
| Terminal run evidence unavailable, malformed references, or failed integrity check | `500 EVIDENCE_REFERENCE_INVALID` |
| Malformed path UUID | `400` using the shared error envelope |

These routes belong to `/v1/runs`. Replay reports and research results expose their own evidence references; there are no corresponding replay/research artifact-download routes. Local run artifact access does not establish remote authorization, retention or hosted retrieval semantics.

## Validation and error examples

- [Successful run result](../../contracts/examples/results/successful-run.valid.json) shows a non-flat portfolio and evidence for configuration, snapshot, and model response. Its digest strings and byte sizes are synthetic placeholders, not hashes of checked-in example files.
- [Artifact missing its digest](../../contracts/examples/results/missing-artifact-hash.schema-invalid.json) fails with result validation error `RESULT_SCHEMA_INVALID` at `$.sha256`.
- [Path traversal reference](../../contracts/examples/results/path-traversal.semantic-invalid.json) is schema-valid but fails with `EVIDENCE_REFERENCE_INVALID`; it may not resolve outside the run evidence root.

These artifacts support provenance, not by themselves a claim of deterministic replay. Captured observations and model responses, engine version, contract version, and the selected invocation mode must all be available before making a stronger reproducibility claim.
