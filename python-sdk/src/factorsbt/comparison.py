"""Typed inputs for comparing independent synthetic runs."""

from __future__ import annotations

from dataclasses import dataclass

from .configuration import RunConfiguration
from .responses import RunResult
from .types import validate_identifier


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


@dataclass(frozen=True)
class ComparisonReport:
    baseline_label: str
    runs: tuple[ComparisonRun, ...]
