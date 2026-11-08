# Sequential scenario comparison

**Status:** Implemented in the Python SDK over independent v1 runs. There is no backend comparison job, parallel scheduler, response cache or model-output reuse. Run the [quickstart](quickstart.md) to see the existing three-fixture smoke example.

## Build an experiment

Continue with a typed `config` and `client` from the [SDK guide](python-sdk.md), and a compatible running stateless model service:

```python
from dataclasses import replace
from pathlib import Path
import tempfile
from uuid import uuid4
from factorsbt import ComparisonScenario, ComparisonSpec, DataFixtureConfiguration

spec = ComparisonSpec(
    baseline=ComparisonScenario("baseline", config),
    variants=(
        ComparisonScenario("price-only", replace(
            config, data=DataFixtureConfiguration("mvp-compare-price-only", "v1"),
        )),
        ComparisonScenario("delayed-event", replace(
            config, data=DataFixtureConfiguration("mvp-compare-delayed-event", "v1"),
        )),
    ),
)
report = client.compare(spec, timeout_per_run=60.0)
for difference in report.metric_differences:
    print(difference.variant_label, difference.pnl_usd, difference.same_interval)
path = Path(tempfile.gettempdir()) / f"factorsbt-comparison-{uuid4()}.json"
report.save(path)
```

Labels must be unique identifiers, with one baseline and at least one variant. Each scenario has its own configuration, submission and run ID; internal run scenario IDs can still be `baseline`. Runs execute in specification order with independent portfolio/pending-order state and fresh model calls. External stateful models are not verified for isolation.

Changing only news visibility may produce no decision difference if the chosen model does not consume those observations. The controlled smoke model does not establish that adding news improves outcomes. Configuration/output differences are observations, not causal attribution.

## Interpret metrics and coverage

| Field | Meaning |
| --- | --- |
| `initial_nav` | Starting marked portfolio NAV from verified summary evidence, not merely initial cash. |
| `final_nav` | Final NAV from the successful run result. |
| `pnl_usd` | `final_nav - initial_nav`. |
| `return_fraction` | `pnl_usd / initial_nav`; `None` if initial NAV is zero. Nonannualized, dimensionless. |
| Metric differences | Variant minus baseline; unavailable metrics remain `None`. |
| `same_interval` | Whether simulation start/end match. False does not suppress calculated deltas or make intervals comparable. |

No Sharpe ratio, annualized return or drawdown is calculated by this report. Different starting capital or universes can make raw deltas unsuitable for a research comparison; inspect `configuration_changes` and interval/coverage before interpreting them.

`report.overlap(variant_label)` returns shared, baseline-only and variant-only timeline keys `(UTC simulation_time, instrument_id)`. Timeline points come from snapshot positions with quantity, mark, observation revisions and source artifact ID; they are not every possible market observation. Trace differences align model outputs, admitted proposals and fills by time, instrument, category and ordinal, retaining source artifact IDs. Unmatched keys are visible rather than forward-filled. Use `trace_differences(label)` and the original run IDs to fetch verified evidence.

## Partial outcomes and saved reports

Handled per-scenario outcomes include `rejected`, `uncertain`, `failed`, `running` after a local wait timeout, and `unavailable` when result/evidence retrieval fails. They remain in the report and do not prevent later scenarios from running. An uncertain acknowledgment triggers lookup without a second POST. The report is not a live tracker; inspect retained IDs later to learn whether timed-out work completed. Invalid specifications/local inputs and errors outside handled SDK cases may still raise.

`to_dict()` and `save()` emit local report format `1.0.0` with identities, statuses, metrics, configuration-change paths and trace differences. They do not copy full configurations or serialize the full in-memory timeline/overlap data. Retain original configurations and source evidence separately. `save()` refuses to overwrite an existing file; it creates no parent directories. A saved report does not restore backend metadata or provide a resumable experiment API.
