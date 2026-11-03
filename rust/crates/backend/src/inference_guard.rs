//! Per-run guard against stale, mis-correlated, or duplicate model responses.

use factorsbt_contracts::{model::ModelResponse, ModelRequest, Uuid};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferenceIdentity {
    pub inference_id: Uuid,
    pub run_id: Uuid,
    pub scenario_id: String,
    pub step_id: String,
    pub snapshot_id: Uuid,
    pub model_id: String,
    pub model_version: String,
}

impl From<&ModelRequest> for InferenceIdentity {
    fn from(request: &ModelRequest) -> Self {
        Self {
            inference_id: request.inference_id.clone(),
            run_id: request.run_id.clone(),
            scenario_id: request.scenario_id.as_str().to_owned(),
            step_id: request.step_id.as_str().to_owned(),
            snapshot_id: request.snapshot_id.clone(),
            model_id: request.model_id.as_str().to_owned(),
            model_version: request.model_version.as_str().to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InferenceGuardError {
    PendingAlreadyExists,
    NoPendingInference,
    DuplicateResponse,
    CorrelationMismatch(&'static str),
}

#[derive(Debug, Default)]
pub struct InferenceGuard {
    pending: Option<InferenceIdentity>,
    applied_inference_ids: HashSet<Uuid>,
}

impl InferenceGuard {
    pub fn begin(&mut self, request: &ModelRequest) -> Result<(), InferenceGuardError> {
        if self.pending.is_some() {
            return Err(InferenceGuardError::PendingAlreadyExists);
        }
        if self.applied_inference_ids.contains(&request.inference_id) {
            return Err(InferenceGuardError::DuplicateResponse);
        }
        self.pending = Some(InferenceIdentity::from(request));
        Ok(())
    }

    /// Claim the response for application. The pending identity is consumed
    /// only when every correlation field matches exactly.
    pub fn complete(
        &mut self,
        response: &ModelResponse,
    ) -> Result<InferenceIdentity, InferenceGuardError> {
        let response_id = response_inference_id(response);
        if self.applied_inference_ids.contains(response_id) {
            return Err(InferenceGuardError::DuplicateResponse);
        }
        let expected = self
            .pending
            .as_ref()
            .ok_or(InferenceGuardError::NoPendingInference)?;
        validate_identity(expected, response)?;
        let accepted = self.pending.take().expect("pending checked above");
        self.applied_inference_ids
            .insert(accepted.inference_id.clone());
        Ok(accepted)
    }

    pub fn pending(&self) -> Option<&InferenceIdentity> {
        self.pending.as_ref()
    }
}

fn response_inference_id(response: &ModelResponse) -> &Uuid {
    match response {
        ModelResponse::Scores { inference_id, .. }
        | ModelResponse::Targets { inference_id, .. }
        | ModelResponse::OrderIntents { inference_id, .. } => inference_id,
    }
}

fn validate_identity(
    expected: &InferenceIdentity,
    response: &ModelResponse,
) -> Result<(), InferenceGuardError> {
    let (inference_id, run_id, scenario_id, step_id, snapshot_id, model_id, model_version) =
        match response {
            ModelResponse::Scores {
                inference_id,
                run_id,
                scenario_id,
                step_id,
                snapshot_id,
                model_id,
                model_version,
                ..
            }
            | ModelResponse::Targets {
                inference_id,
                run_id,
                scenario_id,
                step_id,
                snapshot_id,
                model_id,
                model_version,
                ..
            }
            | ModelResponse::OrderIntents {
                inference_id,
                run_id,
                scenario_id,
                step_id,
                snapshot_id,
                model_id,
                model_version,
                ..
            } => (
                inference_id,
                run_id,
                scenario_id.as_str(),
                step_id.as_str(),
                snapshot_id,
                model_id.as_str(),
                model_version.as_str(),
            ),
        };

    if inference_id != &expected.inference_id {
        return Err(InferenceGuardError::CorrelationMismatch("inference_id"));
    }
    if run_id != &expected.run_id {
        return Err(InferenceGuardError::CorrelationMismatch("run_id"));
    }
    if scenario_id != expected.scenario_id {
        return Err(InferenceGuardError::CorrelationMismatch("scenario_id"));
    }
    if step_id != expected.step_id {
        return Err(InferenceGuardError::CorrelationMismatch("step_id"));
    }
    if snapshot_id != &expected.snapshot_id {
        return Err(InferenceGuardError::CorrelationMismatch("snapshot_id"));
    }
    if model_id != expected.model_id {
        return Err(InferenceGuardError::CorrelationMismatch("model_id"));
    }
    if model_version != expected.model_version {
        return Err(InferenceGuardError::CorrelationMismatch("model_version"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use factorsbt_contracts::model::ModelResponse;

    fn request() -> ModelRequest {
        serde_json::from_str(include_str!(
            "../../../../contracts/examples/model/scores-request.inline-snapshot.json"
        ))
        .unwrap()
    }

    fn response() -> ModelResponse {
        serde_json::from_str(include_str!(
            "../../../../contracts/examples/model/scores-response.valid.json"
        ))
        .unwrap()
    }

    #[test]
    fn accepts_a_matching_response_once_and_rejects_duplicates() {
        let request = request();
        let response = response();
        let mut guard = InferenceGuard::default();
        guard.begin(&request).unwrap();
        assert_eq!(
            guard.complete(&response).unwrap(),
            InferenceIdentity::from(&request)
        );
        assert_eq!(
            guard.complete(&response),
            Err(InferenceGuardError::DuplicateResponse)
        );
        assert!(guard.pending().is_none());
    }

    #[test]
    fn stale_mismatched_response_does_not_consume_current_pending_identity() {
        let request = request();
        let response = response();
        let mut guard = InferenceGuard::default();
        guard.begin(&request).unwrap();
        let mut stale = response.clone();
        if let ModelResponse::Scores { snapshot_id, .. } = &mut stale {
            *snapshot_id = Uuid::new("ba6f75d4-403e-4f40-9019-87a9247a2fe2").unwrap();
        }
        assert_eq!(
            guard.complete(&stale),
            Err(InferenceGuardError::CorrelationMismatch("snapshot_id"))
        );
        assert!(guard.pending().is_some());
        assert_eq!(
            guard.complete(&response).unwrap(),
            InferenceIdentity::from(&request)
        );
    }

    #[test]
    fn allows_only_one_pending_inference_per_run_guard() {
        let request = request();
        let mut guard = InferenceGuard::default();
        guard.begin(&request).unwrap();
        assert_eq!(
            guard.begin(&request),
            Err(InferenceGuardError::PendingAlreadyExists)
        );
    }
}
