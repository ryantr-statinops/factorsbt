# Simulation lifecycle

**Status:** Accepted conceptual requirements; executable ordering, financial conventions, and failure policies are Open.

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

The cycle describes logical responsibilities, not an established scheduling algorithm. Pending orders may fill between decision cycles. The ordering of market updates, fills, and model triggers at equal timestamps must be specified before engine implementation.

## Multiple data frequencies

Price bars, irregular news, and quarterly disclosures need not have identical timestamps or row counts. A snapshot represents what is known at one simulated moment. Any carry-forward, freshness limit, missing-data handling, or decision-trigger rule must be explicit for the selected data types.

A bar's closing information is unavailable before the bar completes. Whether a decision using that bar can execute at a particular price depends on the declared execution model; it must not receive an impossible historical fill.

## Decisions, orders, and fills

- A **decision** is a model or policy proposal. Its representation might be a prediction, target allocation, or order intent; the first output contract is still Open.
- An **order** is an instruction admitted by portfolio/execution rules. Predictions require a defined conversion policy before becoming actionable.
- A **fill** is a simulated execution event that changes positions and cash under the selected accounting rules.

Asset iteration order must not accidentally determine capital allocation. Record the selected admission policy and explain reductions or rejected proposals. Partial fills, order types, buying-power rules, and rounding remain Open.

## Multiple scenarios and models

Every scenario owns its evolving state. Shared market inputs do not imply identical model inputs when holdings, capital, history, or model state differ. External stateful model services need a declared isolation/reset mechanism before they can be used safely across runs.

Comparing multiple models is distinct from combining their outputs. Combining requires an explicit policy for conflicts and ranking; no aggregation algorithm is accepted yet.

## Replay and fresh inference

**Recorded-response replay** uses captured data versions, ordering, configuration, model responses, and applicable random seeds to repeat the simulated transitions. The implementation must establish the scope within which those transitions are reproducible.

**Fresh inference** calls a model again. It may produce different output even when its endpoint and apparent inputs match. A mutable endpoint or seed alone is not proof of model identity or deterministic behavior.

Evidence should associate input snapshots, configuration, model identity/version where available, returned outputs, admission decisions, orders, fills, and state transitions. Exact manifests, persistence, and retention are Open.

## Failure and execution decisions required before implementation

- Decision triggers, equal-time ordering, and end-of-run treatment of pending orders.
- Timestamp conventions, market calendar, freshness, missing-data, and revision policies.
- Model-output representation, conflict resolution, and capital/risk rules.
- Fill eligibility, fees, slippage, latency, numeric precision, and instrument accounting.
- Timeout/retry limits, malformed or missing outputs, and run failure behavior.
- Whether service latency is ignored by simulated time, modeled explicitly, or evaluated as a separate scenario.

Do not silently skip a failure, replace data, or generate a trading action. Select and record the needed behavior for the [first implementation slice](../roadmap.md).
