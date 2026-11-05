"""Command line output, argument and failure behavior."""

from __future__ import annotations

from contextlib import redirect_stderr, redirect_stdout
from datetime import datetime, timezone
from decimal import Decimal
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from factorsbt import cli
from factorsbt.client import HealthResponse
from factorsbt.errors import (
    HttpStatusError,
    ReplayAcknowledgementUnknown,
    ReplayFailedError,
    RunFailedError,
    SubmissionAcknowledgementUnknown,
    WaitTimeout,
)
from factorsbt.responses import RunFailure, RunStatus
from factorsbt.replay import ReplayStatus
from factorsbt.types import UtcInstant, WireDecimal


def invoke(argv: list[str]) -> tuple[int, str, str]:
    stdout, stderr = io.StringIO(), io.StringIO()
    with redirect_stdout(stdout), redirect_stderr(stderr):
        code = cli.main(argv)
    return code, stdout.getvalue(), stderr.getvalue()


class CliTests(unittest.TestCase):
    def test_health_prints_typed_json(self):
        with patch.object(cli, "Client") as client_type:
            client_type.return_value.health.return_value = HealthResponse("ok", "factorsbt-backend", "1.0.0")
            code, stdout, stderr = invoke(["health"])
        self.assertEqual((code, stderr), (0, ""))
        self.assertEqual(json.loads(stdout), {"status": "ok", "service": "factorsbt-backend", "contract_version": "1.0.0"})

    def test_base_url_precedence_and_loopback_restriction(self):
        with patch.dict(os.environ, {"FACTORSBT_BASE_URL": "http://localhost:8765"}):
            with patch.object(cli, "Client") as client_type:
                client_type.return_value.health.return_value = HealthResponse("ok", "factorsbt-backend", "1.0.0")
                self.assertEqual(invoke(["health"])[0], 0)
                self.assertEqual(client_type.call_args.args, ("http://localhost:8765",))
                invoke(["--base-url", "http://127.0.0.1:7654", "health"])
                self.assertEqual(client_type.call_args.args, ("http://127.0.0.1:7654",))
        code, stdout, stderr = invoke(["--base-url", "https://example.com", "health"])
        self.assertEqual(code, 2)
        self.assertEqual(stdout, "")
        self.assertEqual(json.loads(stderr)["code"], "INVALID_ARGUMENT")

    def test_invalid_arguments_are_json_errors(self):
        for args in (["bogus"], ["run"], ["replay", "status"], ["run", "wait", "x", "--timeout", "nan"]):
            with self.subTest(args=args):
                code, stdout, stderr = invoke(list(args))
                self.assertEqual(code, 2)
                self.assertEqual(stdout, "")
                self.assertEqual(json.loads(stderr)["code"], "INVALID_ARGUMENT")

    def test_help_remains_human_readable(self):
        code, stdout, stderr = invoke(["--help"])
        self.assertEqual((code, stderr), (0, ""))
        self.assertIn("FactorsBT local backtest", stdout)

    def test_submit_reads_object_and_calls_sdk_once(self):
        with tempfile.TemporaryDirectory() as temp:
            config_path = Path(temp) / "run.json"
            config_path.write_text('{"initial_portfolio":{"cash":"1.000000000000000001"}}', encoding="utf-8")
            with patch.object(cli, "Client") as client_type:
                client_type.return_value.submit.return_value = {"submission_id": "s", "run_id": "r", "status": "queued"}
                code, stdout, stderr = invoke(["run", "submit", str(config_path), "--submission-id", "key"])
            self.assertEqual((code, stderr), (0, ""))
            self.assertEqual(client_type.return_value.submit.call_count, 1)
            self.assertEqual(client_type.return_value.submit.call_args.args[0]["initial_portfolio"]["cash"], "1.000000000000000001")
            self.assertEqual(client_type.return_value.submit.call_args.kwargs, {"submission_id": "key"})
            self.assertEqual(json.loads(stdout)["run_id"], "r")

    def test_invalid_json_and_non_object_use_exit_two_without_stdout(self):
        with tempfile.TemporaryDirectory() as temp:
            config_path = Path(temp) / "run.json"
            for content in ("{", "[]"):
                config_path.write_text(content, encoding="utf-8")
                code, stdout, stderr = invoke(["run", "submit", str(config_path)])
                self.assertEqual(code, 2)
                self.assertEqual(stdout, "")
                self.assertEqual(json.loads(stderr)["code"], "INVALID_ARGUMENT")

    def test_acknowledgment_unknown_includes_lookup_id_without_retry(self):
        key = "00000000-0000-4000-8000-000000000001"
        with tempfile.TemporaryDirectory() as temp:
            config_path = Path(temp) / "run.json"
            config_path.write_text("{}", encoding="utf-8")
            with patch.object(cli, "Client") as client_type:
                client_type.return_value.submit.side_effect = SubmissionAcknowledgementUnknown(key, "lost")
                code, stdout, stderr = invoke(["run", "submit", str(config_path), "--submission-id", key])
            self.assertEqual((code, stdout), (5, ""))
            self.assertEqual(client_type.return_value.submit.call_count, 1)
            error = json.loads(stderr)
            self.assertEqual(error["submission_id"], key)
            self.assertIn(f"factorsbt run lookup {key}", error["message"])

    def test_replay_acknowledgment_unknown_has_recovery_command(self):
        replay_id = "00000000-0000-4000-8000-000000000002"
        with patch.object(cli, "Client") as client_type:
            client_type.return_value.submit_replay.side_effect = ReplayAcknowledgementUnknown(replay_id, "lost")
            code, stdout, stderr = invoke(["replay", "submit", "00000000-0000-4000-8000-000000000003", "--replay-id", replay_id])
        self.assertEqual((code, stdout), (5, ""))
        self.assertIn(f"factorsbt replay status {replay_id}", json.loads(stderr)["message"])

    def test_wait_failures_and_timeout_have_distinct_codes(self):
        run_id = "00000000-0000-4000-8000-000000000004"
        failed_status = RunStatus(run_id, "failed", UtcInstant.from_wire("2026-01-01T00:00:00Z"), None, None, None, RunFailure("BROKEN", "failed"))
        with patch.object(cli, "Client") as client_type:
            client_type.return_value.wait.side_effect = RunFailedError(failed_status)
            code, stdout, stderr = invoke(["run", "wait", run_id])
        self.assertEqual((code, stdout), (3, ""))
        self.assertEqual(json.loads(stderr)["run_id"], run_id)
        with patch.object(cli, "Client") as client_type:
            client_type.return_value.wait.side_effect = WaitTimeout(run_id, None)
            code, stdout, stderr = invoke(["run", "wait", run_id])
        self.assertEqual((code, stdout), (4, ""))
        self.assertEqual(json.loads(stderr)["run_id"], run_id)

    def test_replay_wait_timeout_names_replay_id(self):
        replay_id = "00000000-0000-4000-8000-000000000007"
        with patch.object(cli, "Client") as client_type:
            client_type.return_value.wait_replay.side_effect = WaitTimeout(replay_id, None)
            code, stdout, stderr = invoke(["replay", "wait", replay_id])
        self.assertEqual((code, stdout), (4, ""))
        self.assertEqual(json.loads(stderr)["replay_id"], replay_id)

    def test_http_failure_keeps_backend_code_and_request_id(self):
        with patch.object(cli, "Client") as client_type:
            client_type.return_value.health.side_effect = HttpStatusError(
                503, code="BACKEND_UNAVAILABLE", message="busy", request_id="request-1"
            )
            code, stdout, stderr = invoke(["health"])
        self.assertEqual((code, stdout), (1, ""))
        self.assertEqual(json.loads(stderr), {
            "code": "BACKEND_UNAVAILABLE", "message": "busy", "status": 503, "request_id": "request-1"
        })

    def test_keyboard_interrupt_returns_130_and_run_identity(self):
        run_id = "00000000-0000-4000-8000-000000000008"
        with patch.object(cli, "Client") as client_type:
            client_type.return_value.get_run.side_effect = KeyboardInterrupt
            code, stdout, stderr = invoke(["run", "status", run_id])
        self.assertEqual((code, stdout), (130, ""))
        error = json.loads(stderr)
        self.assertEqual(error["code"], "INTERRUPTED")
        self.assertEqual(error["run_id"], run_id)

    def test_replay_failure_wait_has_exit_three_and_readable_mismatch_is_success(self):
        replay_id = "00000000-0000-4000-8000-000000000005"
        replay_status = ReplayStatus(replay_id, "00000000-0000-4000-8000-000000000006", "failed", UtcInstant.from_wire("2026-01-01T00:00:00Z"), None, None, "BROKEN", "failed")
        with patch.object(cli, "Client") as client_type:
            client_type.return_value.wait_replay.side_effect = ReplayFailedError(replay_status)
            code, stdout, stderr = invoke(["replay", "wait", replay_id])
        self.assertEqual((code, stdout), (3, ""))
        with patch.object(cli, "Client") as client_type:
            client_type.return_value.get_replay_report.return_value = {"matched": False, "first_mismatch": "fills[0]"}
            code, stdout, stderr = invoke(["replay", "report", replay_id])
        self.assertEqual((code, stderr), (0, ""))
        self.assertEqual(json.loads(stdout)["matched"], False)

    def test_financial_and_utc_types_serialize_without_float_conversion(self):
        instant = UtcInstant.from_wire("2026-01-01T00:00:00.123456789Z")
        wire = cli._json_value({"amount": WireDecimal(Decimal("0.123456789012345678")), "time": instant})
        self.assertEqual(wire["amount"], "0.123456789012345678")
        self.assertEqual(wire["time"], "2026-01-01T00:00:00.123456789Z")


if __name__ == "__main__":
    unittest.main()
