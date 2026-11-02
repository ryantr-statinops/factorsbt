# Architecture decision records

ADRs capture decisions that shape component boundaries, simulation semantics, interoperability, or long-lived operational constraints. They explain why a choice was made and what follows from it.

## Decision index

| ADR | Status | Decision |
| --- | --- | --- |
| [0001](0001-python-sdk-rust-backend.md) | Accepted | Pure Python SDK, Rust backend/engine, initial HTTP/JSON communication |

## When to write an ADR

Use an ADR for a material choice with alternatives and consequences: for example, model-output representation, execution conventions, persistence, or public contract versioning. Routine implementation details can remain in the relevant guide or change description.

Use the next unused four-digit number and a descriptive filename. Numbering identifies the record; it does not imply priority. The current index does not imply a separate ADR is required for every open question.

## Record structure

Each ADR should contain a title/number, status, context, decision or proposal, alternatives considered, consequences, and links to related material. An Accepted record describes an agreed direction; implementation progress belongs in code evidence and the roadmap.

Use Proposed for an undecided choice, Accepted for an agreed choice, and Superseded when a later ADR replaces it. Identify the replacing ADR explicitly. Preserve the previous context rather than rewriting history as though the old decision never existed.

An agent must not label a new architecture choice Accepted merely because it authored the record. Acceptance requires an explicit project decision or existing authorization that settles the choice.

## Current boundary

Only the technology and communication direction in ADR 0001 is accepted here. Frameworks, deployment, data/model contracts, financial rules, and infrastructure choices remain open in the [architecture](../architecture/overview.md), [contracts](../interfaces/service-contracts.md), and [simulation lifecycle](../architecture/simulation-lifecycle.md).
