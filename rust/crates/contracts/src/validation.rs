//! Runtime validation against the repository's versioned local JSON Schemas.

use jsonschema::Registry;
use serde_json::Value;
use std::error::Error;

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
const FIXTURE_MANIFEST: &str =
    include_str!("../../../../contracts/fixtures/schemas/manifest.v1.schema.json");
const FIXTURE_DATASET: &str =
    include_str!("../../../../contracts/fixtures/schemas/dataset.v1.schema.json");
const RESEARCH_JOB_REQUEST: &str =
    include_str!("../../../../contracts/schemas/research-job-request.schema.json");
const RESEARCH_JOB_STATUS: &str =
    include_str!("../../../../contracts/schemas/research-job-status.schema.json");
const RESEARCH_JOB_RESULT: &str =
    include_str!("../../../../contracts/schemas/research-job-result.schema.json");

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
        ("manifest.v1.schema.json", FIXTURE_MANIFEST),
        ("dataset.v1.schema.json", FIXTURE_DATASET),
        ("research-job-request.schema.json", RESEARCH_JOB_REQUEST),
        ("research-job-status.schema.json", RESEARCH_JOB_STATUS),
        ("research-job-result.schema.json", RESEARCH_JOB_RESULT),
    ];
    let mut builder = Registry::new();
    for (name, source) in schemas {
        let schema: Value = serde_json::from_str(source).map_err(|error| {
            SchemaRegistryError(format!("embedded schema {name} is invalid: {error}"))
        })?;
        let id = schema
            .get("$id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| SchemaRegistryError(format!("embedded schema {name} has no $id")))?;
        builder = builder.add(id, schema).map_err(|error| {
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
    validate_document(RUN_CONFIGURATION, "run configuration", value)
}

/// Validate a successful run result against the versioned local schema.
pub fn validate_run_result_document(value: &Value) -> Result<(), Vec<String>> {
    validate_document(RUN_RESULT, "run result", value)
}

/// Validate a model response against the versioned local Draft 2020-12 schema.
pub fn validate_model_response_document(value: &Value) -> Result<(), Vec<String>> {
    validate_document(MODEL_RESPONSE, "model response", value)
}

/// Validate the internal, versioned fixture manifest format.
pub fn validate_fixture_manifest(value: &Value) -> Result<(), Vec<String>> {
    validate_document(FIXTURE_MANIFEST, "fixture manifest", value)
}

/// Validate the internal, versioned fixture dataset format.
pub fn validate_fixture_dataset(value: &Value) -> Result<(), Vec<String>> {
    validate_document(FIXTURE_DATASET, "fixture dataset", value)
}

pub fn validate_research_job_request(value: &Value) -> Result<(), Vec<String>> {
    validate_document(RESEARCH_JOB_REQUEST, "research job request", value)
}

pub fn validate_research_job_status(value: &Value) -> Result<(), Vec<String>> {
    validate_document(RESEARCH_JOB_STATUS, "research job status", value)
}

pub fn validate_research_job_result(value: &Value) -> Result<(), Vec<String>> {
    validate_document(RESEARCH_JOB_RESULT, "research job result", value)
}

fn validate_document(source: &str, name: &str, value: &Value) -> Result<(), Vec<String>> {
    let registry = registry().map_err(|error| vec![error.to_string()])?;
    let schema: Value = serde_json::from_str(source)
        .map_err(|error| vec![format!("embedded {name} schema is invalid: {error}")])?;
    let validator = jsonschema::draft202012::options()
        .with_registry(&registry)
        .offline()
        .should_validate_formats(true)
        .build(&schema)
        .map_err(|error| vec![format!("cannot build {name} validator: {error}")])?;
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
    fn validates_shared_model_response_examples_and_rejects_schema_errors() {
        let valid: Value = serde_json::from_str(include_str!(
            "../../../../contracts/examples/model/scores-response.valid.json"
        ))
        .unwrap();
        assert_eq!(validate_model_response_document(&valid), Ok(()));

        let invalid: Value = serde_json::from_str(include_str!(
            "../../../../contracts/examples/model/response-missing-snapshot.schema-invalid.json"
        ))
        .unwrap();
        assert!(validate_model_response_document(&invalid).is_err());
    }

    #[test]
    fn rejects_schema_invalid_configuration() {
        let mut value = minimal_configuration();
        value["unexpected"] = Value::Bool(true);
        assert!(validate_run_configuration(&value).is_err());
    }

    #[test]
    fn validates_shared_successful_result_and_rejects_missing_evidence_digest() {
        let valid: Value = serde_json::from_str(include_str!(
            "../../../../contracts/examples/results/successful-run.valid.json"
        ))
        .unwrap();
        assert_eq!(validate_run_result_document(&valid), Ok(()));

        let invalid: Value = serde_json::from_str(include_str!(
            "../../../../contracts/examples/results/missing-artifact-hash.schema-invalid.json"
        ))
        .unwrap();
        assert!(validate_run_result_document(&invalid).is_err());
    }

    #[test]
    fn asserts_date_time_formats_without_network_retrieval() {
        let mut value = minimal_configuration();
        value["simulation"]["start_at"] = Value::String("yesterday".into());
        assert!(validate_run_configuration(&value).is_err());
    }

    #[test]
    fn validates_research_job_requests_status_and_results_against_local_schemas() {
        let request = serde_json::json!({
            "contract_version": "1.0.0", "kind": "episode", "fixture_id": "episode-eval",
            "fixture_version": "1.0.0", "actions": ["0.5", "1"]
        });
        assert_eq!(validate_research_job_request(&request), Ok(()));
        let invalid_request = serde_json::json!({
            "contract_version": "1.0.0", "kind": "episode", "fixture_id": "episode-eval",
            "fixture_version": "1.0.0", "actions": ["0.25", "1"]
        });
        assert!(validate_research_job_request(&invalid_request).is_err());

        let status = serde_json::json!({
            "contract_version": "1.0.0", "job_id": "a4ab8716-3d8c-42d7-857c-83e94df28ff6",
            "kind": "episode", "status": "failed", "submitted_at": "2026-11-05T10:00:00Z",
            "started_at": "2026-11-05T10:00:01Z", "finished_at": "2026-11-05T10:00:02Z",
            "failure": {"code": "RESEARCH_EXECUTION_FAILED", "message": "controlled failure"}
        });
        assert_eq!(validate_research_job_status(&status), Ok(()));
        let invalid_status = serde_json::json!({"status":"failed", "failure":null});
        assert!(validate_research_job_status(&invalid_status).is_err());

        let result = serde_json::json!({
            "contract_version": "1.0.0", "job_id": "a4ab8716-3d8c-42d7-857c-83e94df28ff6",
            "kind": "episode", "report": {"failure": "controlled failure"}
        });
        assert_eq!(validate_research_job_result(&result), Ok(()));
        let invalid_result = serde_json::json!({
            "contract_version": "1.0.0", "job_id": "a4ab8716-3d8c-42d7-857c-83e94df28ff6",
            "kind": "episode", "report": []
        });
        assert!(validate_research_job_result(&invalid_result).is_err());
    }
}
