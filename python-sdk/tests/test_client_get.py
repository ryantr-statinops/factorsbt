import json
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parents[1] / "src"))

from factorsbt import Client, HttpStatusError, ProtocolError, RunResult, RunStatus, SubmissionStatus


ROOT = Path(__file__).resolve().parents[2]
RUN_ID = "c827fcbc-c5fc-48a4-b4aa-21a63f64727e"
SUBMISSION_ID = "8a2f5ce2-a6b5-4a71-b587-fefdb4a9fbc5"


class FakeResponse:
    def __init__(self, payload, status=200):
        self.payload = payload
        self.status = status

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        return False

    def getcode(self):
        return self.status

    def read(self):
        return self.payload


class FakeOpener:
    def __init__(self, response):
        self.response = response
        self.calls = []

    def open(self, request, timeout):
        self.calls.append((request, timeout))
        return self.response


class ClientGetTests(unittest.TestCase):
    def test_get_submission_run_and_results_return_typed_responses(self):
        result_payload = json.dumps(
            json.loads((ROOT / "contracts/examples/results/successful-run.valid.json").read_text())
        ).encode()
        responses = (
            {
                "submission_id": SUBMISSION_ID,
                "run_id": RUN_ID,
                "status": "succeeded",
                "submitted_at": "2026-01-05T09:00:00Z",
            },
            {
                "run_id": RUN_ID,
                "status": "succeeded",
                "submitted_at": "2026-01-05T09:00:00Z",
                "started_at": "2026-01-05T09:00:01Z",
                "finished_at": "2026-01-09T16:00:03Z",
                "progress": None,
                "failure": None,
            },
        )
        client = Client("http://127.0.0.1:8080")
        results = []
        for method_name, key, payload, expected_type, path in (
            (
                "get_submission", SUBMISSION_ID, json.dumps(responses[0]).encode(),
                SubmissionStatus, f"/v1/submissions/{SUBMISSION_ID}",
            ),
            (
                "get_run", RUN_ID, json.dumps(responses[1]).encode(),
                RunStatus, f"/v1/runs/{RUN_ID}",
            ),
            (
                "get_results", RUN_ID, result_payload,
                RunResult, f"/v1/runs/{RUN_ID}/results",
            ),
        ):
            opener = FakeOpener(FakeResponse(payload))
            with patch("factorsbt.client.build_opener", return_value=opener):
                result = getattr(client, method_name)(key)
            self.assertIsInstance(result, expected_type)
            self.assertEqual(len(opener.calls), 1)
            request, _timeout = opener.calls[0]
            self.assertEqual(request.get_method(), "GET")
            self.assertEqual(request.full_url, "http://127.0.0.1:8080" + path)
            results.append(result)
        self.assertEqual(results[0].run_id, RUN_ID)
        self.assertEqual(results[2].portfolio.cash.to_wire(), "50000.04")

    def test_get_methods_reject_malformed_ids_and_identity_mismatch(self):
        with patch("factorsbt.client.build_opener") as build:
            with self.assertRaises(ValueError):
                Client("http://127.0.0.1:8080").get_run("../invalid")
        build.assert_not_called()

        opener = FakeOpener(
            FakeResponse(
                json.dumps(
                    {
                        "run_id": SUBMISSION_ID,
                        "status": "queued",
                        "submitted_at": "2026-01-05T09:00:00Z",
                        "started_at": None,
                        "finished_at": None,
                        "progress": None,
                        "failure": None,
                    }
                ).encode()
            )
        )
        with patch("factorsbt.client.build_opener", return_value=opener):
            with self.assertRaises(ProtocolError):
                Client("http://127.0.0.1:8080").get_run(RUN_ID)

    def test_get_surfaces_structured_not_found_response(self):
        from io import BytesIO
        from email.message import Message
        from urllib.error import HTTPError

        body = {
            "error": {
                "code": "RUN_NOT_FOUND",
                "message": "No retained run has this ID.",
                "request_id": SUBMISSION_ID,
            }
        }
        failure = HTTPError(
            "http://127.0.0.1:8080/v1/runs/" + RUN_ID,
            404,
            "Not Found",
            Message(),
            BytesIO(json.dumps(body).encode()),
        )
        opener = FakeOpener(failure)
        with patch("factorsbt.client.build_opener", return_value=opener):
            with self.assertRaises(HttpStatusError) as caught:
                Client("http://127.0.0.1:8080").get_run(RUN_ID)
        self.assertEqual(caught.exception.status, 404)
        self.assertEqual(caught.exception.code, "RUN_NOT_FOUND")
        self.assertEqual(caught.exception.request_id, SUBMISSION_ID)


if __name__ == "__main__":
    unittest.main()
