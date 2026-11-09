import json
import sys
import unittest
from decimal import Decimal
from pathlib import Path
from urllib.error import URLError
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parents[1] / "src"))

from factorsbt import (
    Client, EpisodeReport, ProtocolError, ResearchJobAcknowledgementUnknown,
    ResearchJobFailed, ResearchJobResult, ResearchJobStatus, ResearchReplayReport,
    ResearchWaitTimeout, RequestTimeout, RerankingReport,
)

JOB_ID = "8a2f5ce2-a6b5-4a71-b587-fefdb4a9fbc5"


class Response:
    def __init__(self, payload, status=202): self.payload, self.status = payload, status
    def __enter__(self): return self
    def __exit__(self, *_args): return False
    def getcode(self): return self.status
    def read(self): return self.payload


class Opener:
    def __init__(self, outcome): self.outcome, self.calls = outcome, []
    def open(self, request, timeout):
        self.calls.append((request, timeout))
        if isinstance(self.outcome, Exception): raise self.outcome
        return self.outcome


def ack(kind="episode"):
    return json.dumps({"contract_version":"1.0.0", "job_id":JOB_ID, "kind":kind, "status":"queued", "submitted_at":"2026-01-05T09:00:00Z"}).encode()


class ResearchClientTests(unittest.TestCase):
    def _status(self, state):
        return ResearchJobStatus.from_wire({
            "contract_version": "1.0.0", "job_id": JOB_ID, "kind": "episode",
            "status": state, "submitted_at": "2026-01-05T09:00:00Z",
            "started_at": None if state == "queued" else "2026-01-05T09:00:01Z",
            "finished_at": "2026-01-05T09:00:02Z" if state in {"succeeded", "failed"} else None,
            "failure": {"code": "RESEARCH_EXECUTION_FAILED", "message": "failed"} if state == "failed" else None,
        })

    def test_wait_research_bounds_requests_and_keeps_last_observed_status(self):
        client = Client("http://127.0.0.1:8080")
        for state in ("succeeded", "failed"):
            with self.subTest(state=state):
                terminal = self._status(state)
                with patch.object(Client, "_get_research_job", return_value=terminal) as get_job:
                    with patch("factorsbt.client.time.monotonic", side_effect=[0, 0, 0.02]):
                        with self.assertRaises(ResearchWaitTimeout) as caught:
                            client.wait_research_job(JOB_ID, timeout=0.01)
                self.assertIs(caught.exception.last_status, terminal)
                get_job.assert_called_once_with(JOB_ID, request_timeout=0.01)

    def test_wait_research_distinguishes_http_timeout_and_terminal_failure(self):
        client = Client("http://127.0.0.1:8080")
        with patch.object(Client, "_get_research_job", side_effect=RequestTimeout("network")):
            with patch("factorsbt.client.time.monotonic", side_effect=[0, 0, 0.005]):
                with self.assertRaises(RequestTimeout) as caught:
                    client.wait_research_job(JOB_ID, timeout=0.01)
        self.assertNotIsInstance(caught.exception, ResearchWaitTimeout)
        with patch.object(Client, "_get_research_job", side_effect=RequestTimeout("network")):
            with patch("factorsbt.client.time.monotonic", side_effect=[0, 0, 0.02]):
                with self.assertRaises(ResearchWaitTimeout):
                    client.wait_research_job(JOB_ID, timeout=0.01)
        with patch.object(Client, "_get_research_job", return_value=self._status("failed")):
            with patch("factorsbt.client.time.monotonic", side_effect=[0, 0, 0.005]):
                with self.assertRaises(ResearchJobFailed):
                    client.wait_research_job(JOB_ID, timeout=0.01)

    def test_wait_research_rejects_nonfinite_and_boolean_parameters(self):
        client = Client("http://127.0.0.1:8080")
        for options in ({"timeout": float("nan")}, {"timeout": True}, {"poll_interval": float("inf")}, {"poll_interval": 0}):
            with self.subTest(options=options), self.assertRaises(ValueError):
                client.wait_research_job(JOB_ID, **options)

    def test_episode_submission_posts_typed_actions_once(self):
        opener = Opener(Response(ack()))
        with patch("factorsbt.client.build_opener", return_value=opener):
            accepted = Client("http://127.0.0.1:8080").submit_episode("episode-eval", "1.0.0", [Decimal("0.5"), Decimal("1")], JOB_ID)
        self.assertEqual(accepted.job_id, JOB_ID)
        self.assertEqual(len(opener.calls), 1)
        request, _ = opener.calls[0]
        self.assertEqual(request.full_url, "http://127.0.0.1:8080/v1/research/jobs")
        self.assertEqual(request.get_header("Idempotency-key"), JOB_ID)
        self.assertEqual(json.loads(request.data)["actions"], ["0.5", "1"])

    def test_unknown_acknowledgment_keeps_lookup_id_and_does_not_retry(self):
        opener = Opener(URLError(ConnectionResetError()))
        with patch("factorsbt.client.build_opener", return_value=opener):
            with self.assertRaises(ResearchJobAcknowledgementUnknown) as caught:
                Client("http://127.0.0.1:8080").submit_reranking("reranking-eval", "1.0.0", JOB_ID)
        self.assertEqual(caught.exception.job_id, JOB_ID)
        self.assertEqual(len(opener.calls), 1)

    def test_status_decoder_validates_failure_lifecycle(self):
        value = {"contract_version":"1.0.0", "job_id":JOB_ID, "kind":"episode", "status":"queued", "submitted_at":"2026-01-05T09:00:00Z", "started_at":None, "finished_at":None, "failure":None}
        self.assertIsInstance(ResearchJobStatus.from_wire(value), ResearchJobStatus)
        value["status"] = "failed"
        with self.assertRaises(ProtocolError): ResearchJobStatus.from_wire(value)

    def test_episode_actions_rejected_before_transport(self):
        with self.assertRaises(ValueError):
            Client("http://127.0.0.1:8080").submit_episode("episode-eval", "1.0.0", [Decimal("0.25"), Decimal("0.5")], JOB_ID)
        with self.assertRaises(TypeError):
            Client("http://127.0.0.1:8080").submit_episode("episode-eval", "1.0.0", [0.5, 1.0], JOB_ID)

    def test_research_results_decode_to_job_specific_typed_reports(self):
        episode = {
            "contract_version": "1.0.0", "job_id": JOB_ID, "kind": "episode",
            "report": {
                "fixture_id": "episode-eval", "fixture_version": "1.0.0",
                "transitions": [{
                    "step": 0, "observation_start": {"nav": "100000"},
                    "action": {"target_weight": "0.5"}, "admitted_quantity": "500",
                    "fills": [{"quantity": "500", "price": "102", "fee": "1.02"}],
                    "observation_end": {"nav": "99998.98"}, "reward_usd": "-1.02",
                }],
                "final_observation": {"nav": "99998.98"},
                "canceled_pending_orders": [], "failure": None,
            },
        }
        decoded = ResearchJobResult.from_wire(episode)
        self.assertIsInstance(decoded.report, EpisodeReport)
        self.assertEqual(decoded.report.transitions[0].reward_usd, Decimal("-1.02"))
        self.assertEqual(decoded.report.transitions[0].target_weight, Decimal("0.5"))
        episode["report"]["transitions"][0]["reward_usd"] = 2
        with self.assertRaises(ProtocolError):
            ResearchJobResult.from_wire(episode)

        branch = {"final_nav": "100000", "pnl_usd": "0", "total_filled_value": "0", "selections": [], "fills": []}
        reranking = {
            "contract_version": "1.0.0", "job_id": JOB_ID, "kind": "reranking",
            "report": {
                "fixture_id": "reranking-eval", "fixture_version": "1.0.0",
                "baseline": {"Ok": branch}, "reranker": {"Err": "controlled branch error"},
                "paired_decisions": [], "outcome_vs_baseline": None, "pnl_delta_usd": None,
            },
        }
        self.assertIsInstance(ResearchJobResult.from_wire(reranking).report, RerankingReport)

        replay = {
            "contract_version": "1.0.0", "job_id": JOB_ID, "kind": "replay",
            "report": {
                "source_job_id": JOB_ID, "source_kind": "episode", "source_outcome": "failed",
                "matched": True, "first_mismatch": None,
                "source_report": {"failure": "controlled"}, "replayed_report": {"failure": "controlled"},
            },
        }
        self.assertIsInstance(ResearchJobResult.from_wire(replay).report, ResearchReplayReport)


if __name__ == "__main__": unittest.main()
