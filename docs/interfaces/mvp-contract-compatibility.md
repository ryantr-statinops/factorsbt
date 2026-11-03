# MVP 1 contract versioning and compatibility

**Status:** Accepted MVP 1 compatibility rules. They govern the agreed contract design; local implementation and verification do not establish a stable public API.

## One contract set

- [`contracts/openapi.json`](../../contracts/openapi.json) is OpenAPI 3.1.0 and declares JSON Schema Draft 2020-12 as its schema dialect.
- The canonical schemas are the JSON files under [`contracts/schemas`](../../contracts/schemas). OpenAPI references those files directly instead of maintaining copied request/result shapes.
- Every schema has a versioned `$id` under `https://schemas.factorsbt.invalid/v1/`. The `.invalid` domain is an identifier namespace only; it is not a schema download service. Repository tools resolve references from the checked-in files.
- Request, model-exchange, and result documents carry `contract_version: "1.0.0"`. The health response and OpenAPI info use the same release version. The HTTP run paths are prefixed `/v1`.
- A missing `contract_version` is a schema error. A syntactically valid but unsupported value returns `422 CONTRACT_VERSION_UNSUPPORTED` before run creation.

## Wire rules

- Validate with Draft 2020-12 and assert `format` for UUID, URI, and date-time fields. A regular-expression check alone is insufficient for timestamps and URIs.
- Unknown object fields and unknown enum values are rejected. Consumers must not silently ignore a new value that changes financial or lifecycle meaning.
- Financial values are decimal strings with no exponent notation, no leading zeroes except zero itself, and at most 18 fractional digits. Semantic validation also limits precision to 28 significant digits, rejects negative zero, and applies field-specific ranges. JSON binary floating-point values are invalid for financial fields.
- Timestamps are RFC 3339 UTC values ending in `Z`. Event time, availability time, simulation time, and service processing time remain distinct fields.
- A JSON-schema-valid message may still fail semantic validation: references must resolve, output mode must match, collateral must be sufficient, event order must be valid, and evidence paths must remain inside the local run root.
- Contract examples under [`contracts/examples`](../../contracts/examples) are the shared fixtures for schema, Rust, Python, and later integration checks. Invalid schema examples and schema-valid semantic failures are labeled separately.

## Compatibility changes

| Change | Compatibility rule |
|---|---|
| Editorial clarification that changes no wire shape or meaning | Patch documentation without changing the contract version. |
| Add an independent operation while preserving every existing path and schema | May extend `/v1`; update OpenAPI and examples. Existing schemas keep their current `contract_version`. |
| Add, remove, rename, or change the meaning of a field; change an enum, status, error code meaning, timestamp rule, unit, precision, retry behavior, or simulation meaning | Breaking change. Publish a new `/v2` API path and `/v2/` schema-ID namespace with `contract_version: "2.0.0"`. Keep the v1 documents and examples available for existing artifacts. |
| Change only a result's stored evidence bytes | Preserve its original contract version and update its own content digest; do not rewrite old result documents under a newer schema. |

The schemas set `additionalProperties: false`, including for outputs. Even an optional field addition can break an existing strict consumer, so it requires the coordinated major-version path above. A minor version may be used for publication metadata or an additive operation that does not alter an existing schema; it must not imply that existing documents changed shape.

Submission equality is separate from schema compatibility. It ignores JSON object member ordering, preserves array ordering and scalar values exactly, and includes `contract_version`. A changed payload under a previously recorded idempotency key remains a conflict.

## Implementation boundary

The exact operation paths, schemas, lifecycle, error envelope, and compatibility policy are selected for the MVP contract. Phase A implemented `GET /healthz`. Phase B Stage 06 implemented the four `/v1` run operations, local configuration validation, lifecycle and result retrieval, and evidence-integrity checks. These are local implementations without a public stability guarantee. Local artifact references do not define remote artifact retrieval; the typed Python backtest SDK and full MVP acceptance remain in Stage 07.

See the [MVP HTTP API](mvp-http-api.md), [submission behavior](mvp-submissions.md), and [service contract overview](service-contracts.md).
