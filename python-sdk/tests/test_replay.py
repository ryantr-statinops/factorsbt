import json
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parents[1] / "src"))

from factorsbt import (
    Client, ProtocolError, ReplayAcknowledgement, ReplayFailedError, ReplayReport,
    ReplayStatus, RequestTimeout, WaitTimeout,
)

REPLAY_ID = "8a2f5ce2-a6b5-4a71-b587-fefdb4a9fbc5"
SOURCE_ID = "c827fcbc-c5fc-48a4-b4aa-21a63f64727e"


class Response:
    def __init__(self, body): self.body = body
    def __enter__(self): return self
    def __exit__(self, *_args): return False
    def getcode(self): return 202
    def read(self): return self.body


class ReplaySdkTests(unittest.TestCase):
    def _status(self, state):
        return ReplayStatus.from_wire({
            "contract_version": "1.0.0", "replay_id": REPLAY_ID, "source_run_id": SOURCE_ID,
            "status": state, "submitted_at": "2026-01-05T09:00:00Z",
            "started_at": None if state == "queued" else "2026-01-05T09:00:01Z",
            "finished_at": "2026-01-05T09:00:02Z" if state in {"succeeded", "failed"} else None,
            "failure": {"code": "REPLAY_DIVERGED", "message": "diverged"} if state == "failed" else None,
        })

    def test_wait_replay_bounds_each_request_and_rejects_late_terminal_status(self):
        client = Client("http://127.0.0.1:8080")
        for state in ("succeeded", "failed"):
            with self.subTest(state=state):
                with patch.object(Client, "_get_replay", return_value=self._status(state)) as get_replay:
                    with patch("factorsbt.client.time.monotonic", side_effect=[0, 0, 0.02]):
                        with self.assertRaises(WaitTimeout):
                            client.wait_replay(REPLAY_ID, timeout=0.01)
                get_replay.assert_called_once_with(REPLAY_ID, request_timeout=0.01)

    def test_wait_replay_distinguishes_http_timeout_from_expired_wait(self):
        client = Client("http://127.0.0.1:8080")
        with patch.object(Client, "_get_replay", side_effect=RequestTimeout("network")):
            with patch("factorsbt.client.time.monotonic", side_effect=[0, 0, 0.005]):
                with self.assertRaises(RequestTimeout) as caught:
                    client.wait_replay(REPLAY_ID, timeout=0.01)
        self.assertNotIsInstance(caught.exception, WaitTimeout)
        with patch.object(Client, "_get_replay", side_effect=RequestTimeout("network")):
            with patch("factorsbt.client.time.monotonic", side_effect=[0, 0, 0.02]):
                with self.assertRaises(WaitTimeout):
                    client.wait_replay(REPLAY_ID, timeout=0.01)

    def test_wait_replay_keeps_terminal_failure_and_validates_polling_parameters(self):
        client = Client("http://127.0.0.1:8080")
        with patch.object(Client, "_get_replay", return_value=self._status("succeeded")):
            with patch("factorsbt.client.time.monotonic", side_effect=[0, 0, 0.005]):
                self.assertEqual(client.wait_replay(REPLAY_ID, timeout=0.01).status, "succeeded")
        with patch.object(Client, "_get_replay", return_value=self._status("failed")):
            with patch("factorsbt.client.time.monotonic", side_effect=[0, 0, 0.005]):
                with self.assertRaises(ReplayFailedError):
                    client.wait_replay(REPLAY_ID, timeout=0.01)
        for options in ({"timeout": float("nan")}, {"timeout": True}, {"poll_interval": float("inf")}, {"poll_interval": 0}):
            with self.subTest(options=options), self.assertRaises(ValueError):
                client.wait_replay(REPLAY_ID, **options)

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
