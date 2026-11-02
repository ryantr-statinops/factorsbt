# Roadmap

**Status:** Proposed sequence. Stages describe intended outcomes, not implemented features or delivery dates.

There are no performance targets, provider guarantees, or release promises attached to these stages. The language/API direction is recorded in [ADR 0001](decisions/0001-python-sdk-rust-backend.md).

## Stage 0: Foundation documentation

Establish the product definition, component boundaries, conceptual lifecycle/contracts, agent roles, development workflow, and initial ADR.

Exit criteria: the documents are substantive and linked, accepted choices are separated from proposals/open questions, and no unimplemented API or command is presented as available. This stage does not establish an executable MVP.

## Stage 1: First complete backtest path

Proposed scope: multiple assets sharing capital, price bars plus one event source, one external model service, Python SDK submission, a Rust backend/engine, and inspectable results.

Before implementation, select the market/instrument subset, data source and availability assumptions, model-output representation, decision trigger, capital/risk admission, execution/accounting conventions, and required service tooling. Use synthetic or controlled fixtures for correctness checks alongside any selected provider.

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
