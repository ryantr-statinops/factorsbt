# Historical bar time and session semantics

**Status:** D0 semantics are Accepted. The versioned D1a clock, coverage checks, strict-later scheduling, and spot session-close flatten primitives are Implemented and locally verified. Full intraday run wiring and an authoritative maintained exchange-calendar catalog are not implemented.

## Timestamp model

Each minute bar describes a half-open interval `[interval_start, interval_end)`. Store both boundaries as UTC instants. Interpret exchange sessions in the IANA timezone `America/New_York`; do not encode Eastern time as a fixed UTC offset. Keep the provider's source timestamp and the normalized UTC interval boundaries in provenance.

Massive Custom Bars returns `t` as the Unix-millisecond timestamp for the start of the aggregate window. For a one-minute bar with `t = 14:30:00Z`, the interval is `[14:30:00Z, 14:31:00Z)`. During US daylight time that example is 10:30–10:31 ET; during standard time it is 09:30–09:31 ET. Convert through the timezone database so DST changes do not shift the local session rules.

Keep three notions of time distinct:

| Field | Meaning |
| --- | --- |
| `interval_start` / `interval_end` | The market interval represented by the OHLCV bar. |
| `available_at` | For this historical-snapshot mode, an explicit assumption equal to `interval_end`; it is not proof of the provider's original publication time. |
| `ingested_at` | When this particular snapshot was downloaded and sealed locally. |

The provider can revise historical bars after the fact. A later download is a new immutable snapshot with its own `ingested_at` and identity, not an overwrite that changes an earlier backtest's inputs. The first slice therefore does not claim exact point-in-time reconstruction of what was known historically.

## Session calendar

The selected symbols are Nasdaq-listed. The initial dataset contains regular-session bars only, interpreted using the official Nasdaq US trading calendar applicable to each frozen session date. The regular session currently runs 09:30–16:00 ET on full sessions, with holidays and scheduled early closes such as 13:00 ET. Pin a versioned calendar identifier and calendar source/version with the dataset; do not infer sessions from weekdays or a fixed UTC schedule. Recheck the effective Nasdaq rules and official schedule when implementing the runtime because exchange hours and calendars can change.

The session is half-open: a normal full session contains minute intervals starting at 09:30 through 15:59 ET and ending at 16:00 ET. A 13:00 ET early close has its final regular-session interval start at 12:59 and end at 13:00. Count actual calendar intervals, not a hard-coded 390 bars per session. Exclude pre-market and after-hours bars even if the provider returns them. The D1a clock consumes an explicit, versioned offline schedule; its contract records each session date, UTC open/close instants, session type, source, and calendar version. Tests use a synthetic schedule; it does not establish an authoritative complete calendar dataset.

Official references: [Nasdaq US holiday and trading-hour schedule](https://www.nasdaq.com/market-activity/stock-market-holiday-schedule), [Nasdaq trading calendar](https://www.nasdaqtrader.com/Trader.aspx?id=Calendar), and [Massive Custom Bars](https://massive.com/docs/rest/stocks/aggregates/custom-bars).

## Decisions and execution eligibility

- Make a decision only after the input bar's `interval_end`, when its OHLCV is assumed available in this mode.
- An order may use a bar's open only if `bar.interval_start > decision_time`. Equality is ineligible. For example, a decision at 09:31 ET cannot fill at the bar starting 09:31; the earliest candidate starts at 09:32 ET.
- At the final regular-session bar close, the engine primitive can flatten spot positions using the final bar close and configured costs. It fails atomically if any required close is absent or invalid. The flatten is a deterministic session boundary action, not a new model decision based on data after the close.
- Do not carry positions or pending orders across sessions. Cancel an order that has no eligible in-session fill. If the final required close is absent, fail the run with a coverage error; do not invent a close price or claim the portfolio is flat.
- Keep the existing synthetic daily UTC clock and strict v1 run contract unchanged. Add intraday event ordering and session rules only under the separately versioned contract planned for D1a.

## Missing intervals and quality

The provider may omit an aggregate when no eligible trades occur in the interval. Preserve the gap and report it as a data-quality condition. Do not create a zero-volume bar, forward-fill OHLC, or reinterpret a missing provider response as a no-trade interval. Ingestion must distinguish an absent aggregate from a failed/incomplete request before a dataset can be sealed.

The 20-session selection is frozen by its exact session-date list and calendar version. Re-evaluating “most recent 20” creates a different dataset identity; it must not mutate the original snapshot. D1a does not yet submit complete intraday runs through the SDK/API or load an authoritative calendar catalog.
