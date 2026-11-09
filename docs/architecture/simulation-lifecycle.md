# Simulation lifecycle

**Status:** Accepted lifecycle requirements and synthetic MVP ordering. The local engine/orchestrator implement the bounded rules linked below; real-market conventions and stateful provider/model extensions remain Open.

## Time and data visibility

Distinguish three kinds of time:

- **Event time:** when an observation occurred or the period it describes.
- **Availability time:** when that version of the observation could have been used under the declared historical-access assumption.
- **Processing time:** when FactorsBT or an external service actually handles a request.

A simulation at time T may expose only data whose declared availability is at or before T. A quarter ending in March does not make its financial report visible in March if publication occurs in April. Revisions also need version-specific availability; a current corrected value must not silently replace what was knowable historically.

Missing publication or availability metadata must be identified. An approximation needs a disclosed assumption before use. Provider timestamps alone do not establish the quality of a point-in-time dataset.

## One decision cycle

1. **Admit eligible data.** Advance according to the chosen trigger and event-order policy. Bring forward only information available to this scenario at the simulation clock.
2. **Build a snapshot.** Capture eligible observations and current portfolio state. Attach identities sufficient to link later outputs to these inputs.
3. **Invoke the model.** Send the snapshot or an authorized reference, simulation context, and relevant constraints through the model adapter.
4. **Validate the response.** Correlate it with the correct run, scenario, step, and snapshot. Reject incompatible or stale output according to an explicit error policy.
5. **Evaluate a decision group.** Interpret proposals against the same pre-application state, then resolve conflicts, capital, and risk limits using the declared policy.
6. **Create and execute orders.** Convert admitted proposals into orders and apply the chosen execution, fee, slippage, and latency rules. A proposal is not a guaranteed fill.
7. **Update and record.** Apply fills and other admitted portfolio events, record the transition, and continue until the configured end condition.

The cycle describes logical responsibilities. The accepted synthetic open/close ordering below is implemented for the MVP. Pending orders execute at a later open; a different scheduler or market calendar requires its own explicit ordering policy.

## Multiple data frequencies

Price bars, irregular news, and quarterly disclosures need not have identical timestamps or row counts. A snapshot represents what is known at one simulated moment. Any carry-forward, freshness limit, missing-data handling, or decision-trigger rule must be explicit for the selected data types.

A bar's closing information is unavailable before the bar completes. Whether a decision using that bar can execute at a particular price depends on the declared execution model; it must not receive an impossible historical fill.

## Decisions, orders, and fills

- A **decision** is a model or policy proposal. MVP 1 selects scores, targets, or market order intents under the [model exchange contract](../interfaces/model-exchanges.md); other representations require a separate conversion policy.
- An **order** is an instruction admitted by portfolio/execution rules. Predictions require a defined conversion policy before becoming actionable.
- A **fill** is a simulated execution event that changes positions and cash under the selected accounting rules.

Asset iteration order must not determine capital allocation. The synthetic MVP selects grouped admission, market orders, full execution of affordable quantities, explicit cash/margin/reserves, and declared rounding. See [decision policies](decision-policies.md), [accounting](synthetic-accounting.md), and [margin risk](margin-risk.md). Partial execution and other order types are unsupported, not hidden defaults.

## Multiple scenarios and models

Every scenario owns its evolving state. Shared market inputs do not imply identical model inputs when holdings, capital, history, or model state differ. External stateful model services need a declared isolation/reset mechanism before they can be used safely across runs.

Comparing multiple models is distinct from combining their outputs. The local SDK compares independent runs sequentially. Bounded [research reranking](../interfaces/research-jobs.md) selects candidates under a fixed policy; it does not establish generic multi-model aggregation.

## Replay and fresh inference

**Recorded-response replay** uses captured data versions, ordering, configuration, model responses, and applicable random seeds to repeat the simulated transitions. The implementation must establish the scope within which those transitions are reproducible.

**Fresh inference** calls a model again. It may produce different output even when its endpoint and apparent inputs match. A mutable endpoint or seed alone is not proof of model identity or deterministic behavior.

Local sealed evidence associates snapshots, configuration, model identity/version, responses, admissions, orders, fills, and state transitions. [Recorded replay](../interfaces/recorded-replay.md) checks sealed inputs under the same executable digest and contract. Durable metadata, retention, and cross-build/platform replay remain outside this guarantee.

## Decisions required for extensions beyond the synthetic MVP

- Decision triggers, equal-time ordering, and end-of-run treatment of pending orders.
- Timestamp conventions, market calendar, freshness, missing-data, and revision policies.
- Model-output representation, conflict resolution, and capital/risk rules.
- Fill eligibility, fees, slippage, latency, numeric precision, and instrument accounting.
- Timeout/retry limits, malformed or missing outputs, and run failure behavior.
- Whether service latency is ignored by simulated time, modeled explicitly, or evaluated as a separate scenario.

The synthetic MVP resolves these choices in the ordering below and the linked contracts/accounting rules. Model failures fail visibly without retries/fallback; processing latency does not advance simulation time. Extensions must explicitly revisit the applicable choices instead of silently skipping failures, replacing data, or generating actions. See the [roadmap](../roadmap.md).

## Accepted MVP 1 synthetic daily ordering

**Status:** Accepted for the synthetic MVP only. This is not a real exchange calendar, market-data convention, or execution guarantee.

The fixture has one session per calendar day from 09:00 to 16:00 UTC, with a 09:00 open event and a 16:00 close event. There are no holidays or daylight-saving changes. Bar and option quote fields for the interval are available at 16:00 UTC; the open-price field is available at the 09:00 open. A close-time decision cannot consume close data before 16:00 or fill at that session's open.

At each open, apply a previously scheduled whole-portfolio liquidation before other queued fills. If liquidation is scheduled, cancel other pending orders and do not admit new orders at that open. Otherwise gather the entire execution group and its required open quotes, then recheck cash, margin, delivery reserves, fees, slippage, and lot rounding using those opens. Gap moves can reduce a previously admitted increase; apply one common scale factor to the eligible risk-increasing changes, preserve valid risk-reducing closes, and record any open-time adjustment. Rejecting an unaffordable increase does not cancel a valid close in the same group. The controlled fixture fills the affordable group fully; it has no partial execution. Stage the group's ledger postings on an isolated portfolio, then check cash, margin, and underlying collateral on its final state before publishing any fills; intermediate legs may temporarily use capital or collateral supplied by another leg in the same group. Standalone trades retain their immediate checks. If any required price is missing or a group cannot be validated, do not invent a fill; fail the run and retain its last valid state.

At each close, apply this fixed sequence:

1. Admit the completed bar, close marks, and source observations whose availability is no later than this close. A revision becomes eligible at its own availability time; a later revision does not replace an earlier snapshot retroactively.
2. Settle each open future against its current daily settlement mark, using its previous settlement mark or entry price on its first session.
3. Apply the expiry and explicit American exercise/assignment events scheduled for this close. European options settle for cash intrinsic value at expiry. American physical delivery updates underlying quantities and cash atomically; reject an event if the account cannot settle it.
4. Refresh delivery reservations and test maintenance margin against post-settlement state. On breach, record a risk event, cancel pending orders, and schedule liquidation at the next open. Do not call the model or admit fresh orders during the breached close.
5. If the account remains valid, create a snapshot from eligible close data and post-event portfolio state. Keep the complete instrument catalog in the model request, but exclude expired derivatives from its eligible IDs. Invoke the model once for the run's selected output type and evaluate the response group against that snapshot. A targets response may omit expired derivatives or specify only zero for them.
6. Convert admitted proposals into market orders for a later open. Record rejected or reduced proposals and their reasons. A model proposal is not a fill.

If a liquidation's next-open prices are available, close all remaining positions at that open and apply fees/slippage. Resume daily decision cycles only if the resulting cash, positions, and delivery obligations are valid. If any position cannot be liquidated or the resulting account remains insolvent, fail the run with its last ledger and error. At configured run end, cancel unfilled orders, report remaining positions and reservations, and make no forced end-of-run trade.

An event published after a close is invisible to the model until a later close that is at or after its `availability_time`. Processing time records the actual wall-clock handling and never advances the synthetic clock. Unavailable, stale, or malformed observations remain distinguishable from valid empty content; MVP 1 has no implicit carry-forward policy.
