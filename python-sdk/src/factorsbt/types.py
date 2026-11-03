"""Small, dependency-free parsers for shared JSON wire primitives."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timezone
from decimal import Decimal, InvalidOperation
import re
from uuid import UUID


class ContractValueError(ValueError):
    """A value does not match the FactorsBT wire representation."""


def validate_uuid(raw: object) -> str:
    """Return a lowercase, hyphenated UUID in the shared wire format."""
    if not isinstance(raw, str) or not _UUID.fullmatch(raw):
        raise ContractValueError("expected a lowercase UUID string")
    try:
        UUID(raw)
    except ValueError as exc:
        raise ContractValueError("expected a lowercase UUID string") from exc
    return raw


def validate_identifier(raw: object) -> str:
    """Validate a general contract identifier (1–128 allowed characters)."""
    if not isinstance(raw, str) or not _IDENTIFIER.fullmatch(raw):
        raise ContractValueError("expected a contract identifier")
    return raw


def validate_instrument_id(raw: object) -> str:
    """Validate a stable uppercase instrument identifier."""
    if not isinstance(raw, str) or not _INSTRUMENT_ID.fullmatch(raw):
        raise ContractValueError("expected an instrument ID")
    return raw


def _validate_decimal_text(raw: str) -> Decimal:
    if len(raw) > 48 or not _DECIMAL.fullmatch(raw):
        raise ContractValueError("expected a decimal string without exponent notation")
    try:
        value = Decimal(raw)
    except InvalidOperation as exc:
        raise ContractValueError("invalid decimal string") from exc
    if not value.is_finite() or (value.is_zero() and value.is_signed()):
        raise ContractValueError("decimal must be finite and cannot be negative zero")
    digits = [character for character in raw.lstrip("-") if character.isdigit()]
    first_nonzero = next((index for index, digit in enumerate(digits) if digit != "0"), None)
    significant_digits = 1 if first_nonzero is None else len(digits) - first_nonzero
    if significant_digits > 28:
        raise ContractValueError("decimal exceeds 28 significant digits")
    return value


def decimal_to_wire(raw: object) -> str:
    """Convert a Decimal or decimal string to wire form without float coercion."""
    if isinstance(raw, Decimal):
        text = format(raw, "f")
    elif isinstance(raw, str):
        text = raw
    else:
        raise ContractValueError("financial values must be Decimal or decimal strings")
    _validate_decimal_text(text)
    return text


_DECIMAL = re.compile(r"^-?(?:0|[1-9][0-9]*)(?:\.[0-9]{1,18})?$")
_UTC_TIMESTAMP = re.compile(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]+)?Z$")
_UUID = re.compile(r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
_IDENTIFIER = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$")
_INSTRUMENT_ID = re.compile(r"^[A-Z][A-Z0-9_-]{0,31}$")


@dataclass(frozen=True)
class WireDecimal:
    """Finite base-10 value whose JSON representation is a string."""

    value: Decimal

    def __post_init__(self) -> None:
        if not isinstance(self.value, Decimal):
            raise ContractValueError("WireDecimal value must be a Decimal")
        _validate_decimal_text(format(self.value, "f"))

    @classmethod
    def from_wire(cls, raw: object) -> WireDecimal:
        if not isinstance(raw, str):
            raise ContractValueError("expected a decimal string without exponent notation")
        return cls(_validate_decimal_text(raw))

    @classmethod
    def from_value(cls, raw: object) -> WireDecimal:
        """Build a wire decimal without accepting a lossy float conversion."""
        return cls(Decimal(decimal_to_wire(raw)))

    def to_wire(self) -> str:
        return decimal_to_wire(self.value)


@dataclass(frozen=True)
class UtcInstant:
    """RFC 3339 instant normalized to UTC, retaining the input timestamp."""

    value: datetime
    _wire_value: str | None = None

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
        # datetime stores microseconds; retain extra RFC 3339 fractional digits
        # in the wire value so parsing and re-serialization do not lose precision.
        return cls(value.astimezone(timezone.utc), raw)

    def to_wire(self) -> str:
        if self._wire_value is not None:
            return self._wire_value
        return self.value.isoformat(timespec="auto").replace("+00:00", "Z")
