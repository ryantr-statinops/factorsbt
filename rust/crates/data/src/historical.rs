//! Offline normalization and validation for versioned historical records.

use factorsbt_contracts::{HistoricalInstrumentMapping, InstrumentId};
use std::{
    collections::{BTreeMap, HashMap},
    error::Error,
    fmt,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoricalDataError {
    DuplicateInstrument(String),
    ProviderSymbolCollision(String),
    UnknownInstrumentMapping(String),
    InvalidMapping(String),
    InvalidBar(String),
    DuplicateRevision(String),
}

impl fmt::Display for HistoricalDataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateInstrument(id) => write!(f, "instrument mapping duplicates {id}"),
            Self::ProviderSymbolCollision(symbol) => {
                write!(f, "provider symbol collision for {symbol}")
            }
            Self::UnknownInstrumentMapping(symbol) => {
                write!(f, "no instrument mapping for {symbol}")
            }
            Self::InvalidMapping(id) => write!(f, "instrument mapping {id} is invalid"),
            Self::InvalidBar(reason) => write!(f, "historical bar is invalid: {reason}"),
            Self::DuplicateRevision(identity) => {
                write!(f, "duplicate historical revision {identity}")
            }
        }
    }
}

impl Error for HistoricalDataError {}

/// Validate mappings and return provider/symbol lookups with collision checks.
pub fn validate_instrument_mappings(
    mappings: &[HistoricalInstrumentMapping],
) -> Result<BTreeMap<(String, String), HistoricalInstrumentMapping>, HistoricalDataError> {
    let mut by_provider_symbol = BTreeMap::new();
    let mut instrument_ids = HashMap::new();
    for mapping in mappings {
        let id = mapping.instrument_id.as_str();
        if mapping.provider_symbol.is_empty()
            || mapping.exchange_mic.len() != 4
            || !mapping
                .exchange_mic
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
            || mapping.currency.len() != 3
            || !mapping.currency.bytes().all(|b| b.is_ascii_uppercase())
        {
            return Err(HistoricalDataError::InvalidMapping(id.to_owned()));
        }
        if instrument_ids.insert(id.to_owned(), ()).is_some() {
            return Err(HistoricalDataError::DuplicateInstrument(id.to_owned()));
        }
        let key = (
            mapping.provider.as_str().to_owned(),
            mapping.provider_symbol.clone(),
        );
        if by_provider_symbol
            .insert(key.clone(), mapping.clone())
            .is_some()
        {
            return Err(HistoricalDataError::ProviderSymbolCollision(format!(
                "{}:{}",
                key.0, key.1
            )));
        }
    }
    Ok(by_provider_symbol)
}

/// Resolve an incoming provider symbol to the stable FactorsBT instrument ID.
pub fn normalize_instrument_id(
    provider: &str,
    provider_symbol: &str,
    mappings: &[HistoricalInstrumentMapping],
) -> Result<InstrumentId, HistoricalDataError> {
    let index = validate_instrument_mappings(mappings)?;
    index
        .get(&(provider.to_owned(), provider_symbol.to_owned()))
        .map(|mapping| mapping.instrument_id.clone())
        .ok_or_else(|| {
            HistoricalDataError::UnknownInstrumentMapping(format!("{provider}:{provider_symbol}"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use factorsbt_contracts::{Identifier, InstrumentId};

    fn mapping(provider_symbol: &str, instrument_id: &str) -> HistoricalInstrumentMapping {
        HistoricalInstrumentMapping {
            instrument_id: InstrumentId::new(instrument_id).unwrap(),
            provider: Identifier::new("provider-a").unwrap(),
            provider_symbol: provider_symbol.into(),
            exchange_mic: "XNAS".into(),
            currency: "USD".into(),
            mapping_version: Identifier::new("v1").unwrap(),
        }
    }

    #[test]
    fn maps_provider_symbol_to_stable_instrument_identity() {
        let mappings = vec![mapping("AAPL", "XNAS_AAPL")];
        assert_eq!(
            normalize_instrument_id("provider-a", "AAPL", &mappings)
                .unwrap()
                .as_str(),
            "XNAS_AAPL"
        );
    }

    #[test]
    fn rejects_instrument_and_provider_symbol_collisions() {
        assert!(matches!(
            validate_instrument_mappings(&[
                mapping("AAPL", "XNAS_AAPL"),
                mapping("MSFT", "XNAS_AAPL")
            ]),
            Err(HistoricalDataError::DuplicateInstrument(_))
        ));
        assert!(matches!(
            validate_instrument_mappings(&[
                mapping("AAPL", "XNAS_AAPL"),
                mapping("AAPL", "XNAS_MSFT")
            ]),
            Err(HistoricalDataError::ProviderSymbolCollision(_))
        ));
    }
}
