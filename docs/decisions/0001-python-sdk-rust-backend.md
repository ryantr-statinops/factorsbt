# ADR 0001: Python SDK and Rust backend/engine

**Status:** Accepted

## Context

FactorsBT is intended as API-first infrastructure connecting external data and model services to multi-asset simulation. Python is the user-facing research interface. The service and engine own run coordination and portfolio state.

The project discussion selected a Python SDK communicating with a Rust backend and Rust engine. The repository is at the documentation stage; this record captures that selection without claiming an implementation exists.

## Decision

- Write the SDK in pure Python.
- Write the backend and backtest engine in Rust.
- Use HTTP/JSON initially between the SDK and backend and for the planned external API integrations, with adapters for provider-specific details.
- Keep external model and data implementations language-independent.
- Keep SDK, orchestration, and simulation responsibilities separate. Backend and engine can be internal components of the same Rust deployment; this decision does not require microservices.

## Alternatives considered

- **Embedded Rust engine through Python bindings:** relevant to a future in-process/local-engine product, but not selected for the initial API-client boundary. Native Python bindings and their packaging are not required by this decision.
- **Go backend:** considered for orchestration-oriented infrastructure; Rust was selected for both the backend and simulation engine to share a language across those components.
- **Pure Python simulation service:** possible for prototyping, but not the accepted implementation direction.

## Consequences

The SDK can focus on Python ergonomics while the Rust components own simulation and service behavior. Users of the SDK interact with a backend endpoint; the backend may be self-hosted or managed depending on a later deployment decision.

Development needs explicit cross-language service contracts and separate SDK/backend verification. Choosing Rust does not establish throughput, latency, deterministic replay, or correctness guarantees. Those require implementation and evidence.

This ADR does not select a Rust framework, Python HTTP dependency, storage system, queue, authentication mechanism, deployment topology, streaming protocol, or exact endpoint/schema set. Revisit the record through a superseding ADR if the component-language or transport direction changes materially.

## Related documents

- [Architecture overview](../architecture/overview.md)
- [Service contracts](../interfaces/service-contracts.md)
- [Simulation lifecycle](../architecture/simulation-lifecycle.md)
- [Roadmap](../roadmap.md)
