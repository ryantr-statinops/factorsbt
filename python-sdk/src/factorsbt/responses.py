"""Typed decoders for the MVP run API responses."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from .errors import ProtocolError
from .types import (
    UtcInstant,
    WireDecimal,
    validate_identifier,
    validate_instrument_id,
    validate_uuid,
)


def _object(value: object, required: set[str], optional: set[str] = frozenset()) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ProtocolError("response value must be an object")
    keys = set(value)
    if not required <= keys or keys - required - optional:
        raise ProtocolError("response object fields do not match the contract")
    return value


def _array(value: object, name: str) -> list[Any]:
    if not isinstance(value, list):
        raise ProtocolError(f"{name} must be an array")
    return value


def _wire(parser, value: object):
    try:
        return parser(value)
    except (ValueError, TypeError) as exc:
        raise ProtocolError("response contains a value outside the wire contract") from exc


def _uint(value: object, name: str) -> int:
    if type(value) is not int or value < 0:
        raise ProtocolError(f"{name} must be a nonnegative integer")
    return value


def _text(value: object, name: str) -> str:
    if not isinstance(value, str) or not value:
        raise ProtocolError(f"{name} must be a nonempty string")
    return value


@dataclass(frozen=True)
class SubmissionAccepted:
    submission_id: str
    run_id: str
    status: str
    submitted_at: UtcInstant

    @classmethod
    def from_wire(cls, value: object) -> SubmissionAccepted:
        row = _object(value, {"submission_id", "run_id", "status", "submitted_at"})
        if row["status"] != "queued":
            raise ProtocolError("submission acknowledgment status must be queued")
        return cls(
            _wire(validate_uuid, row["submission_id"]),
            _wire(validate_uuid, row["run_id"]),
            "queued",
            _wire(UtcInstant.from_wire, row["submitted_at"]),
        )


@dataclass(frozen=True)
class SubmissionStatus:
    submission_id: str
    run_id: str
    status: str
    submitted_at: UtcInstant

    @classmethod
    def from_wire(cls, value: object) -> SubmissionStatus:
        row = _object(value, {"submission_id", "run_id", "status", "submitted_at"})
        if row["status"] not in {"queued", "running", "succeeded", "failed"}:
            raise ProtocolError("submission status is not a supported lifecycle state")
        return cls(
            _wire(validate_uuid, row["submission_id"]),
            _wire(validate_uuid, row["run_id"]),
            row["status"],
            _wire(UtcInstant.from_wire, row["submitted_at"]),
        )


@dataclass(frozen=True)
class RunProgress:
    scenario_id: str
    step_id: str
    simulation_time: UtcInstant
    completed_steps: int
    total_steps: int

    @classmethod
    def from_wire(cls, value: object) -> RunProgress:
        row = _object(value, {"scenario_id", "step_id", "simulation_time", "completed_steps", "total_steps"})
        completed = _uint(row["completed_steps"], "completed_steps")
        total = _uint(row["total_steps"], "total_steps")
        if completed > total:
            raise ProtocolError("completed_steps cannot exceed total_steps")
        return cls(
            _wire(validate_identifier, row["scenario_id"]),
            _wire(validate_identifier, row["step_id"]),
            _wire(UtcInstant.from_wire, row["simulation_time"]),
            completed,
            total,
        )


@dataclass(frozen=True)
class RunFailure:
    code: str
    message: str
    scenario_id: str | None = None
    step_id: str | None = None
    last_valid_state_ref: str | None = None

    @classmethod
    def from_wire(cls, value: object) -> RunFailure:
        row = _object(
            value,
            {"code", "message"},
            {"scenario_id", "step_id", "last_valid_state_ref"},
        )
        for key in ("scenario_id", "step_id", "last_valid_state_ref"):
            if key in row and row[key] is not None and not isinstance(row[key], str):
                raise ProtocolError(f"{key} must be a string or null")
        scenario = row.get("scenario_id")
        step = row.get("step_id")
        if scenario is not None:
            scenario = _wire(validate_identifier, scenario)
        if step is not None:
            step = _wire(validate_identifier, step)
        return cls(
            _text(row["code"], "failure code"),
            _text(row["message"], "failure message"),
            scenario,
            step,
            row.get("last_valid_state_ref"),
        )


@dataclass(frozen=True)
class RunStatus:
    run_id: str
    status: str
    submitted_at: UtcInstant
    started_at: UtcInstant | None
    finished_at: UtcInstant | None
    progress: RunProgress | None
    failure: RunFailure | None

    @classmethod
    def from_wire(cls, value: object) -> RunStatus:
        row = _object(
            value,
            {"run_id", "status", "submitted_at", "started_at", "finished_at", "progress", "failure"},
        )
        lifecycle = row["status"]
        if lifecycle not in {"queued", "running", "succeeded", "failed"}:
            raise ProtocolError("run status is not a supported lifecycle state")
        started = None if row["started_at"] is None else _wire(UtcInstant.from_wire, row["started_at"])
        finished = None if row["finished_at"] is None else _wire(UtcInstant.from_wire, row["finished_at"])
        progress = None if row["progress"] is None else RunProgress.from_wire(row["progress"])
        failure = None if row["failure"] is None else RunFailure.from_wire(row["failure"])
        if (lifecycle == "failed") != (failure is not None):
            raise ProtocolError("run failure payload must be present only for a failed run")
        return cls(
            _wire(validate_uuid, row["run_id"]),
            lifecycle,
            _wire(UtcInstant.from_wire, row["submitted_at"]),
            started,
            finished,
            progress,
            failure,
        )


@dataclass(frozen=True)
class PositionSnapshot:
    instrument_id: str
    quantity: WireDecimal
    mark: WireDecimal
    market_value: WireDecimal

    @classmethod
    def from_wire(cls, value: object) -> PositionSnapshot:
        row = _object(value, {"instrument_id", "quantity", "mark", "market_value"})
        return cls(
            _wire(validate_instrument_id, row["instrument_id"]),
            _wire(WireDecimal.from_wire, row["quantity"]),
            _wire(WireDecimal.from_wire, row["mark"]),
            _wire(WireDecimal.from_wire, row["market_value"]),
        )


@dataclass(frozen=True)
class UnderlyingReservation:
    instrument_id: str
    quantity: WireDecimal

    @classmethod
    def from_wire(cls, value: object) -> UnderlyingReservation:
        row = _object(value, {"instrument_id", "quantity"})
        return cls(
            _wire(validate_instrument_id, row["instrument_id"]),
            _wire(WireDecimal.from_wire, row["quantity"]),
        )


@dataclass(frozen=True)
class PortfolioSnapshot:
    cash: WireDecimal
    nav: WireDecimal
    positions: tuple[PositionSnapshot, ...]
    reserved_cash: WireDecimal
    reserved_underlying: tuple[UnderlyingReservation, ...]
    initial_margin_requirement: WireDecimal
    maintenance_margin_requirement: WireDecimal
    free_buying_power: WireDecimal

    @classmethod
    def from_wire(cls, value: object) -> PortfolioSnapshot:
        row = _object(
            value,
            {
                "cash", "nav", "positions", "reserved_cash", "reserved_underlying",
                "initial_margin_requirement", "maintenance_margin_requirement", "free_buying_power",
            },
        )
        return cls(
            _wire(WireDecimal.from_wire, row["cash"]),
            _wire(WireDecimal.from_wire, row["nav"]),
            tuple(PositionSnapshot.from_wire(item) for item in _array(row["positions"], "positions")),
            _wire(WireDecimal.from_wire, row["reserved_cash"]),
            tuple(
                UnderlyingReservation.from_wire(item)
                for item in _array(row["reserved_underlying"], "reserved_underlying")
            ),
            _wire(WireDecimal.from_wire, row["initial_margin_requirement"]),
            _wire(WireDecimal.from_wire, row["maintenance_margin_requirement"]),
            _wire(WireDecimal.from_wire, row["free_buying_power"]),
        )


@dataclass(frozen=True)
class ResultCounts:
    decision_steps: int
    admissions: int
    orders: int
    fills: int
    settlements: int
    ledger_postings: int
    risk_events: int

    @classmethod
    def from_wire(cls, value: object) -> ResultCounts:
        keys = {
            "decision_steps", "admissions", "orders", "fills", "settlements",
            "ledger_postings", "risk_events",
        }
        row = _object(value, keys)
        return cls(**{key: _uint(row[key], key) for key in keys})


@dataclass(frozen=True)
class DataFixtureReference:
    fixture_id: str
    fixture_version: str

    @classmethod
    def from_wire(cls, value: object) -> DataFixtureReference:
        row = _object(value, {"fixture_id", "fixture_version"})
        return cls(
            _wire(validate_identifier, row["fixture_id"]),
            _wire(validate_identifier, row["fixture_version"]),
        )


@dataclass(frozen=True)
class ModelInvocation:
    mode: str
    model_id: str
    model_version: str
    inference_count: int

    @classmethod
    def from_wire(cls, value: object) -> ModelInvocation:
        row = _object(value, {"mode", "model_id", "model_version", "inference_count"})
        if row["mode"] not in {"fresh_inference", "recorded_response_replay"}:
            raise ProtocolError("model invocation mode is unsupported")
        return cls(
            row["mode"],
            _wire(validate_identifier, row["model_id"]),
            _wire(validate_identifier, row["model_version"]),
            _uint(row["inference_count"], "inference_count"),
        )


@dataclass(frozen=True)
class ResultProvenance:
    data_fixtures: tuple[DataFixtureReference, ...]
    model_invocation: ModelInvocation

    @classmethod
    def from_wire(cls, value: object) -> ResultProvenance:
        row = _object(value, {"data_fixtures", "model_invocation"})
        fixtures = tuple(
            DataFixtureReference.from_wire(item)
            for item in _array(row["data_fixtures"], "data_fixtures")
        )
        if not fixtures:
            raise ProtocolError("result provenance must include a data fixture")
        return cls(fixtures, ModelInvocation.from_wire(row["model_invocation"]))


_EVIDENCE_KINDS = {
    "run_manifest", "configuration", "observation", "snapshot", "model_request",
    "model_response", "decision_admission", "order", "fill", "settlement", "ledger",
    "risk", "summary",
}


@dataclass(frozen=True)
class EvidenceReference:
    artifact_id: str
    kind: str
    relative_path: str
    content_type: str
    sha256: str
    size_bytes: int
    created_at: UtcInstant

    @classmethod
    def from_wire(cls, value: object) -> EvidenceReference:
        row = _object(
            value,
            {"artifact_id", "kind", "relative_path", "content_type", "sha256", "size_bytes", "created_at"},
        )
        path = _text(row["relative_path"], "relative_path")
        components = path.split("/")
        if not path.startswith("evidence/") or any(part in {"", ".", ".."} for part in components):
            raise ProtocolError("evidence path must remain under the run evidence directory")
        if row["kind"] not in _EVIDENCE_KINDS:
            raise ProtocolError("evidence kind is unsupported")
        digest = _text(row["sha256"], "sha256")
        if len(digest) != 64 or any(character not in "0123456789abcdef" for character in digest):
            raise ProtocolError("evidence sha256 must contain 64 lowercase hexadecimal digits")
        return cls(
            _wire(validate_uuid, row["artifact_id"]),
            row["kind"],
            path,
            _text(row["content_type"], "content_type"),
            digest,
            _uint(row["size_bytes"], "size_bytes"),
            _wire(UtcInstant.from_wire, row["created_at"]),
        )


@dataclass(frozen=True)
class RunResult:
    contract_version: str
    run_id: str
    status: str
    scenario_id: str
    output_mode: str
    simulation_start: UtcInstant
    simulation_end: UtcInstant
    finished_at: UtcInstant
    initial_cash: WireDecimal
    total_fees: WireDecimal
    counts: ResultCounts
    portfolio: PortfolioSnapshot
    provenance: ResultProvenance
    evidence: tuple[EvidenceReference, ...]

    @classmethod
    def from_wire(cls, value: object) -> RunResult:
        row = _object(
            value,
            {
                "contract_version", "run_id", "status", "scenario_id", "output_mode",
                "simulation_start", "simulation_end", "finished_at", "initial_cash", "total_fees",
                "counts", "portfolio", "provenance", "evidence",
            },
        )
        if row["contract_version"] != "1.0.0" or row["status"] != "succeeded":
            raise ProtocolError("run result contract version or terminal status is unsupported")
        if row["output_mode"] not in {"scores", "targets", "order_intents"}:
            raise ProtocolError("run result output mode is unsupported")
        evidence = tuple(
            EvidenceReference.from_wire(item) for item in _array(row["evidence"], "evidence")
        )
        if not evidence:
            raise ProtocolError("successful run result must include evidence")
        return cls(
            row["contract_version"],
            _wire(validate_uuid, row["run_id"]),
            "succeeded",
            _wire(validate_identifier, row["scenario_id"]),
            row["output_mode"],
            _wire(UtcInstant.from_wire, row["simulation_start"]),
            _wire(UtcInstant.from_wire, row["simulation_end"]),
            _wire(UtcInstant.from_wire, row["finished_at"]),
            _wire(WireDecimal.from_wire, row["initial_cash"]),
            _wire(WireDecimal.from_wire, row["total_fees"]),
            ResultCounts.from_wire(row["counts"]),
            PortfolioSnapshot.from_wire(row["portfolio"]),
            ResultProvenance.from_wire(row["provenance"]),
            evidence,
        )
