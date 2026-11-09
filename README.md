# FactorsBT

FactorsBT is API-first infrastructure for multi-asset backtesting, synchronized data, and multi-model decision research. A pure Python SDK communicates over HTTP/JSON with a Rust backend and portfolio/execution engine.

**Status:** Implemented and locally verified for controlled synthetic workflows. This repository is for researchers tracing how available information and grouped decisions become orders, fills and portfolio outcomes, and for developers studying those component boundaries.

## Start here

Follow the [local quickstart](docs/development/quickstart.md) for prerequisites, package installation and a self-contained demo. Existing smoke scripts launch their own backend/model fixtures, exercise the SDK and installed CLI, and clean up temporary outputs. No market-data account or credentials are required.

Then use the [Python SDK guide](docs/development/python-sdk.md), [CLI guide](docs/development/cli.md), [scenario comparison guide](docs/development/comparison.md) and [local operations guide](docs/development/local-operations.md). Standalone SDK/CLI submissions require separately running compatible services; the package does not include a model-fixture launcher. The [documentation index](docs/README.md) is the complete reading map.

## Current capabilities and limits

- Controlled USD spot, linear futures and European/American options with declared synthetic accounting, margin, settlement and execution rules.
- Availability-aware versioned snapshots and correlated stateless HTTP model exchanges in scores, targets or order-intents mode.
- Asynchronous local runs with synchronous SDK polling, inspectable results and integrity-checked evidence/artifact reads.
- Sequential SDK scenario comparison and recorded-response replay using sealed inputs with the same executable digest and contract.
- Bounded fixed-policy reranking, fixed-action episodes and research replay; an installed run/replay CLI.

These are local synthetic capabilities, not real-market execution or pricing guarantees. Metadata and idempotency are process-local; sealed evidence is local filesystem data, not durable job recovery. There is no accepted hosted deployment, real-provider integration, public API stability, general RL training framework or PyPI release. See [product scope](docs/product/mvp-scope.md) and the [roadmap](docs/roadmap.md) before depending on a feature.

## Repository layout

| Directory | Purpose |
| --- | --- |
| [python-sdk](python-sdk) | Standard-library Python client, CLI, examples and tests. |
| [rust](rust) | Cargo workspace for contracts, data, engine and backend. |
| [contracts](contracts) | OpenAPI, JSON Schemas, shared examples and synthetic fixture datasets. |
| [docs](docs/README.md) | Product, architecture, interfaces, development and verification records. |
| [plan](plan/implementation/README.md) | Implementation stages, dependencies and historical delivery tracking. |
| [.agents](.agents/README.md) | Repository role guides and workflow skills. |

## Development and verification

Read [AGENTS.md](AGENTS.md), the relevant role guide and the [development workflow](docs/development/workflow.md) before changing behavior. Current setup and check commands live in the quickstart; the latest executed local checks are in the [regression verification record](docs/development/regression-verification-2026-11-09.md). The [acceptance mapping](docs/examples/mvp/acceptance-cases.md#verification-traceability) distinguishes focused tests from full scenario coverage.

Only describe behavior as Implemented when code and appropriate evidence support it. Preserve temporal visibility, shared-capital consistency, isolated mutable scenario state and the distinction between recorded replay and fresh inference. Use synthetic examples and never commit credentials or private datasets.

## License

**Open:** A repository license has not been selected or published. This README does not grant an open-source license. Resolve licensing before public distribution or reuse that depends on a license grant.
