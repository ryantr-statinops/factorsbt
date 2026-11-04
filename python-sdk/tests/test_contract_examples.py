import json
import hashlib
from pathlib import Path
import unittest

from jsonschema import Draft202012Validator, FormatChecker
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_DIR = ROOT / "contracts" / "schemas"
FIXTURE_SCHEMA_DIR = ROOT / "contracts" / "fixtures" / "schemas"
FIXTURE_DATA_DIR = ROOT / "contracts" / "fixtures" / "data"
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

    def test_replay_contract_schemas_and_openapi_routes_are_wired(self):
        openapi = load_document(ROOT / "contracts" / "openapi.json")
        paths = openapi["paths"]
        self.assertIn("/v1/replays", paths)
        self.assertIn("post", paths["/v1/replays"])
        self.assertIn("/v1/replays/{replay_id}", paths)
        self.assertIn("/v1/replays/{replay_id}/report", paths)

        references = []

        def collect_refs(value):
            if isinstance(value, dict):
                reference = value.get("$ref")
                if isinstance(reference, str) and reference.startswith("schemas/"):
                    references.append(reference.split("#", 1)[0])
                for child in value.values():
                    collect_refs(child)
            elif isinstance(value, list):
                for child in value:
                    collect_refs(child)

        collect_refs(openapi)
        self.assertTrue(references)
        for reference in references:
            with self.subTest(reference=reference):
                self.assertTrue((ROOT / "contracts" / reference).is_file())

        request = {"contract_version": "1.0.0", "source_run_id": "a4ab8716-3d8c-42d7-857c-83e94df28ff6"}
        validator = Draft202012Validator(
            self.schemas["replay-request.schema.json"],
            registry=self.registry,
            format_checker=FormatChecker(),
        )
        self.assertEqual(list(validator.iter_errors(request)), [])
        invalid = {**request, "unexpected": True}
        self.assertTrue(list(validator.iter_errors(invalid)))

    def test_internal_fixture_v1_1_schemas_accept_controlled_shape_and_reject_traversal(self):
        manifest = {
            "format_version": "1.1.0",
            "fixture_id": "mvp-synthetic",
            "fixture_version": "v1",
            "dataset_path": "datasets/baseline.json",
            "dataset_sha256": "a" * 64,
        }
        dataset = {
            "format_version": "1.1.0",
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
        dataset["settlement_events"] = [{
            "fixture_event_id": "ASSIGN-1",
            "instrument_id": "AM_CALL_A",
            "event_type": "american_assignment",
            "event_time": "2026-01-07T16:00:00Z",
            "available_at": "2026-01-07T16:00:00Z",
            "contracts": 1,
            "underlying_price": "102",
            "observation_id": "SPOT-A-CLOSE",
            "source_revision": 1,
        }]
        self.assertEqual(list(dataset_validator.iter_errors(dataset)), [])
        dataset["settlement_events"][0]["contracts"] = 0
        self.assertTrue(list(dataset_validator.iter_errors(dataset)))
        del dataset["settlement_events"][0]["contracts"]
        self.assertTrue(list(dataset_validator.iter_errors(dataset)))
        manifest["dataset_path"] = "../outside.json"
        self.assertTrue(list(manifest_validator.iter_errors(manifest)))

    def test_versioned_fixture_files_match_schemas_and_content_digests(self):
        manifest_validator = Draft202012Validator(
            self.schemas["manifest.v1.schema.json"], registry=self.registry, format_checker=FormatChecker()
        )
        dataset_validator = Draft202012Validator(
            self.schemas["dataset.v1.schema.json"], registry=self.registry, format_checker=FormatChecker()
        )
        manifests = sorted(FIXTURE_DATA_DIR.glob("*/manifest.json"))
        self.assertGreaterEqual(len(manifests), 7)
        for manifest_path in manifests:
            with self.subTest(fixture=manifest_path.parent.name):
                manifest = load_document(manifest_path)
                self.assertEqual(list(manifest_validator.iter_errors(manifest)), [])
                dataset_path = manifest_path.parent / manifest["dataset_path"]
                dataset_bytes = dataset_path.read_bytes()
                self.assertEqual(hashlib.sha256(dataset_bytes).hexdigest(), manifest["dataset_sha256"])
                dataset = json.loads(dataset_bytes)
                self.assertEqual(list(dataset_validator.iter_errors(dataset)), [])
                self.assertEqual(dataset["fixture_id"], manifest["fixture_id"])
                self.assertEqual(dataset["fixture_version"], manifest["fixture_version"])

        baseline = load_document(FIXTURE_DATA_DIR / "mvp-synthetic" / "dataset.json")
        self.assertEqual({row["instrument_id"] for row in baseline["initial_marks"]},
                         {"SPOT_A", "SPOT_B", "FUT_A", "EU_CALL_A", "EU_PUT_A", "AM_CALL_A", "AM_PUT_A"})
        self.assertTrue(any(row["observation_id"] == "NEWS-TIME01" and row["revision"] == 2
                            and row["available_at"] == "2026-01-07T17:00:00Z" for row in baseline["observations"]))
        jan6_spot_open = next(row for row in baseline["execution_prices"]
                              if row["instrument_id"] == "SPOT_A" and row["session_open"] == "2026-01-06T09:00:00Z")
        self.assertEqual(jan6_spot_open["price"], "100.50")
        self.assertTrue(load_document(FIXTURE_DATA_DIR / "mvp-settle-01" / "dataset.json")["settlement_events"])
        self.assertFalse(load_document(FIXTURE_DATA_DIR / "mvp-settle-02-itm" / "dataset.json")["settlement_events"])
        risk = load_document(FIXTURE_DATA_DIR / "mvp-risk-02" / "dataset.json")
        future_seed = next(row for row in risk["initial_marks"] if row["instrument_id"] == "FUT_A")
        recovery_open = next(row for row in risk["execution_prices"]
                             if row["instrument_id"] == "FUT_A" and row["session_open"] == "2026-01-08T09:00:00Z")
        self.assertEqual(future_seed["mark"], "20")
        self.assertEqual(recovery_open["price"], "20")
        failed_recovery = load_document(FIXTURE_DATA_DIR / "mvp-risk-02-fail" / "dataset.json")
        failed_open = next(row for row in failed_recovery["execution_prices"]
                           if row["instrument_id"] == "FUT_A" and row["session_open"] == "2026-01-08T09:00:00Z")
        self.assertEqual(failed_open["price"], "18")
        option_pricing = load_document(FIXTURE_DATA_DIR / "mvp-opt-01" / "dataset.json")
        rates = [row["payload"]["risk_free_rate"] for row in option_pricing["observations"]
                 if row["kind"] == "option_pricing_input"]
        self.assertEqual(set(rates), {"0.05"})


if __name__ == "__main__":
    unittest.main()
