# Controlled MVP fixture datasets

**Status:** Implemented fixture v1 data format, datasets, local registry, digest validation, and availability-aware selectors.

Each folder under [`data`](data) contains a `manifest.json` and `dataset.json`. Fixture format `1.1.0` records a positive contract count on each explicit American exercise/assignment event so partial and excessive settlement quantities are unambiguous. The manifest identifies fixture format, fixture ID/version, a relative dataset path, and SHA-256 of the exact dataset bytes. The dataset keeps decision observations separate from open execution prices, initial marks, and settlement events.

| Fixture | Acceptance use |
| --- | --- |
| `mvp-synthetic` | `BASE-01`–`03`, `TIME-01`, `EXEC-01`, `END-01`, and shared instrument/observation inputs. Output-mode variations in `OUT-01`, fee/slippage settings, and `RISK-01` portfolio variants are run configuration/model-response changes over this data. |
| `mvp-risk-02` | Futures price path for `RISK-02` with Jan 8 open recovery to 20. The instrument multiplier and margins are configured by that acceptance case. |
| `mvp-risk-02-fail` | The same stress with Jan 8 open remaining at 18, expected to fail liquidation. |
| `mvp-opt-01` | Point-in-time rate `0.05` and volatility `0.2` inputs for the one-year `OPT-01` pricing reference case. |
| `mvp-settle-01` | `SETTLE-01` American call assignment event at Jan 7 close. |
| `mvp-settle-02-itm` | `SETTLE-02` in-the-money expiry with no exercise/assignment event; run must fail for incomplete settlement input. |
| `mvp-settle-02-otm` | Out-of-the-money expiry with no event; the option may expire worthless. |
| `mvp-compare-price-only` | Same baseline prices and execution inputs with news observations removed. |
| `mvp-compare-delayed-event` | Same baseline observations, with both news revisions available one day later. |

The baseline observations include two revisions of the delayed `TIME-01` news item. Its event time is Jan 6 at 15:00 UTC, revision 1 becomes available at 17:00 that day, and revision 2 becomes available Jan 7 at 17:00. Jan 6 execution opens are separate data rows; for example, `SPOT_A` opens at 100.50 and is not available to the Jan 5 close snapshot.

The files contain only synthetic values. A manifest digest must be regenerated after any byte change to its dataset. Python contract tests verify every manifest, dataset schema, identity pair, and digest; the Rust registry validates the same schemas offline and verifies each digest before registering a fixture.

## Add or revise a local data fixture

**Status:** Working guidance for the Implemented registry. Changes to datasets are outside this documentation update; the following procedure is for a future explicitly scoped fixture change.

1. Select a unique fixture ID/version pair and a folder under `data/`. Preserve a version used by existing experiments; publish a different identity/version for changed research inputs instead of silently replacing recorded assumptions. The registry keys by ID/version, not folder name alone.
2. Use the checked-in [manifest schema](schemas/manifest.v1.schema.json) and [dataset schema](schemas/dataset.v1.schema.json). Match `fixture_id`, `fixture_version` and `format_version` across manifest and dataset. Current format is `1.1.0`; `dataset_path` must be a permitted relative path under the fixture folder. Do not confuse format version with dataset version `v1` or API contract `1.0.0`.
3. Define synthetic observations, initial marks, open execution prices and settlement events separately. Supply the configured instrument references, decimal strings, UTC event/availability times, source identity and revisions. A later revision has its own availability; an open quote does not become model-visible close data. Exercise/assignment events have explicit positive integer quantities and unique event IDs.
4. Include required coverage for the requested universe and interval. Schema validity does not establish temporal eligibility, sufficient execution quotes, valid underlying references, settlement feasibility or portfolio admission. Check the [snapshot contract](../../docs/interfaces/temporal-snapshots.md) and [acceptance cases](../../docs/examples/mvp/acceptance-cases.md).
5. Finish editing/serializing `dataset.json` before hashing it. From the repository root in PowerShell, compute the exact-byte digest using `Get-FileHash -Algorithm SHA256 -LiteralPath <dataset-path>` and lowercase its hexadecimal `Hash` value. Write that value into the manifest's `dataset_sha256`. Any later byte change, including line endings, requires recalculation; hashing parsed/reformatted JSON is not equivalent.
6. Run the checks below and use the fixture's ID/version in the run configuration. Backend startup loads and validates the registry; changed files do not provide a live reload guarantee. Restart explicitly after preparing valid fixtures. Do not modify sealed run/replay source evidence when updating the registry.

From the repository root in x64 Developer PowerShell, after the [development setup](../../docs/development/quickstart.md):

```powershell
$env:PYTHONPATH = Join-Path (Get-Location) 'python-sdk/src'
$fixturePython = Join-Path $env:TEMP 'factorsbt-docs-completeness-venv/Scripts/python.exe'
& $fixturePython -B -m unittest discover -s python-sdk/tests -p test_contract_examples.py -v
cargo test --locked --offline --manifest-path rust/Cargo.toml -p factorsbt-data
```

The Python checks validate shared schemas, fixture identities and exact content digests. The Rust data tests exercise registry loading, coverage, selectors and snapshots. Add behavior-specific acceptance checks when changing data assumptions; existing test success alone does not cover a new financial scenario. Do not run `cargo update` or add dependencies as a fixture-validation step.

## Research fixtures have a separate registry

Phase D `reranking-dev`, `reranking-eval`, `episode-dev` and `episode-eval` fixtures at `1.0.0` are defined in [Rust research fixtures](../../rust/crates/backend/src/research/fixtures.rs), not discovered from these data folders. `FACTORSBT_FIXTURE_DIR` selects the data fixture registry; it does not register arbitrary research jobs. A research-fixture change requires an explicit code/contract scope, updated expected ranking/reward evidence, and appropriate research/replay checks. See the [research interface](../../docs/interfaces/research-jobs.md).
