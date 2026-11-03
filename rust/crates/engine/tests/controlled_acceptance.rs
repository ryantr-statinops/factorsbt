use factorsbt_contracts::{
    instruments::Instrument,
    model::{DecisionPolicy, ModelResponse, OutputMode, ScorePolicy},
    run_configuration::RunConfiguration,
    Identifier, UtcTimestamp,
};
use factorsbt_data::{execution_prices_available_at, observations_available_at, FixtureRegistry};
use factorsbt_engine::{
    admit_target_group, assess_maintenance, execute_pending_group, finalize_run_state,
    queue_target_group, scores_to_targets, value_instruments, FinalRunStatus, PortfolioState,
};
use rust_decimal::Decimal;
use std::path::Path;

fn baseline() -> (RunConfiguration, factorsbt_data::Fixture) {
    let mut config: RunConfiguration = serde_json::from_str(include_str!(
        "../../../../contracts/examples/instruments/minimal-valid-spot-run.json"
    ))
    .unwrap();
    config.instruments.push(serde_json::from_str(
        r#"{"instrument_id":"SPOT_B","instrument_type":"spot","quote_currency":"USD","price_tick":"0.01","quantity_step":"0.001","contract_multiplier":"1","fee_bps":"0","fee_per_unit":"0","slippage_bps":"0"}"#,
    ).unwrap());
    assert!(config
        .instruments
        .iter()
        .all(|item| matches!(item, Instrument::Spot(_))));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../contracts/fixtures/data");
    let registry = FixtureRegistry::load(root).unwrap();
    let fixture = (*registry.get_by("mvp-synthetic", "v1").unwrap()).clone();
    (config, fixture)
}

fn scores_response() -> ModelResponse {
    serde_json::from_str(
        r#"{"contract_version":"1.0.0","inference_id":"de87564a-ae4a-4768-bb05-97bc1d816d8f","run_id":"c827fcbc-c5fc-48a4-b4aa-21a63f64727e","scenario_id":"baseline","step_id":"step-0001","snapshot_id":"4fe68f00-0ddb-4e72-ad9b-7bc09d84d5aa","model_id":"synthetic-model","model_version":"v1","responded_at":"2026-01-05T16:00:01Z","response_type":"scores","outputs":{"scores":[{"instrument_id":"SPOT_B","score":"1"},{"instrument_id":"SPOT_A","score":"2"}]}}"#,
    )
    .unwrap()
}

#[test]
fn accepted_spot_decision_runs_through_admission_next_open_and_finalization() {
    let (config, fixture) = baseline();
    let mut portfolio = PortfolioState::initialize(&config, &fixture).unwrap();
    let close = UtcTimestamp::new("2026-01-05T16:00:00Z").unwrap();
    let observations = observations_available_at(&fixture.dataset, &close).unwrap();
    let marks = value_instruments(&config.instruments, &observations, &close).unwrap();
    portfolio.apply_valuation_marks(&marks).unwrap();

    let policy = DecisionPolicy {
        output_mode: OutputMode::Scores,
        score_policy: Some(ScorePolicy {
            top_k: 2,
            derivative_score_targets: Vec::new(),
        }),
    };
    let targets =
        scores_to_targets(&policy, &scores_response(), &config.instruments, &portfolio).unwrap();
    assert_eq!(targets[0].instrument_id.as_str(), "SPOT_A");
    assert_eq!(targets[0].target_quantity, Decimal::from(500));
    assert_eq!(targets[1].instrument_id.as_str(), "SPOT_B");
    assert_eq!(targets[1].target_quantity, Decimal::from(1000));
    let admission = admit_target_group(&portfolio, &targets).unwrap();
    assert!(admission.adjustments.iter().all(|entry| !entry.scaled));

    let step = Identifier::new("step-0001").unwrap();
    let open = UtcTimestamp::new("2026-01-06T09:00:00Z").unwrap();
    let group = queue_target_group(&portfolio, &step, &close, &open, &admission).unwrap();
    assert_eq!(group.orders.len(), 2);
    let prices = execution_prices_available_at(&fixture.dataset, &open).unwrap();
    let execution = execute_pending_group(&mut portfolio, &group, &open, &prices).unwrap();
    assert_eq!(execution.fills.len(), 2);
    assert!(execution
        .admission_adjustments
        .iter()
        .all(|entry| entry.scaled));
    assert!(portfolio.cash() >= Decimal::ZERO);
    assert!(portfolio.position("SPOT_A").unwrap().quantity < Decimal::from(500));
    assert!(portfolio.position("SPOT_B").unwrap().quantity < Decimal::from(1000));

    let close_next_day = UtcTimestamp::new("2026-01-06T16:00:00Z").unwrap();
    let next_observations = observations_available_at(&fixture.dataset, &close_next_day).unwrap();
    let next_marks =
        value_instruments(&config.instruments, &next_observations, &close_next_day).unwrap();
    portfolio.apply_valuation_marks(&next_marks).unwrap();
    assert_eq!(
        portfolio.position("SPOT_A").unwrap().mark,
        Decimal::from(101)
    );
    assert!(!assess_maintenance(&portfolio).unwrap().is_breach());

    let mut pending = Vec::new();
    let finalization = finalize_run_state(&portfolio, &mut pending, None).unwrap();
    assert_eq!(finalization.status, FinalRunStatus::Succeeded);
    assert!(finalization.canceled_order_ids.is_empty());
    assert_eq!(finalization.open_positions.len(), 2);
    assert!(finalization.nav > Decimal::from(100_000));
}
