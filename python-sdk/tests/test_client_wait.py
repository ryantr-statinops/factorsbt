import sys
import unittest
from pathlib import Path
from unittest.mock import call, patch

sys.path.insert(0, str(Path(__file__).parents[1] / "src"))

from factorsbt import (
    Client,
    RequestTimeout,
    RunFailedError,
    RunStatus,
    WaitTimeout,
)


RUN_ID = "c827fcbc-c5fc-48a4-b4aa-21a63f64727e"


def status(state, failure=None):
    return RunStatus.from_wire(
        {
            "run_id": RUN_ID,
            "status": state,
            "submitted_at": "2026-01-05T09:00:00Z",
            "started_at": None if state == "queued" else "2026-01-05T09:00:01Z",
            "finished_at": "2026-01-05T09:00:02Z" if state in {"succeeded", "failed"} else None,
            "progress": None,
            "failure": failure,
        }
    )


class ClientWaitTests(unittest.TestCase):
    def test_wait_returns_successful_terminal_status_without_fetching_result(self):
        client = Client("http://127.0.0.1:8080")
        succeeded = status("succeeded")
        with patch.object(Client, "_get_run", return_value=succeeded) as get_run:
            with patch("factorsbt.client.time.monotonic", side_effect=[1.0, 1.0]):
                result = client.wait(RUN_ID)
        self.assertIs(result, succeeded)
        get_run.assert_called_once_with(RUN_ID, request_timeout=5.0)

    def test_wait_polls_using_monotonic_deadline_and_default_interval(self):
        client = Client("http://127.0.0.1:8080")
        states = [status("queued"), status("running"), status("succeeded")]
        with patch.object(Client, "_get_run", side_effect=states) as get_run:
            with patch("factorsbt.client.time.monotonic", side_effect=[0, 0, 0, 0.25, 0.25, 0.5]):
                with patch("factorsbt.client.time.sleep") as sleep:
                    result = client.wait(RUN_ID, timeout=10)
        self.assertEqual(result.status, "succeeded")
        self.assertEqual(get_run.call_count, 3)
        self.assertEqual(sleep.call_args_list, [call(0.25), call(0.25)])

    def test_wait_raises_typed_run_failure_with_status_context(self):
        client = Client("http://127.0.0.1:8080")
        failed = status(
            "failed",
            {"code": "MODEL_TIMEOUT", "message": "fixture call timed out"},
        )
        with patch.object(Client, "_get_run", return_value=failed):
            with patch("factorsbt.client.time.monotonic", side_effect=[1.0, 1.0]):
                with self.assertRaises(RunFailedError) as caught:
                    client.wait(RUN_ID)
        self.assertEqual(caught.exception.run_id, RUN_ID)
        self.assertIs(caught.exception.status, failed)
        self.assertEqual(caught.exception.failure.code, "MODEL_TIMEOUT")

    def test_wait_timeout_keeps_last_status_and_does_not_cancel(self):
        client = Client("http://127.0.0.1:8080")
        queued = status("queued")
        with patch.object(Client, "_get_run", return_value=queued):
            with patch("factorsbt.client.time.monotonic", side_effect=[0, 0, 0, 0.5]):
                with patch("factorsbt.client.time.sleep") as sleep:
                    with self.assertRaises(WaitTimeout) as caught:
                        client.wait(RUN_ID, timeout=0.5)
        self.assertEqual(caught.exception.run_id, RUN_ID)
        self.assertIs(caught.exception.last_status, queued)
        sleep.assert_called_once_with(0.25)
        self.assertFalse(hasattr(client, "cancel"))

    def test_wait_classifies_request_that_consumes_local_deadline_as_wait_timeout(self):
        client = Client("http://127.0.0.1:8080")
        with patch.object(Client, "_get_run", side_effect=RequestTimeout("network")):
            with patch("factorsbt.client.time.monotonic", side_effect=[0, 0, 1]):
                with self.assertRaises(WaitTimeout):
                    client.wait(RUN_ID, timeout=1)

    def test_wait_rejects_invalid_deadline_or_poll_interval(self):
        client = Client("http://127.0.0.1:8080")
        for options in ({"timeout": 0}, {"timeout": float("inf")}, {"poll_interval": 0}):
            with self.subTest(options=options), self.assertRaises(ValueError):
                client.wait(RUN_ID, **options)


if __name__ == "__main__":
    unittest.main()
