# MVP 1 synthetic cash and position accounting

**Status:** Accepted for the synthetic MVP only. No accounting engine is implemented yet.

This describes the controlled USD ledger used by MVP 1. Instrument configuration is defined in the [instrument model](instrument-model.md), while margin and delivery risk are detailed separately.

## Numeric representation

Wire amounts, prices, margins, fees, and fractional spot quantities are decimal strings. Do not parse financial quantities into binary floating point. Invalid, non-finite, out-of-range, or arithmetically overflowing decimals are rejected with a typed error before they change state.

The USD cash ledger is rounded to two decimal places using half-even after every cash posting. Instrument quantities are rounded toward zero to the configured `quantity_step` after each executed order; derivatives use integral contract quantities. Price rounding uses the configured price tick and is adverse to the portfolio on a fill: buys round up and sells round down. A fill whose adjusted price is missing, nonpositive, or outside the available quote fails the run without an invented price.

Decimal strings use a plain base-10 form without exponent notation or leading zeroes. Limit them to 28 significant digits and at most 18 fractional digits; a leading minus sign is allowed, positive signs and whitespace are not. Normalize negative zero to zero. Python's `decimal` and the Rust decimal type must reject inputs beyond these limits identically. Overflow during arithmetic is a typed run error; never clamp a value silently.

## Cash postings

Let `q` be signed executed quantity, `p` the tick-adjusted fill price, and `m` the contract multiplier.

- Spot purchase/sale: `cash_delta = -q × p × m - fees`. Signed negative quantity therefore adds sale proceeds, subject to the no-short-spot rule.
- Option purchase/sale: `cash_delta = -q × p × m - fees`. A short option has negative `q` and receives premium, then carries a negative signed option mark in equity.
- Future entry: no notional principal is exchanged. The configured contract margin is reserved. Closing a signed position releases its margin and posts realized variation `q × m × (p - previous_settlement_price)`, where `q` is the signed open quantity and `p` is the close fill. Fees are posted separately.
- Future daily variation settlement: `cash_delta = signed_open_contracts × multiplier × (today_settlement_price - previous_settlement_price)`. On the entry session, use entry price as the previous mark. After settlement, store today's settlement as the new previous mark so variation is not posted twice.
- Exercise/delivery, option expiry payoff, fees, and forced-liquidation proceeds are posted as separate, identified ledger events.

All postings reference run, scenario, event, instrument, and simulation time where applicable. A failed event leaves the last valid state and records why the new state was not applied.

## Equity and positions

At each close after futures settlement:

`NAV = cash + Σ(spot quantity × spot close mark × multiplier) + Σ(signed option contracts × option close mark × multiplier)`

Settled future positions have zero additional carrying value after variation is posted to cash. Before settlement, do not both include unrealized future P&L in NAV and post the same variation to cash. Delivery reservations restrict buying power but do not remove owned cash or underlying from the marked ledger value. Report cash, marked positions, reserved cash, reserved underlying, initial requirement, maintenance requirement, and free buying power separately.

Option close mark uses either the current controlled quote mark or the configured theoretical valuation mark. This selection is part of run configuration and provenance. A theoretical mark never changes fill price. Missing close marks required to calculate NAV fail the run; no carry-forward or last-known substitution is made.

## Fees and slippage

An instrument may configure `fee_bps`, `fee_per_unit`, and `slippage_bps` as finite nonnegative decimals. Zero is the baseline. Fee calculation is `abs(q × adjusted_price × multiplier) × fee_bps / 10000 + abs(q) × fee_per_unit`, rounded once to cents using half-even. Slippage adjusts a quote adversely by `slippage_bps / 10000` before rounding to tick. Do not apply a hidden spread or extra cost. The fixture supplies sufficient liquidity; fees and slippage do not model impact or partial execution.

## Hand-checkable examples

1. Starting with USD 100,000, buy 10 spot units at USD 100 with multiplier 1 and zero fees. Cash becomes USD 99,000, spot value is USD 1,000, and NAV remains USD 100,000.
2. Buy 2 futures at 100 with multiplier 5 and no fees. No notional principal changes cash. If the settlement mark is 110, post USD 100 variation to cash once. An initial-margin requirement of USD 600 per contract reserves USD 1,200 but is not an expense or a NAV subtraction.
3. Buy 2 option contracts at premium USD 3 with multiplier 10 and zero fees. Cash falls by USD 60; the opening mark is USD 60, so NAV initially does not change. A short position receives that premium and begins with a USD 60 mark liability.

## Cost and verification boundaries

The examples above intentionally use synthetic figures. Do not use them as contract specifications or predictions. Rounding and settlement are part of the public result contract: save cash postings and pre/post balances so an acceptance test can independently recompute each transition. Collateral, margin tests, delivery, and liquidation are defined in the [synthetic risk rules](margin-risk.md).
