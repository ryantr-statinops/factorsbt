import json
from pathlib import Path
import unittest

from jsonschema import Draft202012Validator, FormatChecker
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_DIR = ROOT / "contracts" / "schemas"
FIXTURE_SCHEMA_DIR = ROOT / "contracts" / "fixtures" / "schemas"
EXAMPLE_DIR = ROOT / "contracts" / "examples"


def load_document(path):
    return json.loads(path.read_text(encoding="utf-8"))


def build_registry():
    registry = Registry()
    schemas = {}
    for path in sorted(SCHEMA_DIR.glob("*.schema.json")):
        schema = load_document(path)
        Draft202012Validator.check_schema(schema)
        schemas[path.name] = schema
        registry = registry.with_resource(schema["$id"], Resource.from_contents(schema))
    for path in sorted(FIXTURE_SCHEMA_DIR.glob("*.schema.json")):
        schema = load_document(path)
        Draft202012Validator.check_schema(schema)
        schemas[path.name] = schema
        registry = registry.with_resource(schema["$id"], Resource.from_contents(schema))
    return schemas, registry


class ContractExampleTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.schemas, cls.registry = build_registry()
        cls.schemas["evidence-artifact.schema.json"] = {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://schemas.factorsbt.invalid/v1/evidence-artifact-test.schema.json",
            **cls.schemas["run-result.schema.json"]["$defs"]["EvidenceArtifactRef"],
        }

    def check_example(self, schema_name, example_name, expected_valid):
        schema = self.schemas[schema_name]
        validator = Draft202012Validator(
            schema,
            registry=self.registry,
            format_checker=FormatChecker(),
        )
        path = EXAMPLE_DIR / example_name
        document = load_document(path)
        errors = list(validator.iter_errors(document))
        self.assertEqual(
            not errors,
            expected_valid,
            f"{path.relative_to(ROOT)}: " + "; ".join(error.message for error in errors[:3]),
        )

    def test_shared_valid_examples_and_semantic_invalid_examples_match_schemas(self):
        cases = [
            ("run-configuration.schema.json", "instruments/minimal-valid-spot-run.json", True),
            ("run-configuration.schema.json", "instruments/unresolved-underlying.semantic-invalid.json", True),
            ("instrument.schema.json", "instruments/spot-missing-fee.schema-invalid.json", False),
            ("model-request.schema.json", "model/scores-request.inline-snapshot.json", True),
            ("model-response.schema.json", "model/scores-response.valid.json", True),
            ("model-response.schema.json", "model/targets-response.valid-shape.json", True),
            ("model-response.schema.json", "model/order-intents-response.valid-shape.json", True),
            ("model-response.schema.json", "model/response-missing-snapshot.schema-invalid.json", False),
            ("model-response.schema.json", "model/response-mismatched-step.semantic-invalid.json", True),
            ("observation.schema.json", "temporal/late-news.available-later.json", True),
            ("observation.schema.json", "temporal/option-pricing-input.available.json", True),
            ("observation.schema.json", "temporal/missing-availability.schema-invalid.json", False),
            ("snapshot.schema.json", "temporal/unavailable-data.snapshot-semantic-invalid.json", True),
            ("engine-outcome.schema.json", "engine/admission-adjusted.valid.json", True),
            ("engine-outcome.schema.json", "engine/order-created.valid.json", True),
            ("engine-outcome.schema.json", "engine/fill.valid.json", True),
            ("engine-outcome.schema.json", "engine/fill-zero-quantity.semantic-invalid.json", True),
            ("engine-outcome.schema.json", "engine/fill-missing-price.schema-invalid.json", False),
            ("engine-outcome.schema.json", "engine/ledger-spot-purchase.valid.json", True),
            ("engine-outcome.schema.json", "engine/futures-variation.ledger-valid.json", True),
            ("engine-outcome.schema.json", "engine/futures-variation.settlement-valid.json", True),
            ("engine-outcome.schema.json", "engine/settlement-assignment.valid.json", True),
            ("engine-outcome.schema.json", "engine/risk-liquidation-scheduled.valid.json", True),
            ("run-result.schema.json", "results/successful-run.valid.json", True),
            ("run-result.schema.json", "results/missing-artifact-hash.schema-invalid.json", False),
            ("evidence-artifact.schema.json", "results/path-traversal.semantic-invalid.json", True),
        ]
        for schema_name, example_name, expected_valid in cases:
            with self.subTest(example=example_name):
                self.check_example(schema_name, example_name, expected_valid)

    def test_internal_fixture_v1_schemas_accept_controlled_shape_and_reject_traversal(self):
        manifest = {
            "format_version": "1.0.0",
            "fixture_id": "mvp-synthetic",
            "fixture_version": "v1",
            "dataset_path": "datasets/baseline.json",
            "dataset_sha256": "a" * 64,
        }
        dataset = {
            "format_version": "1.0.0",
            "fixture_id": "mvp-synthetic",
            "fixture_version": "v1",
            "calendar_id": "synthetic-daily-utc-v1",
            "observations": [],
            "initial_marks": [{
                "instrument_id": "SPOT_A", "mark": "100", "available_at": "2026-01-05T16:00:00Z",
                "observation_id": "SPOT-A-INIT", "source_revision": 1,
            }],
            "execution_prices": [{
                "instrument_id": "SPOT_A", "session_open": "2026-01-06T09:00:00Z", "price": "100.5",
                "available_at": "2026-01-06T09:00:00Z", "observation_id": "SPOT-A-OPEN", "source_revision": 1,
            }],
            "settlement_events": [],
        }
        manifest_validator = Draft202012Validator(
            self.schemas["manifest.v1.schema.json"], registry=self.registry, format_checker=FormatChecker()
        )
        dataset_validator = Draft202012Validator(
            self.schemas["dataset.v1.schema.json"], registry=self.registry, format_checker=FormatChecker()
        )
        self.assertEqual(list(manifest_validator.iter_errors(manifest)), [])
        self.assertEqual(list(dataset_validator.iter_errors(dataset)), [])
        manifest["dataset_path"] = "../outside.json"
        self.assertTrue(list(manifest_validator.iter_errors(manifest)))


if __name__ == "__main__":
    unittest.main()
