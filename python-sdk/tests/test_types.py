import sys
import unittest
from decimal import Decimal
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parents[1] / "src"))

from factorsbt import (
    ContractValueError,
    UtcInstant,
    WireDecimal,
    decimal_to_wire,
    validate_identifier,
    validate_instrument_id,
    validate_uuid,
)


class WireDecimalTests(unittest.TestCase):
    def test_round_trip_keeps_decimal_string_scale(self):
        amount = WireDecimal.from_wire("12.3400")
        self.assertEqual(amount.value, Decimal("12.3400"))
        self.assertEqual(amount.to_wire(), "12.3400")

    def test_rejects_json_numbers_exponents_and_negative_zero(self):
        for invalid in (100, 1.25, "1e3", "01", "-0", "NaN", "12345678901234567890123456789"):
            with self.subTest(invalid=invalid):
                with self.assertRaises(ContractValueError):
                    WireDecimal.from_wire(invalid)

    def test_decimal_conversion_expands_decimal_exponents_and_rejects_float(self):
        self.assertEqual(decimal_to_wire(Decimal("1.20E+2")), "120")
        self.assertEqual(WireDecimal.from_value(Decimal("0.000000000000000001")).to_wire(),
                         "0.000000000000000001")
        for invalid in (1.25, 100, Decimal("-0.00"), Decimal("1E-19")):
            with self.subTest(invalid=invalid):
                with self.assertRaises(ContractValueError):
                    decimal_to_wire(invalid)

    def test_direct_constructor_cannot_bypass_decimal_wire_validation(self):
        with self.assertRaises(ContractValueError):
            WireDecimal(Decimal("NaN"))


class UtcInstantTests(unittest.TestCase):
    def test_parses_and_emits_utc_z_timestamp(self):
        instant = UtcInstant.from_wire("2026-01-05T16:00:00Z")
        self.assertEqual(instant.to_wire(), "2026-01-05T16:00:00Z")

    def test_rejects_offset_and_invalid_calendar_timestamp(self):
        for invalid in (
            "2026-01-05T16:00:00+00:00",
            "2026-01-05T16:00Z",
            "2026-02-30T16:00:00Z",
        ):
            with self.subTest(invalid=invalid):
                with self.assertRaises(ContractValueError):
                    UtcInstant.from_wire(invalid)

    def test_round_trip_preserves_fractional_precision_beyond_datetime_microseconds(self):
        instant = UtcInstant.from_wire("2026-01-05T16:00:00.123456789Z")
        self.assertEqual(instant.to_wire(), "2026-01-05T16:00:00.123456789Z")


class IdentifierTests(unittest.TestCase):
    def test_validates_shared_uuid_identifier_and_instrument_rules(self):
        self.assertEqual(validate_uuid("8a2f5ce2-a6b5-4a71-b587-fefdb4a9fbc5"),
                         "8a2f5ce2-a6b5-4a71-b587-fefdb4a9fbc5")
        self.assertEqual(validate_identifier("model:v1.2"), "model:v1.2")
        self.assertEqual(validate_instrument_id("SPOT_A-1"), "SPOT_A-1")

    def test_rejects_invalid_uuid_and_identifier_characters_or_case(self):
        invalid = (
            (validate_uuid, "8A2F5CE2-A6B5-4A71-B587-FEFDB4A9FBC5"),
            (validate_uuid, "not-a-uuid"),
            (validate_identifier, "-starts-with-punctuation"),
            (validate_identifier, "has space"),
            (validate_instrument_id, "spot_a"),
            (validate_instrument_id, "A" * 33),
        )
        for validator, raw in invalid:
            with self.subTest(raw=raw):
                with self.assertRaises(ContractValueError):
                    validator(raw)


if __name__ == "__main__":
    unittest.main()
