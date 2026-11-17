//! Versioned references and records for offline historical market data.

use crate::{DecimalString, Identifier, InstrumentId, UtcTimestamp};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoricalDatasetReference {
    pub contract_version: String,
    pub dataset_id: Identifier,
    pub dataset_version: Identifier,
    pub manifest_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoricalPartitionReference {
    pub instrument_id: InstrumentId,
    pub session_date: String,
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub row_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoricalInstrumentMapping {
    pub instrument_id: InstrumentId,
    pub provider: Identifier,
    pub provider_symbol: String,
    pub exchange_mic: String,
    pub currency: String,
    pub mapping_version: Identifier,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoricalBar {
    pub instrument_id: InstrumentId,
    pub provider_symbol: String,
    pub interval_start: UtcTimestamp,
    pub interval_end: UtcTimestamp,
    pub available_at: UtcTimestamp,
    pub open: DecimalString,
    pub high: DecimalString,
    pub low: DecimalString,
    pub close: DecimalString,
    pub volume: DecimalString,
    pub source_identity: Identifier,
    pub source_revision: u64,
}

/// A source's revision of one market interval, available to a run only at its
/// explicit availability instant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoricalRevision {
    pub source_identity: Identifier,
    pub source_revision: u64,
    pub event_time: UtcTimestamp,
    pub available_at: UtcTimestamp,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versioned_dataset_reference_round_trips() {
        let reference = HistoricalDatasetReference {
            contract_version: "1.0.0".into(),
            dataset_id: Identifier::new("us-equities-1m").unwrap(),
            dataset_version: Identifier::new("snapshot-2026-11-16").unwrap(),
            manifest_sha256: "a".repeat(64),
        };
        let encoded = serde_json::to_vec(&reference).unwrap();
        let decoded: HistoricalDatasetReference = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, reference);
    }
}
