//! Run-owned coordinator for point-in-time data, model calls, and engine state.

use crate::{
    inference_guard::InferenceGuard,
    model_client::{ModelHttpClient, ModelServiceError},
    model_validation::validate_model_response,
};
use factorsbt_contracts::{
    compare_utc_timestamps,
    instruments::{ExerciseStyle, Instrument, SettlementStyle},
    model::ModelRequest,
    outcomes::RunProgress as ApiRunProgress,
    temporal::{DecisionSnapshot, PortfolioSnapshot, PositionSnapshot, UnderlyingReservation},
    Identifier, InstrumentId, RunConfiguration, UtcTimestamp, Uuid,
};
use factorsbt_data::{
    assemble_snapshot, execution_prices_available_at, observations_available_at,
    settlement_events_available_at, validate_run_coverage, Fixture, SnapshotContext,
};
use factorsbt_engine::{
    admit_target_group, execute_liquidation_group, execute_pending_group, finalize_run_state,
    queue_order_intents, queue_target_group, response_to_proposal, schedule_liquidation,
    value_instruments, AdmissionResult, DecisionProposal, ExecutedOrderGroup, FinalRunStatus,
    LiquidationOutcome, PendingOrderGroup, PortfolioState, RunFinalization, SimulationClock,
    SimulationEventKind,
};
use rust_decimal::{Decimal, RoundingStrategy};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunIdentity {
    pub run_id: Uuid,
    pub scenario_id: Identifier,
}

#[derive(Debug, Clone)]
pub struct DecisionCycleRecord {
    pub snapshot: DecisionSnapshot,
    pub request: ModelRequest,
    pub request_body: Vec<u8>,
    pub response: factorsbt_contracts::ModelResponse,
    pub response_body: Vec<u8>,
    pub proposal: DecisionProposal,
    pub admission: Option<AdmissionResult>,
    pub queued_order_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct OrchestrationReport {
    pub identity: RunIdentity,
    pub fixture_id: String,
    pub fixture_version: String,
    pub fixture_digest_sha256: String,
    pub cycles: Vec<DecisionCycleRecord>,
    pub execution_events: Vec<Value>,
    pub liquidation_events: Vec<Value>,
    pub settlement_events: Vec<Value>,
    pub risk_events: Vec<Value>,
    pub finalization: RunFinalization,
    pub final_portfolio: PortfolioSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchestrationError {
    pub stage: &'static str,
    pub reason: String,
}

impl OrchestrationError {
    fn at(stage: &'static str, error: impl fmt::Display) -> Self {
        Self {
            stage,
            reason: error.to_string(),
        }
    }
}

impl fmt::Display for OrchestrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} failed: {}", self.stage, self.reason)
    }
}

impl Error for OrchestrationError {}

#[derive(Debug)]
pub struct RunExecutionFailure {
    pub identity: RunIdentity,
    pub error: OrchestrationError,
    pub partial_report: Option<Box<OrchestrationReport>>,
    pub failed_inference: Option<Box<FailedInferenceExchange>>,
}

#[derive(Debug)]
pub struct FailedInferenceExchange {
    pub request: ModelRequest,
    pub request_body: Vec<u8>,
    pub response_body: Option<Vec<u8>>,
}

impl fmt::Display for RunExecutionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl Error for RunExecutionFailure {}

struct RunProgress {
    identity: RunIdentity,
    fixture_id: String,
    fixture_version: String,
    fixture_digest_sha256: String,
    last_valid_portfolio: Option<PortfolioState>,
    pending_groups: Vec<PendingOrderGroup>,
    liquidation_group: Option<PendingOrderGroup>,
    cycles: Vec<DecisionCycleRecord>,
    execution_events: Vec<Value>,
    liquidation_events: Vec<Value>,
    settlement_events: Vec<Value>,
    risk_events: Vec<Value>,
    failed_request: Option<ModelRequest>,
    failed_request_body: Option<Vec<u8>>,
    failed_response_body: Option<Vec<u8>>,
}

impl RunProgress {
    fn new(identity: RunIdentity, fixture: &Fixture) -> Self {
        Self {
            identity,
            fixture_id: fixture.manifest.fixture_id.as_str().to_owned(),
            fixture_version: fixture.manifest.fixture_version.as_str().to_owned(),
            fixture_digest_sha256: fixture.digest_sha256.clone(),
            last_valid_portfolio: None,
            pending_groups: Vec::new(),
            liquidation_group: None,
            cycles: Vec::new(),
            execution_events: Vec::new(),
            liquidation_events: Vec::new(),
            settlement_events: Vec::new(),
            risk_events: Vec::new(),
            failed_request: None,
            failed_request_body: None,
            failed_response_body: None,
        }
    }

    fn capture_state(
        &mut self,
        portfolio: &PortfolioState,
        pending_groups: &[PendingOrderGroup],
        liquidation_group: Option<&PendingOrderGroup>,
    ) {
        self.last_valid_portfolio = Some(portfolio.clone());
        self.pending_groups = pending_groups.to_vec();
        self.liquidation_group = liquidation_group.cloned();
    }

    fn failure_report(&self, error: &OrchestrationError) -> Option<OrchestrationReport> {
        let portfolio = self.last_valid_portfolio.as_ref()?;
        let mut pending = self.pending_groups.clone();
        if let Some(group) = &self.liquidation_group {
            pending.push(group.clone());
        }
        let finalization =
            finalize_run_state(portfolio, &mut pending, Some(error.to_string())).ok()?;
        let final_portfolio = portfolio_snapshot(portfolio).ok()?;
        Some(OrchestrationReport {
            identity: self.identity.clone(),
            fixture_id: self.fixture_id.clone(),
            fixture_version: self.fixture_version.clone(),
            fixture_digest_sha256: self.fixture_digest_sha256.clone(),
            cycles: self.cycles.clone(),
            execution_events: self.execution_events.clone(),
            liquidation_events: self.liquidation_events.clone(),
            settlement_events: self.settlement_events.clone(),
            risk_events: self.risk_events.clone(),
            finalization,
            final_portfolio,
        })
    }
}

/// Run one isolated scenario. Fixture data is immutable and can be shared;
/// every mutable engine, pending-order, inference, and trace value is local.
pub async fn run_simulation(
    client: &ModelHttpClient,
    endpoint: &str,
    timeout: Duration,
    configuration: &RunConfiguration,
    fixture: &Fixture,
    identity: RunIdentity,
) -> Result<OrchestrationReport, Box<RunExecutionFailure>> {
    run_simulation_with_progress(
        client,
        endpoint,
        timeout,
        configuration,
        fixture,
        identity,
        |_| {},
    )
    .await
}

/// Execute a run while reporting completed decision steps to its owner.
pub async fn run_simulation_with_progress<F>(
    client: &ModelHttpClient,
    endpoint: &str,
    timeout: Duration,
    configuration: &RunConfiguration,
    fixture: &Fixture,
    identity: RunIdentity,
    mut on_progress: F,
) -> Result<OrchestrationReport, Box<RunExecutionFailure>>
where
    F: FnMut(ApiRunProgress),
{
    let mut progress = RunProgress::new(identity.clone(), fixture);
    match run_simulation_inner(
        RunSimulationContext {
            client,
            endpoint,
            timeout,
            configuration,
            fixture,
            identity,
        },
        &mut progress,
        &mut on_progress,
    )
    .await
    {
        Ok(report) => Ok(report),
        Err(error) => Err(Box::new(RunExecutionFailure {
            identity: progress.identity.clone(),
            partial_report: progress.failure_report(&error).map(Box::new),
            failed_inference: progress
                .failed_request
                .zip(progress.failed_request_body)
                .map(|(request, request_body)| {
                    Box::new(FailedInferenceExchange {
                        request,
                        request_body,
                        response_body: progress.failed_response_body,
                    })
                }),
            error,
        })),
    }
}

struct RunSimulationContext<'a> {
    client: &'a ModelHttpClient,
    endpoint: &'a str,
    timeout: Duration,
    configuration: &'a RunConfiguration,
    fixture: &'a Fixture,
    identity: RunIdentity,
}

async fn run_simulation_inner<F>(
    context: RunSimulationContext<'_>,
    progress: &mut RunProgress,
    on_progress: &mut F,
) -> Result<OrchestrationReport, OrchestrationError>
where
    F: FnMut(ApiRunProgress),
{
    let RunSimulationContext {
        client,
        endpoint,
        timeout,
        configuration,
        fixture,
        identity,
    } = context;
    validate_run_coverage(&fixture.dataset, configuration)
        .map_err(|issues| OrchestrationError::at("fixture coverage", format_issues(&issues)))?;
    let mut portfolio = PortfolioState::initialize(configuration, fixture)
        .map_err(|error| OrchestrationError::at("portfolio initialization", error))?;
    let mut clock = SimulationClock::new(
        &configuration.simulation.start_at,
        &configuration.simulation.end_at,
        &configuration.simulation.calendar_id,
    )
    .map_err(|error| OrchestrationError::at("simulation clock", error))?;
    let schedule = std::iter::from_fn(|| clock.next_event()).collect::<Vec<_>>();
    let total_steps = schedule
        .iter()
        .filter(|event| event.kind == SimulationEventKind::Close)
        .count() as u64;

    let mut pending_groups = Vec::<PendingOrderGroup>::new();
    let mut liquidation_group = None::<PendingOrderGroup>;
    let mut inference_guard = InferenceGuard::default();
    let mut processed_settlement_ids = HashSet::<String>::new();
    progress.capture_state(&portfolio, &pending_groups, liquidation_group.as_ref());
    let mut step_number = 0_u64;

    for (event_index, event) in schedule.iter().enumerate() {
        match event.kind {
            SimulationEventKind::Open => {
                let prices = execution_prices_available_at(&fixture.dataset, &event.at)
                    .map_err(|error| OrchestrationError::at("execution inputs", error))?;
                if let Some(group) = liquidation_group.clone() {
                    let outcome =
                        execute_liquidation_group(&mut portfolio, &group, &event.at, &prices)
                            .map_err(|error| OrchestrationError::at("forced liquidation", error))?;
                    match &outcome {
                        LiquidationOutcome::Completed { fills, .. } => {
                            progress.liquidation_events.push(json!({
                                "type":"liquidation_completed",
                                "step_id":group.step_id.as_str(),
                                "execution_time":event.at.as_str(),
                                "fills":fills.iter().map(fill_value).collect::<Vec<_>>(),
                            }));
                            liquidation_group = None;
                            progress.capture_state(&portfolio, &pending_groups, None);
                        }
                        LiquidationOutcome::Failed { reason, .. } => {
                            progress.liquidation_events.push(json!({
                                "type":"liquidation_failed",
                                "step_id":group.step_id.as_str(),
                                "execution_time":event.at.as_str(),
                                "reason":reason,
                            }));
                            return Err(OrchestrationError::at("forced liquidation", reason));
                        }
                    }
                } else {
                    let mut due = Vec::new();
                    let mut remaining = Vec::new();
                    for group in pending_groups.drain(..) {
                        if compare_utc_timestamps(&group.eligible_at, &event.at).is_eq() {
                            due.push(group);
                        } else {
                            remaining.push(group);
                        }
                    }
                    pending_groups = remaining;
                    while !due.is_empty() {
                        let group = due.remove(0);
                        let executed =
                            execute_pending_group(&mut portfolio, &group, &event.at, &prices)
                                .map_err(|error| {
                                    OrchestrationError::at("next-open execution", error)
                                })?;
                        progress.execution_events.push(execution_value(&executed));
                        let mut not_yet_executed = pending_groups.clone();
                        not_yet_executed.extend(due.iter().cloned());
                        progress.capture_state(&portfolio, &not_yet_executed, None);
                    }
                }
            }
            SimulationEventKind::Close => {
                step_number = step_number.saturating_add(1);
                let step_id = Identifier::new(format!("step-{step_number:04}"))
                    .map_err(|error| OrchestrationError::at("step identity", error))?;
                let observations = observations_available_at(&fixture.dataset, &event.at)
                    .map_err(|error| OrchestrationError::at("snapshot data", error))?;
                let marks = value_instruments(&configuration.instruments, &observations, &event.at)
                    .map_err(|error| OrchestrationError::at("valuation", error))?;
                portfolio
                    .apply_valuation_marks(&marks)
                    .map_err(|error| OrchestrationError::at("portfolio marks", error))?;
                progress.capture_state(&portfolio, &pending_groups, liquidation_group.as_ref());
                settle_close(
                    SettlementContext {
                        configuration,
                        fixture,
                        at: &event.at,
                        marks: &marks,
                        pending_groups: &pending_groups,
                        liquidation_group: liquidation_group.as_ref(),
                    },
                    &mut portfolio,
                    &mut processed_settlement_ids,
                    progress,
                )?;

                let maintenance_breach =
                    factorsbt_engine::enforce_maintenance(&portfolio, &mut pending_groups)
                        .map_err(|error| OrchestrationError::at("maintenance check", error))?;
                progress.capture_state(&portfolio, &pending_groups, liquidation_group.as_ref());
                if let Some(breach) = maintenance_breach {
                    progress
                        .risk_events
                        .push(maintenance_value(&event.at, &breach));
                    let next_open = schedule
                        .get(event_index + 1)
                        .filter(|next| next.kind == SimulationEventKind::Open);
                    let next_open = next_open.ok_or_else(|| {
                        OrchestrationError::at(
                            "maintenance liquidation",
                            "no later session open is inside the configured run interval",
                        )
                    })?;
                    liquidation_group = Some(
                        schedule_liquidation(&portfolio, &step_id, &event.at, &next_open.at)
                            .map_err(|error| {
                                OrchestrationError::at("maintenance liquidation schedule", error)
                            })?,
                    );
                    progress.capture_state(&portfolio, &pending_groups, liquidation_group.as_ref());
                    on_progress(ApiRunProgress {
                        scenario_id: identity.scenario_id.clone(),
                        step_id: step_id.clone(),
                        simulation_time: event.at.clone(),
                        completed_steps: step_number,
                        total_steps,
                    });
                    continue;
                }

                let assembled_at = system_utc_timestamp()
                    .map_err(|error| OrchestrationError::at("processing timestamp", error))?;
                let snapshot_id = derived_uuid(&identity, &step_id, "snapshot");
                let snapshot = assemble_snapshot(
                    &fixture.dataset,
                    SnapshotContext {
                        snapshot_id: snapshot_id.clone(),
                        run_id: identity.run_id.clone(),
                        scenario_id: identity.scenario_id.clone(),
                        step_id: step_id.clone(),
                        simulation_time: event.at.clone(),
                        assembled_at: assembled_at.clone(),
                    },
                    portfolio_snapshot(&portfolio)
                        .map_err(|error| OrchestrationError::at("portfolio snapshot", error))?,
                )
                .map_err(|error| OrchestrationError::at("snapshot assembly", error))?;
                let mut instruments = configuration.instruments.clone();
                instruments.sort_by(|left, right| instrument_id(left).cmp(instrument_id(right)));
                let mut eligible_instrument_ids = configuration
                    .instruments
                    .iter()
                    .map(|instrument| InstrumentId::new(instrument_id(instrument)))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| OrchestrationError::at("eligible instruments", error))?;
                eligible_instrument_ids.sort_by(|left, right| left.as_str().cmp(right.as_str()));
                let request = ModelRequest {
                    contract_version: configuration.contract_version.clone(),
                    inference_id: derived_uuid(&identity, &step_id, "inference"),
                    run_id: identity.run_id.clone(),
                    scenario_id: identity.scenario_id.clone(),
                    step_id: step_id.clone(),
                    snapshot_id,
                    model_id: configuration.model.model_id.clone(),
                    model_version: configuration.model.model_version.clone(),
                    requested_at: system_utc_timestamp()
                        .map_err(|error| OrchestrationError::at("processing timestamp", error))?,
                    simulation_time: event.at.clone(),
                    decision_policy: configuration.decision_policy.clone(),
                    eligible_instrument_ids,
                    instruments,
                    snapshot: snapshot.clone(),
                };
                let request_body = serde_json::to_vec(&request)
                    .map_err(|error| OrchestrationError::at("model request encoding", error))?;
                progress.failed_request = Some(request.clone());
                progress.failed_request_body = Some(request_body.clone());
                progress.failed_response_body = None;
                inference_guard.begin(&request).map_err(|error| {
                    OrchestrationError::at("inference guard", format!("{error:?}"))
                })?;
                let exchange =
                    client
                        .infer(endpoint, timeout, &request)
                        .await
                        .map_err(|error| {
                            progress.failed_response_body =
                                error.response_body().map(ToOwned::to_owned);
                            model_error("model request", error)
                        })?;
                progress.failed_response_body = Some(exchange.response_body.clone());
                validate_model_response(&request, &exchange.response).map_err(|error| {
                    OrchestrationError::at("model response validation", format!("{error:?}"))
                })?;
                inference_guard
                    .complete(&exchange.response)
                    .map_err(|error| {
                        OrchestrationError::at("inference correlation", format!("{error:?}"))
                    })?;
                let proposal = response_to_proposal(
                    &configuration.decision_policy,
                    &exchange.response,
                    &configuration.instruments,
                    &portfolio,
                    &event.at,
                )
                .map_err(|error| OrchestrationError::at("decision proposal", error))?;
                let (admission, group) = queue_decision(
                    &portfolio,
                    &step_id,
                    &event.at,
                    &configuration.simulation.end_at,
                    &proposal,
                )?;
                let queued_order_ids = group
                    .as_ref()
                    .map(|value| {
                        value
                            .orders
                            .iter()
                            .map(|order| order.order_id.as_str().to_owned())
                            .collect()
                    })
                    .unwrap_or_default();
                if let Some(group) = group {
                    pending_groups.push(group);
                }
                progress.cycles.push(DecisionCycleRecord {
                    snapshot,
                    request,
                    request_body,
                    response: exchange.response,
                    response_body: exchange.response_body,
                    proposal,
                    admission,
                    queued_order_ids,
                });
                progress.failed_request = None;
                progress.failed_request_body = None;
                progress.failed_response_body = None;
                progress.capture_state(&portfolio, &pending_groups, liquidation_group.as_ref());
                on_progress(ApiRunProgress {
                    scenario_id: identity.scenario_id.clone(),
                    step_id,
                    simulation_time: event.at.clone(),
                    completed_steps: step_number,
                    total_steps,
                });
            }
        }
    }

    let finalization = finalize_run_state(&portfolio, &mut pending_groups, None)
        .map_err(|error| OrchestrationError::at("run finalization", error))?;
    if finalization.status != FinalRunStatus::Succeeded {
        return Err(OrchestrationError::at(
            "run finalization",
            finalization
                .failure_reason
                .as_deref()
                .unwrap_or("run failed"),
        ));
    }
    let final_portfolio = portfolio_snapshot(&portfolio)
        .map_err(|error| OrchestrationError::at("run finalization snapshot", error))?;
    Ok(OrchestrationReport {
        identity,
        fixture_id: progress.fixture_id.clone(),
        fixture_version: progress.fixture_version.clone(),
        fixture_digest_sha256: progress.fixture_digest_sha256.clone(),
        cycles: progress.cycles.clone(),
        execution_events: progress.execution_events.clone(),
        liquidation_events: progress.liquidation_events.clone(),
        settlement_events: progress.settlement_events.clone(),
        risk_events: progress.risk_events.clone(),
        finalization,
        final_portfolio,
    })
}

fn queue_decision(
    portfolio: &PortfolioState,
    step_id: &Identifier,
    decision_time: &UtcTimestamp,
    end_at: &UtcTimestamp,
    proposal: &DecisionProposal,
) -> Result<(Option<AdmissionResult>, Option<PendingOrderGroup>), OrchestrationError> {
    let eligible_at = next_open(decision_time)
        .map_err(|error| OrchestrationError::at("next-open timestamp", error))?;
    if compare_utc_timestamps(&eligible_at, end_at).is_gt() {
        return Ok((None, None));
    }
    if !proposal.target_positions.is_empty() {
        let admission = admit_target_group(portfolio, &proposal.target_positions)
            .map_err(|error| OrchestrationError::at("group admission", error))?;
        let group = queue_target_group(portfolio, step_id, decision_time, &eligible_at, &admission)
            .map_err(|error| OrchestrationError::at("target order queue", error))?;
        let group = (!group.orders.is_empty()).then_some(group);
        Ok((Some(admission), group))
    } else if !proposal.order_intents.is_empty() {
        let group = queue_order_intents(
            step_id,
            decision_time,
            &eligible_at,
            &proposal.order_intents,
        )
        .map_err(|error| OrchestrationError::at("intent order queue", error))?;
        let group = (!group.orders.is_empty()).then_some(group);
        Ok((None, group))
    } else {
        Ok((None, None))
    }
}

struct SettlementContext<'a> {
    configuration: &'a RunConfiguration,
    fixture: &'a Fixture,
    at: &'a UtcTimestamp,
    marks: &'a [factorsbt_engine::ValuationMark],
    pending_groups: &'a [PendingOrderGroup],
    liquidation_group: Option<&'a PendingOrderGroup>,
}

fn settle_close(
    context: SettlementContext<'_>,
    portfolio: &mut PortfolioState,
    processed_settlement_ids: &mut HashSet<String>,
    progress: &mut RunProgress,
) -> Result<(), OrchestrationError> {
    let SettlementContext {
        configuration,
        fixture,
        at,
        marks,
        pending_groups,
        liquidation_group,
    } = context;
    let marks_by_id = marks
        .iter()
        .map(|mark| (mark.instrument_id.as_str(), mark))
        .collect::<HashMap<_, _>>();
    for instrument in &configuration.instruments {
        let Instrument::LinearFuture(future) = instrument else {
            continue;
        };
        let Some(position) = portfolio.position(future.instrument_id.as_str()) else {
            continue;
        };
        if position.quantity.is_zero() {
            continue;
        }
        let mark = marks_by_id
            .get(future.instrument_id.as_str())
            .ok_or_else(|| OrchestrationError::at("futures settlement", "missing future mark"))?;
        let provenance = mark.provenance.first().ok_or_else(|| {
            OrchestrationError::at("futures settlement", "future mark lacks source provenance")
        })?;
        let settlement_price = mark.mark;
        let result = if compare_utc_timestamps(at, &future.expiry_at).is_ge() {
            portfolio.settle_future_expiry(
                future.instrument_id.as_str(),
                at,
                settlement_price,
                &provenance.observation_id,
                provenance.source_revision,
            )
        } else {
            portfolio.settle_future_daily(
                future.instrument_id.as_str(),
                settlement_price,
                &provenance.observation_id,
                provenance.source_revision,
            )
        }
        .map_err(|error| OrchestrationError::at("futures settlement", error))?;
        progress.settlement_events.push(json!({
            "type":"future_settlement",
            "instrument_id":result.instrument_id.as_str(),
            "settled_quantity":result.settled_quantity.to_string(),
            "settlement_price":result.settlement_price.to_string(),
            "variation_profit_loss":result.variation_profit_loss.to_string(),
            "expired":result.expired,
            "source_observation_id":result.source_observation_id,
            "source_revision":result.source_revision,
            "ledger_posting":posting_value(result.posting),
        }));
        progress.capture_state(portfolio, pending_groups, liquidation_group);
    }

    let available_events = settlement_events_available_at(&fixture.dataset, at)
        .map_err(|error| OrchestrationError::at("settlement event selection", error))?;
    for event in &available_events {
        let id = event.fixture_event_id.as_str().to_owned();
        if processed_settlement_ids.contains(&id) {
            continue;
        }
        let result = portfolio
            .apply_american_settlement_event(event)
            .map_err(|error| OrchestrationError::at("American option settlement", error))?;
        processed_settlement_ids.insert(id);
        progress.settlement_events.push(json!({
            "type":"american_physical_settlement",
            "fixture_event_id":result.fixture_event_id,
            "instrument_id":result.instrument_id.as_str(),
            "event_type":format!("{:?}", result.event_type),
            "contracts":result.contracts,
            "underlying_price":result.underlying_price.to_string(),
            "cash_amount":result.cash_amount.to_string(),
            "underlying_quantity_delta":result.underlying_quantity_delta.to_string(),
            "event_time":result.event_time.as_str(),
            "source_observation_id":result.source_observation_id,
            "source_revision":result.source_revision,
            "option_posting":posting_value(result.option_posting),
            "underlying_posting":posting_value(result.underlying_posting),
        }));
        progress.capture_state(portfolio, pending_groups, liquidation_group);
    }

    for instrument in &configuration.instruments {
        let Instrument::Option(option) = instrument else {
            continue;
        };
        if compare_utc_timestamps(at, &option.expiry_at).is_lt()
            || portfolio
                .position(option.instrument_id.as_str())
                .is_none_or(|position| position.quantity.is_zero())
        {
            continue;
        }
        let underlying = marks_by_id
            .get(option.underlying_id.as_str())
            .ok_or_else(|| {
                OrchestrationError::at("option expiry", "missing current underlying mark")
            })?;
        let provenance = underlying.provenance.first().ok_or_else(|| {
            OrchestrationError::at("option expiry", "underlying mark lacks source provenance")
        })?;
        match (option.exercise_style, option.settlement_style) {
            (ExerciseStyle::European, SettlementStyle::Cash) => {
                let result = portfolio
                    .settle_european_option_expiry(
                        option.instrument_id.as_str(),
                        at,
                        underlying.mark,
                        &provenance.observation_id,
                        provenance.source_revision,
                    )
                    .map_err(|error| OrchestrationError::at("European option expiry", error))?;
                progress.settlement_events.push(json!({
                    "type":"european_cash_expiry",
                    "instrument_id":result.instrument_id.as_str(),
                    "settled_contracts":result.settled_contracts.to_string(),
                    "underlying_price":result.underlying_price.to_string(),
                    "intrinsic_value_per_unit":result.intrinsic_value_per_unit.to_string(),
                    "cash_amount":result.cash_amount.to_string(),
                    "source_observation_id":result.source_observation_id,
                    "source_revision":result.source_revision,
                    "ledger_posting":posting_value(result.posting),
                }));
                progress.capture_state(portfolio, pending_groups, liquidation_group);
            }
            (ExerciseStyle::American, SettlementStyle::Physical) => {
                let result = portfolio
                    .expire_american_option_without_event(
                        option.instrument_id.as_str(),
                        at,
                        underlying.mark,
                        &provenance.observation_id,
                        provenance.source_revision,
                    )
                    .map_err(|error| OrchestrationError::at("American option expiry", error))?;
                progress.settlement_events.push(json!({
                    "type":"american_worthless_expiry",
                    "instrument_id":result.instrument_id.as_str(),
                    "expired_contracts":result.expired_contracts.to_string(),
                    "underlying_price":result.underlying_price.to_string(),
                    "event_time":result.event_time.as_str(),
                    "source_observation_id":result.source_observation_id,
                    "source_revision":result.source_revision,
                    "ledger_posting":posting_value(result.posting),
                }));
                progress.capture_state(portfolio, pending_groups, liquidation_group);
            }
            _ => {
                return Err(OrchestrationError::at(
                    "option expiry",
                    "unsupported option style and settlement pairing",
                ));
            }
        }
    }
    Ok(())
}

fn portfolio_snapshot(portfolio: &PortfolioState) -> Result<PortfolioSnapshot, String> {
    let mut positions = portfolio
        .positions()
        .iter()
        .map(|(id, position)| {
            Ok(PositionSnapshot {
                instrument_id: InstrumentId::new(id).map_err(|error| error.to_string())?,
                quantity: wire_decimal(position.quantity)?,
                mark: wire_decimal(position.mark)?,
                market_value: wire_decimal(
                    portfolio
                        .position_market_value(id)
                        .map_err(|error| error.to_string())?,
                )?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    positions.sort_by(|left, right| {
        left.instrument_id
            .as_str()
            .cmp(right.instrument_id.as_str())
    });
    let mut reserved_underlying = portfolio
        .reserved_underlying()
        .iter()
        .map(|(id, quantity)| {
            Ok(UnderlyingReservation {
                instrument_id: InstrumentId::new(id).map_err(|error| error.to_string())?,
                quantity: wire_decimal(*quantity)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    reserved_underlying.sort_by(|left, right| {
        left.instrument_id
            .as_str()
            .cmp(right.instrument_id.as_str())
    });
    let free_buying_power = portfolio
        .cash()
        .checked_sub(portfolio.cash_encumbrance())
        .ok_or_else(|| "free buying power arithmetic overflow".to_owned())?;
    Ok(PortfolioSnapshot {
        cash: wire_decimal(portfolio.cash())?,
        nav: wire_decimal(
            portfolio
                .net_asset_value()
                .map_err(|error| error.to_string())?,
        )?,
        positions,
        reserved_cash: wire_decimal(portfolio.reserved_cash())?,
        reserved_underlying,
        initial_margin_requirement: wire_decimal(portfolio.initial_margin_requirement())?,
        maintenance_margin_requirement: wire_decimal(portfolio.maintenance_margin_requirement())?,
        free_buying_power: wire_decimal(free_buying_power)?,
    })
}

fn wire_decimal(value: Decimal) -> Result<factorsbt_contracts::DecimalString, String> {
    let value = value.round_dp_with_strategy(18, RoundingStrategy::MidpointNearestEven);
    factorsbt_contracts::DecimalString::new(value.normalize().to_string())
        .map_err(|error| error.to_string())
}

fn maintenance_value(at: &UtcTimestamp, breach: &factorsbt_engine::MaintenanceBreach) -> Value {
    json!({
        "type":"maintenance_breach",
        "simulation_time":at.as_str(),
        "cash":breach.assessment.cash.to_string(),
        "nav":breach.assessment.nav.to_string(),
        "maintenance_requirement":breach.assessment.maintenance_requirement.to_string(),
        "maintenance_cash_requirement":breach.assessment.maintenance_cash_requirement.to_string(),
        "negative_cash":breach.assessment.negative_cash,
        "cash_collateral_shortfall":breach.assessment.cash_collateral_shortfall,
        "nav_shortfall":breach.assessment.nav_shortfall,
        "underlying_shortfalls":breach.assessment.underlying_shortfalls.iter().map(|(id, required, available)| json!({
            "instrument_id":id,
            "required":required.to_string(),
            "available":available.to_string(),
        })).collect::<Vec<_>>(),
        "canceled_order_ids":breach.canceled_order_ids,
    })
}

fn execution_value(group: &ExecutedOrderGroup) -> Value {
    json!({
        "type":"executed_order_group",
        "step_id":group.step_id.as_str(),
        "execution_time":group.execution_time.as_str(),
        "admission_adjustments":group.admission_adjustments.iter().map(|adjustment| json!({
            "instrument_id":adjustment.instrument_id,
            "requested_quantity":adjustment.requested_quantity.to_string(),
            "admitted_quantity":adjustment.admitted_quantity.to_string(),
            "common_scale":adjustment.common_scale.to_string(),
            "scaled":adjustment.scaled,
        })).collect::<Vec<_>>(),
        "fills":group.fills.iter().map(fill_value).collect::<Vec<_>>(),
    })
}

fn fill_value(fill: &factorsbt_engine::ExecutionFill) -> Value {
    match fill {
        factorsbt_engine::ExecutionFill::Spot(value) => json!({
            "type":"spot",
            "instrument_id":value.instrument_id.as_str(),
            "signed_quantity":value.signed_quantity.to_string(),
            "fill_price":value.fill_price.to_string(),
            "fee":value.fee.to_string(),
            "source_observation_id":value.source_observation_id,
            "source_revision":value.source_revision,
            "trade_posting":posting_value(value.trade_posting),
            "fee_posting":posting_value(value.fee_posting),
        }),
        factorsbt_engine::ExecutionFill::Future(value) => json!({
            "type":"future",
            "instrument_id":value.instrument_id.as_str(),
            "signed_quantity":value.signed_quantity.to_string(),
            "fill_price":value.fill_price.to_string(),
            "fee":value.fee.to_string(),
            "realized_variation":value.realized_variation.to_string(),
            "source_observation_id":value.source_observation_id,
            "source_revision":value.source_revision,
            "trade_posting":posting_value(value.trade_posting),
            "fee_posting":posting_value(value.fee_posting),
        }),
        factorsbt_engine::ExecutionFill::Option(value) => json!({
            "type":"option",
            "instrument_id":value.instrument_id.as_str(),
            "signed_contracts":value.signed_contracts.to_string(),
            "fill_premium":value.fill_premium.to_string(),
            "fee":value.fee.to_string(),
            "source_observation_id":value.source_observation_id,
            "source_revision":value.source_revision,
            "premium_posting":posting_value(value.premium_posting),
            "fee_posting":posting_value(value.fee_posting),
        }),
    }
}

fn posting_value(posting: factorsbt_engine::LedgerPosting) -> Value {
    json!({
        "cash_before":posting.cash_before.to_string(),
        "cash_delta":posting.cash_delta.to_string(),
        "cash_after":posting.cash_after.to_string(),
        "quantity_before":posting.quantity_before.to_string(),
        "quantity_delta":posting.quantity_delta.to_string(),
        "quantity_after":posting.quantity_after.to_string(),
    })
}

fn format_issues(issues: &[factorsbt_data::CoverageIssue]) -> String {
    issues
        .iter()
        .map(|issue| issue.message.as_str())
        .collect::<Vec<_>>()
        .join("; ")
}

fn instrument_id(instrument: &Instrument) -> &str {
    match instrument {
        Instrument::Spot(value) => value.instrument_id.as_str(),
        Instrument::LinearFuture(value) => value.instrument_id.as_str(),
        Instrument::Option(value) => value.instrument_id.as_str(),
    }
}

fn model_error(stage: &'static str, error: ModelServiceError) -> OrchestrationError {
    OrchestrationError::at(stage, error)
}

fn derived_uuid(identity: &RunIdentity, step_id: &Identifier, purpose: &str) -> Uuid {
    let digest = Sha256::digest(
        format!(
            "{}:{}:{}:{}",
            identity.run_id.as_str(),
            identity.scenario_id.as_str(),
            step_id.as_str(),
            purpose
        )
        .as_bytes(),
    );
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Uuid::new(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
    .expect("derived hash has valid lowercase UUID wire form")
}

fn next_open(close: &UtcTimestamp) -> Result<UtcTimestamp, &'static str> {
    if !close.as_str().ends_with("T16:00:00Z") {
        return Err("decision timestamp must be a synthetic daily close");
    }
    let date = &close.as_str()[..10];
    let mut parts = date.split('-');
    let (Some(year), Some(month), Some(day)) = (parts.next(), parts.next(), parts.next()) else {
        return Err("invalid synthetic date");
    };
    let (Ok(year), Ok(month), Ok(day)) = (
        year.parse::<i64>(),
        month.parse::<i64>(),
        day.parse::<i64>(),
    ) else {
        return Err("invalid synthetic date");
    };
    let next = civil_from_days(days_from_civil(year, month, day) + 1);
    UtcTimestamp::new(format!(
        "{:04}-{:02}-{:02}T09:00:00Z",
        next.0, next.1, next.2
    ))
    .map_err(|_| "next synthetic open is outside timestamp range")
}

pub(crate) fn system_utc_timestamp() -> Result<UtcTimestamp, &'static str> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock precedes Unix epoch")?
        .as_secs();
    let day_number = i64::try_from(seconds / 86_400).map_err(|_| "system time is out of range")?;
    let seconds_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(day_number);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    UtcTimestamp::new(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
    ))
    .map_err(|_| "system clock does not produce a supported UTC timestamp")
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let adjusted_year = year - i64::from(month <= 2);
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year - era * 400;
    let adjusted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * adjusted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn civil_from_days(day_number: i64) -> (i64, i64, i64) {
    let shifted = day_number + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_fixture::HttpModelFixture;
    use axum::{extract::State, routing::post, Json, Router};
    use factorsbt_data::FixtureRegistry;
    use serde_json::json;
    use std::{
        net::SocketAddr,
        path::Path,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        time::{SystemTime, UNIX_EPOCH},
    };
    use tokio::net::TcpListener;
    use tokio::sync::{mpsc, Notify};

    struct ConcurrentModelState {
        failing_run_id: String,
        requests: tokio::sync::Mutex<Vec<String>>,
        responses: tokio::sync::Mutex<Vec<String>>,
        request_seen: mpsc::UnboundedSender<String>,
        both_requests: Notify,
        slow_response_release: Notify,
        slow_response_released: AtomicBool,
    }

    async fn concurrent_model_response(
        State(state): State<Arc<ConcurrentModelState>>,
        Json(request): Json<ModelRequest>,
    ) -> Json<Value> {
        let run_id = request.run_id.as_str().to_owned();
        let is_failing_run = run_id == state.failing_run_id;
        let count = {
            let mut requests = state.requests.lock().await;
            requests.push(run_id.clone());
            requests.len()
        };
        let _ = state.request_seen.send(run_id.clone());
        if count == 2 {
            state.both_requests.notify_one();
        }
        loop {
            let notified = state.both_requests.notified();
            if state.requests.lock().await.len() >= 2 {
                break;
            }
            notified.await;
        }

        if is_failing_run {
            loop {
                let notified = state.slow_response_release.notified();
                if state.slow_response_released.load(Ordering::Acquire) {
                    break;
                }
                notified.await;
            }
        }

        let response_type = if is_failing_run { "targets" } else { "scores" };
        let outputs = if is_failing_run {
            json!({"spot_weights":[],"derivative_contracts":[]})
        } else {
            json!({"scores":[{"instrument_id":"SPOT_A","score":"2"}]})
        };
        let body = json!({
            "contract_version":request.contract_version,
            "inference_id":request.inference_id,
            "run_id":request.run_id,
            "scenario_id":request.scenario_id,
            "step_id":request.step_id,
            "snapshot_id":request.snapshot_id,
            "model_id":request.model_id,
            "model_version":request.model_version,
            "responded_at":system_utc_timestamp().unwrap(),
            "response_type":response_type,
            "outputs":outputs,
        });
        state.responses.lock().await.push(run_id);
        Json(body)
    }

    fn configuration() -> RunConfiguration {
        let mut configuration: RunConfiguration = serde_json::from_str(include_str!(
            "../../../../contracts/examples/instruments/minimal-valid-spot-run.json"
        ))
        .unwrap();
        configuration.simulation.end_at = UtcTimestamp::new("2026-01-06T16:00:00Z").unwrap();
        configuration
    }

    fn fixture() -> Fixture {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures/data");
        FixtureRegistry::load(root)
            .unwrap()
            .get_by("mvp-synthetic", "v1")
            .unwrap()
            .as_ref()
            .clone()
    }

    async fn serve(router: Router) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address: SocketAddr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        (format!("http://{address}/infer"), task)
    }

    #[tokio::test]
    async fn one_spot_decision_cycle_executes_at_next_open_and_keeps_provenance() {
        let mut configuration = configuration();
        configuration.decision_policy.output_mode = factorsbt_contracts::model::OutputMode::Scores;
        let response_time = system_utc_timestamp().unwrap();
        let fixture_service = HttpModelFixture::new(
            "scores",
            json!({"scores":[{"instrument_id":"SPOT_A","score":"2"}]}),
            response_time,
        );
        let (endpoint, server) = serve(fixture_service.router()).await;
        let mut progress_updates = Vec::new();
        let report = run_simulation_with_progress(
            &ModelHttpClient::new().unwrap(),
            &endpoint,
            Duration::from_secs(2),
            &configuration,
            &fixture(),
            RunIdentity {
                run_id: Uuid::new("a1139f93-8119-4be6-8c84-b6835fb3647e").unwrap(),
                scenario_id: Identifier::new("baseline").unwrap(),
            },
            |progress| progress_updates.push(progress),
        )
        .await
        .unwrap();
        server.abort();

        assert_eq!(report.cycles.len(), 2);
        assert_eq!(progress_updates.len(), 2);
        assert_eq!(progress_updates[0].completed_steps, 1);
        assert_eq!(progress_updates[0].total_steps, 2);
        assert_eq!(progress_updates[1].completed_steps, 2);
        assert_eq!(
            progress_updates[1].simulation_time,
            report.cycles[1].snapshot.simulation_time
        );
        assert_eq!(report.execution_events.len(), 1);
        assert_eq!(
            report.cycles[0].snapshot.simulation_time.as_str(),
            "2026-01-05T16:00:00Z"
        );
        assert_eq!(
            report.cycles[1].snapshot.simulation_time.as_str(),
            "2026-01-06T16:00:00Z"
        );
        assert_eq!(
            report.execution_events[0]["execution_time"],
            "2026-01-06T09:00:00Z"
        );
        assert!(!report.cycles[0].response_body.is_empty());
        assert_eq!(report.finalization.status, FinalRunStatus::Succeeded);
        assert_eq!(report.finalization.open_positions.len(), 1);
        assert_eq!(
            report.finalization.open_positions[0].instrument_id,
            "SPOT_A"
        );
    }

    #[tokio::test]
    async fn a_model_mode_mismatch_fails_before_any_order_is_queued() {
        let configuration = configuration();
        let fixture_service = HttpModelFixture::new(
            "targets",
            json!({"spot_weights":[],"derivative_contracts":[]}),
            system_utc_timestamp().unwrap(),
        );
        let (endpoint, server) = serve(fixture_service.router()).await;
        let error = run_simulation(
            &ModelHttpClient::new().unwrap(),
            &endpoint,
            Duration::from_secs(2),
            &configuration,
            &fixture(),
            RunIdentity {
                run_id: Uuid::new("a1139f93-8119-4be6-8c84-b6835fb3647e").unwrap(),
                scenario_id: Identifier::new("baseline").unwrap(),
            },
        )
        .await
        .unwrap_err();
        server.abort();
        assert_eq!(error.error.stage, "model response validation");
        let partial = error.partial_report.unwrap();
        assert_eq!(partial.finalization.status, FinalRunStatus::Failed);
        assert_eq!(
            partial.finalization.failure_reason.as_deref(),
            Some("model response validation failed: ModeMismatch")
        );
        assert_eq!(partial.finalization.cash, Decimal::from(100_000));
        let failed_exchange = error.failed_inference.unwrap();
        assert_eq!(failed_exchange.request.step_id.as_str(), "step-0001");
        assert!(!failed_exchange.request_body.is_empty());
        assert!(failed_exchange
            .response_body
            .as_ref()
            .is_some_and(|body| !body.is_empty()));
    }

    #[tokio::test]
    async fn concurrent_runs_keep_model_responses_portfolios_and_evidence_isolated() {
        let failing_run_id = "b2139f93-8119-4be6-8c84-b6835fb3647e".to_owned();
        let successful_run_id = "c3139f93-8119-4be6-8c84-b6835fb3647e".to_owned();
        let (request_seen, mut request_receiver) = mpsc::unbounded_channel();
        let state = Arc::new(ConcurrentModelState {
            failing_run_id: failing_run_id.clone(),
            requests: tokio::sync::Mutex::new(Vec::new()),
            responses: tokio::sync::Mutex::new(Vec::new()),
            request_seen,
            both_requests: Notify::new(),
            slow_response_release: Notify::new(),
            slow_response_released: AtomicBool::new(false),
        });
        let router = Router::new()
            .route("/infer", post(concurrent_model_response))
            .with_state(Arc::clone(&state));
        let (endpoint, server) = serve(router).await;
        let client = ModelHttpClient::new().unwrap();
        let mut failing_configuration = configuration();
        failing_configuration.decision_policy.output_mode =
            factorsbt_contracts::model::OutputMode::Scores;
        let successful_configuration = failing_configuration.clone();
        let failing_fixture = fixture();
        let successful_fixture = failing_fixture.clone();
        let failing_endpoint = endpoint.clone();
        let failing_client = client.clone();
        let failing = tokio::spawn(async move {
            run_simulation(
                &failing_client,
                &failing_endpoint,
                Duration::from_secs(2),
                &failing_configuration,
                &failing_fixture,
                RunIdentity {
                    run_id: Uuid::new(failing_run_id).unwrap(),
                    scenario_id: Identifier::new("slow-failure").unwrap(),
                },
            )
            .await
        });

        assert_eq!(
            request_receiver.recv().await.as_deref(),
            Some("b2139f93-8119-4be6-8c84-b6835fb3647e")
        );
        let successful = tokio::spawn(async move {
            run_simulation(
                &client,
                &endpoint,
                Duration::from_secs(2),
                &successful_configuration,
                &successful_fixture,
                RunIdentity {
                    run_id: Uuid::new(successful_run_id).unwrap(),
                    scenario_id: Identifier::new("fast-success").unwrap(),
                },
            )
            .await
        });
        assert_eq!(
            request_receiver.recv().await.as_deref(),
            Some("c3139f93-8119-4be6-8c84-b6835fb3647e")
        );
        assert!(state.requests.lock().await.len() >= 2);

        // Do not release the older failing response until the newer run has
        // received all of its model responses and completed its portfolio.
        let success_report = successful.await.unwrap().unwrap();
        state.slow_response_released.store(true, Ordering::Release);
        state.slow_response_release.notify_one();
        let failure = failing.await.unwrap().unwrap_err();
        server.abort();

        let requests = state.requests.lock().await.clone();
        let responses = state.responses.lock().await.clone();
        assert_eq!(requests[0], "b2139f93-8119-4be6-8c84-b6835fb3647e");
        assert!(requests.iter().all(|run_id| {
            run_id == "b2139f93-8119-4be6-8c84-b6835fb3647e"
                || run_id == "c3139f93-8119-4be6-8c84-b6835fb3647e"
        }));
        assert_eq!(
            responses.last().map(String::as_str),
            Some("b2139f93-8119-4be6-8c84-b6835fb3647e")
        );
        assert!(responses[..responses.len() - 1]
            .iter()
            .all(|run_id| run_id == "c3139f93-8119-4be6-8c84-b6835fb3647e"));

        assert_eq!(
            success_report.identity.run_id.as_str(),
            "c3139f93-8119-4be6-8c84-b6835fb3647e"
        );
        assert_eq!(
            success_report.finalization.status,
            FinalRunStatus::Succeeded
        );
        assert_eq!(success_report.cycles.len(), 2);
        assert_eq!(success_report.execution_events.len(), 1);
        assert!(success_report
            .cycles
            .iter()
            .all(|cycle| cycle.request.run_id.as_str() == "c3139f93-8119-4be6-8c84-b6835fb3647e"));
        assert_eq!(
            failure.identity.run_id.as_str(),
            "b2139f93-8119-4be6-8c84-b6835fb3647e"
        );
        assert_eq!(failure.error.stage, "model response validation");
        let failed_report = failure.partial_report.as_deref().unwrap();
        assert_eq!(failed_report.finalization.status, FinalRunStatus::Failed);
        assert_eq!(failed_report.execution_events.len(), 0);
        assert!(failed_report.cycles.is_empty());
        assert_eq!(
            failure
                .failed_inference
                .as_deref()
                .unwrap()
                .request
                .run_id
                .as_str(),
            "b2139f93-8119-4be6-8c84-b6835fb3647e"
        );

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let evidence_root = std::env::temp_dir().join(format!(
            "factorsbt-concurrent-evidence-{}-{nonce}",
            std::process::id()
        ));
        let fixture = fixture();
        let success_evidence = crate::evidence::write_successful_run(
            &evidence_root,
            &configuration(),
            &fixture,
            &success_report,
        )
        .unwrap();
        let failure_evidence =
            crate::evidence::write_failed_run(&evidence_root, &configuration(), &fixture, &failure)
                .unwrap();
        assert_ne!(
            success_evidence.run_directory,
            failure_evidence.run_directory
        );
        let success_manifest: Value = serde_json::from_slice(
            &std::fs::read(
                success_evidence
                    .run_directory
                    .join("evidence/run-manifest.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let failure_manifest: Value = serde_json::from_slice(
            &std::fs::read(
                failure_evidence
                    .run_directory
                    .join("evidence/run-manifest.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            success_manifest["run_id"],
            "c3139f93-8119-4be6-8c84-b6835fb3647e"
        );
        assert_eq!(
            failure_manifest["run_id"],
            "b2139f93-8119-4be6-8c84-b6835fb3647e"
        );
        let success_orders: Value = serde_json::from_slice(
            &std::fs::read(
                success_evidence
                    .run_directory
                    .join("evidence/executions.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let failed_orders: Value = serde_json::from_slice(
            &std::fs::read(
                failure_evidence
                    .run_directory
                    .join("evidence/executions.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(success_orders.as_array().unwrap().len(), 1);
        assert!(failed_orders.as_array().unwrap().is_empty());
        std::fs::remove_dir_all(evidence_root).unwrap();
    }

    #[test]
    fn derived_ids_are_stable_and_processing_time_is_wall_clock_utc() {
        let identity = RunIdentity {
            run_id: Uuid::new("a1139f93-8119-4be6-8c84-b6835fb3647e").unwrap(),
            scenario_id: Identifier::new("baseline").unwrap(),
        };
        let step = Identifier::new("step-0001").unwrap();
        assert_eq!(
            derived_uuid(&identity, &step, "snapshot"),
            derived_uuid(&identity, &step, "snapshot")
        );
        assert_ne!(
            derived_uuid(&identity, &step, "snapshot"),
            derived_uuid(&identity, &step, "inference")
        );
        assert!(system_utc_timestamp().unwrap().as_str().ends_with('Z'));
        assert!(matches!(
            validate_model_response(
                &serde_json::from_str::<ModelRequest>(include_str!(
                    "../../../../contracts/examples/model/scores-request.inline-snapshot.json"
                ))
                .unwrap(),
                &serde_json::from_str(include_str!(
                    "../../../../contracts/examples/model/scores-response.valid.json"
                ))
                .unwrap()
            ),
            Ok(())
        ));
    }
}
