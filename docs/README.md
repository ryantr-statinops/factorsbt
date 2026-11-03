# FactorsBT documentation

FactorsBT is API-first infrastructure for multi-asset backtesting, synchronized data, and multi-model decision research.

## Project status

The repository contains accepted product and interface documentation, a verified Phase A foundation, and locally verified Phase B data, engine primitives, model orchestration, and run API behavior (Stages 03–06). The Rust backend exposes `GET /healthz` and the four `/v1` run operations. The Python SDK currently exposes health only; typed backtest configuration, submission, polling, and the complete MVP acceptance gate remain in Stage 07. The implemented local API has no public stability guarantee.

The accepted technology direction is a pure Python SDK and a Rust backend/engine, communicating initially over HTTP/JSON. Acceptance records design intent, not implementation completion.

## Reading order

1. [Product overview](product/overview.md): the problem, intended users, examples, and boundaries.
2. [Architecture overview](architecture/overview.md): components and ownership.
3. [Simulation lifecycle](architecture/simulation-lifecycle.md): data visibility, decisions, execution, and replay.
4. [Service contracts](interfaces/service-contracts.md): MVP component boundaries, operations, schemas, and remaining open infrastructure choices.
5. [Roadmap](roadmap.md): staged goals and exit criteria.
6. [Development workflow](development/workflow.md): how to propose, implement, and verify changes.
7. [Architecture decisions](decisions/README.md): accepted choices and their rationale.
8. [MVP 1 scope](product/mvp-scope.md): accepted first synthetic implementation slice and its explicit limits.
9. [MVP 1 acceptance fixtures](examples/mvp/acceptance-cases.md): controlled expected behaviors used to check implementations.
10. [MVP 1 OpenAPI and compatibility rules](interfaces/mvp-contract-compatibility.md): shared [OpenAPI](../contracts/openapi.json), JSON Schemas, examples, and version policy.

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
- Exchanges across boundaries belong in service contracts. MVP 1 operations and schema documents are linked from [the service contract overview](interfaces/service-contracts.md); unresolved provider and deployment decisions stay there as Open.
- Durable architecture choices belong in ADRs. Planned work and its dependencies belong in the roadmap.
- Accepted synthetic instrument and accounting rules live in the owning architecture documents linked from the [MVP 1 scope](product/mvp-scope.md). Real-market behavior remains open unless separately accepted.
- The sequence and progress evidence for implementation phases live in the [implementation plan](../plan/implementation/README.md).
- Workflow instructions belong in the development guide and agent instructions, not in public interface descriptions.

Keep related documents linked instead of duplicating detailed rules. Record open questions near their owning topic.
