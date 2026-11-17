use factorsbt_contracts::{
    validate_historical_data_document, HistoricalBar, HistoricalInstrumentMapping, Identifier,
    InstrumentId,
};
use factorsbt_data::{
    normalize_instrument_id, validate_historical_bar_prices, validate_historical_bar_timestamps,
    FixtureRegistry,
};
use serde_json::Value;
use std::path::Path;

#[test]
fn schema_normalization_and_existing_v1_fixtures_work_together() {
    let value: Value = serde_json::from_str(include_str!(
        "../../../../contracts/examples/data/bar.valid.json"
    ))
    .unwrap();
    validate_historical_data_document(&value).unwrap();
    let bar: HistoricalBar = serde_json::from_value(value).unwrap();
    validate_historical_bar_timestamps(&bar).unwrap();
    validate_historical_bar_prices(&bar).unwrap();

    let mapping = HistoricalInstrumentMapping {
        instrument_id: InstrumentId::new("XNAS_AAPL").unwrap(),
        provider: Identifier::new("provider-a").unwrap(),
        provider_symbol: "AAPL".into(),
        exchange_mic: "XNAS".into(),
        currency: "USD".into(),
        mapping_version: Identifier::new("v1").unwrap(),
    };
    assert_eq!(
        normalize_instrument_id("provider-a", "AAPL", &[mapping]).unwrap(),
        bar.instrument_id
    );

    let fixture_root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures/data");
    assert_eq!(FixtureRegistry::load(fixture_root).unwrap().len(), 9);
}

#[test]
fn schema_rejects_missing_volume_without_rewriting_it_as_zero() {
    let invalid: Value = serde_json::from_str(include_str!(
        "../../../../contracts/examples/data/bar.missing-volume.schema-invalid.json"
    ))
    .unwrap();
    assert!(validate_historical_data_document(&invalid).is_err());
}

#[test]
fn manifest_provenance_is_versioned_and_rejects_secret_fields() {
    let valid: Value = serde_json::from_str(include_str!(
        "../../../../contracts/examples/data/manifest.valid.json"
    ))
    .unwrap();
    validate_historical_data_document(&valid).unwrap();
    let invalid: Value = serde_json::from_str(include_str!(
        "../../../../contracts/examples/data/manifest.secret-request-field.schema-invalid.json"
    ))
    .unwrap();
    assert!(validate_historical_data_document(&invalid).is_err());
}
