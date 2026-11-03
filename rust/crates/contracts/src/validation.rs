//! Runtime validation against the repository's versioned local JSON Schemas.

use jsonschema::Registry;
use serde_json::Value;
use std::error::Error;

const BASE: &str = "https://schemas.factorsbt.invalid/v1/";
const COMMON: &str = include_str!("../../../../contracts/schemas/common.schema.json");
const INSTRUMENT: &str = include_str!("../../../../contracts/schemas/instrument.schema.json");
const OBSERVATION: &str = include_str!("../../../../contracts/schemas/observation.schema.json");
const MODEL_REQUEST: &str = include_str!("../../../../contracts/schemas/model-request.schema.json");
const MODEL_RESPONSE: &str =
    include_str!("../../../../contracts/schemas/model-response.schema.json");
const RUN_CONFIGURATION: &str =
    include_str!("../../../../contracts/schemas/run-configuration.schema.json");
const RUN_RESULT: &str = include_str!("../../../../contracts/schemas/run-result.schema.json");
const SNAPSHOT: &str = include_str!("../../../../contracts/schemas/snapshot.schema.json");
const ENGINE_OUTCOME: &str =
    include_str!("../../../../contracts/schemas/engine-outcome.schema.json");

#[derive(Debug)]
pub struct SchemaRegistryError(String);

impl std::fmt::Display for SchemaRegistryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for SchemaRegistryError {}

/// Build all known schemas into an offline registry. References absent from
/// this list fail closed; validation never downloads a schema at runtime.
pub fn registry() -> Result<Registry<'static>, SchemaRegistryError> {
    let schemas = [
        ("common.schema.json", COMMON),
        ("instrument.schema.json", INSTRUMENT),
        ("observation.schema.json", OBSERVATION),
        ("model-request.schema.json", MODEL_REQUEST),
        ("model-response.schema.json", MODEL_RESPONSE),
        ("run-configuration.schema.json", RUN_CONFIGURATION),
        ("run-result.schema.json", RUN_RESULT),
        ("snapshot.schema.json", SNAPSHOT),
        ("engine-outcome.schema.json", ENGINE_OUTCOME),
    ];
    let mut builder = Registry::new();
    for (name, source) in schemas {
        let schema: Value = serde_json::from_str(source).map_err(|error| {
            SchemaRegistryError(format!("embedded schema {name} is invalid: {error}"))
        })?;
        builder = builder
            .add(format!("{BASE}{name}"), schema)
            .map_err(|error| {
                SchemaRegistryError(format!("cannot register schema {name}: {error}"))
            })?;
    }
    builder.prepare().map_err(|error| {
        SchemaRegistryError(format!("cannot prepare local schema registry: {error}"))
    })
}

/// Return all schema errors. Cross-field relationships such as underlying
/// existence remain semantic validation responsibilities.
pub fn validate_run_configuration(value: &Value) -> Result<(), Vec<String>> {
    let registry = registry().map_err(|error| vec![error.to_string()])?;
    let schema: Value = serde_json::from_str(RUN_CONFIGURATION).map_err(|error| {
        vec![format!(
            "embedded run configuration schema is invalid: {error}"
        )]
    })?;
    let validator = jsonschema::draft202012::options()
        .with_registry(&registry)
        .offline()
        .should_validate_formats(true)
        .build(&schema)
        .map_err(|error| vec![format!("cannot build run configuration validator: {error}")])?;
    let errors = validator
        .iter_errors(value)
        .map(|error| format!("{} at {}", error, error.instance_path()))
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_configuration() -> Value {
        serde_json::from_str(include_str!(
            "../../../../contracts/examples/instruments/minimal-valid-spot-run.json"
        ))
        .unwrap()
    }

    #[test]
    fn accepts_the_shared_valid_run_example() {
        assert_eq!(validate_run_configuration(&minimal_configuration()), Ok(()));
    }

    #[test]
    fn rejects_schema_invalid_configuration() {
        let mut value = minimal_configuration();
        value["unexpected"] = Value::Bool(true);
        assert!(validate_run_configuration(&value).is_err());
    }

    #[test]
    fn asserts_date_time_formats_without_network_retrieval() {
        let mut value = minimal_configuration();
        value["simulation"]["start_at"] = Value::String("yesterday".into());
        assert!(validate_run_configuration(&value).is_err());
    }
}
