"""Typed inputs for comparing independent synthetic runs."""

from __future__ import annotations

from dataclasses import dataclass
from decimal import Decimal

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
