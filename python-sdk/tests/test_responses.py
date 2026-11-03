import copy
import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parents[1] / "src"))

from factorsbt import ProtocolError, RunResult, RunStatus, SubmissionAccepted, SubmissionStatus

ROOT = Path(__file__).resolve().parents[2]


class TypedResponseTests(unittest.TestCase):
    def test_decodes_submission_and_lifecycle_responses(self):
        ack = SubmissionAccepted.from_wire(
            {
                "submission_id": "8a2f5ce2-a6b5-4a71-b587-fefdb4a9fbc5",
                "run_id": "c827fcbc-c5fc-48a4-b4aa-21a63f64727e",
                "status": "queued",
                "submitted_at": "2026-01-05T09:00:00Z",
            }
        )
        self.assertEqual(ack.status, "queued")
        self.assertEqual(ack.submission_id, "8a2f5ce2-a6b5-4a71-b587-fefdb4a9fbc5")

        submission = SubmissionStatus.from_wire(
            {
                "submission_id": ack.submission_id,
                "run_id": ack.run_id,
                "status": "running",
                "submitted_at": "2026-01-05T09:00:00Z",
            }
        )
        self.assertEqual(submission.status, "running")

        run = RunStatus.from_wire(
            {
                "run_id": ack.run_id,
                "status": "failed",
                "submitted_at": "2026-01-05T09:00:00Z",
                "started_at": "2026-01-05T09:00:01Z",
                "finished_at": "2026-01-05T09:00:02Z",
                "progress": {
                    "scenario_id": "baseline",
                    "step_id": "step-0001",
                    "simulation_time": "2026-01-05T16:00:00Z",
                    "completed_steps": 1,
                    "total_steps": 5,
                },
                "failure": {
                    "code": "MODEL_TIMEOUT",
                    "message": "The controlled model endpoint timed out.",
                    "scenario_id": "baseline",
                    "step_id": "step-0002",
                },
            }
        )
        self.assertEqual(run.status, "failed")
        self.assertEqual(run.failure.code, "MODEL_TIMEOUT")

    def test_decodes_successful_shared_result_as_typed_nested_values(self):
        value = json.loads(
            (ROOT / "contracts/examples/results/successful-run.valid.json").read_text()
        )
        result = RunResult.from_wire(value)
        self.assertEqual(result.output_mode, "scores")
        self.assertEqual(result.counts.fills, 1)
        self.assertEqual(result.portfolio.positions[0].quantity.to_wire(), "497.512")
        self.assertEqual(result.evidence[0].kind, "configuration")
        self.assertEqual(result.provenance.model_invocation.mode, "fresh_inference")

    def test_rejects_wrong_fields_wire_values_and_unsafe_evidence_path(self):
        value = json.loads(
            (ROOT / "contracts/examples/results/successful-run.valid.json").read_text()
        )
        cases = []

        unknown_field = copy.deepcopy(value)
        unknown_field["unexpected"] = True
        cases.append(unknown_field)

        numeric_decimal = copy.deepcopy(value)
        numeric_decimal["initial_cash"] = 100000
        cases.append(numeric_decimal)

        traversal = copy.deepcopy(value)
        traversal["evidence"][0]["relative_path"] = "evidence/../outside.json"
        cases.append(traversal)

        missing_evidence = copy.deepcopy(value)
        missing_evidence["evidence"] = []
        cases.append(missing_evidence)

        for invalid in cases:
            with self.subTest(invalid=invalid):
                with self.assertRaises(ProtocolError):
                    RunResult.from_wire(invalid)

    def test_status_requires_failure_payload_only_for_failed_runs(self):
        queued = {
            "run_id": "c827fcbc-c5fc-48a4-b4aa-21a63f64727e",
            "status": "queued",
            "submitted_at": "2026-01-05T09:00:00Z",
            "started_at": None,
            "finished_at": None,
            "progress": None,
            "failure": None,
        }
        RunStatus.from_wire(queued)
        queued["failure"] = {"code": "BROKEN", "message": "unexpected"}
        with self.assertRaises(ProtocolError):
            RunStatus.from_wire(queued)


if __name__ == "__main__":
    unittest.main()
