# Phase C recorded-response replay

**Design status:** Accepted for the controlled local synthetic scope.

**Implementation status:** Implemented and locally verified within the limits below; see the [Phase C verification record](../development/phase-c-local.md).

## Source and compatibility

Replay loads a sealed source run from the configured evidence directory by `source_run_id`, including after backend restart. It accepts a succeeded or failed run only when the full manifest and referenced bytes are present and verified. It checks contract version, internal evidence format, and SHA-256 of the executable against the running backend before admission. Older Phase B manifests lack this fingerprint and return `REPLAY_SOURCE_INCOMPATIBLE`; they are not rewritten.

Admission verifies the whole source bundle. The worker validates it again when the job starts, retains that verified index for the job, and checks each artifact's path, size and SHA-256 when reading its bytes. This detects a source artifact changed after admission without rescanning unrelated files for each read. There is no shared evidence cache across jobs.

The source configuration, fixture bytes, ordered snapshots, model responses, and structured terminal failure (when present) are replay inputs. Current fixture registry contents and model endpoint availability do not replace recorded bytes. Failure to persist or seal source evidence makes that source ineligible.

## Operations

- `POST /v1/replays` accepts `{ "contract_version": "1.0.0", "source_run_id": "<uuid>" }` and a UUID `Idempotency-Key`. The key deduplicates the same request within the backend process and conflicts with a different source. Admission validates source integrity before queue creation.
- `GET /v1/replays/{replay_id}` reports `queued`, `running`, `succeeded`, or `failed`. These describe the verification job, not the source's financial outcome.
- `GET /v1/replays/{replay_id}/report` gives source/replay identity, source outcome, matched/diverged result, first mismatch when present, transition counts and replay evidence references. A matched source failure produces `succeeded` verification with `source_outcome: failed`.

Replay jobs share the local worker pool and queue limits with fresh runs. Replay metadata is process-local; its evidence is written to a separate directory. Repeating a POST after an uncertain acknowledgment follows lookup by its idempotency key; no automatic POST retry is performed by the SDK.

## Verification semantics

The replay inference provider uses recorded responses or injects the recorded terminal model failure at its original decision boundary. It makes zero HTTP model calls. Correlation fields are mapped to the replay job's fresh IDs without changing output values. The verifier compares ordered admissions, orders, fills, ledger entries, cash, positions and NAV by financial value. It ignores regenerated IDs, service timestamps and artifact IDs. A mismatch yields `REPLAY_DIVERGED` with a report and leaves the source untouched.

For a failed source, replay also compares every completed transition, the last valid portfolio, and the terminal failure stage, reason and step when recorded. A failed source with a partial portfolio requires its sealed summary and financial trace; a genuine failure before any portfolio transition can have no summary. Matching only the terminal error is insufficient. A failed replay with different completed transitions returns a report with `matched: false` and `first_mismatch`.

This establishes only same-binary, same-contract reproducibility for the controlled fixture path. It does not promise byte-identical artifacts, deterministic fresh inference, or cross-build/platform replay.

See [ADR 0003](../decisions/0003-phase-c-comparison-replay.md) and [Stage 09](../../plan/implementation/09-recorded-replay.md).
