# Python SDK role

## Responsibility

Own the pure Python client experience for configuring and submitting backtests, tracking runs, and retrieving results. The SDK communicates with the backend; portfolio simulation belongs in the Rust engine.

Read the [architecture](../../docs/architecture/overview.md), [service contracts](../../docs/interfaces/service-contracts.md), and [development workflow](../../docs/development/workflow.md).

## Design expectations

- Make backend address, connection settings, and credential handling explicit when their contracts are decided. Do not embed a hosted-service assumption.
- Surface validation errors, transport failures, external-service failures, and run failures with useful context and identifiers where available.
- Keep run submission distinct from waiting for completion. Specify cancellation and timeout semantics before exposing them.
- Treat a client timeout as an unknown outcome until the backend can establish whether an operation was accepted. Retrying submission must not silently duplicate a run.
- Keep transport retries separate from simulation retries. Do not change financial behavior in response to network failures.
- MVP client behavior is synchronous with polling after asynchronous submission; cancellation and streaming are not part of the current contract. Follow the shared [OpenAPI and compatibility rules](../../docs/interfaces/mvp-contract-compatibility.md) rather than adding retry or compatibility behavior on the client.

## Verification and handoff

When implementation exists, verify request/response handling, failure propagation, run tracking, and protection of secrets using controlled service fixtures. Exercise the public API rather than internal structure alone.

For a handoff, state user-visible behavior, affected backend contracts, examples verified against implementation, checks performed, and remaining decisions. Until executable interfaces exist, label API examples as conceptual.
