# FactorsBT documentation

FactorsBT is API-first infrastructure for multi-asset backtesting, synchronized data, and multi-model decision research.

## Project status

Phases A–E have passed their local verification gates across Stages 00–12. The backend provides local run, replay and bounded research job operations; the Python SDK provides typed run/replay/research operations and sequential scenario comparison; the installed Python package exposes the local `factorsbt` run/replay CLI. These remain controlled local workflows: metadata is process-local, evidence is local, replay requires the same executable digest, and no public API stability, hosted deployment, or PyPI release is claimed. See the [Phase B verification record](development/phase-b-local.md), [Phase C verification record](development/phase-c-local.md), [Phase D verification record](development/phase-d-local.md), and [Phase E verification record](development/phase-e-local.md).

The accepted technology direction is a pure Python SDK and a Rust backend/engine, communicating initially over HTTP/JSON. Acceptance records design intent, not implementation completion.

## Start by task

| Goal | Read |
| --- | --- |
| Run a first self-contained synthetic demo | [Quickstart](development/quickstart.md) |
| Configure/submit runs and inspect evidence in Python | [Python SDK](development/python-sdk.md) |
| Use the installed run/replay command | [CLI](development/cli.md) |
| Compare independent scenarios and interpret deltas | [Comparison](development/comparison.md) |
| Start/configure a local backend or diagnose failures | [Local operations](development/local-operations.md) |
| Change synthetic datasets safely | [Fixture workflow](../contracts/fixtures/README.md) |
| Run bounded reranking or fixed-action episodes | [Research interface](interfaces/research-jobs.md) and [example](../python-sdk/examples/research_workflows.py) |
| Review verification limits | [Acceptance traceability](examples/mvp/acceptance-cases.md#verification-traceability) and phase records below |
| Check the latest local regression results | [2026-11-09 code verification](development/regression-verification-2026-11-09.md) |

The [root README](../README.md) introduces the repository. Quickstart smoke programs start their own fixtures; SDK/CLI examples using standalone services have explicit prerequisites. No standalone model-fixture launcher is provided.

## Product and architecture reading order

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
11. [Phase C recorded replay design](interfaces/recorded-replay.md): source evidence, verification lifecycle, and compatibility boundary.
12. [Phase D research job interface](interfaces/research-jobs.md): controlled reranking, fixed-action episode jobs, and sealed research replay.
13. [FactorsBT CLI](development/cli.md): install the Python package and use local run/replay commands.

Contributors should also read [AGENTS.md](../AGENTS.md). Agent-specific responsibilities live in [.agents](../.agents/README.md).

## Verification records and open distribution decisions

Phase [A](development/phase-a-local.md), [B](development/phase-b-local.md), [C](development/phase-c-local.md), [D](development/phase-d-local.md) and [E](development/phase-e-local.md) records describe historical executed checks, environments and boundaries. The [documentation refresh record](development/documentation-verification.md) covers its Markdown and demo checks; the [2026-11-09 regression record](development/regression-verification-2026-11-09.md) records the latest local code suites and smoke checks. Test totals are observations, not proof of unspecified acceptance cases or a platform support matrix.

**Open:** Repository licensing has not been selected. No `LICENSE` or package license grant is supplied by this documentation. Hosted deployment, real-provider integration, durable metadata/recovery and public release decisions remain outside the local verification claim.

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
- Current setup and smoke commands live in the [quickstart](development/quickstart.md); historical executed build/test commands live in the [phase verification records](#verification-records-and-open-distribution-decisions). Contributor guidance lives in the [development workflow](development/workflow.md).
- Workflow instructions belong in the development guide and agent instructions, not in public interface descriptions.

Keep related documents linked instead of duplicating detailed rules. Record open questions near their owning topic.
