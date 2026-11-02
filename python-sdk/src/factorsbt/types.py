"""Small, dependency-free parsers for shared JSON wire primitives."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timezone
from decimal import Decimal, InvalidOperation
import re


class ContractValueError(ValueError):
    """A value does not match the FactorsBT wire representation."""


_DECIMAL = re.compile(r"^-?(?:0|[1-9][0-9]*)(?:\.[0-9]{1,18})?$")
_UTC_TIMESTAMP = re.compile(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]+)?Z$")


@dataclass(frozen=True)
class WireDecimal:
    """Finite base-10 value whose JSON representation is a string."""

    value: Decimal

    @classmethod
    def from_wire(cls, raw: object) -> WireDecimal:
        if not isinstance(raw, str) or len(raw) > 48 or not _DECIMAL.fullmatch(raw):
            raise ContractValueError("expected a decimal string without exponent notation")
        try:
            value = Decimal(raw)
        except InvalidOperation as exc:
            raise ContractValueError("invalid decimal string") from exc
        if not value.is_finite() or (value.is_zero() and value.is_signed()):
            raise ContractValueError("decimal must be finite and cannot be negative zero")
        if len(value.as_tuple().digits) > 28:
            raise ContractValueError("decimal exceeds 28 significant digits")
        return cls(value)

    def to_wire(self) -> str:
        text = str(self.value)
        if not _DECIMAL.fullmatch(text):
            raise ContractValueError("decimal cannot be represented in the wire format")
        return text


@dataclass(frozen=True)
class UtcInstant:
    """RFC 3339 instant normalized to UTC, retaining the input timestamp."""

    value: datetime

    @classmethod
    def from_wire(cls, raw: object) -> UtcInstant:
        if not isinstance(raw, str) or not _UTC_TIMESTAMP.fullmatch(raw):
            raise ContractValueError("timestamp must be an RFC 3339 UTC string ending in Z")
        try:
            value = datetime.fromisoformat(raw[:-1] + "+00:00")
        except ValueError as exc:
            raise ContractValueError("invalid UTC timestamp") from exc
        if value.utcoffset() != timezone.utc.utcoffset(value):
            raise ContractValueError("timestamp must use UTC")
        return cls(value.astimezone(timezone.utc))

    def to_wire(self) -> str:
        return self.value.isoformat(timespec="auto").replace("+00:00", "Z")
