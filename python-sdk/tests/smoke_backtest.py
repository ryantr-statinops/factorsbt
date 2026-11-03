"""Run typed SDK configurations through the local Rust backend and HTTP fixture."""

from datetime import datetime, timezone
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))

from factorsbt import (
    Client,
    DataFixtureConfiguration,
    DerivativeScoreTarget,
    InitialPortfolio,
    LinearFutureInstrument,
    ModelConfiguration,
    OptionInstrument,
    OrderIntentsPolicy,
    RunConfiguration,
    ScoresPolicy,
    SimulationConfiguration,
    SpotInstrument,
    TargetsPolicy,
)
from factorsbt.errors import FactorsBTError, HttpStatusError


ROOT = Path(__file__).resolve().parents[2]
INSTRUMENT_IDS = {
    "SPOT_A", "SPOT_B", "FUT_A", "EU_CALL_A", "EU_PUT_A", "AM_CALL_A", "AM_PUT_A"
}
RUN_DATE_START = "2026-01-05T16:00:00Z"
RUN_DATE_END = "2026-01-08T16:00:00Z"
EXPIRY = "2026-02-20T16:00:00Z"


def choose_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as candidate:
        candidate.bind(("127.0.0.1", 0))
        return candidate.getsockname()[1]


def instruments():
    common_spot = dict(
        price_tick="0.01",
        quantity_step="0.001",
        contract_multiplier="1",
        fee_bps="0",
        fee_per_unit="0",
        slippage_bps="0",
    )
    common_derivative = dict(
        price_tick="0.01",
        quantity_step="1",
        contract_multiplier="1",
        fee_bps="0",
        fee_per_unit="0",
        slippage_bps="0",
        initial_margin_per_contract="250",
        maintenance_margin_per_contract="200",
    )
    result = [
        SpotInstrument(instrument_id="SPOT_A", **common_spot),
        SpotInstrument(instrument_id="SPOT_B", **common_spot),
        LinearFutureInstrument(
            instrument_id="FUT_A",
            underlying_id="SPOT_A",
            expiry_at=EXPIRY,
            **common_derivative,
        ),
    ]
    for identifier, option_type, exercise, settlement in (
        ("EU_CALL_A", "call", "european", "cash"),
        ("EU_PUT_A", "put", "european", "cash"),
        ("AM_CALL_A", "call", "american", "physical"),
        ("AM_PUT_A", "put", "american", "physical"),
    ):
        result.append(
            OptionInstrument(
                instrument_id=identifier,
                underlying_id="SPOT_A",
                option_type=option_type,
                strike="100",
                expiry_at=EXPIRY,
                exercise_style=exercise,
                settlement_style=settlement,
                valuation_mode="quote",
                **common_derivative,
            )
        )
    return tuple(result)


class ControlledModelHandler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.0"
    requests = []
    requests_lock = threading.Lock()

    def do_POST(self):
        if self.path != "/infer":
            self.send_error(404)
            return
        try:
            size = int(self.headers["Content-Length"])
            request = json.loads(self.rfile.read(size))
            mode = request["decision_policy"]["output_mode"]
            outputs = {
                "scores": {
                    "scores": [
                        {"instrument_id": "SPOT_A", "score": "3"},
                        {"instrument_id": "EU_CALL_A", "score": "2"},
                        {"instrument_id": "AM_CALL_A", "score": "1"},
                    ]
                },
                "targets": {
                    "spot_weights": [{"instrument_id": "SPOT_A", "weight": "0.25"}],
                    "derivative_contracts": [
                        {"instrument_id": "FUT_A", "contracts": 1},
                        {"instrument_id": "AM_PUT_A", "contracts": -1},
                    ],
                },
                "order_intents": {
                    "orders": [
                        {"instrument_id": "SPOT_B", "order_type": "market", "quantity": "2"},
                        {"instrument_id": "FUT_A", "order_type": "market", "quantity": "-1"},
                        {"instrument_id": "EU_PUT_A", "order_type": "market", "quantity": "1"},
                        {"instrument_id": "AM_CALL_A", "order_type": "market", "quantity": "1"},
                    ]
                },
            }[mode]
            with self.requests_lock:
                self.requests.append(
                    (mode, {row["instrument_id"] for row in request["instruments"]})
                )
            response = {
                key: request[key]
                for key in (
                    "contract_version", "inference_id", "run_id", "scenario_id",
                    "step_id", "snapshot_id", "model_id", "model_version",
                )
            }
            response.update(
                {
                    "responded_at": datetime.now(timezone.utc)
                    .isoformat(timespec="milliseconds")
                    .replace("+00:00", "Z"),
                    "response_type": mode,
                    "outputs": outputs,
                }
            )
            body = json.dumps(response, separators=(",", ":")).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        except Exception:
            self.send_error(500)

    def log_message(self, _format, *_args):
        return


def run_configuration(policy, model_endpoint):
    return RunConfiguration(
        simulation=SimulationConfiguration(RUN_DATE_START, RUN_DATE_END),
        initial_portfolio=InitialPortfolio("100000"),
        instruments=instruments(),
        data=DataFixtureConfiguration("mvp-synthetic", "v1"),
        model=ModelConfiguration(
            "controlled-http-fixture",
            "v1",
            model_endpoint,
            5000,
        ),
        decision_policy=policy,
    )


def configurations(model_endpoint):
    return (
        (
            "scores",
            run_configuration(
                ScoresPolicy(
                    top_k=2,
                    derivative_score_targets=(
                        DerivativeScoreTarget("EU_CALL_A", 1),
                        DerivativeScoreTarget("AM_CALL_A", 1),
                    ),
                ),
                model_endpoint,
            ),
        ),
        (
            "targets",
            run_configuration(TargetsPolicy(), model_endpoint),
        ),
        (
            "order_intents",
            run_configuration(OrderIntentsPolicy(), model_endpoint),
        ),
    )


def start_backend(binary, environment):
    port = choose_port()
    environment["FACTORSBT_BIND_ADDR"] = f"127.0.0.1:{port}"
    process = subprocess.Popen(
        [str(binary)],
        cwd=ROOT,
        env=environment,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    client = Client(f"http://127.0.0.1:{port}", timeout=0.5)
    deadline = time.monotonic() + 15
    try:
        while time.monotonic() < deadline:
            if process.poll() is not None:
                raise RuntimeError("Rust backend exited before its health endpoint became ready")
            try:
                client.health()
                return process, client
            except FactorsBTError:
                time.sleep(0.1)
        raise RuntimeError("Rust backend health endpoint did not respond within 15 seconds")
    except Exception:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)
        raise


def main() -> int:
    cargo = shutil.which("cargo")
    if cargo is None:
        raise RuntimeError("cargo was not found on PATH")

    with tempfile.TemporaryDirectory(prefix="factorsbt-backtest-smoke-") as temp:
        temporary = Path(temp)
        environment = os.environ.copy()
        target_directory = Path(
            environment.get("CARGO_TARGET_DIR", str(temporary / "target"))
        )
        environment["CARGO_TARGET_DIR"] = str(target_directory)
        environment["FACTORSBT_FIXTURE_DIR"] = str(ROOT / "contracts" / "fixtures" / "data")
        environment["FACTORSBT_EVIDENCE_DIR"] = str(temporary / "evidence")
        build = subprocess.run(
            [
                cargo,
                "build",
                "--offline",
                "--manifest-path",
                str(ROOT / "rust" / "Cargo.toml"),
                "-p",
                "factorsbt-backend",
                "--bin",
                "factorsbt-backend",
            ],
            cwd=ROOT / "rust",
            env=environment,
            capture_output=True,
            text=True,
            check=False,
        )
        if build.returncode:
            sys.stderr.write(build.stdout)
            sys.stderr.write(build.stderr)
            return build.returncode

        suffix = ".exe" if os.name == "nt" else ""
        binary = target_directory / "debug" / f"factorsbt-backend{suffix}"
        model_server = ThreadingHTTPServer(("127.0.0.1", 0), ControlledModelHandler)
        model_thread = threading.Thread(target=model_server.serve_forever, daemon=True)
        model_thread.start()
        model_endpoint = f"http://127.0.0.1:{model_server.server_port}/infer"
        backend = None
        try:
            backend, client = start_backend(binary, environment)
            ControlledModelHandler.requests = []
            for expected_mode, config in configurations(model_endpoint):
                try:
                    accepted = client.submit(config)
                except HttpStatusError as error:
                    context = f" {error.details}" if error.details else ""
                    raise RuntimeError(
                        f"{expected_mode} configuration was rejected: "
                        f"{error.code or error.status} {error.message or ''}{context}"
                    ) from error
                terminal = client.wait(accepted.run_id, timeout=30)
                if terminal.status != "succeeded":
                    raise RuntimeError(f"{expected_mode} run did not succeed")
                result = client.get_results(accepted.run_id)
                if result.output_mode != expected_mode or result.counts.fills < 1:
                    raise RuntimeError(
                        f"{expected_mode} run returned an unexpected mode or no fills"
                    )
                if not result.evidence:
                    raise RuntimeError(f"{expected_mode} result has no evidence references")
                print(
                    f"{expected_mode}: run={result.run_id} fills={result.counts.fills} "
                    f"positions={len(result.portfolio.positions)} evidence={len(result.evidence)}"
                )

            observed_modes = {mode for mode, _ids in ControlledModelHandler.requests}
            if observed_modes != {"scores", "targets", "order_intents"}:
                raise RuntimeError("controlled fixture did not receive all three output modes")
            if not ControlledModelHandler.requests:
                raise RuntimeError("controlled model fixture received no inference requests")
            if any(ids != INSTRUMENT_IDS for _mode, ids in ControlledModelHandler.requests):
                raise RuntimeError("model requests did not carry the full seven-instrument catalog")
            print("Python typed SDK completed all three run modes through Rust and the local model fixture.")
        finally:
            if backend is not None:
                backend.terminate()
                try:
                    backend.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    backend.kill()
                    backend.wait(timeout=5)
            model_server.shutdown()
            model_server.server_close()
            model_thread.join(timeout=5)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except RuntimeError as error:
        print(error, file=sys.stderr)
        raise SystemExit(1)
