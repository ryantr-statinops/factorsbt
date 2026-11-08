# MVP 1 instrument model

**Status:** Accepted synthetic configuration rules, Implemented locally in shared contracts, SDK builders, and Rust validation/engine behavior. The [Phase B verification record](../development/phase-b-local.md) bounds the evidence; these rules do not establish real-market coverage.

See the [accepted MVP scope](../product/mvp-scope.md). This document defines type-specific configuration and validation; shared time, accounting, and valuation rules remain in their owning documents.

## Common instrument fields

Every instrument has:

- A stable `instrument_id` matching `^[A-Z][A-Z0-9_-]{0,31}$`.
- An `instrument_type` discriminator and `quote_currency`, which is `USD` in MVP 1.
- A positive `price_tick`, positive `quantity_step`, and positive `contract_multiplier`, all represented as decimal strings.
- Exactly one instrument definition per ID in a run. References must resolve within that same run.

Reject unknown instrument types, duplicate IDs, unsupported currencies, non-finite/invalid decimal strings, non-positive ticks/steps/multipliers, or unresolved references before accepting a run.

## Spot

Spot instruments require common fields only. The engine supports non-negative holdings. A negative target or an order that would open a short spot position is invalid. Do not silently add borrowing or margin.

The fixture uses a `quantity_step` of `0.001` for both spot instruments. Each bar quote uses the configured `price_tick` and `contract_multiplier`.

## Linear future

The one MVP future has:

- An underlying reference to a configured spot instrument.
- A required `expiry_at` on the fixture calendar.
- Positive `initial_margin_per_contract` and `maintenance_margin_per_contract`, denominated in USD per contract; maintenance cannot exceed initial margin.
- A signed integer contract position. Futures are linear: one contract changes P&L by price change × multiplier × signed quantity.

There is no calendar spread, netting, portfolio offset, exchange margin schedule, variation intraday, or physical delivery. Post one daily variation settlement at the fixture close relative to the prior settlement mark (or entry price on the first settlement). At expiry, settle the final variation at the fixture close, then remove the contract. If no settlement price is available, fail the run and retain the last valid ledger.

The fixture uses margin values in the instrument definition rather than asserting a real futures contract's margin.

## Options

Each option identifies a supported spot underlying and has:

- `option_type` (`call` or `put`), positive `strike`, and a positive expiry on the fixture calendar.
- `exercise_style` (`european` or `american`) and `settlement_style` (`cash` or `physical`); MVP 1 accepts only `european + cash` and `american + physical`.
- A valuation selection: use the instrument's supplied quote mark or the matching built-in theoretical pricer. Execution always comes from fixture quote/bar data.
- A fixed positive `initial_margin_per_contract` and `maintenance_margin_per_contract` for a written option, in USD per contract, with maintenance no greater than initial. There is no volatility-based exchange/broker margin.

The contract multiplier scales premium, intrinsic value, and delivery quantity. Long and short contract positions are allowed. Exercise/assignment events must reference an existing American option, occur no later than expiry, use a positive integer quantity no greater than the eligible position, and have a unique event ID. Events for European options or already closed/expired quantities fail with a run error.

For short physical delivery obligations, reserve the underlying quantity for short calls and strike × multiplier × contracts in cash for short puts. Long exercise events must also have the cash/underlying needed to settle. The engine may not create an implicit loan or negative spot holding to complete delivery.

## Fixture inventory

The controlled universe is:

| ID | Type | Contract detail |
|---|---|---|
| `SPOT_A` | Spot | Synthetic quote series, step `0.001` |
| `SPOT_B` | Spot | Synthetic quote series, step `0.001` |
| `FUT_A` | Linear future | References `SPOT_A`; fixed configured margins and expiry |
| `EU_CALL_A` | European call | Cash settlement; references `SPOT_A` |
| `EU_PUT_A` | European put | Cash settlement; references `SPOT_A` |
| `AM_CALL_A` | American call | Physical settlement; references `SPOT_A` |
| `AM_PUT_A` | American put | Physical settlement; references `SPOT_A` |

The fixture quantity step for derivatives is one contract. Fixture tick, multiplier, strike, margin, expiry, valuation inputs, and quotes are explicit values in the test data. They are synthetic constants, not claims about listed contracts.

## Compatibility

An `instrument_type` or option settlement combination outside the accepted set is rejected at run validation. Adding an instrument type requires a versioned contract change, fixture cases for its accounting and lifecycle, and an explicit implementation decision; consumers must not infer new semantics from shared fields alone.
