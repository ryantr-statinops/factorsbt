"""Exercise SDK submission and run failures against the local Rust backend."""

from dataclasses import replace
from http.client import HTTPConnection
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
from urllib.parse import urlsplit

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))

import smoke_backtest as smoke
from factorsbt import (
    Client,
    OrderIntentsPolicy,
    RunFailedError,
    ScoresPolicy,
    SubmissionAcknowledgementUnknown,
    TargetsPolicy,
    WaitTimeout,
)
from factorsbt.errors import HttpStatusError


ROOT = smoke.ROOT
UNCERTAIN_SUBMISSION_ID = "11111111-1111-4111-8111-111111111111"


class AckDropProxyHandler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.0"
    backend_address = None
    drop_next_post_ack = False
    lock = threading.Lock()

    def do_POST(self):
        self._forward()

    def do_GET(self):
        self._forward()

    def _forward(self):
        size = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(size) if size else None
        parsed = urlsplit(f"http://{self.backend_address[0]}:{self.backend_address[1]}{self.path}")
        headers = {
            key: value
            for key, value in self.headers.items()
            if key.lower() in {"accept", "content-type", "idempotency-key"}
        }
        connection = HTTPConnection(
            self.backend_address[0], self.backend_address[1], timeout=5
        )
        try:
            connection.request(self.command, parsed.path, body=body, headers=headers)
            response = connection.getresponse()
            response_body = response.read()
            if self.command == "POST" and parsed.path == "/v1/runs":
                with self.lock:
                    drop_ack = self.drop_next_post_ack
                    type(self).drop_next_post_ack = False
                if drop_ack:
                    self.close_connection = True
                    try:
                        self.connection.shutdown(socket.SHUT_RDWR)
                    except OSError:
                        pass
                    return
            self.send_response(response.status)
            for key, value in response.getheaders():
                if key.lower() not in {"connection", "content-length", "transfer-encoding"}:
                    self.send_header(key, value)
            self.send_header("Content-Length", str(len(response_body)))
            self.end_headers()
            self.wfile.write(response_body)
        finally:
            connection.close()

    def log_message(self, _format, *_args):
        return


def build_backend(cargo, environment, temporary):
    target_directory = Path(
        environment.get("CARGO_TARGET_DIR", str(Path(temporary) / "target"))
    )
    environment["CARGO_TARGET_DIR"] = str(target_directory)
    environment["FACTORSBT_FIXTURE_DIR"] = str(ROOT / "contracts" / "fixtures" / "data")
    environment["FACTORSBT_EVIDENCE_DIR"] = str(Path(temporary) / "evidence")
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
        raise RuntimeError(f"backend build failed with exit code {build.returncode}")
    suffix = ".exe" if os.name == "nt" else ""
    return target_directory / "debug" / f"factorsbt-backend{suffix}"


def with_model(config, model_id):
    return replace(
        config,
        model=replace(config.model, model_id=model_id),
    )


def main() -> int:
    cargo = shutil.which("cargo")
    if cargo is None:
        raise RuntimeError("cargo was not found on PATH")

    with tempfile.TemporaryDirectory(prefix="factorsbt-sdk-failures-") as temp:
        environment = os.environ.copy()
        binary = build_backend(cargo, environment, temp)
        model_server = ThreadingHTTPServer(
            ("127.0.0.1", 0), smoke.ControlledModelHandler
        )
        smoke.ControlledModelHandler.requests = []
        smoke.ControlledModelHandler.failing_model_ids = {"failing-model"}
        smoke.ControlledModelHandler.delayed_model_ids = {"slow-model": 0.2}
        model_thread = threading.Thread(target=model_server.serve_forever, daemon=True)
        model_thread.start()
        model_endpoint = f"http://127.0.0.1:{model_server.server_port}/infer"
        backend = None
        proxy = None
        proxy_thread = None
        try:
            backend, backend_client = smoke.start_backend(binary, environment)
            backend_port = urlsplit(backend_client.base_url).port
            proxy = ThreadingHTTPServer(("127.0.0.1", 0), AckDropProxyHandler)
            AckDropProxyHandler.backend_address = ("127.0.0.1", backend_port)
            AckDropProxyHandler.drop_next_post_ack = True
            proxy_thread = threading.Thread(target=proxy.serve_forever, daemon=True)
            proxy_thread.start()
            proxy_client = Client(f"http://127.0.0.1:{proxy.server_port}", timeout=3)

            scores_config = smoke.run_configuration(ScoresPolicy(1), model_endpoint)
            try:
                proxy_client.submit(
                    scores_config, submission_id=UNCERTAIN_SUBMISSION_ID
                )
            except SubmissionAcknowledgementUnknown as error:
                if error.submission_id != UNCERTAIN_SUBMISSION_ID:
                    raise RuntimeError("lost acknowledgment did not retain its submission ID")
            else:
                raise RuntimeError("proxy did not drop the first submission acknowledgment")

            recovered = proxy_client.get_submission(UNCERTAIN_SUBMISSION_ID)
            if recovered.submission_id != UNCERTAIN_SUBMISSION_ID:
                raise RuntimeError("uncertain submission could not be resolved by key")

            changed_config = smoke.run_configuration(TargetsPolicy(), model_endpoint)
            try:
                proxy_client.submit(
                    changed_config, submission_id=UNCERTAIN_SUBMISSION_ID
                )
            except HttpStatusError as error:
                if error.status != 409 or error.code != "SUBMISSION_ID_REUSED":
                    raise RuntimeError("changed configuration returned an unexpected conflict")
            else:
                raise RuntimeError("backend accepted a changed body under the same key")

            recovered_run = proxy_client.wait(recovered.run_id, timeout=30)
            if recovered_run.status != "succeeded":
                raise RuntimeError("run with dropped acknowledgment did not finish successfully")

            failing_config = with_model(
                smoke.run_configuration(ScoresPolicy(1), model_endpoint), "failing-model"
            )
            failed_ack = backend_client.submit(failing_config)
            try:
                backend_client.wait(failed_ack.run_id, timeout=15)
            except RunFailedError as error:
                if not error.failure or not error.failure.code:
                    raise RuntimeError("model failure did not retain typed failure context")
            else:
                raise RuntimeError("model HTTP 503 did not fail the run")

            slow_config = with_model(
                smoke.run_configuration(ScoresPolicy(1), model_endpoint), "slow-model"
            )
            slow_ack = backend_client.submit(slow_config)
            try:
                backend_client.wait(
                    slow_ack.run_id, timeout=0.03, poll_interval=0.005
                )
            except WaitTimeout as error:
                if error.run_id != slow_ack.run_id:
                    raise RuntimeError("local wait timeout lost the run ID")
            else:
                raise RuntimeError("short local wait unexpectedly reached terminal state")
            final_status = backend_client.wait(slow_ack.run_id, timeout=15)
            if final_status.status != "succeeded":
                raise RuntimeError("backend work did not continue after local wait timeout")
            print(
                "SDK failure paths verified: lost acknowledgment lookup, "
                "idempotency conflict, model failure, and non-cancelling local wait timeout."
            )
        finally:
            if proxy is not None:
                proxy.shutdown()
                proxy.server_close()
            if proxy_thread is not None:
                proxy_thread.join(timeout=5)
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
