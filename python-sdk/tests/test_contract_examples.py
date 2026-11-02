import json
from pathlib import Path
import unittest

from jsonschema import Draft202012Validator, FormatChecker
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_DIR = ROOT / "contracts" / "schemas"
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


if __name__ == "__main__":
    unittest.main()
