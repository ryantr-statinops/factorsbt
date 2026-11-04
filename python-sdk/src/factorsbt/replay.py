"""Typed request and response values for recorded replay jobs."""

from __future__ import annotations

from dataclasses import dataclass
from .errors import ProtocolError
from .responses import EvidenceReference, _array, _object, _text, _wire
from .types import UtcInstant, validate_uuid


@dataclass(frozen=True)
class ReplayAcknowledgement:
    replay_id: str
    source_run_id: str
    status: str
    submitted_at: UtcInstant

    @classmethod
    def from_wire(cls, value: object) -> ReplayAcknowledgement:
        row = _object(value, {"contract_version", "replay_id", "source_run_id", "status", "submitted_at"})
        if row["contract_version"] != "1.0.0" or row["status"] != "queued":
            raise ProtocolError("replay acknowledgement has an unsupported version or status")
        return cls(_wire(validate_uuid, row["replay_id"]), _wire(validate_uuid, row["source_run_id"]), "queued", _wire(UtcInstant.from_wire, row["submitted_at"]))


@dataclass(frozen=True)
class ReplayStatus:
    replay_id: str
    source_run_id: str
    status: str
    submitted_at: UtcInstant
    started_at: UtcInstant | None
    finished_at: UtcInstant | None
    failure_code: str | None
    failure_message: str | None

    @classmethod
    def from_wire(cls, value: object) -> ReplayStatus:
        row = _object(value, {"contract_version", "replay_id", "source_run_id", "status", "submitted_at", "started_at", "finished_at", "failure"})
        if row["contract_version"] != "1.0.0" or row["status"] not in {"queued", "running", "succeeded", "failed"}:
            raise ProtocolError("replay status has an unsupported version or lifecycle")
        failure = row["failure"]
        if failure is not None:
            failure = _object(failure, {"code", "message"})
        if (row["status"] == "failed") != (failure is not None):
            raise ProtocolError("replay failure payload must be present only for failed jobs")
        started = None if row["started_at"] is None else _wire(UtcInstant.from_wire, row["started_at"])
        finished = None if row["finished_at"] is None else _wire(UtcInstant.from_wire, row["finished_at"])
        return cls(_wire(validate_uuid, row["replay_id"]), _wire(validate_uuid, row["source_run_id"]), row["status"], _wire(UtcInstant.from_wire, row["submitted_at"]), started, finished, None if failure is None else _text(failure["code"], "failure code"), None if failure is None else _text(failure["message"], "failure message"))


@dataclass(frozen=True)
class ReplayReport:
    replay_id: str
    source_run_id: str
    source_outcome: str
    matched: bool
    first_mismatch: str | None
    source_transition_count: int
    replay_transition_count: int
    evidence_refs: tuple[EvidenceReference, ...]

    @classmethod
    def from_wire(cls, value: object) -> ReplayReport:
        row = _object(value, {"contract_version", "replay_id", "source_run_id", "source_outcome", "matched", "first_mismatch", "source_transition_count", "replay_transition_count", "evidence_refs"})
        if row["contract_version"] != "1.0.0" or row["source_outcome"] not in {"succeeded", "failed"} or type(row["matched"]) is not bool:
            raise ProtocolError("replay report contains unsupported values")
        if row["first_mismatch"] is not None and not isinstance(row["first_mismatch"], str):
            raise ProtocolError("first_mismatch must be a string or null")
        counts = (row["source_transition_count"], row["replay_transition_count"])
        if any(type(count) is not int or count < 0 for count in counts):
            raise ProtocolError("transition counts must be nonnegative integers")
        refs = tuple(EvidenceReference.from_wire(item) for item in _array(row["evidence_refs"], "evidence_refs"))
        return cls(_wire(validate_uuid, row["replay_id"]), _wire(validate_uuid, row["source_run_id"]), row["source_outcome"], row["matched"], row["first_mismatch"], *counts, refs)
