use factorsbt_contracts::{EngineEvent, Instrument, ModelRequest, ModelResponse, RunResult};

#[test]
fn request_and_all_model_response_modes_deserialize_shared_examples() {
    let request: ModelRequest = serde_json::from_str(include_str!(
        "../../../../contracts/examples/model/scores-request.inline-snapshot.json"
    ))
    .unwrap();
    assert_eq!(request.contract_version, "1.0.0");
    assert_eq!(
        request.snapshot["snapshot_id"],
        request.snapshot_id.as_str()
    );

    let responses = [
        include_str!("../../../../contracts/examples/model/scores-response.valid.json"),
        include_str!("../../../../contracts/examples/model/targets-response.valid-shape.json"),
        include_str!(
            "../../../../contracts/examples/model/order-intents-response.valid-shape.json"
        ),
    ];
    for payload in responses {
        serde_json::from_str::<ModelResponse>(payload).unwrap();
    }
}

#[test]
fn configured_instruments_and_run_result_deserialize_shared_examples() {
    let run: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../contracts/examples/instruments/minimal-valid-spot-run.json"
    ))
    .unwrap();
    let instrument: Instrument = serde_json::from_value(run["instruments"][0].clone()).unwrap();
    assert!(matches!(instrument, Instrument::Spot(_)));

    let result: RunResult = serde_json::from_str(include_str!(
        "../../../../contracts/examples/results/successful-run.valid.json"
    ))
    .unwrap();
    assert_eq!(result.contract_version, "1.0.0");
}

#[test]
fn shared_engine_examples_and_schema_invalid_case_match_typed_events() {
    let valid_events = [
        include_str!("../../../../contracts/examples/engine/admission-adjusted.valid.json"),
        include_str!("../../../../contracts/examples/engine/order-created.valid.json"),
        include_str!("../../../../contracts/examples/engine/fill.valid.json"),
        include_str!(
            "../../../../contracts/examples/engine/fill-zero-quantity.semantic-invalid.json"
        ),
        include_str!("../../../../contracts/examples/engine/ledger-spot-purchase.valid.json"),
        include_str!("../../../../contracts/examples/engine/futures-variation.ledger-valid.json"),
        include_str!(
            "../../../../contracts/examples/engine/futures-variation.settlement-valid.json"
        ),
        include_str!("../../../../contracts/examples/engine/settlement-assignment.valid.json"),
        include_str!("../../../../contracts/examples/engine/risk-liquidation-scheduled.valid.json"),
    ];
    for payload in valid_events {
        serde_json::from_str::<EngineEvent>(payload).unwrap();
    }

    assert!(serde_json::from_str::<EngineEvent>(include_str!(
        "../../../../contracts/examples/engine/fill-missing-price.schema-invalid.json"
    ))
    .is_err());
}
