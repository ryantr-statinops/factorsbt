# Roadmap

**Status:** Proposed sequence. Stages describe intended outcomes, not implemented features or delivery dates. The first synthetic scope within Stage 1 has been accepted in the [MVP 1 scope](product/mvp-scope.md); the rest of Stage 1 remains contingent on verified implementation and any choices explicitly left Open.

There are no performance targets, provider guarantees, or release promises attached to these stages. The language/API direction is recorded in [ADR 0001](decisions/0001-python-sdk-rust-backend.md).

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
