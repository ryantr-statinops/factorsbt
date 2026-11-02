# MVP 1 synthetic acceptance fixtures

**Status:** Accepted scenarios and expected behavior; these examples are test specifications, not an executable data schema.

These cases use no provider credentials or customer data. The first Stage 01 schema examples will turn the wire fields into machine-validated JSON. Values are intentionally synthetic and must not be interpreted as listed-market terms.

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

Common underlying and option configuration refers to `SPOT_A`. The baseline option mode uses quote marks. The pricer case switches that mode to theoretical and supplies rate `0`, annual volatility `0.2`, ACT/365F, and 256 CRR steps. Baseline fees/slippage are zero; nonzero-cost cases specify them explicitly.

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
| `OPT-01` | Pricing-only sample (separate from baseline run expiry): value European and American calls/puts with underlying 100, strike 100, rate 0.05, volatility 0.2, and expiry one ACT/365F year away. | Black–Scholes and 256-step CRR match the reference values and tolerances in [option valuation](../../architecture/options-valuation.md). Theoretical prices become valuation marks only; fills still use fixture open quotes. Missing/future volatility inputs or an invalid CRR probability fail without quote fallback. |
| `RISK-01` | Start with a short physical American call but no free `SPOT_A` shares for delivery, or two short calls that would reserve the same shares. | Admission rejects the opening exposure before changing cash or positions; no short spot, shared delivery reserve, or unbacked contract is created. |
| `RISK-02` | In an isolated `order_intents` stress variant, override `FUT_A` multiplier to 1,000, entry price to 20, initial/maintenance margins to USD 1,000/USD 800 per contract, Jan 6 close to 20, Jan 7 close to 18, and Jan 8 open to 20. At the Jan 5 close, submit a market buy for 100 futures with initial cash USD 100,000. | Futures entry reserves USD 100,000; the Jan 7 close posts a USD 200,000 variation loss once, so close-time cash becomes negative and a margin breach is recorded. Pending orders are canceled and the entire portfolio is liquidated at Jan 8 open before any model request. The open-price recovery variation posts once; final cash returns to USD 100,000 and the run resumes. If the next open remains at 18, cash stays negative and the run fails with `liquidation_failed` and the last valid ledger. |
| `SETTLE-01` | In an isolated `order_intents` variant, start with USD 90,000 cash and 100 `SPOT_A` shares at mark 100; sell one `AM_CALL_A` contract at its Jan 6 open quote; supply an assignment event for Jan 7 close. | Remove the short option, release its reservation, post USD 10,000 strike proceeds, and reduce spot holdings by 100 atomically. Duplicate event ID, European event, unbacked long exercise, or excess event quantity fails without partially applying it. |
| `SETTLE-02` | Hold one in-the-money American contract at its expiry close without an exercise/assignment fixture event. | Fail for incomplete physical-settlement input; do not infer exercise from intrinsic value or silently expire the delivery obligation. An out-of-the-money contract without an event expires worthless as specified. |
| `END-01` | End the run with unfilled market orders and open valid positions. | Cancel unfilled orders, preserve and report positions/reserves, and do not force close at a fabricated end-of-run price. |

## Cross-case requirements

- The Python request and Rust contract layer consume the same JSON examples and agree on validity/error codes. A semantically invalid but schema-valid case is rejected by run validation rather than being accepted as runnable work.
- Every observation, model response, target/order, fill, settlement, risk event, and transition has the appropriate run/scenario/step/instrument identity and simulation time.
- Transport failures, unavailable fixtures, malformed outputs, and missing execution prices remain visible; no error path invents data, capital, model decisions, or fills.
- Examples stay synthetic. These expected results verify implementation against accepted MVP assumptions; they do not establish exchange fidelity or investment performance.
