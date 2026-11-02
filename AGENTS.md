# FactorsBT agent instructions

FactorsBT is API-first infrastructure for multi-asset backtesting, synchronized data, and multi-model decision research.

## Current state and accepted direction

The repository currently contains foundation documentation. There is no implemented SDK, backend, simulation engine, or executable API contract. Treat capabilities described in the docs as intended behavior unless implementation evidence says otherwise.

The accepted direction is a pure Python SDK communicating with a Rust backend and Rust backtest engine over HTTP/JSON. External data providers and model services connect through adapters. See [ADR 0001](docs/decisions/0001-python-sdk-rust-backend.md).

## Reading and authority

1. Read the [documentation index](docs/README.md) and the documents relevant to the task.
2. Consult the matching [role guide](.agents/README.md); use multiple guides when a change crosses boundaries.
3. Check applicable instructions in the directories you change.

Role guides are working instructions, not an automatic agent launcher. They do not authorize delegation or external actions. Follow the user's authorized scope and the host's instruction hierarchy. Keep shared repository rules here rather than duplicating them across role guides.

## Design and implementation rules

- Keep SDK ergonomics, backend orchestration, and engine simulation responsibilities separate.
- Preserve the distinction between event time, data availability time, and actual service processing time.
- Treat decisions, orders, and fills as separate concepts. Evaluate a decision group against a consistent snapshot before resolving shared-capital constraints.
- Isolate mutable state between backtest scenarios. Share immutable data only when its identity and version are explicit.
- Distinguish replaying recorded responses from invoking a model again; a fresh call may return a different result.
- Do not silently choose open product or financial assumptions. Record proposals and resolve decisions material to the requested implementation.

## Contracts and documentation

- Use the status vocabulary in [the documentation index](docs/README.md): Accepted, Proposed, Open, and Implemented.
- Describe interface changes and their effects on callers, adapters, and stored results. Do not present conceptual contracts as stable endpoints or schemas.
- Update the relevant documentation with behavior changes. Record significant architecture decisions as ADRs; preserve the history of superseded decisions.
- Keep examples clearly labeled as conceptual until they match an executable interface.
- Do not invent build commands, supported versions, performance claims, provider guarantees, or completed features.

## Verification and handoff

No build, test, lint, or formatting commands have been established yet. Discover actual manifests and tooling before choosing commands, and document verified commands when tooling is introduced.

For documentation changes, check relative links, Markdown structure, terminology, status labels, and the Git diff. For code changes, run checks appropriate to the behavior and available tooling; report what ran, the result, and any unverified assumptions. Avoid tests that merely duplicate the implementation.

Summarize what changed, why, and how it was checked. Preserve unrelated work. Do not add dependencies, infrastructure, commits, or other changes outside the user's task scope.

## Credentials and external systems

Never commit real API keys, tokens, credentials, or private datasets. Use synthetic examples and secret references. Avoid exposing credentials or confidential payloads in logs, errors, documentation, and review output. Explain required access without choosing an authentication mechanism that has not been accepted.
