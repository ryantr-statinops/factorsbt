# Service contracts

**Status:** Accepted MVP 1 contract design. Data, engine, orchestration and run API slices have local implementation evidence; public stability is not established.

This document defines component responsibilities. Detailed MVP request, response, error, and event shapes are in the linked API and schema documents. Stages 03–07 implement the synthetic backtest and typed SDK. Phase C adds verified run artifact access, recorded replay and sequential SDK comparison; Phase D adds bounded research jobs; Phase E adds the SDK-based run/replay CLI. All remain local without public stability guarantees.

The [run HTTP API](mvp-http-api.md) covers four original run operations plus two evidence/artifact reads; health is separate. Three [recorded replay operations](recorded-replay.md) and three [research job operations](research-jobs.md) have separate resources. There is no backend comparison resource: the SDK coordinates independent runs. The shared [compatibility rules](mvp-contract-compatibility.md) apply; sealed replay requires matching executable digest and contract version.

The current request shape and type-specific instrument requirements are summarized in [MVP run and instrument configuration](instrument-configuration.md); the JSON Schema files under `contracts/schemas/` are the wire-shape authority.

Data observations and inline, point-in-time snapshots are specified in [MVP observations and decision snapshots](temporal-snapshots.md). Their availability and portfolio-consistency checks are semantic validation, not properties guaranteed by JSON Schema alone.

The correlated inference request and three response modes are specified in [MVP model exchanges](model-exchanges.md). A response is a proposal that still passes engine admission; it is not an order or fill.

Internal engine outputs are event records specified in [MVP engine outcomes](engine-outcomes.md), keeping admission, orders, fills, settlements, ledger postings, and risk actions independently traceable.

Successful run summaries, provenance, and local artifact references are specified in [MVP results and evidence](results-evidence.md). Failed or incomplete runs remain operational status outcomes, not successful results.

## SDK and backend

The SDK lets a user configure an experiment, submit work, inspect progress, and retrieve results/evidence. The backend owns run identity and operational lifecycle; the engine owns simulated financial state.

| Exchange | Information to account for |
| --- | --- |
| Run configuration | Asset universe, simulation interval, data sources, model references, initial portfolio/capital, and declared simulation policies |
| Submission acknowledgment | Run identity and enough information to distinguish accepted work from a rejected request |
| Run inspection | Current operational phase, useful progress context, and failure details when applicable |
| Results | Outcome summary and access to associated evidence and configuration |

Submission, waiting, and execution failure are different outcomes. A lost HTTP response does not establish whether a run was created. The selected identity, deduplication, lookup, and retry rules are in [submission behavior](mvp-submissions.md).

Run lifecycle, asynchronous submission, and SDK polling are selected. Streaming and cancellation are excluded from MVP 1. Authentication, remote deployment, and result pagination remain Open for a later slice.

## Data adapters and normalized data

Data adapters translate provider-specific material into the forms accepted by the data layer. Source observations need asset/context identity, event time, availability information, origin, and version/revision context where available. Retain units and instrument meaning needed to interpret values.

The data layer must be able to assemble a simulation snapshot without silently introducing information from later revisions or unpublished observations. Missing coverage, incomplete metadata, provider errors, and stale observations must remain distinguishable from a successful empty response.

The MVP fixture observation and snapshot shapes are selected in [temporal contracts](temporal-snapshots.md). Real provider APIs, historical fetch versus push ingestion, provider credentials, source terms, and persistent storage remain Open. Adapters do not create rights to access or redistribute source data.

## Model gateway and model services

The MVP request identifies the run, scenario, step, snapshot, model version, decision policy, eligible instruments, and inline data/portfolio snapshot. The correlated request and response shapes are in [model exchanges](model-exchanges.md).

A response must be attributable to that request and interpretable under the chosen model-output contract. Model identity/version information should be captured where available; an endpoint URL alone may identify a changing service.

One response may propose actions for multiple assets. Scores, targets, and order intents are the selected alternative output types; they are not interchangeable payloads. Stateful models and their reset/isolation behavior remain outside MVP 1.

Stateful models additionally require a declared session, reset, or isolation mechanism. Comparing scenarios with a shared mutable model state can contaminate results.

Malformed responses, timeouts, late responses, and duplicates fail visibly with correlation identity. MVP 1 does not retry inference or substitute a fallback decision.

## Orchestrator and engine

This is an internal Rust boundary. It does not require an HTTP service between the components.

The orchestrator supplies the scenario configuration, eligible events, and correlated decision groups. The engine supplies the portfolio snapshot needed for inference and returns admission outcomes, orders, fills, and state transitions. Both must agree on step identity and simulation-time semantics.

The engine must not reclassify transport failures as financial outcomes. The orchestrator must not bypass capital and execution rules. MVP event shapes are specified in [engine outcomes](engine-outcomes.md); local ordering and atomic group transitions are implemented under the synthetic lifecycle. This does not select distributed transaction semantics.

## Results and evidence

Results should connect experiment configuration to data snapshot/version identities, model outputs, admitted or rejected decisions, orders, fills, and portfolio outcomes. A researcher should be able to inspect both the aggregate result and the events responsible for a difference between scenarios.

Recorded-response replay and fresh inference must be distinguishable in result provenance. Report unavailable evidence rather than claiming complete reproducibility.

Result serialization and verified local run artifact reads are specified in [results and evidence](results-evidence.md). Replay/research references do not imply artifact endpoints for those resources. Persistent metadata storage, hosted artifact retrieval, retention, and deployment-wide redaction policy remain Open. Avoid credential-bearing URLs or unrestricted private-data references in result payloads.

## Contract acceptance checklist

The MVP contract decisions and shared examples are recorded in this interface set. The local MVP implementation has been checked across Rust producers/consumers, the typed Python SDK, schemas, and controlled fixtures. Any future contract change still needs coordinated compatibility evidence and tests; current verification does not establish a stable public API.

See the [architecture](../architecture/overview.md), [simulation lifecycle](../architecture/simulation-lifecycle.md), and [development workflow](../development/workflow.md).
