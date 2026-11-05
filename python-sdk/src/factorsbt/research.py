"""Typed values for bounded research jobs."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from .errors import ProtocolError
from .responses import _object, _text, _wire
from .types import UtcInstant, validate_uuid


@dataclass(frozen=True)
class ResearchJobAcknowledgement:
    job_id: str
    kind: str
    status: str
    submitted_at: UtcInstant

    @classmethod
    def from_wire(cls, value: object) -> ResearchJobAcknowledgement:
        row = _object(value, {"contract_version", "job_id", "kind", "status", "submitted_at"})
        if row["contract_version"] != "1.0.0" or row["status"] != "queued" or row["kind"] not in {"reranking", "episode", "replay"}:
            raise ProtocolError("research acknowledgment has unsupported values")
        return cls(_wire(validate_uuid, row["job_id"]), row["kind"], "queued", _wire(UtcInstant.from_wire, row["submitted_at"]))


@dataclass(frozen=True)
class ResearchJobStatus:
    job_id: str
    kind: str
    status: str
    submitted_at: UtcInstant
    started_at: UtcInstant | None
    finished_at: UtcInstant | None
    failure_code: str | None
    failure_message: str | None

    @classmethod
    def from_wire(cls, value: object) -> ResearchJobStatus:
        row = _object(value, {"contract_version", "job_id", "kind", "status", "submitted_at", "started_at", "finished_at", "failure"})
        if row["contract_version"] != "1.0.0" or row["kind"] not in {"reranking", "episode", "replay"} or row["status"] not in {"queued", "running", "succeeded", "failed"}:
            raise ProtocolError("research status has unsupported values")
        failure = None if row["failure"] is None else _object(row["failure"], {"code", "message"})
        if (row["status"] == "failed") != (failure is not None):
            raise ProtocolError("research failure payload must be present only for failed jobs")
        started = None if row["started_at"] is None else _wire(UtcInstant.from_wire, row["started_at"])
        finished = None if row["finished_at"] is None else _wire(UtcInstant.from_wire, row["finished_at"])
        return cls(_wire(validate_uuid, row["job_id"]), row["kind"], row["status"], _wire(UtcInstant.from_wire, row["submitted_at"]), started, finished, None if failure is None else _text(failure["code"], "failure code"), None if failure is None else _text(failure["message"], "failure message"))


@dataclass(frozen=True)
class ResearchJobResult:
    job_id: str
    kind: str
    report: dict[str, Any]

    @classmethod
    def from_wire(cls, value: object) -> ResearchJobResult:
        row = _object(value, {"contract_version", "job_id", "kind", "report"})
        if row["contract_version"] != "1.0.0" or row["kind"] not in {"reranking", "episode", "replay"} or not isinstance(row["report"], dict):
            raise ProtocolError("research result has unsupported values")
        return cls(_wire(validate_uuid, row["job_id"]), row["kind"], row["report"])
