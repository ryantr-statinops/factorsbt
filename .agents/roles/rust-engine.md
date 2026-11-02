# Rust engine role

## Responsibility

Own simulated time, decision application, portfolio accounting, and execution behavior. The backend coordinates jobs and external calls; the engine owns the financial state transitions inside a scenario.

Read the [simulation lifecycle](../../docs/architecture/simulation-lifecycle.md), [service contracts](../../docs/interfaces/service-contracts.md), and [architecture decision](../../docs/decisions/0001-python-sdk-rust-backend.md).

## Correctness expectations

- Admit data according to its availability at the simulation clock, not merely the period it describes.
- Assemble a consistent data and portfolio snapshot for each decision group. Resolve conflicting proposals and shared-capital limits using an explicit policy.
- Define event ordering, execution eligibility, and accounting conventions before relying on them. Input iteration order must not become an accidental allocation rule.
- Represent proposals, admitted orders, fills, and portfolio changes separately. Do not assume immediate or complete execution.
- Keep mutable portfolio, pending-order, model, and random state isolated between scenarios where such state exists.
- Record the inputs, configuration, versions, responses, and ordering information needed to explain a run. Scope reproducibility claims to what is actually captured.
- Use reproducible fixtures for time eligibility, accounting, and replay. Benchmark representative workloads before making optimization claims.

## Open decisions and handoff

Instrument conventions, numeric representation, corporate actions, fees, slippage, fills, and execution latency remain open. Obtain explicit definitions for the subset required by a task; document unsupported behavior.

Report the state transitions changed, financial assumptions, cross-scenario isolation implications, checks performed, and any limits on replay. Coordinate external inference and persistence requirements with the backend role.
