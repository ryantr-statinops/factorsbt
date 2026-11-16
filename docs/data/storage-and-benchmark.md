# Historical dataset storage and benchmark

**Status:** Accepted design for the first US equity slice; not implemented. The benchmark below is a local synthetic observation, not a provider-data performance guarantee.

## Selected local format

Use a versioned manifest that references immutable JSON Lines (`.jsonl`) partitions organized by instrument and session. Each line is one normalized bar. A sealed manifest records the dataset and format versions, instrument mapping, exact session-date list, calendar identity/version, source and request parameters, raw/adjusted choice, `ingested_at`, partition path, byte size, row count, and SHA-256 digest. Use decimal strings for price and volume fields when the contract requires exact decimal representation. Readers verify manifest identity, safe relative paths, size, and digest before exposing data.

Write new partition files first and publish the completed manifest last. Never overwrite a sealed dataset. A corrected re-download creates a new dataset identity/version, leaving the original available for offline replay as long as the data license permits local retention. Private provider partitions and their manifests stay outside Git; committed tests use synthetic fixtures only.

JSONL is selected for bounded-memory streaming and simple symbol/session partitioning. This is not a claim that JSONL has the fastest full-dataset scan. If later measured workloads justify a columnar format, compare it against the same partition selection, validation, memory, and compatibility requirements before adding a dependency.

## Synthetic benchmark method

The benchmark generated 39,000, 390,000, and 3,900,000 deterministic synthetic bar records. Each record used a small shape with symbol index, timestamp integer, decimal-text OHLC values, and integer volume. The same records were written either as one JSON object containing a `bars` array or as one JSON object per JSONL line. No provider responses, API keys, or committed market data were used.

For each format and size, the harness wrote one temporary file, then launched a fresh Python process for one warm-up read and five timed reads. The JSON-array reader used `json.load` and summed volume over the decoded array; the JSONL reader parsed and consumed one line at a time, summing the same values without retaining all rows. Per-process peak working set was measured with `psutil`. Reported read values are median, minimum, and maximum over the five timed runs. Filesystem cache was warm; this does not measure cold remote storage or network ingestion. Serialization time is one observed write per generated file, not a five-run statistic.

### Observed results

Environment: Windows 11 x64, Python 3.13.13 64-bit, `psutil` 7.2.2, 31.8 GiB total RAM, approximately 14.2 GiB available before the run. Disk and CPU model were not captured. Values are for synthetic local files only.

| Records | Format | File size | Write time | Read median (min–max) | Peak process working set |
| ---: | --- | ---: | ---: | ---: | ---: |
| 39,000 | JSON array | 3,140,150 B | 0.231 s | 0.071 s (0.068–0.089) | 43.3 MiB |
| 39,000 | JSONL | 3,101,138 B | 0.264 s | 0.126 s (0.122–0.129) | 19.9 MiB |
| 390,000 | JSON array | 31,918,432 B | 2.692 s | 1.334 s (0.769–1.456) | 252.5 MiB |
| 390,000 | JSONL | 31,528,420 B | 3.537 s | 1.265 s (1.223–1.390) | 19.9 MiB |
| 3,900,000 | JSON array | 323,238,995 B | 27.980 s | 8.188 s (8.109–8.714) | 2,343.3 MiB |
| 3,900,000 | JSONL | 319,338,983 B | 26.844 s | 14.325 s (14.050–15.080) | 20.0 MiB |

Both readers produced the same volume checksum in each case. At 3.9 million records, streaming JSONL used about 117 times less peak process working set, while taking about 1.75 times as long to scan the entire file. File size was about 1.2% smaller in this synthetic shape. The result supports JSONL for bounded-memory processing; it does not show a full-scan speed advantage. The per-instrument/session partition layout is intended to avoid reading unrelated bars, but selective partition reads were not benchmarked here.

## Decision boundary and follow-up

The manifest-plus-JSONL design is sufficient for the initial 5-symbol, 20-session slice and does not require a Rust or Python storage dependency. D1 implementation must validate round-trip decimal and timestamp fidelity, path safety, digests, incomplete writes, and v1 synthetic fixture compatibility.

The benchmark is not an acceptance result for real provider data, compression, cold cache, concurrent readers, engine simulation time, import throughput across pagination, or long-running storage growth. D4 must measure the actual selected workload and record its environment. Revisit JSONL versus a columnar reader only if those measurements show a concrete bottleneck.
