import json
import sys
import unittest
from decimal import Decimal
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parents[1] / "src"))

from factorsbt import LinearFutureInstrument, OptionInstrument, SpotInstrument


ROOT = Path(__file__).resolve().parents[2]


class InstrumentBuilderTests(unittest.TestCase):
    def setUp(self):
        self.spot = SpotInstrument(
            instrument_id="SPOT_A",
            price_tick=Decimal("0.01"),
            quantity_step=Decimal("0.001"),
            contract_multiplier=Decimal("1"),
            fee_bps=Decimal("0"),
            fee_per_unit=Decimal("0"),
            slippage_bps=Decimal("0"),
        )

    def test_spot_builder_matches_shared_wire_example(self):
        example = json.loads(
            (ROOT / "contracts/examples/instruments/minimal-valid-spot-run.json").read_text()
        )
        self.assertEqual(self.spot.to_wire(), example["instruments"][0])

    def test_future_builder_emits_explicit_margin_and_underlying(self):
        future = LinearFutureInstrument(
            instrument_id="FUT_A",
            underlying_id="SPOT_A",
            expiry_at="2026-01-09T16:00:00Z",
            price_tick="0.01",
            quantity_step="1",
            contract_multiplier="1",
            fee_bps="0",
            fee_per_unit="0",
            slippage_bps="0",
            initial_margin_per_contract="100",
            maintenance_margin_per_contract="80",
        )
        self.assertEqual(future.to_wire()["instrument_type"], "linear_future")
        self.assertEqual(future.to_wire()["underlying_id"], "SPOT_A")
        self.assertEqual(future.to_wire()["initial_margin_per_contract"], "100")

    def test_option_builder_supports_quote_and_theoretical_shapes(self):
        quote = OptionInstrument(
            instrument_id="EU_CALL_A",
            underlying_id="SPOT_A",
            option_type="call",
            strike="100",
            expiry_at="2026-01-09T16:00:00Z",
            exercise_style="european",
            settlement_style="cash",
            valuation_mode="quote",
            price_tick="0.01",
            quantity_step="1",
            contract_multiplier="1",
            fee_bps="0",
            fee_per_unit="0",
            slippage_bps="0",
            initial_margin_per_contract="100",
            maintenance_margin_per_contract="80",
        )
        theoretical = OptionInstrument(
            instrument_id="AM_PUT_A",
            underlying_id="SPOT_A",
            option_type="put",
            strike=Decimal("98"),
            expiry_at="2026-01-09T16:00:00Z",
            exercise_style="american",
            settlement_style="physical",
            valuation_mode="theoretical",
            price_tick="0.01",
            quantity_step="1",
            contract_multiplier="1",
            fee_bps="0",
            fee_per_unit="0",
            slippage_bps="0",
            initial_margin_per_contract="100",
            maintenance_margin_per_contract="80",
            pricer="crr",
            crr_steps=256,
        )
        self.assertNotIn("valuation_inputs", quote.to_wire())
        self.assertEqual(
            theoretical.to_wire()["valuation_inputs"],
            {"pricer": "crr", "day_count": "ACT/365F", "crr_steps": 256},
        )

    def test_builders_reject_instrument_schema_and_semantic_mismatches(self):
        with self.assertRaises(ValueError):
            LinearFutureInstrument(
                "FUT_A", "SPOT_A", "2026-01-09T16:00:00Z", "0.01", "0.5", "1",
                "0", "0", "0", "100", "80",
            ).to_wire()
        with self.assertRaises(ValueError):
            LinearFutureInstrument(
                "FUT_A", "SPOT_A", "2026-01-09T16:00:00Z", "0.01", "1", "1",
                "0", "0", "0", "80", "100",
            ).to_wire()
        with self.assertRaises(ValueError):
            OptionInstrument(
                "EU_CALL_A", "SPOT_A", "call", "100", "2026-01-09T16:00:00Z",
                "european", "physical", "quote", "0.01", "1", "1", "0", "0", "0",
                "100", "80",
            ).to_wire()


if __name__ == "__main__":
    unittest.main()
