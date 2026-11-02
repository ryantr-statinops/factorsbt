# Architecture overview

**Status:** Accepted language and API direction; the component decomposition below is Proposed.

## Accepted foundation

The public client is a pure Python SDK. The backend and backtest engine are written in Rust. Initial communication uses HTTP/JSON. External data and model services can use other languages behind their adapters. See [ADR 0001](../decisions/0001-python-sdk-rust-backend.md).

These boundaries describe responsibilities. They do not require separate deployments for each internal component or an embedded native extension in the SDK.

## Intended data and control flow

```mermaid
flowchart TD
    SDK[Python SDK] -->|Run configuration and control| API[Rust backend API]
    API --> ORCH[Backtest orchestrator]
    SOURCE[External data APIs] --> DA[Data adapters]
    DA --> DATA[Normalized and versioned data]
    DATA -->|Eligible data snapshot| ORCH
    ORCH -->|Model request for a simulation step| MG[Model gateway and adapters]
    MG --> MODEL[External model APIs]
    MODEL -->|Model output| MG
    MG --> ORCH
    ORCH -->|Correlated decision group| ENGINE[Rust portfolio and execution engine]
    ENGINE -->|State and step outcome| ORCH
    ORCH --> RESULTS[Run evidence and results]
    RESULTS --> API
    API -->|Progress and results| SDK
```

The orchestrator advances a scenario by coordinating eligible data, model responses, and engine transitions. State snapshots used for inference come from that scenario's engine state.

## Component responsibilities

| Component | Owns | Boundary |
| --- | --- | --- |
| Python SDK | Client configuration, run submission, tracking, and result access | Does not own simulation accounting or external inference execution |
| Backend API | Client-facing validation and access to run operations | Does not define financial execution rules |
| Orchestrator | Run/step coordination, adapter invocation, response correlation, and progress | Delegates portfolio transitions to the engine |
| Data adapters and layer | Provider integration, normalization, source/version metadata, and snapshot assembly | Do not infer unavailable historical publication times |
| Model gateway and adapters | Model-service integration and output validation/correlation | Do not invent replacement decisions on failure |
| Portfolio and execution engine | Simulation clock semantics, capital constraints, order/fill processing, and state transitions | Does not implement HTTP transport retry policy |
| Evidence and results | Traceable configuration, inputs, responses, transitions, and result references | Storage technology and retention remain open |

## Shared-capital and scenario boundaries

Within a scenario, assets share the declared capital pool and portfolio constraints. A group of decisions is evaluated using a consistent pre-application snapshot. An explicit policy must resolve incompatible proposals or insufficient capital.

Across scenarios, mutable portfolio, pending-order, model, and random state must remain isolated where present. Immutable input data may be shared by explicit version or snapshot identity. If decisions depend on a scenario's holdings or prior actions, that scenario needs its own model invocation or isolated model state; outputs cannot be reused merely because the market timestamp matches.

## Time and failure boundaries

Simulated time controls data visibility and execution eligibility. Wall-clock time controls service operations. A slow inference call does not define a simulated trading delay unless a declared policy maps it to one.

The backend handles transport failures; the engine applies declared financial behavior. Retrying transport must not duplicate a run or decision application. No exactly-once processing guarantee, fallback policy, or persistence mechanism has been selected.

Detailed semantics belong in the [simulation lifecycle](simulation-lifecycle.md). Conceptual exchanges belong in [service contracts](../interfaces/service-contracts.md).

## Open infrastructure decisions

- Self-hosted versus managed deployment, and the first supported operating environment.
- Rust HTTP framework, Python HTTP client, storage formats, database, and queue.
- Authentication, authorization, snapshot access, secret management, and tenancy.
- Worker scheduling, concurrency limits, caching, retention, and operational telemetry.
- Concrete data/model protocol schemas and the first supported providers.

Select these for an agreed implementation slice; do not infer them from the diagram.
