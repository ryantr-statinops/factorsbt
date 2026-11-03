"""Typed builders for complete MVP run configuration documents."""

from __future__ import annotations

from dataclasses import dataclass
from decimal import Decimal
import ipaddress
from typing import TypeAlias
from urllib.parse import urlsplit

from .instruments import LinearFutureInstrument, OptionInstrument, SpotInstrument
from .types import UtcInstant, WireDecimal, validate_identifier, validate_instrument_id


Instrument: TypeAlias = SpotInstrument | LinearFutureInstrument | OptionInstrument


def _timestamp(value: UtcInstant | str) -> str:
    return value.to_wire() if isinstance(value, UtcInstant) else UtcInstant.from_wire(value).to_wire()


def _decimal(value: Decimal | str, *, nonnegative: bool = False) -> str:
    wire = WireDecimal.from_value(value).to_wire()
    if nonnegative and Decimal(wire) < 0:
        raise ValueError("value must be nonnegative")
    return wire


@dataclass(frozen=True)
class InitialPosition:
    instrument_id: str
    quantity: Decimal | str

    def to_wire(self) -> dict[str, str]:
        return {
            "instrument_id": validate_instrument_id(self.instrument_id),
            "quantity": _decimal(self.quantity),
        }


@dataclass(frozen=True)
class InitialPortfolio:
    cash: Decimal | str
    positions: tuple[InitialPosition, ...] = ()

    def __post_init__(self) -> None:
        object.__setattr__(self, "positions", tuple(self.positions))

    def to_wire(self) -> dict[str, str | list[dict[str, str]]]:
        return {
            "cash": _decimal(self.cash, nonnegative=True),
            "positions": [position.to_wire() for position in self.positions],
        }


@dataclass(frozen=True)
class SimulationConfiguration:
    start_at: UtcInstant | str
    end_at: UtcInstant | str

    def to_wire(self) -> dict[str, str]:
        return {
            "start_at": _timestamp(self.start_at),
            "end_at": _timestamp(self.end_at),
            "calendar_id": "synthetic-daily-utc-v1",
        }


@dataclass(frozen=True)
class DataFixtureConfiguration:
    fixture_id: str
    fixture_version: str

    def to_wire(self) -> dict[str, str]:
        return {
            "fixture_id": validate_identifier(self.fixture_id),
            "fixture_version": validate_identifier(self.fixture_version),
        }


@dataclass(frozen=True)
class ModelConfiguration:
    model_id: str
    model_version: str
    endpoint: str
    timeout_ms: int

    def to_wire(self) -> dict[str, str | int]:
        model_id = validate_identifier(self.model_id)
        model_version = validate_identifier(self.model_version)
        if not isinstance(self.endpoint, str) or not self.endpoint:
            raise ValueError("endpoint must be a nonempty loopback HTTP URL")
        parsed = urlsplit(self.endpoint)
        try:
            host = parsed.hostname
            port = parsed.port
        except ValueError as exc:
            raise ValueError("endpoint must be a valid loopback HTTP URL") from exc
        loopback_host = False
        if host:
            try:
                loopback_host = ipaddress.ip_address(host).is_loopback
            except ValueError:
                loopback_host = host.lower() == "localhost"
        if (
            parsed.scheme != "http"
            or not host
            or not loopback_host
            or parsed.username is not None
            or parsed.password is not None
            or parsed.query
            or parsed.fragment
            or port == 0
        ):
            raise ValueError("endpoint must be loopback HTTP without credentials, query, or fragment")
        if type(self.timeout_ms) is not int or self.timeout_ms < 1:
            raise ValueError("timeout_ms must be an integer of at least 1")
        return {
            "adapter": "http_fixture",
            "model_id": model_id,
            "model_version": model_version,
            "endpoint": self.endpoint,
            "timeout_ms": self.timeout_ms,
        }


@dataclass(frozen=True)
class DerivativeScoreTarget:
    instrument_id: str
    contracts: int

    def to_wire(self) -> dict[str, str | int]:
        if type(self.contracts) is not int:
            raise ValueError("derivative score target contracts must be an integer")
        return {
            "instrument_id": validate_instrument_id(self.instrument_id),
            "contracts": self.contracts,
        }


@dataclass(frozen=True)
class ScoresPolicy:
    top_k: int
    derivative_score_targets: tuple[DerivativeScoreTarget, ...] = ()
    output_mode: str = "scores"

    def __post_init__(self) -> None:
        object.__setattr__(self, "derivative_score_targets", tuple(self.derivative_score_targets))

    def to_wire(self) -> dict[str, object]:
        if type(self.top_k) is not int or self.top_k < 1:
            raise ValueError("top_k must be an integer of at least 1")
        if self.output_mode != "scores":
            raise ValueError("ScoresPolicy output_mode must be scores")
        return {
            "output_mode": "scores",
            "score_policy": {
                "top_k": self.top_k,
                "derivative_score_targets": [
                    target.to_wire() for target in self.derivative_score_targets
                ],
            },
        }


@dataclass(frozen=True)
class TargetsPolicy:
    output_mode: str = "targets"

    def to_wire(self) -> dict[str, str]:
        if self.output_mode != "targets":
            raise ValueError("TargetsPolicy output_mode must be targets")
        return {"output_mode": "targets"}


@dataclass(frozen=True)
class OrderIntentsPolicy:
    output_mode: str = "order_intents"

    def to_wire(self) -> dict[str, str]:
        if self.output_mode != "order_intents":
            raise ValueError("OrderIntentsPolicy output_mode must be order_intents")
        return {"output_mode": "order_intents"}


DecisionPolicy: TypeAlias = ScoresPolicy | TargetsPolicy | OrderIntentsPolicy


@dataclass(frozen=True)
class RunConfiguration:
    simulation: SimulationConfiguration
    initial_portfolio: InitialPortfolio
    instruments: tuple[Instrument, ...]
    data: DataFixtureConfiguration
    model: ModelConfiguration
    decision_policy: DecisionPolicy
    contract_version: str = "1.0.0"

    def __post_init__(self) -> None:
        object.__setattr__(self, "instruments", tuple(self.instruments))

    def to_wire(self) -> dict[str, object]:
        if self.contract_version != "1.0.0":
            raise ValueError("this SDK supports contract_version 1.0.0")
        return {
            "contract_version": self.contract_version,
            "simulation": self.simulation.to_wire(),
            "initial_portfolio": self.initial_portfolio.to_wire(),
            "instruments": [instrument.to_wire() for instrument in self.instruments],
            "data": self.data.to_wire(),
            "model": self.model.to_wire(),
            "decision_policy": self.decision_policy.to_wire(),
        }
