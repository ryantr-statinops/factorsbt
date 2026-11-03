import json
import sys
import unittest
from email.message import Message
from io import BytesIO
from pathlib import Path
from urllib.error import HTTPError, URLError
from unittest.mock import patch
from uuid import UUID

sys.path.insert(0, str(Path(__file__).parents[1] / "src"))

from factorsbt import (
    Client,
    HttpStatusError,
    SubmissionAcknowledgementUnknown,
)


SUBMISSION_ID = "8a2f5ce2-a6b5-4a71-b587-fefdb4a9fbc5"
RUN_ID = "c827fcbc-c5fc-48a4-b4aa-21a63f64727e"


class FakeResponse:
    def __init__(self, payload, status=202):
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
    def __init__(self, outcome):
        self.outcome = outcome
        self.calls = []

    def open(self, request, timeout):
        self.calls.append((request, timeout))
        if isinstance(self.outcome, Exception):
            raise self.outcome
        return self.outcome


def accepted_body(submission_id=SUBMISSION_ID):
    return json.dumps(
        {
            "submission_id": submission_id,
            "run_id": RUN_ID,
            "status": "queued",
            "submitted_at": "2026-01-05T09:00:00Z",
        }
    ).encode()


class ClientSubmitTests(unittest.TestCase):
    def test_submit_posts_once_with_idempotency_key_and_decodes_ack(self):
        opener = FakeOpener(FakeResponse(accepted_body()))
        configuration = {"contract_version": "1.0.0", "amount": "100.00"}
        with patch("factorsbt.client.build_opener", return_value=opener) as build:
            result = Client("http://127.0.0.1:8080", timeout=3).submit(
                configuration, submission_id=SUBMISSION_ID
            )

        self.assertEqual(result.submission_id, SUBMISSION_ID)
        self.assertEqual(len(opener.calls), 1)
        build.assert_called_once()
        request, timeout = opener.calls[0]
        self.assertEqual(request.get_method(), "POST")
        self.assertEqual(request.full_url, "http://127.0.0.1:8080/v1/runs")
        self.assertEqual(request.get_header("Idempotency-key"), SUBMISSION_ID)
        self.assertEqual(json.loads(request.data), configuration)
        self.assertEqual(timeout, 3)

    def test_submit_creates_key_before_send_and_unknown_ack_exposes_it(self):
        generated = UUID(SUBMISSION_ID)
        opener = FakeOpener(URLError(ConnectionResetError()))
        with patch("factorsbt.client.uuid4", return_value=generated):
            with patch("factorsbt.client.build_opener", return_value=opener):
                with self.assertRaises(SubmissionAcknowledgementUnknown) as caught:
                    Client("http://127.0.0.1:8080").submit(
                        {"contract_version": "1.0.0"}
                    )

        self.assertEqual(caught.exception.submission_id, SUBMISSION_ID)
        self.assertEqual(len(opener.calls), 1)
        self.assertEqual(
            opener.calls[0][0].get_header("Idempotency-key"), SUBMISSION_ID
        )

    def test_submit_reports_structured_http_errors_without_reclassifying_them(self):
        error = {
            "error": {
                "code": "SUBMISSION_ID_REUSED",
                "message": "Key belongs to another configuration.",
                "request_id": RUN_ID,
                "details": [{"path": "$.decision_policy.output_mode", "reason": "differs"}],
            }
        }
        headers = Message()
        response_error = HTTPError(
            "http://127.0.0.1:8080/v1/runs",
            409,
            "Conflict",
            headers,
            BytesIO(json.dumps(error).encode()),
        )
        opener = FakeOpener(response_error)
        with patch("factorsbt.client.build_opener", return_value=opener):
            with self.assertRaises(HttpStatusError) as caught:
                Client("http://127.0.0.1:8080").submit(
                    {"contract_version": "1.0.0"}, submission_id=SUBMISSION_ID
                )
        self.assertEqual(caught.exception.status, 409)
        self.assertEqual(caught.exception.code, "SUBMISSION_ID_REUSED")
        self.assertEqual(caught.exception.request_id, RUN_ID)
        self.assertEqual(
            caught.exception.details,
            (("$.decision_policy.output_mode", "differs"),),
        )
        self.assertEqual(len(opener.calls), 1)

    def test_submit_treats_unusable_success_ack_as_uncertain_and_never_reposts(self):
        opener = FakeOpener(FakeResponse(accepted_body(RUN_ID)))
        with patch("factorsbt.client.build_opener", return_value=opener):
            with self.assertRaises(SubmissionAcknowledgementUnknown) as caught:
                Client("http://127.0.0.1:8080").submit(
                    {"contract_version": "1.0.0"}, submission_id=SUBMISSION_ID
                )
        self.assertEqual(caught.exception.submission_id, SUBMISSION_ID)
        self.assertEqual(len(opener.calls), 1)


if __name__ == "__main__":
    unittest.main()
