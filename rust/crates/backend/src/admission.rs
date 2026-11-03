//! Schema and semantic admission checks for a submitted MVP run.

use crate::model_client::validate_loopback_endpoint;
use factorsbt_contracts::{
    run_configuration::RunConfiguration, validation::validate_run_configuration,
};
use factorsbt_data::{validate_run_coverage, CoverageIssue, Fixture, FixtureRegistry};
use factorsbt_engine::{PortfolioState, SimulationClock};
use serde_json::Value;
use std::{error::Error, fmt, sync::Arc};

#[derive(Debug, Clone)]
pub struct ValidatedRun {
    pub configuration: RunConfiguration,
    pub fixture: Arc<Fixture>,
}

#[derive(Debug)]
pub enum RunAdmissionError {
    SchemaInvalid(Vec<String>),
    InvalidWireConfiguration(String),
    FixtureNotRegistered { fixture_id: String, version: String },
    InvalidModelEndpoint,
    InvalidSimulation(String),
    InvalidFixtureCoverage(Vec<CoverageIssue>),
    InvalidInitialPortfolio(String),
}

impl fmt::Display for RunAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaInvalid(issues) => write!(
                formatter,
                "run configuration violates its schema: {}",
                issues.join("; ")
            ),
            Self::InvalidWireConfiguration(reason) => {
                write!(formatter, "run configuration cannot be decoded: {reason}")
            }
            Self::FixtureNotRegistered {
                fixture_id,
                version,
            } => {
                write!(
                    formatter,
                    "fixture {fixture_id}@{version} is not registered"
                )
            }
            Self::InvalidModelEndpoint => {
                formatter.write_str("model endpoint must use HTTP on a loopback address")
            }
            Self::InvalidSimulation(reason) => {
                write!(formatter, "simulation interval is invalid: {reason}")
            }
            Self::InvalidFixtureCoverage(issues) => {
                let summary = issues
                    .iter()
                    .map(|issue| issue.message.as_str())
                    .collect::<Vec<_>>()
                    .join("; ");
                write!(formatter, "fixture does not cover this run: {summary}")
            }
            Self::InvalidInitialPortfolio(reason) => {
                write!(formatter, "initial portfolio is invalid: {reason}")
            }
        }
    }
}

impl Error for RunAdmissionError {}

/// Validate a raw JSON request before submission identity or queue state is
/// mutated. Schema checks distinguish wire errors from cross-field semantics.
pub fn validate_run(
    value: &Value,
    fixtures: &FixtureRegistry,
) -> Result<ValidatedRun, RunAdmissionError> {
    validate_run_configuration(value).map_err(RunAdmissionError::SchemaInvalid)?;
    let configuration: RunConfiguration = serde_json::from_value(value.clone())
        .map_err(|error| RunAdmissionError::InvalidWireConfiguration(error.to_string()))?;
    validate_loopback_endpoint(&configuration.model.endpoint)
        .map_err(|_| RunAdmissionError::InvalidModelEndpoint)?;
    let fixture =
        fixtures
            .get(&configuration.data)
            .map_err(|_| RunAdmissionError::FixtureNotRegistered {
                fixture_id: configuration.data.fixture_id.as_str().to_owned(),
                version: configuration.data.fixture_version.as_str().to_owned(),
            })?;
    SimulationClock::new(
        &configuration.simulation.start_at,
        &configuration.simulation.end_at,
        &configuration.simulation.calendar_id,
    )
    .map_err(|error| RunAdmissionError::InvalidSimulation(error.to_string()))?;
    validate_run_coverage(&fixture.dataset, &configuration)
        .map_err(RunAdmissionError::InvalidFixtureCoverage)?;
    PortfolioState::initialize(&configuration, &fixture)
        .map_err(|error| RunAdmissionError::InvalidInitialPortfolio(error.to_string()))?;
    Ok(ValidatedRun {
        configuration,
        fixture,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use factorsbt_data::FixtureRegistry;
    use serde_json::json;
    use std::path::Path;

    fn fixture_registry() -> FixtureRegistry {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures/data");
        FixtureRegistry::load(root).unwrap()
    }

    fn valid_value() -> Value {
        serde_json::from_str(include_str!(
            "../../../../contracts/examples/instruments/minimal-valid-spot-run.json"
        ))
        .unwrap()
    }

    #[test]
    fn accepts_shared_config_only_after_resolving_fixture_coverage_and_starting_portfolio() {
        let validated = validate_run(&valid_value(), &fixture_registry()).unwrap();
        assert_eq!(validated.configuration.contract_version, "1.0.0");
        assert_eq!(
            validated.fixture.manifest.fixture_id.as_str(),
            "mvp-synthetic"
        );
        assert_eq!(validated.fixture.manifest.fixture_version.as_str(), "v1");
    }

    #[test]
    fn separates_schema_errors_semantics_endpoint_and_portfolio_failures() {
        let registry = fixture_registry();

        let mut invalid_schema = valid_value();
        invalid_schema["unexpected"] = json!(true);
        assert!(matches!(
            validate_run(&invalid_schema, &registry),
            Err(RunAdmissionError::SchemaInvalid(_))
        ));

        let mut unregistered = valid_value();
        unregistered["data"]["fixture_version"] = json!("missing");
        assert!(matches!(
            validate_run(&unregistered, &registry),
            Err(RunAdmissionError::FixtureNotRegistered { .. })
        ));

        let mut remote_model = valid_value();
        remote_model["model"]["endpoint"] = json!("http://example.com/infer");
        assert!(matches!(
            validate_run(&remote_model, &registry),
            Err(RunAdmissionError::InvalidModelEndpoint)
        ));

        let mut underfunded = valid_value();
        underfunded["initial_portfolio"]["cash"] = json!("0");
        underfunded["initial_portfolio"]["positions"] =
            json!([{"instrument_id":"FUT_A","quantity":"1"}]);
        underfunded["instruments"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "instrument_type":"linear_future",
                "instrument_id":"FUT_A",
                "quote_currency":"USD",
                "price_tick":"0.01",
                "quantity_step":"1",
                "contract_multiplier":"1",
                "fee_bps":"0",
                "fee_per_unit":"0",
                "slippage_bps":"0",
                "underlying_id":"SPOT_A",
                "expiry_at":"2026-01-09T16:00:00Z",
                "initial_margin_per_contract":"1000",
                "maintenance_margin_per_contract":"800"
            }));
        assert_eq!(validate_run_configuration(&underfunded), Ok(()));
        assert!(matches!(
            validate_run(&underfunded, &registry),
            Err(RunAdmissionError::InvalidInitialPortfolio(_))
        ));
    }

    #[test]
    fn classifies_unresolved_underlying_as_semantic_coverage_error() {
        let mut value = valid_value();
        let unresolved: Value = serde_json::from_str(include_str!(
            "../../../../contracts/examples/instruments/unresolved-underlying.semantic-invalid.json"
        ))
        .unwrap();
        value["instruments"] = unresolved["instruments"].clone();
        value["decision_policy"] = unresolved["decision_policy"].clone();
        assert_eq!(validate_run_configuration(&value), Ok(()));
        assert!(matches!(
            validate_run(&value, &fixture_registry()),
            Err(RunAdmissionError::InvalidFixtureCoverage(_))
        ));
    }
}
