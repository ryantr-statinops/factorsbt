# Product overview

**Status:** Accepted product direction; implementation and MVP details remain open.

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

These are design goals, not claims of available features or superior performance.

## Research example

A researcher connects price history and timestamped news to a model API. At a simulation step, the model proposes increasing one asset, reducing another, and keeping a third unchanged. The engine evaluates the group against one portfolio snapshot, resolves capital/risk constraints under a declared policy, and simulates eligible orders.

The researcher then compares a price-only scenario with a price-and-news scenario and a delayed-news scenario. Each has separate mutable state. The results should explain both aggregate portfolio differences and the individual decisions affected by the data change.

This is an illustrative workflow. The assets, data schemas, model-output representation, and execution rules are not yet specified.

## Meaning of multi-model

One model issuing several decisions is a core use case. Comparing multiple models or policies is an extension of the same research workflow. Combining their outputs into one portfolio additionally requires an explicit selection, ranking, or aggregation policy; the system must not assume a default voting rule.

## Longer-term research directions

- Reranking candidate actions or policies before portfolio admission.
- Reinforcement learning with explicit observations, actions, rewards, and episode boundaries.
- Comparing decision policies across data conditions and execution assumptions.

These directions are Proposed. There is no accepted RL environment API, training loop, ranking algorithm, or promise of model improvement.

## Boundaries and open decisions

The initial scope is historical simulation infrastructure. Live trading, model hosting/training, hosted data distribution, and a managed cloud product are not accepted implementation commitments.

Before defining an executable MVP, choose the initial market, asset class, data provider, data types, and model-output contract. A candidate first slice is multiple assets sharing capital, price bars plus one event source, one model service, and inspectable results. See the [roadmap](../roadmap.md).
