# Service contracts

**Status:** Proposed conceptual contracts. HTTP/JSON is Accepted as the initial boundary transport. No endpoint paths, field names, schemas, status codes, or compatibility guarantees are frozen.

This document defines the information and responsibilities that interfaces must cover. Entries below are semantic requirements for design, not executable request/response specifications.

The [MVP 1 HTTP API proposal](mvp-http-api.md) now selects the initial operation paths and run lifecycle for contract work. Payload schemas, error codes, and compatibility rules are being defined in Stage 01. These proposals do not make the run endpoints implemented or stable.

The current request shape and type-specific instrument requirements are summarized in [MVP run and instrument configuration](instrument-configuration.md); the JSON Schema files under `contracts/schemas/` are the wire-shape authority.

Data observations and inline, point-in-time snapshots are specified in [MVP observations and decision snapshots](temporal-snapshots.md). Their availability and portfolio-consistency checks are semantic validation, not properties guaranteed by JSON Schema alone.

The correlated inference request and three response modes are specified in [MVP model exchanges](model-exchanges.md). A response is a proposal that still passes engine admission; it is not an order or fill.

Internal engine outputs are event records specified in [MVP engine outcomes](engine-outcomes.md), keeping admission, orders, fills, settlements, ledger postings, and risk actions independently traceable.

Successful run summaries, provenance, and local artifact references are specified in [MVP results and evidence](results-evidence.md). Failed or incomplete runs remain operational status outcomes, not successful results.

## SDK and backend

The SDK should let a user configure an experiment, submit work, inspect progress, and retrieve results. The backend owns run identity and operational lifecycle; the engine owns simulated financial state.

| Exchange | Information to account for |
| --- | --- |
| Run configuration | Asset universe, simulation interval, data sources, model references, initial portfolio/capital, and declared simulation policies |
| Submission acknowledgment | Run identity and enough information to distinguish accepted work from a rejected request |
| Run inspection | Current operational phase, useful progress context, and failure details when applicable |
| Results | Outcome summary and access to associated evidence and configuration |

Submission, waiting, and execution failure are different outcomes. A lost HTTP response does not establish whether a run was created. Define submission identity/deduplication and retry behavior before making retries automatic.

Exact lifecycle states, polling or streaming, cancellation, synchronous/asynchronous client behavior, authentication, and result pagination are Open.

## Data adapters and normalized data

Data adapters translate provider-specific material into the forms accepted by the data layer. Source observations need asset/context identity, event time, availability information, origin, and version/revision context where available. Retain units and instrument meaning needed to interpret values.

The data layer must be able to assemble a simulation snapshot without silently introducing information from later revisions or unpublished observations. Missing coverage, incomplete metadata, provider errors, and stale observations must remain distinguishable from a successful empty response.

Source APIs may vary. Historical fetch versus push ingestion, the first supported payload types, schema normalization, timestamp encoding, provider credentials, source terms, and storage layout remain Open. Adapters do not create rights to access or redistribute source data.

## Model gateway and model services

An inference request must identify the scenario and simulation step, provide the relevant data/portfolio snapshot or an authorized reference, and communicate the constraints the model is expected to observe. Large inputs need not be repeated inline; snapshot-reference access and lifetime require a separate decision.

A response must be attributable to that request and interpretable under the chosen model-output contract. Model identity/version information should be captured where available; an endpoint URL alone may identify a changing service.

One response may propose actions for multiple assets. Prediction outputs, target weights, and order intents are alternative contracts with different conversion needs, not interchangeable payloads. The first representation is Open.

Stateful models additionally require a declared session, reset, or isolation mechanism. Comparing scenarios with a shared mutable model state can contaminate results.

Malformed responses, timeouts, late responses, duplicate responses, and failed services require explicit handling. Response correlation and application must prevent an output from affecting the wrong step or being applied twice. Retry limits and fallback behavior have not been selected.

## Orchestrator and engine

This is an internal Rust boundary. It does not require an HTTP service between the components.

The orchestrator supplies the scenario configuration, eligible events, and correlated decision groups. The engine supplies the portfolio snapshot needed for inference and returns admission outcomes, orders, fills, and state transitions. Both must agree on step identity and simulation-time semantics.

The engine must not reclassify transport failures as financial outcomes. The orchestrator must not bypass the engine's capital and execution rules. Internal types, scheduling, conflict resolution, and mutation/transaction boundaries remain Open.

## Results and evidence

Results should connect experiment configuration to data snapshot/version identities, model outputs, admitted or rejected decisions, orders, fills, and portfolio outcomes. A researcher should be able to inspect both the aggregate result and the events responsible for a difference between scenarios.

Recorded-response replay and fresh inference must be distinguishable in result provenance. Report unavailable evidence rather than claiming complete reproducibility.

Metric definitions, serialization, artifact access, storage, retention, redaction, and compatibility/versioning are Open. Avoid credential-bearing URLs or unrestricted private-data references in public result payloads.

## Contract acceptance checklist

Before implementing an interface, select the required representation, validation/error behavior, identifiers, time conventions, retry ownership, and verification examples for that slice. Record accepted choices and coordinate both sides of the boundary. Stable API claims require implemented and tested behavior.

See the [architecture](../architecture/overview.md), [simulation lifecycle](../architecture/simulation-lifecycle.md), and [development workflow](../development/workflow.md).
