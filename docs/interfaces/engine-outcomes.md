# MVP 1 engine outcome events

**Status:** Accepted MVP 1 internal Rust contract design. The schema describes event shape; event references, arithmetic, ordering, atomicity, and portfolio invariants require engine validation.

**Implementation status:** The Rust engine emits the specified event classes through controlled transitions, and the local MVP persists and verifies run evidence. End-to-end verification uses controlled fixtures; see the [Phase B verification record](../development/phase-b-local.md).

The canonical [engine event schema](../../contracts/schemas/engine-outcome.schema.json) covers decision admission, order creation/cancellation, fills, ledger postings, settlements, and risk events. These are separate records linked by stable IDs. A model response is only a proposal; a fill is a distinct execution outcome.

## Event identity and ordering

Every event carries a unique `event_id`, run/scenario/step identity, simulation time, and sequence number. `sequence` is strictly increasing within a run/scenario and resolves ordering when multiple events share a simulation timestamp. The full history is append-only; a rejected transition records a failure without partially changing the last valid ledger.

| Event | Required linkage and meaning |
|---|---|
| Decision admission | References inference and snapshot; records response type, status, shared scale factor, funding constraint, and requested/admitted values by instrument and unit. |
| Order created/canceled | A created order is one signed market quantity for one instrument and decision. Cancellation names the same order and a controlled reason. |
| Fill | References one order, instrument, signed quantity, execution price, fee, and exact source observation/revision. Fills use fixture quote/bar execution data, never a theoretical option mark. MVP liquidity is full for the admitted quantity; no partial fill is implied. |
| Ledger posting | Records one identified cash posting and position-quantity transition from before to after. Fees are separate postings. Futures daily variation and close/expiry variation use their corresponding posting type. |
| Settlement | Records the contractual cash amount and delivery quantity for futures settlement, European cash expiry, American exercise/assignment, or worthless expiry. The linked ledger posting applies the balance change exactly once. American early events reference an explicit fixture event ID. |
| Risk | Records collateral shortfall, maintenance breach, liquidation scheduling/completion/failure, severity, action, effective simulation time, and causes. |

## Semantic checks

The engine rejects schema-valid events or transitions when:

- IDs do not resolve to the same run, scenario, step, snapshot, decision, inference, order, source observation, and instrument as their producer records.
- An event sequence duplicates or moves backwards, an order receives multiple full fills, a fill has zero quantity or nonpositive execution price, or signed quantity exceeds the order.
- An event is out of the declared lifecycle: no fill before an order, no fill for a canceled order, no new order for an expired instrument, and no American settlement without an eligible position and fixture event.
- A cash posting does not satisfy `cash_before + cash_delta = cash_after`, a quantity posting does not satisfy `before + delta = after`, or cent/lot rounding disagrees with the accounting rule.
- A settlement creates an unbacked short spot or loan, delivery reuses reserved collateral, or margin/collateral transitions leave an invalid state.

Groups that represent one decision or delivery transition are applied atomically. If any member is invalid, preserve the last valid state, report the run failure and event identity, and do not invent a compensating fill.

## Examples and expected errors

- [Admission with common capital scaling](../../contracts/examples/engine/admission-adjusted.valid.json), [order creation](../../contracts/examples/engine/order-created.valid.json), [fill](../../contracts/examples/engine/fill.valid.json), [spot ledger posting](../../contracts/examples/engine/ledger-spot-purchase.valid.json), [futures variation settlement](../../contracts/examples/engine/futures-variation.settlement-valid.json) and its [ledger posting](../../contracts/examples/engine/futures-variation.ledger-valid.json), [scheduled liquidation](../../contracts/examples/engine/risk-liquidation-scheduled.valid.json), and [American assignment](../../contracts/examples/engine/settlement-assignment.valid.json) show the event shapes.
- [Fill missing execution price](../../contracts/examples/engine/fill-missing-price.schema-invalid.json) fails run validation with `ENGINE_EVENT_SCHEMA_INVALID` at `$.fill_price`.
- [Zero-quantity fill](../../contracts/examples/engine/fill-zero-quantity.semantic-invalid.json) has valid shape but fails with `ENGINE_FILL_INVALID_QUANTITY`; no ledger transition is applied.

These are contract examples, not verified engine behavior. The engine and producer/consumer checks are scheduled for later Phase A and Phase B work. See [synthetic accounting](../architecture/synthetic-accounting.md), [margin and liquidation](../architecture/margin-risk.md), and the [simulation lifecycle](../architecture/simulation-lifecycle.md) for accepted semantics.
