# Development workflow

**Status:** Working guidance for implementation and documentation.

## Establish the task

Read [AGENTS.md](../../AGENTS.md), the [documentation index](../README.md), and the relevant [role guide](../../.agents/README.md). Inspect the current repository state and preserve unrelated work.

State the intended observable outcome, affected component boundaries, and acceptance criteria. Separate discoverable implementation facts from decisions the project has not made.

## Propose and resolve material choices

Use the documentation status vocabulary. A candidate design is Proposed until accepted; acceptance does not mean the feature exists.

For a behavior change, identify affected SDK, backend, adapter, engine, and result contracts. Resolve assumptions that materially affect correctness or interoperability before implementation depends on them. Significant architecture choices belong in an [ADR](../decisions/README.md).

For the first executable slice, this includes data availability, model-output representation, shared-capital admission, execution timing, and financial conventions. Open decisions are not permission to introduce hidden defaults.

## Implement within the selected slice

Keep responsibilities aligned with the [architecture](../architecture/overview.md). Avoid changing financial semantics through transport retries or adding infrastructure without a need in the selected scope. Keep client and service examples aligned with actual behavior.

When an interface changes, update both sides or document the compatibility transition. Select a versioning policy before promising backward compatibility. Update documentation and ADR status when decisions change.

This guide establishes no automatic commit, publication, deployment, or delegation workflow. Follow the user's authorized scope and active session rules.

## Verify using repository evidence

The repository has a Rust workspace and a standard-library Python SDK. Start with the [quickstart](quickstart.md) for current environment setup, dependency preparation and self-contained demos; use the [SDK guide](python-sdk.md), [comparison guide](comparison.md), [CLI guide](cli.md) and [local operations](local-operations.md) for usage. Data-fixture contribution steps are in the [fixture guide](../../contracts/fixtures/README.md).

Phase [A](phase-a-local.md), [B](phase-b-local.md), [C](phase-c-local.md), [D](phase-d-local.md) and [E](phase-e-local.md) records preserve the environment, commands, counts and limits observed at those milestones. Their counts describe historical runs, not a current supported-platform matrix or a requirement to rerun every suite for an editorial change. Distinguish a command recipe from evidence that it has actually passed.

When tooling changes, document verified prerequisites and commands. Add checks appropriate to observable behavior, concentrating on temporal visibility, portfolio transitions, scenario isolation, replay, and API failure semantics.

For documentation-only work:

- Check that relative links resolve and Markdown fences/sections are well formed.
- Check heading anchors as well as target files, and label service-dependent examples separately from self-contained demos.
- Check the consistent separation of Accepted, Proposed, Open, and Implemented.
- Review diagrams and prose for agreement about ownership and data flow.
- Inspect the Git diff, including newly created files, for accidental changes and secrets.
- Explain checks performed without claiming runtime verification.

## Handoff

Summarize the change, its purpose, verification results, and material limitations. Identify remaining decisions by their owning document. Report failures or unverified behavior honestly and keep credentials and private payloads out of the handoff.
