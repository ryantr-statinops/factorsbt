# FactorsBT documentation

FactorsBT is API-first infrastructure for multi-asset backtesting, synchronized data, and multi-model decision research.

## Project status

This is a foundation documentation repository. The product, architecture, and lifecycle describe the intended system. There is no implemented SDK, backend, engine, stable endpoint set, or published package established by these documents.

The accepted technology direction is a pure Python SDK and a Rust backend/engine, communicating initially over HTTP/JSON. Acceptance records design intent, not implementation completion.

## Reading order

1. [Product overview](product/overview.md): the problem, intended users, examples, and boundaries.
2. [Architecture overview](architecture/overview.md): components and ownership.
3. [Simulation lifecycle](architecture/simulation-lifecycle.md): data visibility, decisions, execution, and replay.
4. [Service contracts](interfaces/service-contracts.md): conceptual exchanges and unresolved contract choices.
5. [Roadmap](roadmap.md): staged goals and exit criteria.
6. [Development workflow](development/workflow.md): how to propose, implement, and verify changes.
7. [Architecture decisions](decisions/README.md): accepted choices and their rationale.

Contributors should also read [AGENTS.md](../AGENTS.md). Agent-specific responsibilities live in [.agents](../.agents/README.md).

## Status vocabulary

| Status | Meaning |
| --- | --- |
| Accepted | An agreed design direction or requirement; implementation may still be absent. |
| Proposed | A candidate design, scope, or behavior requiring a decision before implementation depends on it. |
| Open | A question without an agreed answer. |
| Implemented | Behavior supported by repository code and appropriate verification evidence. |

Documents state the status of their content where relevant. Moving a statement to Implemented requires evidence; an ADR alone is insufficient.

## Where information belongs

- Product goals and users belong in the product overview.
- Component boundaries belong in the architecture overview; simulation semantics belong in the lifecycle.
- Exchanges across boundaries belong in service contracts. Endpoint/schema specifications will be added when decided.
- Durable architecture choices belong in ADRs. Planned work and its dependencies belong in the roadmap.
- Workflow instructions belong in the development guide and agent instructions, not in public interface descriptions.

Keep related documents linked instead of duplicating detailed rules. Record open questions near their owning topic.
