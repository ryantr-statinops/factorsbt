# MVP 1 results and evidence references

**Status:** Accepted MVP 1 result-contract design. It describes successful-run output and local evidence references; it does not provide persistent storage or an artifact-download endpoint.

The canonical [successful result schema](../../contracts/schemas/run-result.schema.json) is returned by `GET /v1/runs/{run_id}/results` only after the run reaches `succeeded`. A failed or unfinished run is reported by its status resource; an unavailable or failed result is never represented as a successful result with missing fields.

## Result contents

The result gives run/scenario identity, simulation interval, backend finish time, initial cash, fees, event counts, and the final portfolio snapshot with cash, NAV, positions and marks, reserves, margin, and free buying power. Positions still open at the end are reported as-is. A run does not create end-of-run fills to make the portfolio appear flat.

Provenance records fixture identity/version and model ID/version, inference count, and invocation mode. `fresh_inference` means the model service was called for this run. `recorded_response_replay` means prior model responses were replayed without invoking the model; these modes must not be conflated. MVP 1 uses a fresh call to the controlled HTTP fixture.

## Evidence references

Each evidence reference includes artifact ID, kind, media type, path relative to that run's evidence root, SHA-256 digest, byte size, and processing-time creation timestamp. Paths resolve only under the backend's configured run directory, are never absolute, and may not contain `.` or `..` path components. When artifact counts are nonzero, the manifest must reference their evidence; omitted event types must agree with zero counts. The result contains no unrestricted URL, credential-bearing reference, or private payload in an error message.

The local backend writes artifacts under its run directory. The API returns references, not artifact bytes, and MVP 1 defines no separate artifact-fetch endpoint. A later remote deployment must choose an authorized retrieval mechanism before exposing remote artifact access; this local path convention is not a remote URL contract.

The artifact digest is SHA-256 of the exact stored artifact bytes. This is distinct from submission equality: submission identity compares parsed configuration values under the rules in [submission identity](mvp-submissions.md). A result validator must check that references exist under the run root and that size/digest match the stored file.

## Validation and error examples

- [Successful run result](../../contracts/examples/results/successful-run.valid.json) shows a non-flat portfolio and evidence for configuration, snapshot, and model response. Its digest strings and byte sizes are synthetic placeholders, not hashes of checked-in example files.
- [Artifact missing its digest](../../contracts/examples/results/missing-artifact-hash.schema-invalid.json) fails with result validation error `RESULT_SCHEMA_INVALID` at `$.sha256`.
- [Path traversal reference](../../contracts/examples/results/path-traversal.semantic-invalid.json) is schema-valid but fails with `EVIDENCE_REFERENCE_INVALID`; it may not resolve outside the run evidence root.

These artifacts support provenance, not by themselves a claim of deterministic replay. Captured observations and model responses, engine version, contract version, and the selected invocation mode must all be available before making a stronger reproducibility claim.
