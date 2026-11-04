import json
import sys
import tempfile
import unittest
from decimal import Decimal
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parents[1] / "src"))

from factorsbt import ComparisonReport, ComparisonRun
from factorsbt.comparison import RunMetrics, TracePoint, TimelinePoint
from factorsbt.types import UtcInstant

ROOT = Path(__file__).resolve().parents[2]


class ComparisonReportTests(unittest.TestCase):
    def setUp(self):
        class Config:
            def to_wire(self):
                return {"fixture": "comparison-fixture"}
        self.config = Config()
        start = UtcInstant.from_wire("2026-01-05T16:00:00Z")
        end = UtcInstant.from_wire("2026-01-09T16:00:00Z")
        base_metrics = RunMetrics(start, end, Decimal("100"), Decimal("110"), Decimal("10"), Decimal("0.1"))
        changed_metrics = RunMetrics(start, end, Decimal("100"), Decimal("115"), Decimal("15"), Decimal("0.15"))
        baseline = ComparisonRun("base", self.config, None, None, "succeeded", None, metrics=base_metrics)
        variant = ComparisonRun("variant", self.config, None, None, "succeeded", None, metrics=changed_metrics)
        self.report = ComparisonReport("base", (baseline, variant))

    def test_metrics_diff_and_saved_report_are_typed_and_reproducible(self):
        diff, = self.report.metric_differences
        self.assertEqual(diff.final_nav_usd, Decimal("5"))
        self.assertEqual(diff.pnl_usd, Decimal("5"))
        self.assertEqual(diff.return_fraction, Decimal("0.05"))
        self.assertTrue(diff.same_interval)
        with tempfile.TemporaryDirectory() as directory:
            path = self.report.save(Path(directory) / "comparison.json")
            self.assertEqual(json.loads(path.read_text())["baseline_label"], "base")
            with self.assertRaises(FileExistsError):
                self.report.save(path)

    def test_overlap_reports_shared_and_unmatched_time_instrument_keys(self):
        time = UtcInstant.from_wire("2026-01-06T16:00:00Z")
        base = self.report.runs[0]
        variant = self.report.runs[1]
        object.__setattr__(base, "timeline", (TimelinePoint(time, "SPOT_A", (), Decimal("1"), Decimal("10"), "a"), TimelinePoint(time, "SPOT_B", (), Decimal("1"), Decimal("20"), "b")))
        object.__setattr__(variant, "timeline", (TimelinePoint(time, "SPOT_A", (), Decimal("2"), Decimal("11"), "c"), TimelinePoint(time, "SPOT_C", (), Decimal("1"), Decimal("5"), "d")))
        common, base_only, variant_only = self.report.overlap("variant")
        self.assertEqual(common, ((time.to_wire(), "SPOT_A"),))
        self.assertEqual(base_only, ((time.to_wire(), "SPOT_B"),))
        self.assertEqual(variant_only, ((time.to_wire(), "SPOT_C"),))

    def test_trace_difference_retains_evidence_references(self):
        left = TracePoint("2026-01-06T16:00:00Z", "SPOT_A", "fill", 0, '{"q":"1"}', "artifact-a")
        right = TracePoint("2026-01-06T16:00:00Z", "SPOT_A", "fill", 0, '{"q":"2"}', "artifact-b")
        object.__setattr__(self.report.runs[0], "trace", (left,))
        object.__setattr__(self.report.runs[1], "trace", (right,))
        difference, = self.report.trace_differences("variant")
        self.assertEqual(difference.baseline_artifact_id, "artifact-a")
        self.assertEqual(difference.variant_artifact_id, "artifact-b")


if __name__ == "__main__":
    unittest.main()
