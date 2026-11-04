import sys
import unittest
from pathlib import Path
from unittest.mock import call, patch

sys.path.insert(0, str(Path(__file__).parents[1] / "src"))

from factorsbt import (
    Client, ComparisonScenario, ComparisonSpec, DataFixtureConfiguration,
    HttpStatusError, InitialPortfolio, ModelConfiguration, RunConfiguration,
    RunFailedError, RunStatus, ScoresPolicy, SimulationConfiguration,
    SpotInstrument, SubmissionAcknowledgementUnknown, SubmissionStatus, UtcInstant,
)


def config():
    return RunConfiguration(
        SimulationConfiguration("2026-01-05T16:00:00Z", "2026-01-09T16:00:00Z"),
        InitialPortfolio("100000"),
        (SpotInstrument("SPOT_A", "0.01", "0.001", "1", "0", "0", "0"),),
        DataFixtureConfiguration("mvp-synthetic", "v1"),
        ModelConfiguration("synthetic-model", "v1", "http://127.0.0.1:8787/infer", 5000),
        ScoresPolicy(1),
    )


def status(run_id, lifecycle):
    return RunStatus(run_id, lifecycle, UtcInstant.from_wire("2026-01-05T09:00:00Z"), None, None, None, None)


class ComparisonClientTests(unittest.TestCase):
    def setUp(self):
        self.spec = ComparisonSpec(
            ComparisonScenario("base", config()),
            (ComparisonScenario("variant-a", config()), ComparisonScenario("variant-b", config())),
        )

    def test_rejected_scenario_is_recorded_and_later_scenarios_continue_in_order(self):
        client = Client("http://127.0.0.1:8080")
        with patch.object(Client, "submit", side_effect=[HttpStatusError(422), HttpStatusError(503), HttpStatusError(422)]) as submit:
            report = client.compare(self.spec)
        self.assertEqual([run.status for run in report.runs], ["rejected", "rejected", "rejected"])
        self.assertEqual(submit.call_count, 3)
        self.assertEqual(report.runs[0].error_code, "SUBMISSION_REJECTED")

    def test_uncertain_ack_uses_lookup_without_second_post_and_keeps_partial_failure(self):
        client = Client("http://127.0.0.1:8080")
        key = "8a2f5ce2-a6b5-4a71-b587-fefdb4a9fbc5"
        run_id = "c827fcbc-c5fc-48a4-b4aa-21a63f64727e"
        recovered = SubmissionStatus(key, run_id, "running", UtcInstant.from_wire("2026-01-05T09:00:00Z"))
        failed = status(run_id, "failed")
        failed = RunStatus(failed.run_id, "failed", failed.submitted_at, None, None, None,
                           __import__("factorsbt").RunFailure("MODEL_TIMEOUT", "fixture timeout"))
        with patch.object(Client, "submit", side_effect=[SubmissionAcknowledgementUnknown(key, "timeout"), HttpStatusError(422), HttpStatusError(422)]) as submit:
            with patch.object(Client, "get_submission", return_value=recovered) as lookup:
                with patch.object(Client, "wait", side_effect=RunFailedError(failed)):
                    report = client.compare(self.spec)
        self.assertEqual(report.runs[0].status, "failed")
        self.assertEqual(report.runs[0].run_id, run_id)
        self.assertEqual(report.runs[0].error_code, "MODEL_TIMEOUT")
        self.assertEqual(submit.call_count, 3)
        lookup.assert_called_once_with(key)


if __name__ == "__main__":
    unittest.main()
