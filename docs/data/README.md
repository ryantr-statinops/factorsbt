# First real-data research slice

**Status:** Accepted design; not implemented. The provider account entitlement and the user's applicable data-use classification have not been verified.

This page records the first historical-data slice selected for FactorsBT. It is a private research workflow, not a claim that the repository can redistribute market data or provide a commercial data service. Check the provider terms that apply to the account before downloading or retaining data. Keep credentials and non-redistributable source data outside Git.

## Selected scope

- **Market:** US listed equities.
- **Resolution:** One-minute OHLCV bars, regular trading session only.
- **Initial universe:** AAPL, MSFT, AMZN, GOOGL, and META.
- **Initial history:** The 20 most recent completed sessions available under the account's entitlement. Freeze the exact session dates and provider query details in each private dataset manifest; do not recompute the dates when replaying it.
- **Workflow:** A historical snapshot backtest through the Python SDK. The first slice is intraday-only and flattens positions at the final regular-session bar close using the configured execution costs. No position carries overnight.
- **Execution timing:** A market order may use the open of a bar only when that bar's start timestamp is strictly later than the decision timestamp. A decision at 09:31 ET cannot fill at the 09:31 bar open; the earliest eligible bar starts at 09:32 ET.
- **Historical availability:** Treat a bar as observed at its interval end for this research mode. The provider may later correct historical bars, so this mode does not reconstruct exactly what was published at each past instant. Preserve each downloaded snapshot and its ingestion time as a separate immutable dataset version.

The 20-session dataset is expected to contain about 39,000 regular-session bars for five symbols only when every session has 390 minutes. Early closes and genuine no-trade intervals make the actual count smaller; never synthesize missing bars by forward filling or by assuming every session has 390 bars.

## Provider and access boundary

[ADR 0004](../decisions/0004-massive-historical-rest.md) records the selected provider candidate and its access gate. The intended source is Massive Stocks Basic historical REST Custom Bars. Its published documentation describes end-of-day access for the free Basic tier, regular and extended-hours data in the endpoint, and a five-request-per-minute Basic limit. These published limits are not proof that a particular account can access the requested symbols or dates. Confirm entitlement with the account before implementation proceeds to online ingestion.

The applicable Massive market-data terms describe personal, non-business use and restrict redistribution. Consequently:

- Keep provider responses and private dataset manifests out of the repository unless separate written rights expressly allow publication.
- Use synthetic fixtures in committed tests and examples.
- Do not expose source bars or outputs that can reasonably recreate them to other users.
- If the intended use is organizational, commercial, or shared with other people, stop and obtain a license that expressly covers it before relying on this provider.

See [Massive Custom Bars](https://massive.com/docs/rest/stocks/aggregates/custom-bars), [Stocks Basic historical availability](https://massive.com/knowledge-base/article/on-the-free-basic-plan-when-can-i-pull-the-previous-trading-days-data), and the [Massive Market Data Terms](https://massive.com/legal/market-data-terms-of-service). Provider plans and terms can change; recheck them before ingestion.

## Decision details

- [Time, sessions, and availability](time-and-session.md) defines timestamp and session behavior.
- [Instrument identity and price adjustment](instrument-identity.md) defines raw-price and corporate-action boundaries.
- [Storage and benchmark](storage-and-benchmark.md) records the candidate local format and measured workload.

## Implementation boundary

The current Rust clock and coverage validation support a synthetic daily UTC calendar. They do not support exchange-local intraday sessions, DST, holidays, early closes, or minute-level execution eligibility. The existing strict v1 run schemas must remain unchanged. Add a separately versioned intraday contract and clock path in the planned D1a phase; do not silently change v1 behavior.

The D1a work items are listed in the [data implementation plan](../../plan/production/01-data-layer.md#phase-d1a-intraday-clock-and-execution-prerequisites). Backend/SDK wiring and end-to-end acceptance remain in D4. This document records a design; it does not mark a feature Implemented.
