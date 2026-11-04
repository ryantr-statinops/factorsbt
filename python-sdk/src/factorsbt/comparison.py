"""Typed inputs for comparing independent synthetic runs."""

from __future__ import annotations

from dataclasses import dataclass
from decimal import Decimal
import json

from .configuration import RunConfiguration
from .errors import ProtocolError
from .responses import RunResult
from .types import UtcInstant, WireDecimal, validate_identifier


@dataclass(frozen=True)
class ComparisonScenario:
    label: str
    configuration: RunConfiguration

    def __post_init__(self) -> None:
        validate_identifier(self.label)
        if not isinstance(self.configuration, RunConfiguration):
            raise TypeError("comparison configuration must be a typed RunConfiguration")


@dataclass(frozen=True)
class ComparisonSpec:
    baseline: ComparisonScenario
    variants: tuple[ComparisonScenario, ...]

    def __post_init__(self) -> None:
        if not isinstance(self.baseline, ComparisonScenario):
            raise TypeError("baseline must be a ComparisonScenario")
        variants = tuple(self.variants)
        if not variants or any(not isinstance(item, ComparisonScenario) for item in variants):
            raise ValueError("comparison requires at least one typed variant")
        labels = [self.baseline.label, *(item.label for item in variants)]
        if len(set(labels)) != len(labels):
            raise ValueError("comparison labels must be unique")
        object.__setattr__(self, "variants", variants)

    @property
    def scenarios(self) -> tuple[ComparisonScenario, ...]:
        return (self.baseline, *self.variants)


@dataclass(frozen=True)
class ComparisonRun:
    label: str
    configuration: RunConfiguration
    submission_id: str | None
    run_id: str | None
    status: str
    result: RunResult | None
    error_code: str | None = None
    metrics: RunMetrics | None = None
    timeline: tuple[TimelinePoint, ...] = ()
    trace: tuple[TracePoint, ...] = ()


@dataclass(frozen=True)
class TracePoint:
    simulation_time: str
    instrument_id: str
    kind: str
    ordinal: int
    value: str
    artifact_id: str

    @property
    def key(self) -> tuple[str, str, str, int]:
        return (self.simulation_time, self.instrument_id, self.kind, self.ordinal)


@dataclass(frozen=True)
class TraceDifference:
    key: tuple[str, str, str, int]
    baseline_value: str | None
    variant_value: str | None
    baseline_artifact_id: str | None
    variant_artifact_id: str | None


def trace_from_artifacts(artifacts: tuple[tuple[str, str, object], ...]) -> tuple[TracePoint, ...]:
    snapshots_by_step = {}
    for kind, _, value in artifacts:
        if kind == "snapshot" and isinstance(value, dict):
            snapshots_by_step[value.get("step_id")] = value.get("simulation_time")
    points = []
    fill_ordinals: dict[tuple[str, str], int] = {}
    for kind, artifact_id, value in artifacts:
        if kind == "decision_admission" and isinstance(value, dict) and "proposal" in value:
            time = snapshots_by_step.get(value.get("step_id"))
            if not isinstance(time, str):
                raise ProtocolError("decision evidence lacks a matching snapshot")
            proposal = value["proposal"]
            for category in ("target_positions", "order_intents"):
                for item in proposal.get(category, []):
                    points.append(TracePoint(time, item["instrument_id"], category, 0, _canonical(item), artifact_id))
        elif kind == "model_response" and isinstance(value, dict) and "outputs" in value:
            time = snapshots_by_step.get(value.get("step_id"))
            if not isinstance(time, str):
                raise ProtocolError("model response evidence lacks a matching snapshot")
            for category, items in value["outputs"].items():
                for item in items:
                    points.append(TracePoint(time, item["instrument_id"], category, 0, _canonical(item), artifact_id))
        elif kind == "fill" and isinstance(value, list):
            for event in value:
                time = event.get("execution_time")
                if not isinstance(time, str):
                    continue
                for fill in event.get("fills", []):
                    pair = (time, fill["instrument_id"])
                    ordinal = fill_ordinals.get(pair, 0)
                    fill_ordinals[pair] = ordinal + 1
                    points.append(TracePoint(time, fill["instrument_id"], "fill", ordinal, _canonical(fill), artifact_id))
    keys = [point.key for point in points]
    if len(keys) != len(set(keys)):
        raise ProtocolError("trace evidence contains duplicate comparison keys")
    return tuple(sorted(points, key=lambda point: point.key))


def _canonical(value: object) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


@dataclass(frozen=True)
class TimelinePoint:
    simulation_time: UtcInstant
    instrument_id: str
    observation_revisions: tuple[tuple[str, int], ...]
    quantity: Decimal
    mark: Decimal
    artifact_id: str

    @property
    def key(self) -> tuple[str, str]:
        return (self.simulation_time.to_wire(), self.instrument_id)


def timeline_from_snapshots(snapshots: tuple[tuple[str, object], ...]) -> tuple[TimelinePoint, ...]:
    points = []
    for artifact_id, snapshot in snapshots:
        if not isinstance(snapshot, dict) or not isinstance(snapshot.get("portfolio"), dict):
            raise ProtocolError("snapshot evidence is not an object with a portfolio")
        try:
            simulation_time = UtcInstant.from_wire(snapshot["simulation_time"])
            observations = snapshot["observations"]
            positions = snapshot["portfolio"]["positions"]
            if not isinstance(observations, list) or not isinstance(positions, list):
                raise TypeError("snapshot arrays are missing")
            revisions: dict[str, list[tuple[str, int]]] = {}
            for observation in observations:
                instrument_id = observation.get("instrument_id")
                if instrument_id is not None:
                    revisions.setdefault(instrument_id, []).append(
                        (observation["observation_id"], observation["revision"])
                    )
            for position in positions:
                points.append(TimelinePoint(
                    simulation_time,
                    position["instrument_id"],
                    tuple(sorted(revisions.get(position["instrument_id"], []))),
                    WireDecimal.from_wire(position["quantity"]).value,
                    WireDecimal.from_wire(position["mark"]).value,
                    artifact_id,
                ))
        except (KeyError, TypeError, ValueError, AttributeError) as exc:
            raise ProtocolError("snapshot evidence contains invalid timeline fields") from exc
    keys = [point.key for point in points]
    if len(keys) != len(set(keys)):
        raise ProtocolError("snapshot evidence contains duplicate time and instrument keys")
    return tuple(sorted(points, key=lambda point: point.key))


@dataclass(frozen=True)
class RunMetrics:
    simulation_start: UtcInstant
    simulation_end: UtcInstant
    initial_nav: Decimal
    final_nav: Decimal
    pnl_usd: Decimal
    return_fraction: Decimal | None

    @classmethod
    def from_summary(cls, summary: object, result: RunResult) -> RunMetrics:
        if not isinstance(summary, dict):
            raise ProtocolError("run summary must be an object")
        try:
            initial = WireDecimal.from_wire(summary["initial_nav"]).value
            start = UtcInstant.from_wire(summary["simulation_start"])
            end = UtcInstant.from_wire(summary["simulation_end"])
        except (KeyError, TypeError, ValueError) as exc:
            raise ProtocolError("run summary lacks comparable initial facts") from exc
        if start != result.simulation_start or end != result.simulation_end or summary.get("run_id") != result.run_id:
            raise ProtocolError("run summary does not match the result identity and interval")
        final = result.portfolio.nav.value
        pnl = final - initial
        return cls(start, end, initial, final, pnl, None if initial == 0 else pnl / initial)


@dataclass(frozen=True)
class ComparisonReport:
    baseline_label: str
    runs: tuple[ComparisonRun, ...]

    def overlap(self, variant_label: str) -> tuple[tuple[tuple[str, str], ...], tuple[tuple[str, str], ...], tuple[tuple[str, str], ...]]:
        baseline = next(run for run in self.runs if run.label == self.baseline_label)
        variant = next(run for run in self.runs if run.label == variant_label and run.label != self.baseline_label)
        left = {point.key for point in baseline.timeline}
        right = {point.key for point in variant.timeline}
        return tuple(sorted(left & right)), tuple(sorted(left - right)), tuple(sorted(right - left))

    def trace_differences(self, variant_label: str) -> tuple[TraceDifference, ...]:
        baseline = next(run for run in self.runs if run.label == self.baseline_label)
        variant = next(run for run in self.runs if run.label == variant_label and run.label != self.baseline_label)
        left = {point.key: point for point in baseline.trace}
        right = {point.key: point for point in variant.trace}
        return tuple(
            TraceDifference(
                key,
                left[key].value if key in left else None,
                right[key].value if key in right else None,
                left[key].artifact_id if key in left else None,
                right[key].artifact_id if key in right else None,
            )
            for key in sorted(left.keys() | right.keys())
            if key not in left or key not in right or left[key].value != right[key].value
        )

    @property
    def configuration_changes(self) -> tuple[ConfigurationChange, ...]:
        baseline = next(run for run in self.runs if run.label == self.baseline_label)
        original = baseline.configuration.to_wire()
        changes = []
        for run in self.runs:
            if run.label == self.baseline_label:
                continue
            for path in _changed_paths(original, run.configuration.to_wire()):
                changes.append(ConfigurationChange(run.label, path))
        return tuple(changes)


@dataclass(frozen=True)
class ConfigurationChange:
    variant_label: str
    path: str


def _changed_paths(left: object, right: object, prefix: str = "$") -> tuple[str, ...]:
    if isinstance(left, dict) and isinstance(right, dict):
        keys = sorted(left.keys() | right.keys())
        return tuple(
            path
            for key in keys
            for path in _changed_paths(left.get(key), right.get(key), f"{prefix}.{key}")
        )
    if isinstance(left, list) and isinstance(right, list):
        return tuple(
            path
            for index in range(max(len(left), len(right)))
            for path in _changed_paths(
                left[index] if index < len(left) else None,
                right[index] if index < len(right) else None,
                f"{prefix}[{index}]",
            )
        )
    return () if left == right else (prefix,)
