import json
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parents[1] / "src"))

from factorsbt import Client, ProtocolError, ReplayAcknowledgement, ReplayReport, ReplayStatus

REPLAY_ID = "8a2f5ce2-a6b5-4a71-b587-fefdb4a9fbc5"
SOURCE_ID = "c827fcbc-c5fc-48a4-b4aa-21a63f64727e"


class Response:
    def __init__(self, body): self.body = body
    def __enter__(self): return self
    def __exit__(self, *_args): return False
    def getcode(self): return 202
    def read(self): return self.body


class ReplaySdkTests(unittest.TestCase):
    def test_submit_replay_posts_once_with_idempotency_key(self):
        payload = {"contract_version": "1.0.0", "replay_id": REPLAY_ID, "source_run_id": SOURCE_ID,
                   "status": "queued", "submitted_at": "2026-01-05T09:00:00Z"}
        opener = type("Opener", (), {"calls": [], "open": lambda self, request, timeout: (self.calls.append((request, timeout)) or Response(json.dumps(payload).encode()))})()
        with patch("factorsbt.client.build_opener", return_value=opener):
            result = Client("http://127.0.0.1:8080").submit_replay(SOURCE_ID, REPLAY_ID)
        self.assertIsInstance(result, ReplayAcknowledgement)
        self.assertEqual(len(opener.calls), 1)
        request, _ = opener.calls[0]
        self.assertEqual(request.get_method(), "POST")
        self.assertEqual(request.get_header("Idempotency-key"), REPLAY_ID)

    def test_replay_decoders_enforce_terminal_failure_and_report_shape(self):
        queued = {"contract_version": "1.0.0", "replay_id": REPLAY_ID, "source_run_id": SOURCE_ID,
                  "status": "queued", "submitted_at": "2026-01-05T09:00:00Z",
                  "started_at": None, "finished_at": None, "failure": None}
        self.assertEqual(ReplayStatus.from_wire(queued).status, "queued")
        queued["failure"] = {"code": "FAILED", "message": "error"}
        with self.assertRaises(ProtocolError): ReplayStatus.from_wire(queued)
        report = {"contract_version": "1.0.0", "replay_id": REPLAY_ID, "source_run_id": SOURCE_ID,
                  "source_outcome": "succeeded", "matched": True, "first_mismatch": None,
                  "source_transition_count": 3, "replay_transition_count": 3, "evidence_refs": []}
        self.assertIsInstance(ReplayReport.from_wire(report), ReplayReport)


if __name__ == "__main__":
    unittest.main()
