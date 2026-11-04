# ADR 0003: Local comparison and recorded replay boundaries

**Status:** Accepted for Phase C synthetic research.

## Context

Phase B runs one scenario per run through the Rust backend and records local evidence. The v1 request and result schemas are strict. Phase C must compare independent runs and verify recorded-response replay without changing the meaning or shape of existing v1 operations.

## Decision

- The Python SDK coordinates comparison as separate v1 runs, submitting and waiting sequentially. Each logical comparison label maps to a distinct run ID; each run retains its internal `baseline` scenario ID. The comparison report may be saved locally, but the backend creates no experiment job.
- Accept any valid v1 configuration in a comparison. Report each run's interval, initial/final NAV, USD P&L and nonannualized return. Align step and instrument differences only on shared UTC simulation times and instrument IDs; report unmatched coverage explicitly. Changed configuration and observed output differences do not establish causal attribution.
- A failed or uncertain run yields a partial comparison report. An uncertain POST is resolved through submission lookup without an automatic second POST.
- Add independent v1 operations to enumerate and read verified local evidence by artifact ID. Existing run request and result shapes stay unchanged. Artifact paths are never supplied by clients.
- Add an independent replay resource. A replay job reads sealed evidence from disk, including after backend restart, and never calls the HTTP model. Replay jobs share the existing bounded worker capacity. A matched source failure counts as a succeeded verification job; its report preserves the source failure outcome.
- New source manifests include exact recorded inputs, terminal failure information when present, contract and evidence-format versions, and SHA-256 of the backend executable. Replay requires the same executable digest and contract version. Phase B manifests without this fingerprint remain readable but cannot be replayed under this policy.
- Replay compares normalized financial state and ordered events, ignoring regenerated IDs and service processing timestamps. It claims reproducibility only for the controlled synthetic scope under the same binary and recorded inputs.

## Alternatives considered

- A single backend experiment job would own comparison lifecycle, but separate runs preserve the existing submission and isolation model and were chosen for this slice.
- Altering v1 run schemas would conflict with the accepted compatibility policy. Independent operations allow new behavior while retaining existing clients and stored results.
- Byte-for-byte replay would confuse regenerated operational metadata with financial behavior. Comparing transitions and state directly matches the research goal.

## Consequences

Comparison is sequential and may take longer than parallel execution. Different intervals or instrument universes produce partial overlap and visibly unmatched records. The SDK must fetch evidence to explain decisions, while the backend must verify its bytes and containment before serving them. Replay requires a complete sealed source manifest; failures that prevented evidence persistence cannot be replayed. No stateful model isolation, external provider integration, hosted access, or cross-build replay guarantee is introduced.

See [Stage 08](../../plan/implementation/08-scenario-comparison.md), [Stage 09](../../plan/implementation/09-recorded-replay.md), and the [v1 compatibility rules](../interfaces/mvp-contract-compatibility.md).
