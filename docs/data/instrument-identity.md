# Instrument identity and price-adjustment boundary

**Status:** Accepted design for the first US equity slice; not implemented.

## Instrument identity

The initial symbols are AAPL, MSFT, AMZN, GOOGL, and META, all listed on Nasdaq. For this slice, identify each instrument by a versioned mapping containing:

- a FactorsBT instrument identifier derived from the market, exchange MIC (`XNAS`), and listing symbol;
- the provider ticker used in the request;
- listing exchange and quote currency (`USD`);
- mapping version and effective dates, where known.

Do not treat a ticker by itself as a permanent security identity. If the provider symbol, issuer, or listing mapping changes, create a new mapping/dataset version and preserve the old identity for replay. Resolving a permanent issuer/security identifier and general symbol-change history is outside the five-symbol slice and remains open for the broader data layer.

Prices must retain the exact decimal text returned by the provider until normalized into the versioned dataset format. Do not pass source prices through binary floating-point and then serialize them as if exact. Bar currency is USD. The first slice does not select a universal minimum tick, lot size, fractional-share policy, or transaction-cost schedule; execution configuration must provide those assumptions explicitly and the execution workstream must resolve them before claiming realistic fills.

## Raw prices and corporate actions

Request `adjusted=false` explicitly. Massive Custom Bars defaults to split-adjusted aggregates; relying on that default would make results depend on an implicit provider setting. Record the requested adjustment flag and provider response identity in the private manifest.

The first slice consumes raw OHLCV bars. It does not synthesize a total-return series, retrospectively adjust bars, post dividend cashflows, or apply split quantity adjustments. Positions and pending orders are flattened/cancelled by each session close, and signal state resets at each session boundary. Strategy signals must not compare raw price levels across sessions. Results may aggregate realized cash/equity performance across sessions, but must not label that aggregation a total-return index.

Capture corporate-action metadata relevant to the requested symbols and date range when available, so a later run can explain unusual session boundaries. A session with unresolved instrument identity or an action that makes its raw bars ambiguous must be rejected or explicitly excluded with a recorded reason; do not silently rewrite its prices. Adding split/dividend adjustments and cross-session price features requires a separate decision and tests in D3.

## Scope exclusions

- Options and futures are not in this slice; futures expiry and contract rollover do not apply.
- Multi-currency conversion is not in scope; USD is the quote and account currency for this initial run.
- Corporate-action-adjusted historical series, dividend reinvestment, and overnight holdings are not supported by this design.
- Tick size, fractional shares, lot constraints, fees, and slippage values are not selected here. Runs must identify their explicit values; no zero-cost or universal default is implied.

See [Massive Custom Bars](https://massive.com/docs/rest/stocks/aggregates/custom-bars) for the `adjusted` parameter behavior and the [first-slice provider decision](../decisions/0004-massive-historical-rest.md) for data rights and acquisition constraints.
