import json
import sys
import unittest
from decimal import Decimal
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parents[1] / "src"))

from factorsbt import (
    DataFixtureConfiguration,
    DerivativeScoreTarget,
    InitialPortfolio,
    InitialPosition,
    ModelConfiguration,
    OrderIntentsPolicy,
    RunConfiguration,
    ScoresPolicy,
    SimulationConfiguration,
    SpotInstrument,
    TargetsPolicy,
)


ROOT = Path(__file__).resolve().parents[2]


def make_run(policy):
    instrument = SpotInstrument(
        "SPOT_A", "0.01", "0.001", "1", "0", "0", "0"
    )
    return RunConfiguration(
        simulation=SimulationConfiguration(
            "2026-01-05T16:00:00Z", "2026-01-09T16:00:00Z"
        ),
        initial_portfolio=InitialPortfolio(
            cash=Decimal("100000"), positions=()
        ),
        instruments=(instrument,),
        data=DataFixtureConfiguration("mvp-synthetic", "v1"),
        model=ModelConfiguration(
            "synthetic-model", "v1", "http://127.0.0.1:8787/infer", 5000
        ),
        decision_policy=policy,
    )


class RunConfigurationBuilderTests(unittest.TestCase):
    def test_typed_spot_run_matches_shared_run_configuration_example(self):
        example = json.loads(
            (ROOT / "contracts/examples/instruments/minimal-valid-spot-run.json").read_text()
        )
        built = make_run(ScoresPolicy(top_k=1)).to_wire()
        self.assertEqual(built, example)

    def test_policy_builders_emit_only_the_selected_output_shape(self):
        cases = (
            (TargetsPolicy(), {"output_mode": "targets"}),
            (OrderIntentsPolicy(), {"output_mode": "order_intents"}),
            (
                ScoresPolicy(2, (DerivativeScoreTarget("FUT_A", 1),)),
                {
                    "output_mode": "scores",
                    "score_policy": {
                        "top_k": 2,
                        "derivative_score_targets": [
                            {"instrument_id": "FUT_A", "contracts": 1}
                        ],
                    },
                },
            ),
        )
        for policy, expected in cases:
            with self.subTest(policy=policy):
                self.assertEqual(make_run(policy).to_wire()["decision_policy"], expected)

    def test_builder_rejects_invalid_wire_values_without_choosing_financial_defaults(self):
        with self.assertRaises(ValueError):
            InitialPortfolio("-1").to_wire()
        with self.assertRaises(ValueError):
            ModelConfiguration("model", "v1", "http://127.0.0.1/infer", True).to_wire()
        with self.assertRaises(ValueError):
            ScoresPolicy(0).to_wire()
        with self.assertRaises(ValueError):
            InitialPosition("SPOT_A", 1.25).to_wire()

    def test_model_endpoint_is_loopback_http_without_credentials_or_query(self):
        for endpoint in (
            "https://127.0.0.1/infer",
            "http://example.com/infer",
            "http://user:secret@127.0.0.1/infer",
            "http://127.0.0.1/infer?token=secret",
        ):
            with self.subTest(endpoint=endpoint), self.assertRaises(ValueError):
                ModelConfiguration("model", "v1", endpoint, 5000).to_wire()


if __name__ == "__main__":
    unittest.main()
