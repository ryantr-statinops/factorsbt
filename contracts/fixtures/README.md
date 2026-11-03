# Controlled MVP fixture datasets

**Status:** Implemented fixture data format and datasets; loader/runtime use is implemented in the data stage.

Each folder under [`data`](data) contains a `manifest.json` and `dataset.json`. The manifest identifies fixture format, fixture ID/version, a relative dataset path, and SHA-256 of the exact dataset bytes. The dataset keeps decision observations separate from open execution prices, initial marks, and settlement events.

| Fixture | Acceptance use |
| --- | --- |
| `mvp-synthetic` | `BASE-01`–`03`, `TIME-01`, `EXEC-01`, `END-01`, and shared instrument/observation inputs. Output-mode variations in `OUT-01`, fee/slippage settings, and `RISK-01` portfolio variants are run configuration/model-response changes over this data. |
| `mvp-risk-02` | Futures price path for `RISK-02` with Jan 8 open recovery to 20. The instrument multiplier and margins are configured by that acceptance case. |
| `mvp-risk-02-fail` | The same stress with Jan 8 open remaining at 18, expected to fail liquidation. |
| `mvp-opt-01` | Point-in-time rate `0.05` and volatility `0.2` inputs for the one-year `OPT-01` pricing reference case. |
| `mvp-settle-01` | `SETTLE-01` American call assignment event at Jan 7 close. |
| `mvp-settle-02-itm` | `SETTLE-02` in-the-money expiry with no exercise/assignment event; run must fail for incomplete settlement input. |
| `mvp-settle-02-otm` | Out-of-the-money expiry with no event; the option may expire worthless. |

The baseline observations include two revisions of the delayed `TIME-01` news item. Its event time is Jan 6 at 15:00 UTC, revision 1 becomes available at 17:00 that day, and revision 2 becomes available Jan 7 at 17:00. Jan 6 execution opens are separate data rows; for example, `SPOT_A` opens at 100.50 and is not available to the Jan 5 close snapshot.

The files contain only synthetic values. A manifest digest must be regenerated after any byte change to its dataset. The Python contract test verifies every manifest, dataset schema, identity pair, and digest.
