# MVP 1 synthetic acceptance fixtures

**Status:** Accepted scenarios and expected behavior; these examples are test specifications, not an executable data schema.

These cases use no provider credentials or customer data. Stage 01 supplied machine-validated JSON examples; later phases added runtime checks. Values are intentionally synthetic and must not be interpreted as listed-market terms. The mapping below distinguishes existing checks from exact end-to-end case coverage.

## Shared baseline

- Calendar: one synthetic session every calendar day from 09:00 to 16:00 UTC; all fixture values below are USD.
- Run interval: 2026-01-05 through 2026-01-09 inclusive. Decision snapshots occur at each close. Orders fill no earlier than the following open.
- Initial portfolio: USD 100,000 cash, zero positions, zero pending orders.
- Instruments:

| ID | Type | Tick | Quantity step | Multiplier | Other parameters |
|---|---|---:|---:|---:|---|
| `SPOT_A` | Spot | 0.01 | 0.001 | 1 | Long-only |
| `SPOT_B` | Spot | 0.01 | 0.001 | 1 | Long-only |
| `FUT_A` | Linear future | 0.01 | 1 | 1 | Expires Jan 9 16:00Z; initial margin USD 1,000/contract; maintenance USD 800/contract; score target 1 contract |
| `EU_CALL_A` | European call | 0.01 | 1 | 100 | Strike 100; expires Jan 8 16:00Z; cash; margin USD 600/short contract; score target +1 |
| `EU_PUT_A` | European put | 0.01 | 1 | 100 | Strike 100; expires Jan 8 16:00Z; cash; margin USD 600/short contract; score target +1 |
| `AM_CALL_A` | American call | 0.01 | 1 | 100 | Strike 100; expires Jan 9 16:00Z; physical; margin USD 600/short contract; score target +1 |
| `AM_PUT_A` | American put | 0.01 | 1 | 100 | Strike 100; expires Jan 9 16:00Z; physical; margin USD 600/short contract; score target +1 |

Common underlying and option configuration refers to `SPOT_A`. The baseline option mode uses quote marks. The pricer case switches that mode to theoretical; its pricer algorithm, ACT/365F basis, and 256 CRR steps are static configuration, while rate `0` and annual volatility `0.2` arrive as point-in-time `option_pricing_input` observations. Baseline fees/slippage are zero; nonzero-cost cases specify them explicitly.

## Daily close values

Rows are close marks at 16:00 UTC. The corresponding execution open mark is supplied separately for each next-session order; no fill uses the previous close or a later close.

| Date | `SPOT_A` | `SPOT_B` | `FUT_A` | `EU_CALL_A` | `EU_PUT_A` | `AM_CALL_A` | `AM_PUT_A` |
|---|---:|---:|---:|---:|---:|---:|---:|
| Jan 5 | 100 | 50 | 100 | 5 | 4 | 5 | 4 |
| Jan 6 | 101 | 50.5 | 101 | 5.5 | 3.5 | 5.5 | 3.5 |
| Jan 7 | 102 | 51 | 102 | 6 | 3 | 6 | 3 |
| Jan 8 | 101 | 51.5 | 101 | 5 | 4 | 5 | 4 |
| Jan 9 | 100 | 52 | 100 | — expired — | — expired — | 4 | 5 |

Each source row has an observation ID, instrument ID, event time, availability time, source, and version. A full interval OHLC row becomes eligible at the close; its open field alone becomes eligible at that session's open. Option marks shown here are quotes. Add a controlled exercise event for `AM_CALL_A` assignment at Jan 7 16:00Z in the assignment cases below.

The `BASE-01` eligible Jan 6 09:00 execution opens are `SPOT_A` 100.50, `SPOT_B` 50.25, `FUT_A` 100.50, `EU_CALL_A` 5.25, `EU_PUT_A` 3.75, `AM_CALL_A` 5.50, and `AM_PUT_A` 3.75. They are not visible to the model's Jan 5 close snapshot. Use additional fixed open/close prices per case when stressing fills, slippage, margin, or liquidation.

## Expected cases

| ID | Setup and response | Expected result |
|---|---|---|
| `BASE-01` | At Jan 5 close, `scores` response ranks `SPOT_A`=2, `SPOT_B`=1, all derivatives negative; `top_k`=2. | Targets are 50% NAV in each spot and zero in derivatives. Both proposals use the same pre-decision snapshot. Eligible orders fill only at Jan 6 open, at supplied opens after tick/fee/slippage rules. The close marks themselves are never fill prices. |
| `BASE-02` | Give `SPOT_A` and `SPOT_B` the same positive score and every derivative a nonpositive score. | Select IDs lexicographically (`SPOT_A`, then `SPOT_B`); allocation and evidence stay the same regardless of input-map or asset iteration order. |
| `BASE-03` | Give positive scores only to `SPOT_A` and `FUT_A`; `top_k`=2. | Spot target weight is 1/2 and future target is the configured +1 contract. All other instruments target zero. If cash/collateral cannot cover the joint increases, the shared scale factor reduces them as a group and records the reduction. |
| `OUT-01` | Separate runs use `targets`, a signed futures/option position, and one valid `order_intents` response. | Each response matches the run's configured output mode; derivative quantities are contract counts. A mismatched mode, duplicate instrument, unknown ID, short spot, nonzero option position after expiry, or unsupported order type fails with a typed error and step identity. |
| `TIME-01` | A news event has event time Jan 6 15:00Z but availability Jan 6 17:00Z; a revision arrives Jan 7 17:00Z. | Neither is visible in the Jan 6 close snapshot. The first version is eligible Jan 7 close, and its revision is eligible Jan 8 close. Neither changes an earlier recorded snapshot. |
| `ACCT-01` | Use the hand-checkable purchase, futures variation, option premium, and nonzero-fee examples in [synthetic accounting](../../architecture/synthetic-accounting.md). | Independent recomputation matches cash postings, positions, fees, settlement marks, reserves, and NAV to the specified cent/quantity rounding. Daily futures variation is not posted twice. |
| `EXEC-01` | In an isolated zero-position run, submit one buy for `SPOT_A` against its Jan 6 open quote 100.50. Configure `slippage_bps`=100, `fee_bps`=10, and `fee_per_unit`=0.02. | Slipped buy price is 101.505, rounded up to tick 101.51. Fee is 101.51×0.001 + 0.02 = 0.12151, rounded to USD 0.12. The fill posts USD 101.63 cash cost and quantity 1; after the Jan 6 close mark 101, NAV is USD 99,999.37. |
| `OPT-01` | Pricing-only sample (separate from baseline run expiry): value European and American calls/puts with underlying 100, strike 100, rate 0.05, volatility 0.2, and expiry one ACT/365F year away. Rate and volatility are synthetic observations available at the valuation time. | Black–Scholes and 256-step CRR match the reference values and tolerances in [option valuation](../../architecture/options-valuation.md). Theoretical prices become valuation marks only; fills still use fixture open quotes. Missing/future volatility inputs or an invalid CRR probability fail without quote fallback. |
| `RISK-01` | Start with a short physical American call but no free `SPOT_A` shares for delivery, or two short calls that would reserve the same shares. | Admission rejects the opening exposure before changing cash or positions; no short spot, shared delivery reserve, or unbacked contract is created. |
| `RISK-02` | In an isolated `order_intents` stress variant, override `FUT_A` multiplier to 1,000, entry price to 20, initial/maintenance margins to USD 1,000/USD 800 per contract, Jan 6 close to 20, Jan 7 close to 18, and Jan 8 open to 20. At the Jan 5 close, submit a market buy for 100 futures with initial cash USD 100,000. | Futures entry reserves USD 100,000; the Jan 7 close posts a USD 200,000 variation loss once, so close-time cash becomes negative and a margin breach is recorded. Pending orders are canceled and the entire portfolio is liquidated at Jan 8 open before any model request. The open-price recovery variation posts once; final cash returns to USD 100,000 and the run resumes. If the next open remains at 18, cash stays negative and the run fails with `liquidation_failed` and the last valid ledger. |
| `SETTLE-01` | In an isolated `order_intents` variant, start with USD 90,000 cash and 100 `SPOT_A` shares at mark 100; sell one `AM_CALL_A` contract at its Jan 6 open quote; supply an assignment event for Jan 7 close. | Remove the short option, release its reservation, post USD 10,000 strike proceeds, and reduce spot holdings by 100 atomically. Duplicate event ID, European event, unbacked long exercise, or excess event quantity fails without partially applying it. |
| `SETTLE-02` | Hold one in-the-money American contract at its expiry close without an exercise/assignment fixture event. | Fail for incomplete physical-settlement input; do not infer exercise from intrinsic value or silently expire the delivery obligation. An out-of-the-money contract without an event expires worthless as specified. |
| `END-01` | End the run with unfilled market orders and open valid positions. | Cancel unfilled orders, preserve and report positions/reserves, and do not force close at a fabricated end-of-run price. |
| `GROUP-01` | At one open, buy spot shares and write a call backed by those shares; later buy back the call and sell the shares in one group. Reverse the input order in each group. Also attempt a short call without backing shares. | Valid groups produce the same fills, cash and reservations in either input order. Final group collateral is checked before publication. The unbacked group publishes no member fill and preserves the previous portfolio. |
| `COST-01` | With USD 100,000 cash, target 1,000 `SPOT_A` units at a 100 close, a USD 1 fee per unit and a higher next-open price. Use the configured tick, slippage and quantity step. | Close-time admission reduces the request to a cost-feasible quantity; the next-open check reduces it again after the gap. The accepted quantities and adjustment are recorded, and the applied fill does not leave negative cash. |
| `EXP-01` | Hold a future through its expiry close and request another decision in each of `scores`, `targets`, and `order_intents` mode. | Settle and remove the future, keep it in the request catalog and evidence, and exclude it from eligible IDs. An explicit derivative target `0` is allowed; a nonzero target or new order intent for it is rejected. |

## Verification traceability

This table maps requirements to existing test functions and programs inspected during the documentation review. A related unit test is not proof that the exact full scenario above runs through SDK, backend and engine. Historical executed evidence is in [Phase B](../../development/phase-b-local.md); current documentation verification is recorded separately. No new engine tests are introduced by this table.

| Case | Existing test or program | Level and limits |
| --- | --- | --- |
| `BASE-01` | [controlled_acceptance.rs](../../../rust/crates/engine/tests/controlled_acceptance.rs): `accepted_spot_decision_runs_through_admission_next_open_and_finalization` | Engine integration covers two-spot scores, grouped admission, next-open gap adjustment and finalization. It is not an HTTP test. |
| `BASE-02` | [decisions.rs](../../../rust/crates/engine/src/decisions.rs): `top_k_ties_use_instrument_id_and_spot_weights_convert_at_snapshot_marks`; [admission.rs](../../../rust/crates/engine/src/admission.rs): `scales_the_whole_group_against_shared_cash_independent_of_input_order` | Focused ranking/allocation tests; full evidence equality under every input permutation is not established by this mapping. |
| `BASE-03` | [decisions.rs](../../../rust/crates/engine/src/decisions.rs), [admission.rs](../../../rust/crates/engine/src/admission.rs), and [smoke_backtest.py](../../../python-sdk/tests/smoke_backtest.py) | Related score/admission logic and multi-instrument HTTP smoke exist. The smoke selects different positive scores; the exact SPOT_A/FUT_A top-two stress is not mapped to a dedicated end-to-end test. |
| `OUT-01` | [model_validation.rs](../../../rust/crates/backend/src/model_validation.rs): `accepts_shared_scores_and_rejects_wrong_mode_or_correlation`, `rejects_ineligible_and_duplicate_scores`, `target_weights_are_bounded_and_instrument_kinds_are_checked`, `order_intents_validate_as_a_distinct_mode`; [decisions.rs](../../../rust/crates/engine/src/decisions.rs); [smoke_backtest.py](../../../python-sdk/tests/smoke_backtest.py) | Unit checks cover distinct invalid-output classes; the smoke runs all three valid modes. It does not exercise every invalid combination over HTTP. |
| `TIME-01` | [snapshot.rs](../../../rust/crates/data/src/snapshot.rs): `late_news_and_revisions_are_selected_only_when_available`, `revisions_do_not_mutate_previously_assembled_observations_and_order_is_stable`; [inputs.rs](../../../rust/crates/data/src/inputs.rs): `execution_prices_are_hidden_until_their_open_and_availability` | Data-layer temporal tests. Comparison smoke exercises related datasets; model sensitivity to news is not asserted. |
| `ACCT-01` | [portfolio.rs](../../../rust/crates/engine/src/portfolio.rs): `ledger_postings_round_cash_half_even_and_apply_positions_together`; [futures.rs](../../../rust/crates/engine/src/futures.rs): `futures_entry_has_no_notional_cashflow_and_variation_is_posted_once`; [options.rs](../../../rust/crates/engine/src/options.rs): `option_premium_changes_cash_and_long_position_atomically`; [smoke_backtest.py](../../../python-sdk/tests/smoke_backtest.py): `verify_nav_and_evidence` | Unit ledger checks plus independent final NAV and artifact integrity verification. Smoke does not independently recompute every intermediate ledger posting. |
| `EXEC-01` | [execution.rs](../../../rust/crates/engine/src/execution.rs): `applies_adverse_slippage_tick_rounding_and_fee_separately` | Unit test asserts the exact 101.51 fill, 0.12 fee and 99898.37 cash. The described later-close NAV is not asserted in that same test. |
| `OPT-01` | [pricing.rs](../../../rust/crates/engine/src/pricing.rs): `black_scholes_matches_accepted_european_reference_values`, `crr_matches_accepted_american_reference_values_at_256_steps`, `crr_rejects_out_of_bounds_steps_and_invalid_risk_neutral_probability`; [marks.rs](../../../rust/crates/engine/src/marks.rs): `valuation_does_not_carry_forward_an_older_quote_or_use_a_future_input` | Numerical reference and valuation-input tests; these are not real-market calibration or a full theoretical-pricing HTTP acceptance experiment. |
| `RISK-01` | [portfolio.rs](../../../rust/crates/engine/src/portfolio.rs): `physical_short_call_requires_distinct_underlying_shares`, `short_put_cash_collateral_is_not_double_counted_with_margin` | Focused initial-portfolio/reservation checks. They do not represent every order-admission or assignment path. |
| `RISK-02` | [liquidation.rs](../../../rust/crates/engine/src/liquidation.rs): `liquidation_uses_real_open_variation_and_resumes_only_when_valid`, `missing_open_cannot_invent_a_fill_or_change_the_last_valid_state`; [finalize.rs](../../../rust/crates/engine/src/finalize.rs): `failed_liquidation_reason_survives_end_of_run_finalization` | Related liquidation/recovery/failure primitives. Tests use different amounts; the full 100-contract, 200000-loss timeline is not mapped to a dedicated SDK/backend acceptance test. |
| `SETTLE-01` | [options.rs](../../../rust/crates/engine/src/options.rs): `assignment_releases_call_reserve_and_delivers_underlying_atomically_once`, `physical_event_mismatch_or_excess_contracts_is_rejected_atomically`, `long_exercise_requires_cash_or_deliverable_underlying_and_rolls_back` | Unit delivery/reservation/rollback checks; not an exact HTTP replay of the entire described timeline. |
| `SETTLE-02` | [options.rs](../../../rust/crates/engine/src/options.rs): `american_option_expires_worthless_only_when_out_of_the_money` | Unit checks distinguish missing ITM physical input from worthless OTM expiry. |
| `END-01` | [finalize.rs](../../../rust/crates/engine/src/finalize.rs): `end_of_run_cancels_unfilled_orders_and_reports_positions_without_fills`; [controlled_acceptance.rs](../../../rust/crates/engine/tests/controlled_acceptance.rs) | Focused finalization and spot integration. No claim of every derivative/end-state combination follows. |
| `GROUP-01` | [orders.rs](../../../rust/crates/engine/src/orders.rs): `covered_call_group_opens_and_closes_regardless_of_input_order`, `invalid_group_collateral_rolls_back_all_staged_legs` | Engine tests cover fill equality under input reversal and rollback of an unbacked call group. They do not run the exact scenario through HTTP. |
| `COST-01` | [admission.rs](../../../rust/crates/engine/src/admission.rs): `close_admission_includes_fee_and_adverse_tick_price`; [orders.rs](../../../rust/crates/engine/src/orders.rs): `fee_and_gap_reduce_next_open_quantity_before_any_fill_is_published` | Engine tests check cost-aware close admission, the fee/gap quantity reductions and nonnegative post-fill cash. The exact stress case is not a separate SDK smoke. |
| `EXP-01` | [model_validation.rs](../../../rust/crates/backend/src/model_validation.rs): `expired_derivative_accepts_only_zero_target_and_keeps_identity_checks`; [orchestrator.rs](../../../rust/crates/backend/src/orchestrator.rs): `expired_future_is_settled_and_excluded_in_each_output_mode` | Backend tests cover validator rejections and a loopback model across all three modes with a settled future and full catalog. They do not cover every option expiry variant. |

To rerun the referenced suites, use the commands in the development verification guides and select tests by their actual names. This mapping is not an aggregate pass/fail badge: preserve limitations when reporting MVP coverage, and add scoped tests in a separate behavior change if exact scenario coverage is needed.

## Cross-case requirements

- The Python request and Rust contract layer consume the same JSON examples and agree on validity/error codes. A semantically invalid but schema-valid case is rejected by run validation rather than being accepted as runnable work.
- Every observation, model response, target/order, fill, settlement, risk event, and transition has the appropriate run/scenario/step/instrument identity and simulation time.
- Transport failures, unavailable fixtures, malformed outputs, and missing execution prices remain visible; no error path invents data, capital, model decisions, or fills.
- Examples stay synthetic. These expected results verify implementation against accepted MVP assumptions; they do not establish exchange fidelity or investment performance.
