//! Successful result assembly and evidence-reference integrity checks.

use crate::{
    evidence::{verify_artifacts, EvidenceBundle, EvidenceError},
    orchestrator::OrchestrationReport,
};
use factorsbt_contracts::{
    outcomes::{
        EvidenceArtifactRef, ModelInvocation, ModelInvocationMode, ResultCounts, ResultProvenance,
        RunResult, SuccessfulStatus,
    },
    run_configuration::RunConfiguration,
    validation::validate_run_result_document,
    DecimalString, UtcTimestamp,
};
use factorsbt_data::Fixture;
use rust_decimal::Decimal;
use serde_json::Value;
use std::{error::Error, fmt, fs, path::Path, str::FromStr};

#[derive(Debug)]
pub enum RunResultError {
    RunNotSuccessful,
    FixtureIdentityMismatch,
    InvalidTrace(&'static str),
    InvalidFee,
    ArithmeticOverflow,
    SchemaInvalid(Vec<String>),
    Evidence(EvidenceError),
    ManifestInvalid,
}

impl fmt::Display for RunResultError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RunNotSuccessful => formatter.write_str("run result requires a succeeded run"),
            Self::FixtureIdentityMismatch => {
                formatter.write_str("run result configuration and fixture identity differ")
            }
            Self::InvalidTrace(context) => write!(formatter, "run trace lacks {context}"),
            Self::InvalidFee => formatter.write_str("run trace contains an invalid fee value"),
            Self::ArithmeticOverflow => formatter.write_str("run result aggregation overflowed"),
            Self::SchemaInvalid(issues) => {
                write!(
                    formatter,
                    "run result schema validation failed: {}",
                    issues.join("; ")
                )
            }
            Self::Evidence(error) => error.fmt(formatter),
            Self::ManifestInvalid => formatter.write_str("run evidence manifest is inconsistent"),
        }
    }
}

impl Error for RunResultError {}

impl From<EvidenceError> for RunResultError {
    fn from(value: EvidenceError) -> Self {
        Self::Evidence(value)
    }
}

/// Build the public successful-result contract from an engine report and the
/// artifacts that were already written and integrity-checked for this run.
pub fn build_run_result(
    configuration: &RunConfiguration,
    fixture: &Fixture,
    report: &OrchestrationReport,
    finished_at: UtcTimestamp,
    evidence: &EvidenceBundle,
) -> Result<RunResult, RunResultError> {
    if report.finalization.status != factorsbt_engine::FinalRunStatus::Succeeded {
        return Err(RunResultError::RunNotSuccessful);
    }
    if report.fixture_id != fixture.manifest.fixture_id.as_str()
        || report.fixture_version != fixture.manifest.fixture_version.as_str()
        || report.fixture_digest_sha256 != fixture.digest_sha256
        || configuration.data.fixture_id != fixture.manifest.fixture_id
        || configuration.data.fixture_version != fixture.manifest.fixture_version
    {
        return Err(RunResultError::FixtureIdentityMismatch);
    }
    verify_artifacts(&evidence.run_directory, &evidence.artifacts)?;

    let liquidation_fill_count = count_fills(&report.liquidation_events)?;
    let fills = count_fills(&report.execution_events)?
        .checked_add(liquidation_fill_count)
        .ok_or(RunResultError::ArithmeticOverflow)?;
    let orders = report
        .cycles
        .iter()
        .try_fold(0_u64, |total, cycle| {
            total
                .checked_add(
                    u64::try_from(cycle.queued_order_ids.len())
                        .map_err(|_| RunResultError::ArithmeticOverflow)?,
                )
                .ok_or(RunResultError::ArithmeticOverflow)
        })
        .and_then(|total| {
            total
                .checked_add(liquidation_fill_count)
                .ok_or(RunResultError::ArithmeticOverflow)
        })?;
    let ledger_postings = report
        .execution_events
        .iter()
        .chain(report.liquidation_events.iter())
        .chain(report.settlement_events.iter())
        .try_fold(0_u64, |total, event| {
            total
                .checked_add(count_postings(event)?)
                .ok_or(RunResultError::ArithmeticOverflow)
        })?;
    let risk_events = u64::try_from(report.risk_events.len())
        .ok()
        .and_then(|count| count.checked_add(u64::try_from(report.liquidation_events.len()).ok()?))
        .ok_or(RunResultError::ArithmeticOverflow)?;
    let result = RunResult {
        contract_version: configuration.contract_version.clone(),
        run_id: report.identity.run_id.clone(),
        status: SuccessfulStatus::Succeeded,
        scenario_id: report.identity.scenario_id.clone(),
        output_mode: configuration.decision_policy.output_mode,
        simulation_start: configuration.simulation.start_at.clone(),
        simulation_end: configuration.simulation.end_at.clone(),
        finished_at,
        initial_cash: configuration.initial_portfolio.cash.clone(),
        total_fees: decimal_string(total_fees(report)?)?,
        counts: ResultCounts {
            decision_steps: u64::try_from(report.cycles.len())
                .map_err(|_| RunResultError::ArithmeticOverflow)?,
            admissions: u64::try_from(
                report
                    .cycles
                    .iter()
                    .filter(|cycle| cycle.admission.is_some())
                    .count(),
            )
            .map_err(|_| RunResultError::ArithmeticOverflow)?,
            orders,
            fills,
            settlements: u64::try_from(report.settlement_events.len())
                .map_err(|_| RunResultError::ArithmeticOverflow)?,
            ledger_postings,
            risk_events,
        },
        portfolio: report.final_portfolio.clone(),
        provenance: ResultProvenance {
            data_fixtures: vec![factorsbt_contracts::outcomes::DataFixtureRef {
                fixture_id: fixture.manifest.fixture_id.clone(),
                fixture_version: fixture.manifest.fixture_version.clone(),
            }],
            model_invocation: ModelInvocation {
                mode: ModelInvocationMode::FreshInference,
                model_id: configuration.model.model_id.clone(),
                model_version: configuration.model.model_version.clone(),
                inference_count: u64::try_from(report.cycles.len())
                    .map_err(|_| RunResultError::ArithmeticOverflow)?,
            },
        },
        evidence: evidence.artifacts.clone(),
    };
    let value = serde_json::to_value(&result)
        .map_err(|_| RunResultError::InvalidTrace("a serializable result"))?;
    validate_run_result_document(&value).map_err(RunResultError::SchemaInvalid)?;
    verify_result_evidence(&evidence.run_directory, &result)?;
    Ok(result)
}

/// Verify schema, normalized local references, byte digests, and that the
/// manifest describes every referenced artifact except its own hash reference.
pub fn verify_result_evidence(
    run_directory: impl AsRef<Path>,
    result: &RunResult,
) -> Result<(), RunResultError> {
    let value = serde_json::to_value(result)
        .map_err(|_| RunResultError::InvalidTrace("a serializable result"))?;
    validate_run_result_document(&value).map_err(RunResultError::SchemaInvalid)?;
    verify_artifacts(&run_directory, &result.evidence)?;

    let manifest_ref = result
        .evidence
        .iter()
        .find(|artifact| artifact.relative_path == "evidence/run-manifest.json")
        .ok_or(RunResultError::ManifestInvalid)?;
    let manifest_path = run_directory.as_ref().join(&manifest_ref.relative_path);
    let manifest_bytes = fs::read(manifest_path).map_err(|_| RunResultError::ManifestInvalid)?;
    let manifest: Value =
        serde_json::from_slice(&manifest_bytes).map_err(|_| RunResultError::ManifestInvalid)?;
    if manifest["status"] != "succeeded"
        || manifest["run_id"] != result.run_id.as_str()
        || manifest["scenario_id"] != result.scenario_id.as_str()
        || manifest["fixture"]["fixture_id"]
            != result.provenance.data_fixtures[0].fixture_id.as_str()
        || manifest["fixture"]["fixture_version"]
            != result.provenance.data_fixtures[0].fixture_version.as_str()
    {
        return Err(RunResultError::ManifestInvalid);
    }
    let listed: Vec<EvidenceArtifactRef> = serde_json::from_value(manifest["artifacts"].clone())
        .map_err(|_| RunResultError::ManifestInvalid)?;
    let mut expected = result
        .evidence
        .iter()
        .filter(|artifact| artifact.relative_path != manifest_ref.relative_path)
        .cloned()
        .collect::<Vec<_>>();
    let mut actual = listed;
    expected.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    actual.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    if expected != actual {
        return Err(RunResultError::ManifestInvalid);
    }
    Ok(())
}

fn total_fees(report: &OrchestrationReport) -> Result<Decimal, RunResultError> {
    let mut total = Decimal::ZERO;
    for event in report
        .execution_events
        .iter()
        .chain(report.liquidation_events.iter())
    {
        let fills = event["fills"]
            .as_array()
            .ok_or(RunResultError::InvalidTrace("fill arrays"))?;
        for fill in fills {
            let fee = fill["fee"]
                .as_str()
                .and_then(|value| Decimal::from_str(value).ok())
                .filter(|fee| *fee >= Decimal::ZERO)
                .ok_or(RunResultError::InvalidFee)?;
            total = total
                .checked_add(fee)
                .ok_or(RunResultError::ArithmeticOverflow)?;
        }
    }
    Ok(total)
}

fn count_fills(events: &[Value]) -> Result<u64, RunResultError> {
    events.iter().try_fold(0_u64, |total, event| {
        let count = event["fills"]
            .as_array()
            .ok_or(RunResultError::InvalidTrace("fill arrays"))?;
        total
            .checked_add(
                u64::try_from(count.len()).map_err(|_| RunResultError::ArithmeticOverflow)?,
            )
            .ok_or(RunResultError::ArithmeticOverflow)
    })
}

fn count_postings(value: &Value) -> Result<u64, RunResultError> {
    match value {
        Value::Object(object) => object.iter().try_fold(0_u64, |total, (key, nested)| {
            let count = u64::from(key.ends_with("_posting") && nested.is_object());
            total
                .checked_add(count)
                .and_then(|value| value.checked_add(count_postings(nested).ok()?))
                .ok_or(RunResultError::ArithmeticOverflow)
        }),
        Value::Array(values) => values.iter().try_fold(0_u64, |total, nested| {
            total
                .checked_add(count_postings(nested)?)
                .ok_or(RunResultError::ArithmeticOverflow)
        }),
        _ => Ok(0),
    }
}

fn decimal_string(value: Decimal) -> Result<DecimalString, RunResultError> {
    DecimalString::new(value.normalize().to_string()).map_err(|_| RunResultError::InvalidFee)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn counts_postings_without_counting_nested_fields_twice() {
        let value = json!({
            "fill":{
                "trade_posting":{"cash_delta":"-10","cash_after":"90"},
                "fee_posting":{"cash_delta":"-1","cash_after":"89"}
            }
        });
        assert_eq!(count_postings(&value).unwrap(), 2);
    }
}
