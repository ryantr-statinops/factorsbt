//! Build immutable engine inputs for the bounded synthetic research fixtures.

use super::fixtures::ResearchFixture;
use factorsbt_contracts::{
    instruments::{Currency, Instrument, SpotInstrument},
    outcomes::DataFixtureRef,
    run_configuration::{InitialPortfolio, RunConfiguration, SimulationConfiguration},
    temporal::{BarPayload, Observation, ObservationKind, ObservationPayload},
    DecimalString, Identifier, InstrumentId, UtcTimestamp,
};
use factorsbt_data::{ExecutionPrice, Fixture, FixtureDataset, FixtureManifest, InitialMark};
use rust_decimal::Decimal;
use std::sync::Arc;

pub(crate) struct EngineInputs {
    pub(crate) configuration: RunConfiguration,
    pub(crate) fixture: Fixture,
}

pub(crate) fn build_inputs(
    research: &ResearchFixture,
    fee_bps: Decimal,
) -> Result<EngineInputs, String> {
    let first = research
        .points
        .first()
        .ok_or("research fixture has no prices")?;
    let last = research
        .points
        .last()
        .ok_or("research fixture has no prices")?;
    let start_at = UtcTimestamp::new(first.close_at.clone()).map_err(|e| e.to_string())?;
    let end_at = UtcTimestamp::new(last.close_at.clone()).map_err(|e| e.to_string())?;
    let fixture_id = Identifier::new(research.fixture_id.clone()).map_err(|e| e.to_string())?;
    let fixture_version = Identifier::new(research.version.clone()).map_err(|e| e.to_string())?;
    let instruments = research
        .instruments
        .iter()
        .map(|id| {
            Ok(Instrument::Spot(SpotInstrument {
                instrument_id: InstrumentId::new(id.clone()).map_err(|e| e.to_string())?,
                quote_currency: Currency::Usd,
                price_tick: DecimalString::new("0.01").map_err(|e| e.to_string())?,
                quantity_step: DecimalString::new("1").map_err(|e| e.to_string())?,
                contract_multiplier: DecimalString::new("1").map_err(|e| e.to_string())?,
                fee_bps: DecimalString::new(fee_bps.normalize().to_string())
                    .map_err(|e| e.to_string())?,
                fee_per_unit: DecimalString::new("0").map_err(|e| e.to_string())?,
                slippage_bps: DecimalString::new("0").map_err(|e| e.to_string())?,
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut configuration: RunConfiguration = serde_json::from_str(include_str!(
        "../../../../../contracts/examples/instruments/minimal-valid-spot-run.json"
    ))
    .map_err(|error| error.to_string())?;
    configuration.contract_version = "1.0.0".into();
    configuration.simulation = SimulationConfiguration {
        start_at: start_at.clone(),
        end_at,
        calendar_id: "research-synthetic-daily".into(),
    };
    configuration.initial_portfolio = InitialPortfolio {
        cash: DecimalString::new(research.initial_cash.normalize().to_string())
            .map_err(|e| e.to_string())?,
        positions: Vec::new(),
    };
    configuration.instruments = instruments;
    configuration.data = DataFixtureRef {
        fixture_id: fixture_id.clone(),
        fixture_version: fixture_version.clone(),
    };

    let mut observations = Vec::new();
    let mut initial_marks = Vec::new();
    let mut execution_prices = Vec::new();
    for (point_index, point) in research.points.iter().enumerate() {
        let close = UtcTimestamp::new(point.close_at.clone()).map_err(|e| e.to_string())?;
        let open = UtcTimestamp::new(open_timestamp(&point.close_at)).map_err(|e| e.to_string())?;
        for id in &research.instruments {
            let (mark, execution) = if id == "SPOT_A" {
                (point.close_a, point.open_a)
            } else {
                (point.close_b, point.open_b)
            };
            let observation_id = Identifier::new(format!("R-{id}-{}", point_index + 1))
                .map_err(|e| e.to_string())?;
            observations.push(Observation {
                observation_id: observation_id.clone(),
                kind: ObservationKind::Bar,
                instrument_id: Some(InstrumentId::new(id.clone()).map_err(|e| e.to_string())?),
                source_id: Identifier::new("research-fixture").map_err(|e| e.to_string())?,
                source_version: fixture_version.clone(),
                revision: 1,
                event_time: close.clone(),
                available_at: close.clone(),
                payload: ObservationPayload::Bar(BarPayload {
                    interval_start: open.clone(),
                    interval_end: close.clone(),
                    open: DecimalString::new(execution.normalize().to_string())
                        .map_err(|e| e.to_string())?,
                    high: DecimalString::new(mark.max(execution).normalize().to_string())
                        .map_err(|e| e.to_string())?,
                    low: DecimalString::new(mark.min(execution).normalize().to_string())
                        .map_err(|e| e.to_string())?,
                    close: DecimalString::new(mark.normalize().to_string())
                        .map_err(|e| e.to_string())?,
                    volume: None,
                }),
            });
            if point_index == 0 {
                initial_marks.push(InitialMark {
                    instrument_id: InstrumentId::new(id.clone()).map_err(|e| e.to_string())?,
                    mark: DecimalString::new(mark.normalize().to_string())
                        .map_err(|e| e.to_string())?,
                    available_at: close.clone(),
                    observation_id: observation_id.clone(),
                    source_revision: 1,
                });
            }
            if point_index > 0 {
                execution_prices.push(ExecutionPrice {
                    instrument_id: InstrumentId::new(id.clone()).map_err(|e| e.to_string())?,
                    session_open: open.clone(),
                    price: DecimalString::new(execution.normalize().to_string())
                        .map_err(|e| e.to_string())?,
                    available_at: open.clone(),
                    observation_id,
                    source_revision: 1,
                });
            }
        }
    }
    let dataset = FixtureDataset {
        format_version: "1.1.0".into(),
        fixture_id: fixture_id.clone(),
        fixture_version: fixture_version.clone(),
        calendar_id: configuration.simulation.calendar_id.clone(),
        observations,
        initial_marks,
        execution_prices,
        settlement_events: Vec::new(),
    };
    let manifest = FixtureManifest {
        format_version: "1.1.0".into(),
        fixture_id,
        fixture_version,
        dataset_path: "dataset.json".into(),
        dataset_sha256: "research-in-memory".into(),
    };
    let fixture = Fixture {
        manifest,
        digest_sha256: "research-in-memory".into(),
        dataset: Arc::new(dataset),
        manifest_bytes: Arc::from([]),
        dataset_bytes: Arc::from([]),
    };
    Ok(EngineInputs {
        configuration,
        fixture,
    })
}

pub(crate) fn open_timestamp(close: &str) -> String {
    close
        .split_once('T')
        .map(|(date, _)| format!("{date}T09:00:00Z"))
        .unwrap_or_else(|| close.to_owned())
}
