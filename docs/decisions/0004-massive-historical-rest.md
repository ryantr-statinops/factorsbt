# ADR 0004: Massive historical REST for the first US equity slice

**Status:** Accepted for the scoped private research slice. Account entitlement and applicable data-use rights remain unverified gates; no provider integration is implemented.

## Context

The first real-data slice needs historical one-minute US equity bars for five symbols over 20 completed regular sessions. The project has no provider connector or account credentials. The selected slice must be reproducible offline after data acquisition, while committed fixtures remain redistributable synthetic data.

Massive's Stocks REST Custom Bars endpoint documents historical OHLCV by ticker, date range, and interval. The provider documents end-of-day recency for Stocks Basic, a five-request-per-minute Basic limit, and no aggregate bar for an interval with no eligible trades. The endpoint can include extended-hours trades and defaults to split-adjusted results. A returned bar's `t` identifies the beginning of its aggregate window. Basic REST access is distinct from Massive stock minute flat files, which are not included in Basic.

The current [Market Data Terms](https://massive.com/legal/market-data-terms-of-service) restrict individual-plan information to personal, non-business, non-commercial use and restrict sharing or redistribution. The provider also describes redistribution as requiring a license that covers it. A published API endpoint or a successful request does not grant rights beyond the applicable agreements.

## Decision

Use Massive Stocks Basic historical REST Custom Bars as the provider candidate for the first private research slice, subject to all of the following before online ingestion:

1. The user obtains an account and verifies that the account can retrieve the five selected symbols and the required dates through the authenticated endpoint.
2. The account holder checks the terms and any exchange/provider agreements applicable to their status and intended use. This selection covers only the user's own personal, non-business research if that is what the applicable license permits.
3. If the workflow will support an organization, commercial use, other users, publication, or redistribution, obtain written rights that expressly cover that use first. An individual Basic plan is not treated as sufficient for those cases.
4. Keep the API credential in a local secret store or environment reference; never put it in a dataset manifest, log, source file, or committed example.
5. Keep downloaded source data and private manifests outside Git. Tests and shared examples use synthetic data unless a separately verified license explicitly permits distribution.

Do not silently fall back to another vendor or use flat files when access is denied. If entitlement or rights are unavailable, keep D2 online ingestion blocked and continue offline work with synthetic inputs.

## Acquisition boundary

- Query one ticker and a bounded historical range per request; honor the plan's rate limit and server pagination metadata.
- Request raw (not split-adjusted) bars explicitly for this slice. Record the exact request parameters and ingestion time in a private manifest.
- Filter to the NYSE/Nasdaq regular session using the session calendar; the endpoint may return pre-market and after-hours aggregates.
- Treat an absent aggregate as a missing interval, not a zero-volume or forward-filled bar. Preserve the distinction between no bar, request failure, and incomplete coverage.
- The endpoint's historical snapshot can include later corrections. Do not claim exact point-in-time reconstruction of what the provider published in the past. A new download is a new immutable snapshot version.

## Consequences

- The candidate matches the selected delayed historical workflow, but free-tier recency means current-day bars are not assumed to be available.
- A single date-range request per ticker would require at least five requests, with more possible due to pagination or bounded-range policy. The Basic request limit requires throttling and makes entitlement verification part of D02, not a guessed guarantee.
- The private dataset is not a repository fixture. Offline replay can use a local licensed copy only while the license permits retaining and using it.
- The repository may implement an adapter only after account access and usage rights are verified. Schema and engine work can proceed against synthetic examples without an account.

## References

- [Massive Custom Bars documentation](https://massive.com/docs/rest/stocks/aggregates/custom-bars)
- [Stocks Basic historical availability](https://massive.com/knowledge-base/article/on-the-free-basic-plan-when-can-i-pull-the-previous-trading-days-data)
- [Massive Market Data Terms](https://massive.com/legal/market-data-terms-of-service)
- [Massive redistribution guidance](https://massive.com/knowledge-base/article/how-can-i-redistribute-massives-market-data)
- [First real-data research slice](../data/README.md)
