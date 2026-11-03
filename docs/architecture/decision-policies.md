# MVP 1 model output and portfolio-admission rules

**Design status:** Accepted for the synthetic MVP only.

**Implementation status:** Phase B implements and locally verifies controlled model invocation, response validation, proposal conversion, engine admission, and the typed Python backtest flow. End-to-end smoke covers all three output modes; focused engine tests cover financial rules beyond those smoke paths.

The [MVP scope](../product/mvp-scope.md) defines which output types exist. This document makes their selection and conversion behavior precise while preserving the rule that a proposal, order, and fill are separate events. Wire shapes and correlation identity are specified in the [MVP model exchange](../interfaces/model-exchanges.md).

## One response type per run

`run.output_mode` is exactly one of `scores`, `targets`, or `order_intents`. A model response whose discriminator differs from the run mode is invalid. Each response carries the run, scenario, decision-step, snapshot, and model-response identifiers. The fixture is stateless and returns at most one response per step.

## Scores

1. The run supplies an integer `top_k` from 1 through the configured instrument count.
2. At a decision step the eligible universe contains configured instruments that have valid close marks and are not expired or otherwise unavailable for new exposure at this time.
3. A response must supply exactly one finite score for each eligible instrument and no score for an unknown or ineligible ID. Zero and negative scores are ineligible.
4. Order eligible instruments by score descending, then by `instrument_id` ascending. Select the first `min(top_k, count(positive_scores))`; an empty selection is valid.
5. Each selected spot has target weight `1/top_k` of the pre-decision NAV. If fewer than `top_k` instruments are positive, unused weight remains in cash. Each selected derivative has the target contract quantity configured for that instrument in the run; that value can be signed. Unselected instruments target zero.
6. The resulting full target is admitted as one decision group. If no score is positive, the full target is zero and closes existing positions as execution rules permit.

`top_k` may be less than the number of eligible instruments; the remaining positive instruments are unselected and target zero. If fewer than `top_k` instruments have positive scores, unused spot weight is cash. Scores rank instruments. They do not change the configured derivative contract size or imply a probability.

## Targets

A `targets` response declares a complete target for every configured instrument: nonnegative decimal spot weights and signed integer derivative contract quantities. Spot weights must sum to at most one. A map with missing or duplicate IDs, a weight below zero, or unknown instrument fails validation. Expired or ineligible derivatives must have target zero. The percentages describe spot NAV weights; derivative entries are contract counts, not weights, notional exposure, or delta.

## Order intents

An `order_intents` response may include zero or one nonzero signed market order per eligible instrument. Positive quantity buys/increases a long position; negative quantity sells, closes, or increases a short where the instrument permits it. Quantity must be a multiple of the instrument's step. Limit/stop orders, duplicate instrument intents, and short spot are unsupported and rejected.

## Group evaluation and proportional scaling

Evaluate the entire response from a single pre-application portfolio and cash snapshot. Never mutate cash or positions as a side effect of iterating instruments.

For targets, derive position changes from current to desired quantity. For order intents, use signed requested quantity. For scores, derive changes from the complete generated target above. Partition changes into risk-reducing closes and risk-increasing opens. Compute fees, cash use, futures initial margin, written-option margin, delivery reserves, and available buying power using the configured instrument rules.

Always admit valid risk-reducing closes before funding increases. Find one shared scale factor `λ` for all risk-increasing changes using exactly 60 binary-search iterations with `λ` represented to 18 decimal places. For each feasibility check, round every scaled increase toward zero to its configured quantity increment, then calculate the full group's resulting buying power, fees, margins, and delivery reserves. Feasibility is monotone in `λ`; retain the highest tested feasible candidate and recompute that candidate once at the end before acceptance. If no risk-increase candidate is feasible, accept valid closes and no increases. Do not change admission based on input-map or asset iteration order.

Record requested and admitted quantities/targets, `λ`, per-instrument rounding or reduction, the funding constraint, and final state. A rejected instrument or whole response group is not an order or a fill. This rule manages synthetic capital feasibility only; it is not a risk guarantee.

## Defaults and errors

- The baseline run mode is `scores` with `top_k=2` and derivative quantities of one contract for the four configured options and the future. Other output modes have separate controlled examples.
- An equal score always resolves by stable ID. Non-finite scores, missing/extra response IDs, incompatible output mode, malformed targets, and repeated response application fail the run with a typed error and step context.
- If the input snapshot has no valid close mark for any configured instrument required by the selected output, fail with a missing-data error. Do not silently score only an accidental subset.
- Risk reductions can never bypass required execution prices, valid quantity steps, expiry, or delivery settlement.

## Scope and extensions

These policies are synthetic MVP 1 requirements, not accepted real-market allocation rules. Weighted score transforms, asset-class budgets, multiple model aggregation, priority-based allocation, market-impact optimization, and learned execution remain out of scope. A policy change requires a documented contract change and new expected examples before implementation relies on it.
