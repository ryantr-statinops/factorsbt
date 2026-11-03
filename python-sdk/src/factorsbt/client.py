"""Synchronous HTTP client for the Phase A health operation."""

from __future__ import annotations

from dataclasses import dataclass
import json
import math
import socket
import time
from typing import Mapping
from urllib.error import HTTPError, URLError
from urllib.request import HTTPRedirectHandler, Request, build_opener, urlopen
from uuid import uuid4
from urllib.parse import urlsplit

from .errors import (
    HttpStatusError,
    ProtocolError,
    RequestTimeout,
    RunFailedError,
    SubmissionAcknowledgementUnknown,
    TransportError,
    WaitTimeout,
)
from .responses import RunResult, RunStatus, SubmissionAccepted, SubmissionStatus
from .types import validate_uuid


class _NoRedirect(HTTPRedirectHandler):
    """Prevent a run POST from being replayed by urllib redirect handling."""

    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


@dataclass(frozen=True)
class HealthResponse:
    status: str
    service: str
    contract_version: str

    @classmethod
    def from_wire(cls, value: object) -> HealthResponse:
        if not isinstance(value, dict) or set(value) != {"status", "service", "contract_version"}:
            raise ProtocolError("health response fields do not match the contract")
        if value["status"] != "ok" or value["service"] != "factorsbt-backend":
            raise ProtocolError("health response has an unsupported status or service")
        version = value["contract_version"]
        if version != "1.0.0":
            raise ProtocolError("health response contract_version is unsupported")
        return cls(status="ok", service="factorsbt-backend", contract_version=version)


@dataclass(frozen=True)
class Client:
    base_url: str
    timeout: float = 5.0

    def __post_init__(self) -> None:
        parsed = urlsplit(self.base_url)
        if parsed.scheme not in {"http", "https"} or not parsed.netloc:
            raise ValueError("base_url must be an absolute HTTP or HTTPS URL")
        if parsed.query or parsed.fragment:
            raise ValueError("base_url cannot contain a query or fragment")
        if self.timeout <= 0:
            raise ValueError("timeout must be greater than zero")

    def health(self) -> HealthResponse:
        """Call GET /healthz once and validate its JSON contract."""

        url = self.base_url.rstrip("/") + "/healthz"
        request = Request(url, headers={"Accept": "application/json"}, method="GET")
        try:
            with urlopen(request, timeout=self.timeout) as response:
                status = response.getcode()
                body = response.read()
        except HTTPError as exc:
            raise HttpStatusError(exc.code) from exc
        except (TimeoutError, socket.timeout) as exc:
            raise RequestTimeout("health request timed out") from exc
        except URLError as exc:
            if isinstance(exc.reason, (TimeoutError, socket.timeout)):
                raise RequestTimeout("health request timed out") from exc
            raise TransportError("could not connect to the FactorsBT backend") from exc
        except OSError as exc:
            raise TransportError("could not connect to the FactorsBT backend") from exc

        if status != 200:
            raise HttpStatusError(status)
        try:
            decoded = json.loads(body)
        except (UnicodeDecodeError, json.JSONDecodeError) as exc:
            raise ProtocolError("health response is not valid JSON") from exc
        return HealthResponse.from_wire(decoded)

    def submit(
        self,
        config: object,
        submission_id: str | None = None,
    ) -> SubmissionAccepted:
        """Submit once; an uncertain acknowledgment carries its lookup ID."""
        key = str(uuid4()) if submission_id is None else validate_uuid(submission_id)
        wire_config = config.to_wire() if hasattr(config, "to_wire") else config
        if not isinstance(wire_config, Mapping):
            raise TypeError("config must be a typed builder or mapping")
        try:
            body = json.dumps(
                wire_config,
                ensure_ascii=False,
                allow_nan=False,
                separators=(",", ":"),
            ).encode("utf-8")
        except (TypeError, ValueError) as exc:
            raise TypeError("config must contain only JSON wire values") from exc

        request = Request(
            self.base_url.rstrip("/") + "/v1/runs",
            data=body,
            headers={
                "Accept": "application/json",
                "Content-Type": "application/json",
                "Idempotency-Key": key,
            },
            method="POST",
        )
        try:
            with build_opener(_NoRedirect()).open(request, timeout=self.timeout) as response:
                status = response.getcode()
                response_body = response.read()
        except HTTPError as exc:
            raise _http_status_error(exc.code, exc.read()) from exc
        except (TimeoutError, socket.timeout) as exc:
            raise SubmissionAcknowledgementUnknown(key, "request timed out") from exc
        except URLError as exc:
            if isinstance(exc.reason, (TimeoutError, socket.timeout)):
                raise SubmissionAcknowledgementUnknown(key, "request timed out") from exc
            raise SubmissionAcknowledgementUnknown(key, "connection ended before acknowledgment") from exc
        except OSError as exc:
            raise SubmissionAcknowledgementUnknown(key, "connection ended before acknowledgment") from exc

        if status != 202:
            raise _http_status_error(status, response_body)
        try:
            accepted = SubmissionAccepted.from_wire(json.loads(response_body))
        except (UnicodeDecodeError, json.JSONDecodeError, ProtocolError) as exc:
            raise SubmissionAcknowledgementUnknown(key, "backend returned an unusable acknowledgment") from exc
        if accepted.submission_id != key:
            raise SubmissionAcknowledgementUnknown(key, "backend acknowledgment used a different submission ID")
        return accepted

    def get_submission(self, submission_id: str) -> SubmissionStatus:
        """Resolve an acknowledged or uncertain submission by its UUID key."""
        key = validate_uuid(submission_id)
        response = self._get_json(f"/v1/submissions/{key}")
        result = SubmissionStatus.from_wire(response)
        if result.submission_id != key:
            raise ProtocolError("submission response identity does not match the requested ID")
        return result

    def get_run(self, run_id: str) -> RunStatus:
        """Read one run's lifecycle and any recorded failure context."""
        return self._get_run(run_id)

    def _get_run(self, run_id: str, *, request_timeout: float | None = None) -> RunStatus:
        key = validate_uuid(run_id)
        response = self._get_json(f"/v1/runs/{key}", timeout=request_timeout)
        result = RunStatus.from_wire(response)
        if result.run_id != key:
            raise ProtocolError("run response identity does not match the requested ID")
        return result

    def wait(
        self,
        run_id: str,
        *,
        timeout: float = 60.0,
        poll_interval: float = 0.25,
    ) -> RunStatus:
        """Poll to a terminal status; the local deadline never cancels backend work."""
        key = validate_uuid(run_id)
        if (
            isinstance(timeout, bool)
            or not isinstance(timeout, (int, float))
            or not math.isfinite(timeout)
            or timeout <= 0
        ):
            raise ValueError("timeout must be a finite number greater than zero")
        if (
            isinstance(poll_interval, bool)
            or not isinstance(poll_interval, (int, float))
            or not math.isfinite(poll_interval)
            or poll_interval <= 0
        ):
            raise ValueError("poll_interval must be a finite number greater than zero")

        deadline = time.monotonic() + timeout
        last_status: RunStatus | None = None
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise WaitTimeout(key, last_status)
            try:
                status = self._get_run(key, request_timeout=min(self.timeout, remaining))
            except RequestTimeout as exc:
                if time.monotonic() >= deadline:
                    raise WaitTimeout(key, last_status) from exc
                raise
            last_status = status
            if status.status == "succeeded":
                return status
            if status.status == "failed":
                raise RunFailedError(status)
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise WaitTimeout(key, last_status)
            time.sleep(min(poll_interval, remaining))

    def get_results(self, run_id: str) -> RunResult:
        """Read a successful result and its validated local evidence references."""
        key = validate_uuid(run_id)
        response = self._get_json(f"/v1/runs/{key}/results")
        result = RunResult.from_wire(response)
        if result.run_id != key:
            raise ProtocolError("result response identity does not match the requested ID")
        return result

    def _get_json(self, path: str, *, timeout: float | None = None) -> object:
        request = Request(
            self.base_url.rstrip("/") + path,
            headers={"Accept": "application/json"},
            method="GET",
        )
        try:
            with build_opener(_NoRedirect()).open(
                request, timeout=self.timeout if timeout is None else timeout
            ) as response:
                status = response.getcode()
                body = response.read()
        except HTTPError as exc:
            raise _http_status_error(exc.code, exc.read()) from exc
        except (TimeoutError, socket.timeout) as exc:
            raise RequestTimeout("backend request timed out") from exc
        except URLError as exc:
            if isinstance(exc.reason, (TimeoutError, socket.timeout)):
                raise RequestTimeout("backend request timed out") from exc
            raise TransportError("could not connect to the FactorsBT backend") from exc
        except OSError as exc:
            raise TransportError("could not connect to the FactorsBT backend") from exc

        if status != 200:
            raise _http_status_error(status, body)
        try:
            return json.loads(body)
        except (UnicodeDecodeError, json.JSONDecodeError) as exc:
            raise ProtocolError("backend response is not valid JSON") from exc


def _http_status_error(status: int, body: bytes) -> HttpStatusError:
    try:
        value = json.loads(body)
        error = value["error"]
        if not isinstance(error, dict):
            raise ValueError("error envelope must contain an object")
        code = error.get("code")
        message = error.get("message")
        request_id = error.get("request_id")
        details_value = error.get("details", [])
        if not isinstance(code, str) or not isinstance(message, str):
            raise ValueError("error envelope fields are invalid")
        if request_id is not None:
            request_id = validate_uuid(request_id)
        if not isinstance(details_value, list):
            raise ValueError("error details must be an array")
        details = []
        for detail in details_value:
            if (
                not isinstance(detail, dict)
                or set(detail) != {"path", "reason"}
                or not isinstance(detail["path"], str)
                or not isinstance(detail["reason"], str)
            ):
                raise ValueError("error detail is invalid")
            details.append((detail["path"], detail["reason"]))
        return HttpStatusError(
            status,
            code=code,
            message=message,
            request_id=request_id,
            details=tuple(details),
        )
    except (UnicodeDecodeError, json.JSONDecodeError, KeyError, TypeError, ValueError):
        return HttpStatusError(status)
