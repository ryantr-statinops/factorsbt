//! Schema and semantic checks for correlated model responses.

use factorsbt_contracts::{
    compare_utc_timestamps,
    instruments::Instrument,
    model::{ModelRequest, ModelResponse, OutputMode},
    validation::validate_model_response_document,
    InstrumentId,
};
use rust_decimal::Decimal;
use std::cmp::Ordering;
use std::{collections::HashMap, str::FromStr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelResponseValidationError {
    Schema(Vec<String>),
    ModeMismatch,
    CorrelationMismatch(&'static str),
    IneligibleInstrument(String),
    DuplicateInstrument(String),
    WrongInstrumentKind(String),
    SpotWeightsExceedOne,
    SpotWeightTotalOverflow,
    InvalidDecimal(String),
}

/// Validate both the shared wire schema and rules requiring the request context.
pub fn validate_model_response(
    request: &ModelRequest,
    response: &ModelResponse,
) -> Result<(), ModelResponseValidationError> {
    let value = serde_json::to_value(response)
        .map_err(|error| ModelResponseValidationError::Schema(vec![error.to_string()]))?;
    validate_model_response_document(&value).map_err(ModelResponseValidationError::Schema)?;

    let (
        version,
        inference_id,
        run_id,
        scenario_id,
        step_id,
        snapshot_id,
        model_id,
        model_version,
        mode,
    ) = response_identity(response);
    if version != request.contract_version {
        return Err(ModelResponseValidationError::CorrelationMismatch(
            "contract_version",
        ));
    }
    if inference_id != request.inference_id.as_str() {
        return Err(ModelResponseValidationError::CorrelationMismatch(
            "inference_id",
        ));
    }
    if run_id != request.run_id.as_str() {
        return Err(ModelResponseValidationError::CorrelationMismatch("run_id"));
    }
    if scenario_id != request.scenario_id.as_str() {
        return Err(ModelResponseValidationError::CorrelationMismatch(
            "scenario_id",
        ));
    }
    if step_id != request.step_id.as_str() {
        return Err(ModelResponseValidationError::CorrelationMismatch("step_id"));
    }
    if snapshot_id != request.snapshot_id.as_str() {
        return Err(ModelResponseValidationError::CorrelationMismatch(
            "snapshot_id",
        ));
    }
    if model_id != request.model_id.as_str() {
        return Err(ModelResponseValidationError::CorrelationMismatch(
            "model_id",
        ));
    }
    if model_version != request.model_version.as_str() {
        return Err(ModelResponseValidationError::CorrelationMismatch(
            "model_version",
        ));
    }
    if mode != request.decision_policy.output_mode {
        return Err(ModelResponseValidationError::ModeMismatch);
    }

    let kinds = instrument_kinds(request);
    let eligible = request
        .eligible_instrument_ids
        .iter()
        .map(|instrument_id| instrument_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let mut seen = std::collections::HashSet::new();
    match response {
        ModelResponse::Scores { outputs, .. } => {
            for score in &outputs.scores {
                validate_eligible(&eligible, &score.instrument_id)?;
                validate_unique(&mut seen, &score.instrument_id)?;
                if !kinds.contains_key(&score.instrument_id) {
                    return Err(ModelResponseValidationError::WrongInstrumentKind(
                        score.instrument_id.as_str().to_owned(),
                    ));
                }
                parse_decimal(score.instrument_id.as_str(), score.score.as_str())?;
            }
        }
        ModelResponse::Targets { outputs, .. } => {
            let mut total_spot_weight = Decimal::ZERO;
            for target in &outputs.spot_weights {
                validate_eligible(&eligible, &target.instrument_id)?;
                validate_unique(&mut seen, &target.instrument_id)?;
                if kinds.get(&target.instrument_id) != Some(&InstrumentKind::Spot) {
                    return Err(ModelResponseValidationError::WrongInstrumentKind(
                        target.instrument_id.as_str().to_owned(),
                    ));
                }
                let weight = parse_decimal(target.instrument_id.as_str(), target.weight.as_str())?;
                total_spot_weight = total_spot_weight
                    .checked_add(weight)
                    .ok_or(ModelResponseValidationError::SpotWeightTotalOverflow)?;
            }
            if total_spot_weight > Decimal::ONE {
                return Err(ModelResponseValidationError::SpotWeightsExceedOne);
            }
            for target in &outputs.derivative_contracts {
                if !eligible.contains(target.instrument_id.as_str())
                    && !(target.contracts == 0
                        && request.instruments.iter().any(|instrument| {
                            let expiry = match instrument {
                                Instrument::LinearFuture(value)
                                    if value.instrument_id == target.instrument_id =>
                                {
                                    &value.expiry_at
                                }
                                Instrument::Option(value)
                                    if value.instrument_id == target.instrument_id =>
                                {
                                    &value.expiry_at
                                }
                                _ => return false,
                            };
                            compare_utc_timestamps(&request.simulation_time, expiry)
                                != Ordering::Less
                        }))
                {
                    return Err(ModelResponseValidationError::IneligibleInstrument(
                        target.instrument_id.as_str().to_owned(),
                    ));
                }
                validate_unique(&mut seen, &target.instrument_id)?;
                if kinds.get(&target.instrument_id) != Some(&InstrumentKind::Derivative) {
                    return Err(ModelResponseValidationError::WrongInstrumentKind(
                        target.instrument_id.as_str().to_owned(),
                    ));
                }
            }
        }
        ModelResponse::OrderIntents { outputs, .. } => {
            for order in &outputs.orders {
                validate_eligible(&eligible, &order.instrument_id)?;
                validate_unique(&mut seen, &order.instrument_id)?;
                if !kinds.contains_key(&order.instrument_id) {
                    return Err(ModelResponseValidationError::WrongInstrumentKind(
                        order.instrument_id.as_str().to_owned(),
                    ));
                }
                parse_decimal(order.instrument_id.as_str(), order.quantity.as_str())?;
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstrumentKind {
    Spot,
    Derivative,
}

fn instrument_kinds(request: &ModelRequest) -> HashMap<InstrumentId, InstrumentKind> {
    request
        .instruments
        .iter()
        .map(|instrument| {
            let (id, kind) = match instrument {
                Instrument::Spot(value) => (&value.instrument_id, InstrumentKind::Spot),
                Instrument::LinearFuture(value) => {
                    (&value.instrument_id, InstrumentKind::Derivative)
                }
                Instrument::Option(value) => (&value.instrument_id, InstrumentKind::Derivative),
            };
            (id.clone(), kind)
        })
        .collect()
}

fn response_identity(
    response: &ModelResponse,
) -> (&str, &str, &str, &str, &str, &str, &str, &str, OutputMode) {
    match response {
        ModelResponse::Scores {
            contract_version,
            inference_id,
            run_id,
            scenario_id,
            step_id,
            snapshot_id,
            model_id,
            model_version,
            ..
        } => (
            contract_version,
            inference_id.as_str(),
            run_id.as_str(),
            scenario_id.as_str(),
            step_id.as_str(),
            snapshot_id.as_str(),
            model_id.as_str(),
            model_version.as_str(),
            OutputMode::Scores,
        ),
        ModelResponse::Targets {
            contract_version,
            inference_id,
            run_id,
            scenario_id,
            step_id,
            snapshot_id,
            model_id,
            model_version,
            ..
        } => (
            contract_version,
            inference_id.as_str(),
            run_id.as_str(),
            scenario_id.as_str(),
            step_id.as_str(),
            snapshot_id.as_str(),
            model_id.as_str(),
            model_version.as_str(),
            OutputMode::Targets,
        ),
        ModelResponse::OrderIntents {
            contract_version,
            inference_id,
            run_id,
            scenario_id,
            step_id,
            snapshot_id,
            model_id,
            model_version,
            ..
        } => (
            contract_version,
            inference_id.as_str(),
            run_id.as_str(),
            scenario_id.as_str(),
            step_id.as_str(),
            snapshot_id.as_str(),
            model_id.as_str(),
            model_version.as_str(),
            OutputMode::OrderIntents,
        ),
    }
}

fn validate_eligible(
    eligible: &std::collections::HashSet<&str>,
    instrument_id: &InstrumentId,
) -> Result<(), ModelResponseValidationError> {
    if eligible.contains(instrument_id.as_str()) {
        Ok(())
    } else {
        Err(ModelResponseValidationError::IneligibleInstrument(
            instrument_id.as_str().to_owned(),
        ))
    }
}

fn validate_unique(
    seen: &mut std::collections::HashSet<String>,
    instrument_id: &InstrumentId,
) -> Result<(), ModelResponseValidationError> {
    if seen.insert(instrument_id.as_str().to_owned()) {
        Ok(())
    } else {
        Err(ModelResponseValidationError::DuplicateInstrument(
            instrument_id.as_str().to_owned(),
        ))
    }
}

fn parse_decimal(
    instrument_id: &str,
    value: &str,
) -> Result<Decimal, ModelResponseValidationError> {
    Decimal::from_str(value)
        .map_err(|_| ModelResponseValidationError::InvalidDecimal(instrument_id.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use factorsbt_contracts::{Identifier, Instrument};

    fn request() -> ModelRequest {
        serde_json::from_str(include_str!(
            "../../../../contracts/examples/model/scores-request.inline-snapshot.json"
        ))
        .unwrap()
    }

    #[test]
    fn accepts_shared_scores_and_rejects_wrong_mode_or_correlation() {
        let request = request();
        let response: ModelResponse = serde_json::from_str(include_str!(
            "../../../../contracts/examples/model/scores-response.valid.json"
        ))
        .unwrap();
        assert_eq!(validate_model_response(&request, &response), Ok(()));

        let mut wrong_step = response.clone();
        if let ModelResponse::Scores { step_id, .. } = &mut wrong_step {
            *step_id = Identifier::new("step-9999").unwrap();
        }
        assert_eq!(
            validate_model_response(&request, &wrong_step),
            Err(ModelResponseValidationError::CorrelationMismatch("step_id"))
        );
    }

    #[test]
    fn rejects_ineligible_and_duplicate_scores() {
        let request = request();
        let mut response: ModelResponse = serde_json::from_str(include_str!(
            "../../../../contracts/examples/model/scores-response.valid.json"
        ))
        .unwrap();
        if let ModelResponse::Scores { outputs, .. } = &mut response {
            outputs.scores[0].instrument_id = InstrumentId::new("SPOT_B").unwrap();
        }
        assert!(matches!(
            validate_model_response(&request, &response),
            Err(ModelResponseValidationError::IneligibleInstrument(_))
        ));

        let mut response: ModelResponse = serde_json::from_str(include_str!(
            "../../../../contracts/examples/model/scores-response.valid.json"
        ))
        .unwrap();
        if let ModelResponse::Scores { outputs, .. } = &mut response {
            outputs.scores.push(outputs.scores[0].clone());
        }
        assert!(matches!(
            validate_model_response(&request, &response),
            Err(ModelResponseValidationError::DuplicateInstrument(_))
        ));
    }

    #[test]
    fn target_weights_are_bounded_and_instrument_kinds_are_checked() {
        let mut request = request();
        request.decision_policy.output_mode = OutputMode::Targets;
        let mut response: ModelResponse = serde_json::from_str(include_str!(
            "../../../../contracts/examples/model/targets-response.valid-shape.json"
        ))
        .unwrap();
        assert_eq!(validate_model_response(&request, &response), Ok(()));
        if let ModelResponse::Targets { outputs, .. } = &mut response {
            outputs.spot_weights[0].weight =
                factorsbt_contracts::DecimalString::new("1.1").unwrap();
        }
        assert_eq!(
            validate_model_response(&request, &response),
            Err(ModelResponseValidationError::SpotWeightsExceedOne)
        );

        if let ModelResponse::Targets { outputs, .. } = &mut response {
            outputs.spot_weights.clear();
            outputs
                .derivative_contracts
                .push(factorsbt_contracts::model::DerivativeTarget {
                    instrument_id: InstrumentId::new("SPOT_A").unwrap(),
                    contracts: 1,
                });
        }
        assert_eq!(
            validate_model_response(&request, &response),
            Err(ModelResponseValidationError::WrongInstrumentKind(
                "SPOT_A".to_owned()
            ))
        );
    }

    #[test]
    fn order_intents_validate_as_a_distinct_mode() {
        let mut request = request();
        request.decision_policy.output_mode = OutputMode::OrderIntents;
        let response: ModelResponse = serde_json::from_str(include_str!(
            "../../../../contracts/examples/model/order-intents-response.valid-shape.json"
        ))
        .unwrap();
        assert_eq!(validate_model_response(&request, &response), Ok(()));
    }

    #[test]
    fn expired_derivative_accepts_only_zero_target_and_keeps_identity_checks() {
        let mut request = request();
        request.decision_policy.output_mode = OutputMode::Targets;
        request.instruments.push(serde_json::from_str(
            r#"{"instrument_id":"FUT_A","instrument_type":"linear_future","quote_currency":"USD","price_tick":"0.01","quantity_step":"1","contract_multiplier":"1","fee_bps":"0","fee_per_unit":"0","slippage_bps":"0","underlying_id":"SPOT_A","expiry_at":"2026-01-05T16:00:00Z","initial_margin_per_contract":"100","maintenance_margin_per_contract":"80"}"#,
        ).unwrap());
        let mut response: ModelResponse = serde_json::from_str(include_str!(
            "../../../../contracts/examples/model/targets-response.valid-shape.json"
        ))
        .unwrap();
        if let ModelResponse::Targets { outputs, .. } = &mut response {
            outputs
                .derivative_contracts
                .push(factorsbt_contracts::model::DerivativeTarget {
                    instrument_id: InstrumentId::new("FUT_A").unwrap(),
                    contracts: 0,
                });
        }
        assert_eq!(validate_model_response(&request, &response), Ok(()));
        if let ModelResponse::Targets { outputs, .. } = &mut response {
            outputs.derivative_contracts[0].contracts = 1;
        }
        assert!(matches!(
            validate_model_response(&request, &response),
            Err(ModelResponseValidationError::IneligibleInstrument(_))
        ));
        if let ModelResponse::Targets { outputs, .. } = &mut response {
            outputs.derivative_contracts[0].contracts = 0;
            outputs
                .derivative_contracts
                .push(outputs.derivative_contracts[0].clone());
        }
        assert!(matches!(
            validate_model_response(&request, &response),
            Err(ModelResponseValidationError::DuplicateInstrument(_))
        ));
        if let ModelResponse::Targets { outputs, .. } = &mut response {
            outputs.derivative_contracts.pop();
            outputs.spot_weights.clear();
            outputs.derivative_contracts[0].instrument_id = InstrumentId::new("SPOT_A").unwrap();
        }
        assert!(matches!(
            validate_model_response(&request, &response),
            Err(ModelResponseValidationError::WrongInstrumentKind(_))
        ));
    }

    #[test]
    fn identifies_configured_instrument_kinds() {
        let request = request();
        assert!(matches!(
            request.instruments.first(),
            Some(Instrument::Spot(_))
        ));
    }
}
