# Python SDK guide

**Status:** Implemented synchronous client for the controlled local backend. Install and try the self-contained [quickstart](quickstart.md) first. SDK examples below use the existing API; network sections require services you have started separately.

## Configuration builders

The following Python example builds the shared minimal spot configuration. It can be evaluated without a model service; the loopback endpoint is only a configuration value until submission. Run from the repository root with the installed package. For real submission, replace the endpoint with your running compatible [stateless model fixture service](../interfaces/model-exchanges.md).

```python
from factorsbt import (
    Client, RunConfiguration, SimulationConfiguration, InitialPortfolio,
    SpotInstrument, DataFixtureConfiguration, ModelConfiguration, ScoresPolicy,
)

config = RunConfiguration(
    simulation=SimulationConfiguration(
        start_at="2026-01-05T16:00:00Z", end_at="2026-01-09T16:00:00Z",
    ),
    initial_portfolio=InitialPortfolio(cash="100000"),
    instruments=(SpotInstrument(
        instrument_id="SPOT_A", price_tick="0.01", quantity_step="0.001",
        contract_multiplier="1", fee_bps="0", fee_per_unit="0", slippage_bps="0",
    ),),
    data=DataFixtureConfiguration("mvp-synthetic", "v1"),
    model=ModelConfiguration("synthetic-model", "v1", "http://127.0.0.1:8787/infer", 5000),
    decision_policy=ScoresPolicy(top_k=1),
)
wire_configuration = config.to_wire()
```

`RunConfiguration` combines simulation, initial portfolio, typed instrument definitions, fixture identity/version, model configuration and exactly one policy. Alternatives are `TargetsPolicy()` and `OrderIntentsPolicy()`; scores may additionally require `DerivativeScoreTarget` entries. `InitialPosition`, `LinearFutureInstrument` and `OptionInstrument` extend the same builders; their required fields and financial limits are in [instrument configuration](../interfaces/instrument-configuration.md).

Use strings or `Decimal` for financial values, never floats. `WireDecimal` and `UtcInstant` preserve wire decimal/UTC representations. Wire timestamps end in `Z`; negative zero, invalid precision and unsupported contract versions are rejected. Builders validate local values during serialization, but backend semantic validation remains authoritative. Successful `to_wire()` is not evidence a run is admissible.

## Submit, inspect and retrieve

With backend and model service running, continue the example:

```python
client = Client("http://127.0.0.1:8080", timeout=5.0)
health = client.health()
accepted = client.submit(config)
run_id = accepted.run_id
submission = client.get_submission(accepted.submission_id)
status = client.get_run(run_id)
terminal = client.wait(run_id, timeout=60.0, poll_interval=0.25)
result = client.get_results(run_id)
print(result.portfolio.nav.value)
```

`submit` also accepts a JSON-compatible mapping, such as a loaded shared example; it does not insert missing financial defaults. You may provide a lowercase UUID `submission_id` to retain a lookup key before sending. Return types include `HealthResponse`, `SubmissionAccepted`, `SubmissionStatus`, `RunStatus` and `RunResult`; nested money/time fields remain typed. `get_results` requires success; `wait` returns terminal status and does not fetch the result.

The SDK's `Client.timeout` is a per-HTTP-request timeout (default 5 seconds). Run waiting has its own monotonic deadline (default 60 seconds, polling every 0.25 seconds). Expiration stops waiting only. Backend bind/model endpoint restrictions are described in [local operations](local-operations.md); the CLI applies its own stricter loopback URL validation.

## Recover and handle errors

| Exception | Caller action |
| --- | --- |
| `SubmissionAcknowledgementUnknown` | Save `submission_id`; look up with `get_submission`. Do not automatically repeat the POST. |
| `WaitTimeout` | Save `run_id` and `last_status`; inspect/wait later. Backend work continues. |
| `RunFailedError` | Inspect `status` and `failure`; a failed run has no successful result. Read sealed evidence if available. |
| `HttpStatusError` | Inspect HTTP `status`, backend `code`, `request_id` and safe `details`. |
| `RequestTimeout` / `TransportError` | Diagnose the request/connection; a POST with uncertain acknowledgment uses its specialized exception. |
| `ProtocolError` | Response, correlation or artifact integrity was unusable; do not treat it as a financial result. |
| `ContractValueError`, `ValueError`, `TypeError` | Correct local inputs before sending. |

The [submission contract](../interfaces/mvp-submissions.md) defines explicit retry ownership and process-local deduplication. Lookup protection does not survive backend restart. A new ID creates independent work even if the configuration is identical.

## Read evidence and replay

```python
import json

index = client.get_evidence(run_id)
reference = next(item for item in index.artifacts if item.kind == "run_manifest")
manifest = json.loads(client.get_artifact(run_id, reference))

replay = client.submit_replay(run_id)
client.wait_replay(replay.replay_id, timeout=60.0)
report = client.get_replay_report(replay.replay_id)
print(report.matched)
```

The SDK verifies fetched artifact bytes against their typed reference. Failed runs can also expose sealed evidence. `submit_replay` uses recorded inputs, makes no model call and requires the same executable digest and contract. A matched failed source is successful verification, not a successful source backtest. `ReplayAcknowledgementUnknown` retains `replay_id`; `ReplayFailedError` retains status so a divergence report can be read. Replay metadata is process-local.

For sequential scenarios use the [comparison guide](comparison.md). For reranking/episodes/research replay use the [research interface and example](../interfaces/research-jobs.md). Research jobs have separate acknowledgment/failure/wait-timeout exceptions and job IDs; run artifact routes do not accept research/replay IDs.
