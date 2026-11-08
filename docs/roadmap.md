# Roadmap

**Status:** Accepted synthetic MVP scope with Implemented local slices across the stages below. This roadmap retains the broader product sequence; future provider, deployment, and general research extensions remain Proposed or Open, without delivery dates.

There are no performance targets, provider guarantees, or release promises attached to these stages. The language/API direction is recorded in [ADR 0001](decisions/0001-python-sdk-rust-backend.md).

## Delivered local scope

| Roadmap stage | Implemented scope and evidence | Remaining boundary |
| --- | --- | --- |
| 0 | Foundation documentation and [Phase A tooling](development/phase-a-local.md) | Documentation acceptance alone does not prove runtime behavior. |
| 1 | Controlled synthetic backtest, typed SDK and run API: [Phase B](development/phase-b-local.md) | No real providers, exchange conventions, or hosted service. |
| 2 | Sequential independent-run SDK comparison and same-binary recorded replay: [Phase C](development/phase-c-local.md) | No parallel comparison job or cross-build replay. |
| 3 | Fixed reranker and fixed-action episodes: [Phase D](development/phase-d-local.md) | No general ranking framework, interactive RL, or training. |

The local run/replay CLI is a separate [Phase E extension](development/phase-e-local.md). Roadmap Stages 0–3 are product groupings; implementation Stages 00–12 track the finer delivery sequence in the [implementation plan](../plan/implementation/README.md).

## Stage 0: Foundation documentation

Establish the product definition, component boundaries, conceptual lifecycle/contracts, agent roles, development workflow, and initial ADR.

Exit criteria: the documents are substantive and linked, accepted choices are separated from proposals/open questions, and no unimplemented API or command is presented as available. This stage does not establish an executable MVP.

## Stage 1: First complete backtest path

The accepted first implementation slice is defined in [MVP 1 scope](product/mvp-scope.md): configurable synthetic spot/futures/options, controlled daily bars and quotes, one stateless HTTP model fixture, a Python SDK, a local Rust backend, and inspectable results. It tests temporal visibility, grouped shared-capital admission, order/fill separation, financial transitions, and replayable evidence within controlled assumptions.

The initial service runtime is local only. Real provider/model integrations, deployment, and exchange-specific financial behavior remain extensions rather than prerequisites for this slice.

The synthetic instrument, event-order, model-output, admission, accounting, pricing, collateral, and liquidation assumptions for this slice have been accepted in the linked scope and architecture documents. Before extending beyond it, select the relevant market/provider conventions, stateful-model behavior, network access controls, or deployment needs explicitly. The synthetic suite remains the reference for implementation correctness.

Exit criteria: one end-to-end experiment runs under documented assumptions; results connect model inputs and outputs to orders, fills, and portfolio changes. Checks cover unavailable future data, competing decisions, adapter failure, duplicate effects, and the declared execution rules. Setup commands must correspond to actual tooling.

## Stage 2: Comparative experiments

Extend the selected path to compare data conditions, model/policy choices, and simulation settings across isolated scenarios. Share immutable snapshots where appropriate while maintaining separate mutable state and request correlation.

Exit criteria: controlled scenario differences are explainable, recorded-response replay is verified within its declared scope, and model reruns are identified as fresh inference. Any parallel scheduling must preserve those semantics; scale targets require measurement.

## Stage 3: Reranking and reinforcement-learning research

Explore reranking candidate actions/policies and defining an RL interaction surface over the simulator. These are separate design efforts, not implied by support for multiple models.

Before implementation, define ranking inputs/objectives, aggregation/admission behavior, or RL observations/actions/rewards/episode boundaries as appropriate. Account for historical information access, evaluation splits, and model state.

Exit criteria: an accepted contract for the selected research capability, a bounded example, and verification showing how it interacts with portfolio constraints and replay. No generic training framework or improvement guarantee is committed.

## Decision ownership

- Markets, users, initial data coverage, and product boundaries: [product overview](product/overview.md).
- Deployment, frameworks, storage, queues, and access controls: [architecture overview](architecture/overview.md).
- External payloads, model outputs, lifecycle operations, and compatibility: [service contracts](interfaces/service-contracts.md).
- Clock/order semantics, portfolio rules, fees, fills, slippage, and latency: [simulation lifecycle](architecture/simulation-lifecycle.md).
- Significant accepted architecture choices: [ADRs](decisions/README.md).

Decide only what the next selected slice needs. Keep later-stage proposals visible without treating them as delivered functionality.
