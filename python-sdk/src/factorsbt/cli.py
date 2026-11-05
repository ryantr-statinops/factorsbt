"""Command line interface for the FactorsBT local backend."""

from __future__ import annotations

import argparse
from dataclasses import fields, is_dataclass
from datetime import datetime
from decimal import Decimal
import json
import os
import sys
from typing import Any
from urllib.parse import urlsplit

from .client import Client, HealthResponse
from .types import UtcInstant, WireDecimal

DEFAULT_BASE_URL = "http://127.0.0.1:8080"


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
    parser = argparse.ArgumentParser(prog="factorsbt", description="FactorsBT local backtest and replay client")
    parser.add_argument("--base-url", help="HTTP loopback backend URL (or FACTORSBT_BASE_URL)")
    commands = parser.add_subparsers(dest="resource", required=True)
    health = commands.add_parser("health", help="check backend health and contract version")
    health.set_defaults(operation="health")
    return parser


def _client(args: argparse.Namespace) -> Client:
    base_url = args.base_url or os.environ.get("FACTORSBT_BASE_URL") or DEFAULT_BASE_URL
    return Client(_loopback_url(base_url), timeout=5.0)


def _dispatch(args: argparse.Namespace) -> object:
    client = _client(args)
    if args.operation == "health":
        return client.health()
    raise RuntimeError("unsupported CLI operation")


def main(argv: list[str] | None = None) -> int:
    parser = _parser()
    try:
        args = parser.parse_args(argv)
        _emit(_dispatch(args))
        return 0
    except KeyboardInterrupt:
        return 130
    except SystemExit as exc:
        return int(exc.code) if isinstance(exc.code, int) else 2
    except Exception as exc:
        _emit({"code": "ERROR", "message": str(exc)}, sys.stderr)
        return 1


if __name__ == "__main__":  # pragma: no cover - exercised by the installed entry point
    raise SystemExit(main())
