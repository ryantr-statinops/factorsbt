# Historical data contract v1

**Status:** Implemented for offline schema validation and normalization. It does not provide provider ingestion, persistent dataset storage, or an intraday backtest API.

The historical data contract is separate from the existing synthetic fixture format and run API v1. Its canonical JSON Schema is [`historical-data.v1.schema.json`](../../contracts/schemas/historical-data.v1.schema.json), registered for offline Rust validation and checked by the Python contract suite.

## Records

- A dataset reference identifies an immutable dataset version and the SHA-256 digest of its manifest.
- A manifest records format, source request parameters, ingestion time, timezone, calendar identity/version/source, exact session dates, instrument mappings, and partition references.
- A bar stores instrument/provider identity, a UTC half-open one-minute interval, snapshot availability time, exact decimal text for OHLCV, source identity, and revision number.
- Revision records keep event time and availability time distinct. The selector exposes only revisions available by the requested simulation instant and does not mutate the input snapshot.

The first slice uses `America/New_York`, raw (unadjusted) provider bars, and `available_at = interval_end`. Provider Unix-millisecond interval starts normalize to UTC only when minute-aligned. Request parameters are closed to the documented non-secret fields; credentials must not enter manifests. Rust semantic checks reject invalid minute boundaries, OHLC ordering, negative volume, duplicate mappings, and symbol collisions. Quality diagnostics report missing intervals, duplicates, and rejected timestamp/price rows without echoing provider payloads.

## Compatibility and limits

The Python SDK can build a typed historical dataset reference, but run configuration v1 still accepts only synthetic fixture references. Existing fixture schemas and stored fixture bytes remain unchanged. Intraday session membership and execution timing are provided by the separate D1a clock path; provider acquisition, partition sealing/reading, and SDK-to-backend run submission remain future work in D2 and D4.

Examples under [`contracts/examples/data`](../../contracts/examples/data) are synthetic contract samples. Do not commit provider responses or private manifests.
