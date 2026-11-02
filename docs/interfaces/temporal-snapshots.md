# MVP 1 observations and decision snapshots

**Status:** Proposed data contract. The JSON Schemas define wire shape; availability, revisions, references, and ledger consistency also require semantic validation.

The canonical schemas are [normalized observation](../../contracts/schemas/observation.schema.json) and [decision snapshot](../../contracts/schemas/snapshot.schema.json). Shared decimal, identifier, timestamp, and error definitions are in [common schema](../../contracts/schemas/common.schema.json).

## Observation identity and time

Every observation has a logical `observation_id`, `kind`, `source_id`, `source_version`, positive integer `revision`, `event_time`, `available_at`, and typed payload. Bar and quote observations also identify their instrument. News may be general or may identify an instrument. A new revision retains the same logical observation ID and has a higher revision number and its own availability time.

`event_time` says when the event happened or the bar period occurred. `available_at` says when that specific version could first be used. Both are UTC timestamps. They are not service processing time. A bar can be present in storage before its interval closes, but its completed OHLC payload becomes eligible only at its `available_at` (the synthetic daily close). The bar's open field alone may be used for execution at that session's open through the execution event, not through a later decision snapshot.

## Snapshot assembly

A snapshot contains run, scenario, step, and snapshot IDs; `simulation_time`; actual wall-clock `assembled_at`; eligible observations inline; and a pre-decision portfolio view. Portfolio fields distinguish cash/NAV, marked positions, cash and underlying reserves, initial/maintenance margin requirements, and free buying power.

The data adapter must enforce all of the following before returning a snapshot:

- Include only observations with `available_at <= simulation_time`. A later `event_time` may appear only if that data was genuinely made available by the source before the simulation point; availability time remains the visibility gate.
- For a logical observation, select the greatest revision whose own `available_at` is no later than the simulation time. Do not include a future revision or replace an earlier recorded snapshot with a later correction.
- Reject duplicate `(observation_id, revision)` identities, unresolved instrument IDs, or source/version ambiguity. The model receives one deterministic list sorted by `observation_id` ascending.
- The portfolio is the same engine state used to evaluate the response for that run/scenario/step. `assembled_at` is processing time and must not be substituted for `simulation_time`.
- Recompute portfolio arithmetic and reserve totals in the engine; a schema-valid snapshot with inconsistent NAV or buying power is rejected before model invocation.

The response contract binds to `snapshot_id`, `run_id`, `scenario_id`, and `step_id`. A stale, duplicated, or mismatched response cannot be applied to a different step.

## Examples and expected errors

- [Late news observation](../../contracts/examples/temporal/late-news.available-later.json) occurred at Jan 6 15:00Z but is available at 17:00Z. It cannot appear in the Jan 6 16:00Z snapshot. The Jan 7 revision, if available at Jan 7 17:00Z, first appears at the Jan 8 close.
- [Missing availability](../../contracts/examples/temporal/missing-availability.schema-invalid.json) is valid JSON but fails the data-adapter contract with run error `DATA_OBSERVATION_SCHEMA_INVALID` at `$.available_at`.
- [Unavailable observation in a snapshot](../../contracts/examples/temporal/unavailable-data.snapshot-semantic-invalid.json) is schema-valid but fails snapshot assembly with run error `DATA_SNAPSHOT_CONTAINS_UNAVAILABLE_DATA` because its availability is one hour after `simulation_time`.

These are run-level data failures, not HTTP 422 responses. The API's `SCHEMA_VALIDATION_FAILED` code applies to invalid request bodies such as a submitted run configuration.

These examples use only synthetic data. They do not claim provider publication-time accuracy. See the [simulation lifecycle](../architecture/simulation-lifecycle.md) for the accepted timing policy and the [service contract overview](service-contracts.md) for adapter ownership.
