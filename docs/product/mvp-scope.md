# MVP 1: synthetic multi-instrument backtest

**Design status:** Accepted scope for the first implementation slice.

**Implementation status:** Phase B Stages 03–07 are implemented and locally verified for the controlled synthetic scope, including data snapshots, engine, model orchestration, Rust run API, typed Python SDK, and end-to-end acceptance flows. This is a local MVP verification result, not a public release, hosted service, or guarantee of real-market behavior. See the [Phase B verification record](../development/phase-b-local.md).

## User and outcome

The first audience is a quantitative researcher who wants to trace how a grouped model response affects a portfolio across synthetic instruments. The researcher configures a historical experiment, submits it through a pure Python client, and inspects decisions, orders, fills, and portfolio transitions from a Rust service.

MVP 1 is a controlled local demonstration of the end-to-end contract. It does not imply exchange-data accuracy, market access, investment performance, or support for trading.

## Included instruments

- **Spot:** long positions only.
- **Linear futures:** long and short contracts with a configured multiplier, expiry, fixed initial margin, and fixed maintenance margin.
- **European options:** long and short calls and puts, cash-settled at expiry.
- **American options:** long and short calls and puts, physically settled. Early exercise and assignment are recorded fixture events, not a simulated exercise strategy.

Each instrument has a stable ID, type, quote currency, price tick, quantity increment, multiplier, and type-specific fields. Options identify an underlying, strike, expiry, style, settlement method, and valuation mode. The initial fixture uses one USD quote currency and instruments on one shared synthetic daily calendar.

Options obligations from written contracts must be backed by reserved cash or underlying before admission. Spot borrowing and implicit FX conversion are unsupported. If a fixture attempts an exercise the account cannot fund, fail that run with a recorded error; do not manufacture a loan or short spot position.

## Market clock and fixtures

The synthetic calendar has one session every calendar day, with an open at 09:00 UTC and a close at 16:00 UTC. A completed daily bar is eligible at the close. A decision from that snapshot can execute no earlier than the following session's open.

The baseline universe contains two spot assets, one linear future, and four option contracts (European and American calls and puts). Options reference a configured spot underlying. Dataset examples are deterministic and synthetic. Price bars, option quotes, margin parameters, and fixture exercise/assignment events carry explicit identities and timestamps.

MVP 1 connects controlled data and a stateless local HTTP model fixture. No real provider or model account is required. The model receives the eligible data and portfolio snapshot inline.

## Model output and admission

Each run selects exactly one response type:

1. **Scores:** rank positive scores across the eligible universe, select the configured K, break ties by stable instrument ID, assign selected spot positions equal NAV weights of `1/K`, and use each selected derivative's configured target contract quantity. Nonpositive/unselected scores target zero. If no score is positive, all targets are zero.
2. **Targets:** specify the complete desired spot weight map and derivative contract map. A missing configured instrument means zero target. Spot weights are nonnegative and sum to at most one. Derivative contract quantities can be signed.
3. **Order intents:** specify at most one signed market order per instrument. Duplicate instruments or unsupported order types make the response invalid.

The engine evaluates each response group against one pre-application portfolio snapshot. If increases in exposure exceed available cash/collateral, it scales increases by one common factor, rounds quantity toward zero to each instrument's configured quantity increment, and records reductions. It never assigns more capacity based on instrument iteration order. Reductions/closures can proceed without a new exposure budget, subject to valid prices and lots.

## Accounting and execution assumptions

Amounts use USD. Fixture quantities/prices use decimal arithmetic; wire amounts are decimal strings. Ledger cash is rounded to USD cents using half-even. Quantities are rounded toward zero to their configured increments. Fees and slippage are configurable nonnegative amounts/rates per instrument; baseline values are zero. Controlled cases include nonzero fees and slippage. The fixture assumes full liquidity for admitted quantities.

Spot position value is quantity times mark price and multiplier. Futures use a configured per-contract fixed initial and maintenance margin with no portfolio offsets; daily settlement transfers variation profit/loss to cash. Options trade at their supplied quote. Written options have a per-contract margin requirement and a delivery reserve: short calls reserve deliverable underlying and short puts reserve exercise cash. Avoid double-counting the same cash for the same obligation.

European options settle at expiry by cash intrinsic value. American options settle through underlying delivery at expiry or at an explicitly supplied early-exercise/assignment fixture event. Valuation and settlement are separate operations. The European pricer is Black–Scholes; the American pricer is Cox–Ross–Rubinstein with configured step count. Pricer output is a valuation mark only. Any fill uses a quote/bar execution price from the controlled fixture, followed by configured fees and slippage.

After expiry and fixture exercise/assignment events, update positions, cash and reservations, then test maintenance margin at the daily close. A violation records a margin event, cancels pending orders, and schedules liquidation of the whole portfolio at the next session open. Continue the run after liquidation only if the resulting account is solvent and satisfies its obligations. If liquidation cannot be funded or does not restore a valid account, fail the run with its last ledger and an explicit reason. The simulation does not invent fills when required prices are missing.

At run end, cancel unfilled orders, report remaining positions and reservations, and do not force positions to zero. Cashflows or assignment that cannot settle without borrowing fail the run.

## Explicitly outside MVP 1

Real-market/provider integrations, exchange-specific calendars or margin, FX and multiple quote currencies, short spot, loans, cross-margin/netting, corporate actions, partial fills, limit/stop orders, borrow costs, futures/options volume/liquidity models, automatic early exercise, option Greeks or volatility-surface calibration, live trading, deployment beyond localhost, persistent job recovery, scenario comparison, replay, reranking, RL, and a CLI/console application.

## Acceptance outcome

MVP 1 is accepted for release planning when a local Python client can submit a run to the Rust backend and obtain inspectable output connecting configuration, versioned inputs, model response, grouped admission, orders, fills, cash/positions, and risk events. The run must use the assumptions above and synthetic fixtures. Phase A established the specification, contracts, buildable project skeleton, and health check; Phase B added the local Rust flow. The typed Python client and complete acceptance outcome are still required to meet this runtime criterion.

## Decision record

The product owner accepted this scope in the Phase A planning discussion. Instrument and simulation semantics are detailed in the linked architecture documents and must remain consistent with this scope. A change to scope requires an explicit update to this document and affected contracts before implementation depends on it.

For implementation details, see the [instrument model](../architecture/instrument-model.md), [simulation ordering](../architecture/simulation-lifecycle.md#accepted-mvp-1-synthetic-daily-ordering), [decision policies](../architecture/decision-policies.md), [accounting](../architecture/synthetic-accounting.md), [collateral and liquidation](../architecture/margin-risk.md), [option valuation](../architecture/options-valuation.md), and [controlled acceptance fixtures](../examples/mvp/acceptance-cases.md).
