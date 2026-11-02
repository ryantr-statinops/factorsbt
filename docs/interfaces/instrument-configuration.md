# MVP 1 run and instrument configuration

**Status:** Accepted MVP 1 wire-contract design. The schema describes shape and lexical constraints; semantic checks remain required and no run-submission endpoint is implemented yet.

The canonical JSON Schema files are [shared primitives](../../contracts/schemas/common.schema.json), [instrument types](../../contracts/schemas/instrument.schema.json), and [run configuration](../../contracts/schemas/run-configuration.schema.json). The API accepts a `RunConfiguration` document as its POST body after OpenAPI assembly. Decimal amounts are JSON strings; binary floating-point JSON numbers are not accepted for financial values.

## Shape by instrument type

Every instrument requires a stable ID, the `spot`, `linear_future`, or `option` discriminator, USD quote currency, positive price tick, quantity increment and contract multiplier, plus explicit `fee_bps`, `fee_per_unit`, and `slippage_bps`. Zero is valid for each cost field.

- Spot is long-only and has no extra wire fields.
- A linear future requires a configured spot `underlying_id`, UTC expiry, and initial/maintenance margin per contract. Its quantity step is exactly one contract.
- An option requires a configured spot underlying, call/put type, positive strike, expiry, exercise and settlement styles, quote/theoretical valuation mode, and initial/maintenance written-option margins. Its quantity step is exactly one contract.
- European options use cash settlement; American options use physical delivery. The configured pricer must match the exercise style: Black–Scholes for European and CRR for American. The theoretical-pricer configuration declares ACT/365F and CRR steps where applicable. Risk-free rate and annualized volatility are time-versioned `option_pricing_input` observations, not static instrument fields. Quote mode does not require pricing-input observations.

## Run configuration

The document requires a contract version, interval, synthetic calendar ID, initial USD cash and positions, configured instruments, data fixture identity/version, local HTTP fixture adapter with model ID/version, endpoint and timeout, and one output mode. Score runs also require `top_k` and configured derivative score targets. The schema intentionally does not add defaults; clients send every required value. Static pricer choice is declared in configuration; time-varying pricing inputs come from the versioned fixture and are gated by `available_at`.

## Semantic validation beyond JSON Schema

Reject the configuration with `422 RUN_CONFIGURATION_INVALID` when any of these checks fails:

- Instrument IDs and initial-position IDs are unique; all position and score-target references resolve within the same run.
- A future or option's `underlying_id` resolves to a configured spot instrument. An underlying cannot be inferred from an ID suffix or external metadata.
- Instrument and pricing-input numeric strings satisfy the accepted precision/range rules: at most 28 significant digits and 18 fractional digits; ticks, increments, multipliers, strikes, volatility, margins and timeout-related quantities meet their positive/nonnegative requirements. Risk-free rate is in `[-1, 1]`; annualized volatility is in `(0, 5]`. Maintenance margin cannot exceed initial margin. Negative zero is rejected.
- Linear-future and option quantities are integer contracts. Spot positions and spot targets are nonnegative and align to their configured quantity step. Initial positions cannot refer to expired instruments.
- Exercise/settlement style combinations and theoretical-pricer/style combinations match the accepted MVP rules. Each `option_pricing_input` references a configured option and the newest input available at the valuation time is used. CRR step count is from 2 through 4096. Run start precedes run end and all times are valid instants on the synthetic calendar.
- `top_k` is no greater than the configured instrument count. Each score derivative target identifies a configured derivative at most once; targets never grant a spot short.
- The fixture endpoint uses HTTP on loopback and has no embedded credentials or query-string secret. A nonlocal endpoint is not allowed in this local MVP contract.

Schema validity and semantic validity are separate outcomes. A missing required option field is `422 SCHEMA_VALIDATION_FAILED`; a well-shaped future whose underlying is absent is `422 RUN_CONFIGURATION_INVALID`.

## Examples

[Minimal valid spot run](../../contracts/examples/instruments/minimal-valid-spot-run.json) is a schema example, not the seven-instrument acceptance fixture. [Spot missing a required fee field](../../contracts/examples/instruments/spot-missing-fee.schema-invalid.json) is valid JSON but fails the instrument schema with `422 SCHEMA_VALIDATION_FAILED` at `$.fee_bps`. [Unresolved-underlying example](../../contracts/examples/instruments/unresolved-underlying.semantic-invalid.json) is valid against the run schema and has the expected semantic error `422 RUN_CONFIGURATION_INVALID` at `$.instruments[0].underlying_id` because `SPOT_MISSING` is not defined.

The canonical product and instrument rules remain in the [MVP scope](../product/mvp-scope.md) and [instrument model](../architecture/instrument-model.md). Stage 01 examples will be machine-checked against the shared schemas when the contract verification step is reached.
