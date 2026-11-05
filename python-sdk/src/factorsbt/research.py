"""Typed values for bounded research jobs."""

from __future__ import annotations

from dataclasses import dataclass
from decimal import Decimal, InvalidOperation
from typing import Any, Mapping, TypeAlias

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
class RerankingBranchReport:
    final_nav: Decimal
    pnl_usd: Decimal
    total_filled_value: Decimal
    selections: tuple[Mapping[str, Any], ...]
    fills: tuple[Mapping[str, Any], ...]


@dataclass(frozen=True)
class RerankingReport:
    fixture_id: str
    fixture_version: str
    baseline: RerankingBranchReport | str
    reranker: RerankingBranchReport | str
    outcome_vs_baseline: str | None
    pnl_delta_usd: Decimal | None
    paired_decisions: tuple[Mapping[str, Any], ...]


@dataclass(frozen=True)
class EpisodeTransition:
    step: int
    observation_start: Mapping[str, Any]
    target_weight: Decimal
    admitted_quantity: Decimal
    fills: tuple[Mapping[str, Any], ...]
    observation_end: Mapping[str, Any]
    reward_usd: Decimal


@dataclass(frozen=True)
class EpisodeReport:
    fixture_id: str
    fixture_version: str
    transitions: tuple[EpisodeTransition, ...]
    final_observation: Mapping[str, Any]
    canceled_pending_orders: tuple[str, ...]
    failure: str | None


@dataclass(frozen=True)
class ResearchReplayReport:
    source_job_id: str
    source_kind: str
    source_outcome: str
    matched: bool
    first_mismatch: str | None
    source_report: Mapping[str, Any]
    replayed_report: Mapping[str, Any]


ResearchReport: TypeAlias = RerankingReport | EpisodeReport | ResearchReplayReport


def _decimal(value: object, label: str) -> Decimal:
    if not isinstance(value, str):
        raise ProtocolError(f"research {label} must be a decimal string")
    try:
        return Decimal(value)
    except InvalidOperation as error:
        raise ProtocolError(f"research {label} is not a decimal") from error


def _mapping(value: object, label: str) -> Mapping[str, Any]:
    if not isinstance(value, dict) or not all(isinstance(key, str) for key in value):
        raise ProtocolError(f"research {label} must be an object")
    return value


def _branch(value: object) -> RerankingBranchReport | str:
    row = _mapping(value, "reranking branch")
    if set(row) == {"Err"} and isinstance(row["Err"], str):
        return row["Err"]
    branch = _mapping(row.get("Ok"), "successful reranking branch")
    required = {"final_nav", "pnl_usd", "total_filled_value", "selections", "fills"}
    if set(branch) != required or not isinstance(branch["selections"], list) or not isinstance(branch["fills"], list):
        raise ProtocolError("successful reranking branch has unsupported fields")
    return RerankingBranchReport(
        _decimal(branch["final_nav"], "final NAV"),
        _decimal(branch["pnl_usd"], "P&L"),
        _decimal(branch["total_filled_value"], "filled value"),
        tuple(_mapping(item, "selection") for item in branch["selections"]),
        tuple(_mapping(item, "fill") for item in branch["fills"]),
    )


def _reranking_report(value: object) -> RerankingReport:
    row = _mapping(value, "reranking report")
    required = {"fixture_id", "fixture_version", "baseline", "reranker", "paired_decisions", "outcome_vs_baseline", "pnl_delta_usd"}
    if set(row) != required or not isinstance(row["fixture_id"], str) or not isinstance(row["fixture_version"], str):
        raise ProtocolError("reranking report has unsupported fields")
    direction = row["outcome_vs_baseline"]
    if direction is not None and direction not in {"higher", "lower", "equal"}:
        raise ProtocolError("reranking performance direction is unsupported")
    delta = None if row["pnl_delta_usd"] is None else _decimal(row["pnl_delta_usd"], "P&L delta")
    decisions = row["paired_decisions"]
    if not isinstance(decisions, list):
        raise ProtocolError("reranking paired decisions must be an array")
    return RerankingReport(row["fixture_id"], row["fixture_version"], _branch(row["baseline"]), _branch(row["reranker"]), direction, delta, tuple(_mapping(item, "paired decision") for item in decisions))


def _episode_report(value: object) -> EpisodeReport:
    row = _mapping(value, "episode report")
    required = {"fixture_id", "fixture_version", "transitions", "final_observation", "canceled_pending_orders", "failure"}
    if set(row) != required or not isinstance(row["fixture_id"], str) or not isinstance(row["fixture_version"], str):
        raise ProtocolError("episode report has unsupported fields")
    if not isinstance(row["transitions"], list) or not isinstance(row["canceled_pending_orders"], list) or not all(isinstance(item, str) for item in row["canceled_pending_orders"]):
        raise ProtocolError("episode trace or canceled-order list has an invalid shape")
    transitions = []
    for item in row["transitions"]:
        transition = _mapping(item, "episode transition")
        if set(transition) != {"step", "observation_start", "action", "admitted_quantity", "fills", "observation_end", "reward_usd"}:
            raise ProtocolError("episode transition has unsupported fields")
        action = _mapping(transition["action"], "episode action")
        if set(action) != {"target_weight"}:
            raise ProtocolError("episode action has unsupported fields")
        if not isinstance(transition["step"], int) or not isinstance(transition["fills"], list):
            raise ProtocolError("episode transition has an invalid index or fill list")
        transitions.append(EpisodeTransition(
            transition["step"], _mapping(transition["observation_start"], "start observation"),
            _decimal(action["target_weight"], "target weight"),
            _decimal(transition["admitted_quantity"], "admitted quantity"),
            tuple(_mapping(fill, "episode fill") for fill in transition["fills"]),
            _mapping(transition["observation_end"], "end observation"),
            _decimal(transition["reward_usd"], "reward"),
        ))
    failure = row["failure"]
    if failure is not None and not isinstance(failure, str):
        raise ProtocolError("episode failure must be text or null")
    return EpisodeReport(row["fixture_id"], row["fixture_version"], tuple(transitions), _mapping(row["final_observation"], "final observation"), tuple(row["canceled_pending_orders"]), failure)


def _replay_report(value: object) -> ResearchReplayReport:
    row = _mapping(value, "research replay report")
    required = {"source_job_id", "source_kind", "source_outcome", "matched", "first_mismatch", "source_report", "replayed_report"}
    if set(row) != required or row["source_kind"] not in {"reranking", "episode"} or row["source_outcome"] not in {"succeeded", "failed"} or not isinstance(row["matched"], bool):
        raise ProtocolError("research replay report has unsupported fields")
    mismatch = row["first_mismatch"]
    if mismatch is not None and not isinstance(mismatch, str):
        raise ProtocolError("research replay mismatch path must be text or null")
    return ResearchReplayReport(_wire(validate_uuid, row["source_job_id"]), row["source_kind"], row["source_outcome"], row["matched"], mismatch, _mapping(row["source_report"], "source report"), _mapping(row["replayed_report"], "replayed report"))


@dataclass(frozen=True)
class ResearchJobResult:
    job_id: str
    kind: str
    report: ResearchReport

    @classmethod
    def from_wire(cls, value: object) -> ResearchJobResult:
        row = _object(value, {"contract_version", "job_id", "kind", "report"})
        if row["contract_version"] != "1.0.0" or row["kind"] not in {"reranking", "episode", "replay"}:
            raise ProtocolError("research result has unsupported values")
        decoder = {"reranking": _reranking_report, "episode": _episode_report, "replay": _replay_report}[row["kind"]]
        return cls(_wire(validate_uuid, row["job_id"]), row["kind"], decoder(row["report"]))
