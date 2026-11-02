# Backend API role

## Responsibility

Own the Rust service boundary: accepting work, coordinating runs, invoking data/model adapters, exposing progress, and retrieving results. Keep orchestration behavior separate from the engine's financial rules.

Read the [architecture](../../docs/architecture/overview.md), [service contracts](../../docs/interfaces/service-contracts.md), and [simulation lifecycle](../../docs/architecture/simulation-lifecycle.md).

## Design expectations

- Define run identity, lifecycle, failure reporting, and retry ownership before implementing job handling.
- Coordinate model calls against the correct run, scenario, simulation step, and snapshot. A late or duplicated response must not apply to a different step.
- Treat submission and decision application as operations whose duplicate effects require explicit prevention; do not infer exactly-once behavior from HTTP alone.
- Preserve source/version/availability metadata through data adapters and preserve model-response identity through inference adapters.
- Keep service processing time separate from simulated time. Execution latency belongs to an explicit simulation policy.
- Bound external-call behavior and expose errors without fabricating missing data or a replacement trading decision.
- Avoid leaking keys, private data, or unrestricted snapshot access through logs and result references.

## Contract and tooling discipline

Framework, storage, queue, authentication, deployment topology, exact endpoints, and wire schemas are open. Document a proposal before introducing a choice that affects other components. Keep Python SDK and service expectations aligned.

## Verification and handoff

When implemented, use controlled adapters to check partial failures, retries, timeouts, stale responses, duplicate delivery, and run isolation. Report lifecycle changes, contract impacts, operational failure behavior, and checks performed.
