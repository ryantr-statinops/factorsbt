# MVP 1 synthetic collateral, exercise, and liquidation

**Status:** Accepted for the synthetic MVP only. Risk controls and settlement code are not implemented yet.

This document defines fixed, transparent controls for synthetic fixtures. It does not model exchange/broker risk parameters or guarantee real-world solvency. Read it with the [instrument model](instrument-model.md), [synthetic accounting](synthetic-accounting.md), and [accepted MVP scope](../product/mvp-scope.md).

## Initial and maintenance requirements

For each open future, calculate initial requirement as absolute contracts × configured `initial_margin_per_contract`; calculate maintenance requirement using the corresponding maintenance amount. Sum these amounts across contracts. The MVP does not offset related or opposite positions.

For each written option, calculate the same per-contract initial and maintenance amounts. A short European cash-settled option adds only its configured margin reserve. A short American physical call also locks its delivery quantity in underlying. A short American physical put must reserve the cash required for exercise at strike × multiplier × contracts. For a cash-secured put, cash collateral is at least the configured margin amount and the exercise cash requirement; do not count that same cash twice. A covered-call delivery reserve is separate underlying. Multiple short calls may not reserve the same underlying twice.

Initial funds must be sufficient after fills and fees to cover initial requirements, delivery reserves, and any outstanding buying-power commitments. Run admission uses the entire proposed group and the shared-capital rule. A later increase from market movement does not create funds or change the configured margin amount.

At daily close, after futures variation settlement, option expiry, and scheduled exercise/assignment, calculate NAV, free cash, initial/maintenance requirements, and delivery reserves. Trigger a margin-breach event if any of these is true:

- Cash is negative after contractual cashflows and fees.
- Available cash cannot cover reserved cash obligations.
- NAV is below the summed maintenance requirement plus any cash delivery obligations not already included in that requirement.
- A required underlying delivery is no longer held and reserved.

Do not double-subtract a cash-secured put reserve from both the margin requirement and delivery requirement. Exclude locked underlying from capacity to open further deliveries, but continue to show its market value in portfolio NAV. The event stores the calculation inputs, pre/post ledger balances, instrument quantities, initial/maintenance amounts, reserves, and exact violated condition.

## Exercise and assignment events

An `exercise` fixture event applies only to a positive long American option position. An `assignment` event applies only to a negative short American option position. Each carries a unique event ID, option instrument ID, positive integer contract count, and timestamp at the synthetic close; event quantity cannot exceed the eligible position. European options may not receive these physical exercise/assignment events.

For `n` contracts, multiplier `m`, and strike `K`:

- Call exercise by a long: cash decreases by `n × m × K`; underlying increases by `n × m`.
- Call assignment to a short: cash increases by `n × m × K`; underlying decreases by `n × m` from the pre-reserved holding.
- Put exercise by a long: cash increases by `n × m × K`; underlying decreases by `n × m` from an available holding.
- Put assignment to a short: cash decreases by `n × m × K`; underlying increases by `n × m`.

Remove the corresponding option quantity in the same atomic transition. Release only the collateral belonging to the contracts that were exercised/assigned; retain reservations for remaining short contracts. Missing event references, duplicate event IDs, ineligible quantities, or insufficient funds/underlying fail the run without a partial state change.

At expiry, apply provided American physical events, then settle all remaining European options using cash intrinsic value `n × m × max(S-K, 0)` for calls or `n × m × max(K-S, 0)` for puts, multiplied by the signed open option quantity. American contracts remaining after their final eligible close event expire worthless if out of the money; in-the-money contracts require an explicit exercise/assignment event or the run fails as an incomplete settlement input. No automatic exercise policy is inferred from option valuation or intrinsic value.

## Forced liquidation

On a close-time breach:

1. Record a `margin_breach` event with the calculations above and cancel all pending orders.
2. Freeze new model calls, target changes, and order admission for the breached close.
3. Schedule one whole-portfolio liquidation for the next synthetic open. Do not queue replacement orders.
4. At that open, use available instrument open execution quotes, close spot and derivative positions as one batch, apply normal fees/slippage and release reservations as each position is closed. Do not choose liquidation order by asset iteration.
5. If every position is closed and the resulting ledger has nonnegative cash and no unsettled delivery, record `liquidation_completed`; resume at that day's close. Otherwise record `liquidation_failed`, retain the last valid balances, and fail the run.

Missing open quotes, impossible delivery, or an insolvent post-liquidation account cannot be patched with a synthetic fill, extra capital, or implicit borrowing. The failure result reports the affected positions and required versus available funds.

## Controlled verification cases

- A future loses value at settlement; cash changes by quantity × multiplier × daily price change exactly once.
- Maintenance margin breach records the input ledger, cancels queued market orders, and liquidates all positions at the next open before the next model request.
- Sufficient opening prices restore a valid account and the run resumes; missing quote, remaining delivery obligation, or negative cash produces a failed run with the final reason.
- An option short call cannot open without distinct available underlying for every physical contract; the same shares cannot back two calls.
- An option short put cannot reserve the same cash twice; assignment produces the configured strike cashflow and underlying quantity atomically.
- Long exercise without the needed underlying/cash fails without partially updating option, spot, cash, or collateral.
- Every cash/underlying release is reconciled against remaining positions; no expiry or assignment silently abandons an obligation.

## Extensions outside MVP 1

Cross-margin, exchange margin schedules, netting, margin offsets, intraday calls, partial liquidation, liquidation priority, borrow/loan products, automatic exercise, and broker-specific default handling need separate accepted contracts and evidence.
