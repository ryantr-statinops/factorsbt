# Product overview

**Status:** Accepted product direction and [MVP 1 synthetic scope](mvp-scope.md). The controlled local MVP and bounded Phase C–E extensions are Implemented with local verification: sequential SDK comparison, recorded replay, research jobs, and a run/replay CLI. Real-provider integrations, hosted service, and public API stability remain outside this verified scope.

FactorsBT is API-first infrastructure for multi-asset backtesting, synchronized data, and multi-model decision research.

## Problem

Decision research often spans market prices, irregular events, financial disclosures, external models, and portfolio rules. Connecting these pieces requires more than fetching tables or running independent asset-level simulations. Researchers need to know what information was visible at a historical moment, how simultaneous proposals competed for shared capital, and how those proposals became actual simulated positions.

FactorsBT is intended to coordinate that process through API boundaries while making decisions and their outcomes inspectable.

## Intended users

- Quant researchers evaluating signals, policies, and portfolio behavior.
- ML engineers connecting inference services to historical simulation.
- Infrastructure developers integrating data providers and model services.

These audiences are an initial product hypothesis. Specific market coverage and provider integrations have not been chosen.

## Intended capabilities

- Connect external data sources and model services through adapters.
- Align heterogeneous data by simulated availability, including multiple update frequencies.
- Let one model produce a group of decisions across multiple assets in a shared-capital portfolio.
- Support experiments comparing model, data, allocation, and execution configurations in isolated scenarios.
- Preserve enough evidence to inspect the chain from data to decision, order, fill, and portfolio result.
- Provide a Python SDK for interacting with a Rust backend and simulation engine.

The synthetic data/model-to-portfolio path and sequential comparison of independent runs are implemented locally. [Recorded replay](../interfaces/recorded-replay.md) verifies sealed evidence without fresh inference. Provider integrations remain design goals; no performance claim follows from these local workflows.

## Research example

A researcher connects price history and timestamped news to a model API. At a simulation step, the model proposes increasing one asset, reducing another, and keeping a third unchanged. The engine evaluates the group against one portfolio snapshot, resolves capital/risk constraints under a declared policy, and simulates eligible orders.

The researcher then compares a price-only scenario with a price-and-news scenario and a delayed-news scenario. Each has separate mutable state. The results should explain both aggregate portfolio differences and the individual decisions affected by the data change.

Real price/news integration in this example remains illustrative. The controlled implementation compares baseline, price-only, and delayed-event synthetic fixtures; see the [Phase C record](../development/phase-c-local.md). It does not imply real-market coverage or causal attribution from observed differences.

## Meaning of multi-model

One model issuing several decisions is a core use case. Comparing multiple models or policies is an extension of the same research workflow. Combining their outputs into one portfolio additionally requires an explicit selection, ranking, or aggregation policy; the system must not assume a default voting rule.

## Longer-term research directions

- Reranking candidate actions or policies before portfolio admission.
- Reinforcement learning with explicit observations, actions, rewards, and episode boundaries.
- Comparing decision policies across data conditions and execution assumptions.

The [local research interface](../interfaces/research-jobs.md) implements a fixed turnover-penalty reranker and two-action episodes over registered synthetic fixtures. General ranking objectives, interactive RL environments, policy training, and any promise of model improvement remain Proposed directions, not delivered capabilities.

## Boundaries and open decisions

The initial scope is historical simulation infrastructure. Live trading, model hosting/training, hosted data distribution, and a managed cloud product are not accepted implementation commitments.

Real-market coverage, source providers, and model-output contracts remain open beyond the accepted synthetic first slice. The [roadmap](../roadmap.md) distinguishes the bounded MVP from later provider and service decisions.

For the accepted first implementation slice, those choices are intentionally bounded to configured synthetic instruments, controlled historical bars/quotes, a stateless local model fixture, and inspectable run evidence. That slice is implemented and verified locally. See the [MVP 1 scope](mvp-scope.md), its [acceptance cases](../examples/mvp/acceptance-cases.md), and the [Phase B verification record](../development/phase-b-local.md).
