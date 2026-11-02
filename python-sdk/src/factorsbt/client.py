"""Synchronous HTTP client for the Phase A health operation."""

from __future__ import annotations

from dataclasses import dataclass
import json
import socket
from urllib.error import HTTPError, URLError
from urllib.parse import urlsplit
from urllib.request import Request, urlopen

from .errors import HttpStatusError, ProtocolError, RequestTimeout, TransportError


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
