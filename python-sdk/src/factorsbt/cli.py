"""Command line interface for the FactorsBT local backend."""

from __future__ import annotations

import argparse
from dataclasses import fields, is_dataclass
from datetime import datetime
from decimal import Decimal
import json
import os
from pathlib import Path
import sys
from typing import Any
from urllib.parse import urlsplit

from .client import Client, HealthResponse
from .errors import (
    FactorsBTError,
    HttpStatusError,
    ReplayAcknowledgementUnknown,
    ReplayFailedError,
    RunFailedError,
    SubmissionAcknowledgementUnknown,
    WaitTimeout,
)
from .types import UtcInstant, WireDecimal

DEFAULT_BASE_URL = "http://127.0.0.1:8080"


class _CliArgumentError(Exception):
    pass


class _ArgumentParser(argparse.ArgumentParser):
    def error(self, message: str) -> None:
        raise _CliArgumentError(message)


def _positive_finite(text: str) -> float:
    import math

    try:
        value = float(text)
    except ValueError as exc:
        raise argparse.ArgumentTypeError("must be a positive finite number") from exc
    if not math.isfinite(value) or value <= 0:
        raise argparse.ArgumentTypeError("must be a positive finite number")
    return value


def _loopback_url(value: str) -> str:
    try:
        parsed = urlsplit(value)
        hostname = parsed.hostname
        port = parsed.port
    except ValueError as exc:
        raise argparse.ArgumentTypeError("base URL is invalid") from exc
    if (
        parsed.scheme != "http"
        or hostname not in {"localhost", "127.0.0.1", "::1"}
        or parsed.username is not None
        or parsed.password is not None
        or parsed.path not in {"", "/"}
        or parsed.query
        or parsed.fragment
        or (port is not None and not 1 <= port <= 65535)
    ):
        raise argparse.ArgumentTypeError(
            "base URL must be HTTP loopback (localhost, 127.0.0.1, or [::1]) with an optional port"
        )
    return value.rstrip("/")


def _json_value(value: Any) -> Any:
    if isinstance(value, WireDecimal):
        return value.to_wire()
    if isinstance(value, UtcInstant):
        return value.to_wire()
    if isinstance(value, Decimal):
        return format(value, "f")
    if isinstance(value, datetime):
        if value.utcoffset() is None:
            raise TypeError("cannot serialize a timestamp without a timezone")
        return value.isoformat(timespec="auto").replace("+00:00", "Z")
    if is_dataclass(value) and not isinstance(value, type):
        return {field.name: _json_value(getattr(value, field.name)) for field in fields(value)}
    if isinstance(value, (tuple, list)):
        return [_json_value(item) for item in value]
    if isinstance(value, dict):
        return {str(key): _json_value(item) for key, item in value.items()}
    return value


def _emit(value: object, stream=None) -> None:
    target = sys.stdout if stream is None else stream
    json.dump(_json_value(value), target, ensure_ascii=False, allow_nan=False, separators=(",", ":"))
    target.write("\n")


def _parser() -> argparse.ArgumentParser:
    parser = _ArgumentParser(prog="factorsbt", description="FactorsBT local backtest and replay client")
    parser.add_argument("--base-url", help="HTTP loopback backend URL (or FACTORSBT_BASE_URL)")
    commands = parser.add_subparsers(dest="resource", required=True)
    health = commands.add_parser("health", help="check backend health and contract version")
    health.set_defaults(operation="health")
    run = commands.add_parser("run", help="submit and inspect backtest runs")
    run_commands = run.add_subparsers(dest="run_operation", required=True, parser_class=_ArgumentParser)
    submit = run_commands.add_parser("submit", help="submit a Run Configuration JSON file")
    submit.add_argument("config", type=Path)
    submit.add_argument("--submission-id")
    submit.set_defaults(operation="run_submit")
    lookup = run_commands.add_parser("lookup", help="look up a submission by id")
    lookup.add_argument("submission_id")
    lookup.set_defaults(operation="run_lookup")
    status = run_commands.add_parser("status", help="read run lifecycle status")
    status.add_argument("run_id")
    status.set_defaults(operation="run_status")
    wait = run_commands.add_parser("wait", help="wait for a run to finish")
    wait.add_argument("run_id")
    wait.add_argument("--timeout", type=_positive_finite, default=60.0)
    wait.set_defaults(operation="run_wait")
    result = run_commands.add_parser("result", help="read a successful run result")
    result.add_argument("run_id")
    result.set_defaults(operation="run_result")
    replay = commands.add_parser("replay", help="verify a recorded run without fresh inference")
    replay_commands = replay.add_subparsers(dest="replay_operation", required=True, parser_class=_ArgumentParser)
    replay_submit = replay_commands.add_parser("submit", help="submit a recorded replay")
    replay_submit.add_argument("source_run_id")
    replay_submit.add_argument("--replay-id")
    replay_submit.set_defaults(operation="replay_submit")
    replay_status = replay_commands.add_parser("status", help="read replay lifecycle status")
    replay_status.add_argument("replay_id")
    replay_status.set_defaults(operation="replay_status")
    replay_wait = replay_commands.add_parser("wait", help="wait for a replay to finish")
    replay_wait.add_argument("replay_id")
    replay_wait.add_argument("--timeout", type=_positive_finite, default=60.0)
    replay_wait.set_defaults(operation="replay_wait")
    replay_report = replay_commands.add_parser("report", help="read a terminal replay report")
    replay_report.add_argument("replay_id")
    replay_report.set_defaults(operation="replay_report")
    return parser


def _client(args: argparse.Namespace) -> Client:
    base_url = args.base_url or os.environ.get("FACTORSBT_BASE_URL") or DEFAULT_BASE_URL
    return Client(_loopback_url(base_url), timeout=5.0)


def _dispatch(args: argparse.Namespace) -> object:
    client = _client(args)
    if args.operation == "health":
        return client.health()
    if args.operation == "run_submit":
        try:
            config = json.loads(args.config.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, json.JSONDecodeError) as exc:
            raise ValueError(f"cannot read run configuration JSON: {exc}") from exc
        if not isinstance(config, dict):
            raise ValueError("run configuration JSON must contain an object")
        return client.submit(config, submission_id=args.submission_id)
    if args.operation == "run_lookup":
        return client.get_submission(args.submission_id)
    if args.operation == "run_status":
        return client.get_run(args.run_id)
    if args.operation == "run_wait":
        return client.wait(args.run_id, timeout=args.timeout, poll_interval=0.25)
    if args.operation == "run_result":
        return client.get_results(args.run_id)
    if args.operation == "replay_submit":
        return client.submit_replay(args.source_run_id, replay_id=args.replay_id)
    if args.operation == "replay_status":
        return client.get_replay(args.replay_id)
    if args.operation == "replay_wait":
        return client.wait_replay(args.replay_id, timeout=args.timeout, poll_interval=0.25)
    if args.operation == "replay_report":
        return client.get_replay_report(args.replay_id)
    raise RuntimeError("unsupported CLI operation")


def main(argv: list[str] | None = None) -> int:
    parser = _parser()
    try:
        args = parser.parse_args(argv)
        _emit(_dispatch(args))
        return 0
    except _CliArgumentError as exc:
        _emit({"code": "INVALID_ARGUMENT", "message": str(exc)}, sys.stderr)
        return 2
    except KeyboardInterrupt:
        error = {"code": "INTERRUPTED", "message": "command interrupted; backend work was not cancelled"}
        if "args" in locals():
            if getattr(args, "operation", "").startswith("run_"):
                for name in ("run_id", "submission_id"):
                    value = getattr(args, name, None)
                    if value is not None:
                        error[name] = value
            elif getattr(args, "operation", "").startswith("replay_"):
                value = getattr(args, "replay_id", None)
                if value is not None:
                    error["replay_id"] = value
        _emit(error, sys.stderr)
        return 130
    except SystemExit as exc:
        return int(exc.code) if isinstance(exc.code, int) else 2
    except SubmissionAcknowledgementUnknown as exc:
        _emit({
            "code": "ACKNOWLEDGEMENT_UNKNOWN",
            "message": f"run acknowledgment is unknown; look up with: factorsbt run lookup {exc.submission_id}",
            "submission_id": exc.submission_id,
        }, sys.stderr)
        return 5
    except ReplayAcknowledgementUnknown as exc:
        _emit({
            "code": "ACKNOWLEDGEMENT_UNKNOWN",
            "message": f"replay acknowledgment is unknown; inspect with: factorsbt replay status {exc.replay_id}",
            "replay_id": exc.replay_id,
        }, sys.stderr)
        return 5
    except (RunFailedError, ReplayFailedError) as exc:
        key_name = "run_id" if isinstance(exc, RunFailedError) else "replay_id"
        _emit({"code": "JOB_FAILED", "message": str(exc), key_name: getattr(exc, key_name)}, sys.stderr)
        return 3
    except WaitTimeout as exc:
        if "args" in locals() and getattr(args, "operation", "") == "replay_wait":
            key = {"replay_id": exc.run_id}
        else:
            key = {"run_id": exc.run_id}
        _emit({"code": "WAIT_TIMEOUT", "message": str(exc), **key}, sys.stderr)
        return 4
    except (argparse.ArgumentTypeError, ValueError, TypeError) as exc:
        _emit({"code": "INVALID_ARGUMENT", "message": str(exc)}, sys.stderr)
        return 2
    except HttpStatusError as exc:
        error = {"code": exc.code or "HTTP_ERROR", "message": exc.message or str(exc), "status": exc.status}
        if exc.request_id is not None:
            error["request_id"] = exc.request_id
        _emit(error, sys.stderr)
        return 1
    except FactorsBTError as exc:
        _emit({"code": "FACTORSBT_ERROR", "message": str(exc)}, sys.stderr)
        return 1
    except Exception as exc:
        _emit({"code": "ERROR", "message": str(exc)}, sys.stderr)
        return 1


if __name__ == "__main__":  # pragma: no cover - exercised by the installed entry point
    raise SystemExit(main())
