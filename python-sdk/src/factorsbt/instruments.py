"""Typed builders for MVP instrument configuration objects."""

from __future__ import annotations

from dataclasses import dataclass
from decimal import Decimal

from .types import UtcInstant, decimal_to_wire, validate_instrument_id


def _wire_number(value: Decimal | str, *, positive: bool = False) -> str:
    wire = decimal_to_wire(value)
    number = Decimal(wire)
    if positive and number <= 0:
        raise ValueError("value must be greater than zero")
    if not positive and number < 0:
        raise ValueError("value must be nonnegative")
    return wire


def _timestamp(value: UtcInstant | str) -> str:
    if isinstance(value, UtcInstant):
        return value.to_wire()
    return UtcInstant.from_wire(value).to_wire()


def _common(
    instrument_id: str,
    price_tick: Decimal | str,
    quantity_step: Decimal | str,
    contract_multiplier: Decimal | str,
    fee_bps: Decimal | str,
    fee_per_unit: Decimal | str,
    slippage_bps: Decimal | str,
) -> dict[str, str]:
    return {
        "instrument_id": validate_instrument_id(instrument_id),
        "quote_currency": "USD",
        "price_tick": _wire_number(price_tick, positive=True),
        "quantity_step": _wire_number(quantity_step, positive=True),
        "contract_multiplier": _wire_number(contract_multiplier, positive=True),
        "fee_bps": _wire_number(fee_bps),
        "fee_per_unit": _wire_number(fee_per_unit),
        "slippage_bps": _wire_number(slippage_bps),
    }


@dataclass(frozen=True)
class SpotInstrument:
    """Spot instrument; fees, ticks, lot size and multiplier are explicit."""

    instrument_id: str
    price_tick: Decimal | str
    quantity_step: Decimal | str
    contract_multiplier: Decimal | str
    fee_bps: Decimal | str
    fee_per_unit: Decimal | str
    slippage_bps: Decimal | str

    def to_wire(self) -> dict[str, str]:
        return {"instrument_type": "spot", **_common(
            self.instrument_id, self.price_tick, self.quantity_step, self.contract_multiplier,
            self.fee_bps, self.fee_per_unit, self.slippage_bps,
        )}


@dataclass(frozen=True)
class LinearFutureInstrument:
    """Linear future with explicit underlying, expiry and margin amounts."""

    instrument_id: str
    underlying_id: str
    expiry_at: UtcInstant | str
    price_tick: Decimal | str
    quantity_step: Decimal | str
    contract_multiplier: Decimal | str
    fee_bps: Decimal | str
    fee_per_unit: Decimal | str
    slippage_bps: Decimal | str
    initial_margin_per_contract: Decimal | str
    maintenance_margin_per_contract: Decimal | str

    def to_wire(self) -> dict[str, str | int | dict[str, str | int]]:
        common = _common(
            self.instrument_id, self.price_tick, self.quantity_step, self.contract_multiplier,
            self.fee_bps, self.fee_per_unit, self.slippage_bps,
        )
        if common["quantity_step"] != "1":
            raise ValueError("linear future quantity_step must be exactly 1")
        initial = _wire_number(self.initial_margin_per_contract)
        maintenance = _wire_number(self.maintenance_margin_per_contract)
        if Decimal(maintenance) > Decimal(initial):
            raise ValueError("maintenance margin cannot exceed initial margin")
        return {
            "instrument_type": "linear_future",
            **common,
            "underlying_id": validate_instrument_id(self.underlying_id),
            "expiry_at": _timestamp(self.expiry_at),
            "initial_margin_per_contract": initial,
            "maintenance_margin_per_contract": maintenance,
        }


@dataclass(frozen=True)
class OptionInstrument:
    """European or American option with explicit valuation and margin inputs."""

    instrument_id: str
    underlying_id: str
    option_type: str
    strike: Decimal | str
    expiry_at: UtcInstant | str
    exercise_style: str
    settlement_style: str
    valuation_mode: str
    price_tick: Decimal | str
    quantity_step: Decimal | str
    contract_multiplier: Decimal | str
    fee_bps: Decimal | str
    fee_per_unit: Decimal | str
    slippage_bps: Decimal | str
    initial_margin_per_contract: Decimal | str
    maintenance_margin_per_contract: Decimal | str
    pricer: str | None = None
    crr_steps: int | None = None

    def to_wire(self) -> dict[str, str | int | dict[str, str | int]]:
        common = _common(
            self.instrument_id, self.price_tick, self.quantity_step, self.contract_multiplier,
            self.fee_bps, self.fee_per_unit, self.slippage_bps,
        )
        if common["quantity_step"] != "1":
            raise ValueError("option quantity_step must be exactly 1")
        if self.option_type not in {"call", "put"}:
            raise ValueError("option_type must be 'call' or 'put'")
        if self.exercise_style not in {"european", "american"}:
            raise ValueError("exercise_style must be 'european' or 'american'")
        expected_settlement = "cash" if self.exercise_style == "european" else "physical"
        if self.settlement_style != expected_settlement:
            raise ValueError(f"{self.exercise_style} options require {expected_settlement} settlement")
        if self.valuation_mode not in {"quote", "theoretical"}:
            raise ValueError("valuation_mode must be 'quote' or 'theoretical'")
        initial = _wire_number(self.initial_margin_per_contract)
        maintenance = _wire_number(self.maintenance_margin_per_contract)
        if Decimal(maintenance) > Decimal(initial):
            raise ValueError("maintenance margin cannot exceed initial margin")

        wire: dict[str, str | int | dict[str, str | int]] = {
            "instrument_type": "option",
            **common,
            "underlying_id": validate_instrument_id(self.underlying_id),
            "option_type": self.option_type,
            "strike": _wire_number(self.strike, positive=True),
            "expiry_at": _timestamp(self.expiry_at),
            "exercise_style": self.exercise_style,
            "settlement_style": self.settlement_style,
            "valuation_mode": self.valuation_mode,
            "initial_margin_per_contract": initial,
            "maintenance_margin_per_contract": maintenance,
        }
        if self.valuation_mode == "theoretical":
            if self.exercise_style == "european":
                if self.pricer != "black_scholes" or self.crr_steps is not None:
                    raise ValueError("European theoretical valuation requires black_scholes")
                inputs: dict[str, str | int] = {"pricer": "black_scholes", "day_count": "ACT/365F"}
            else:
                if self.pricer != "crr" or not isinstance(self.crr_steps, int) or not 2 <= self.crr_steps <= 4096:
                    raise ValueError("American theoretical valuation requires CRR steps from 2 through 4096")
                inputs = {"pricer": "crr", "day_count": "ACT/365F", "crr_steps": self.crr_steps}
            wire["valuation_inputs"] = inputs
        elif self.pricer is not None or self.crr_steps is not None:
            raise ValueError("quote valuation does not accept theoretical pricer inputs")
        return wire
