import sys
import unittest
from decimal import Decimal
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parents[1] / "src"))

from factorsbt import ContractValueError, UtcInstant, WireDecimal


class WireDecimalTests(unittest.TestCase):
    def test_round_trip_keeps_decimal_string_scale(self):
        amount = WireDecimal.from_wire("12.3400")
        self.assertEqual(amount.value, Decimal("12.3400"))
        self.assertEqual(amount.to_wire(), "12.3400")

    def test_rejects_json_numbers_exponents_and_negative_zero(self):
        for invalid in (100, 1.25, "1e3", "01", "-0", "NaN"):
            with self.subTest(invalid=invalid):
                with self.assertRaises(ContractValueError):
                    WireDecimal.from_wire(invalid)


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


if __name__ == "__main__":
    unittest.main()
