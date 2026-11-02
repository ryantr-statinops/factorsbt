"""Build and run the local Rust backend, call it through Client, then clean up."""

from pathlib import Path
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))

from factorsbt import Client, FactorsBTError


ROOT = Path(__file__).resolve().parents[2]


def choose_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as candidate:
        candidate.bind(("127.0.0.1", 0))
        return candidate.getsockname()[1]


def main() -> int:
    cargo = shutil.which("cargo")
    if cargo is None:
        raise RuntimeError("cargo was not found on PATH")

    with tempfile.TemporaryDirectory(prefix="factorsbt-health-smoke-") as target_dir:
        env = os.environ.copy()
        env["CARGO_TARGET_DIR"] = str(Path(target_dir) / "target")
        build = subprocess.run(
            [
                cargo,
                "build",
                "--offline",
                "--manifest-path",
                str(ROOT / "rust" / "Cargo.toml"),
                "-p",
                "factorsbt-backend",
            ],
            cwd=ROOT,
            env=env,
            capture_output=True,
            text=True,
            check=False,
        )
        if build.returncode:
            sys.stderr.write(build.stdout)
            sys.stderr.write(build.stderr)
            return build.returncode

        binary = Path(target_dir) / "target" / "debug" / "factorsbt-backend.exe"
        port = choose_port()
        env["FACTORSBT_BIND_ADDR"] = f"127.0.0.1:{port}"
        process = subprocess.Popen(
            [str(binary)],
            cwd=ROOT,
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        try:
            client = Client(f"http://127.0.0.1:{port}", timeout=0.5)
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    raise RuntimeError("backend process exited before health became available")
                try:
                    response = client.health()
                    if response.contract_version != "1.0.0":
                        raise RuntimeError("backend returned an unexpected contract version")
                    print("Python SDK reached the Rust backend at GET /healthz")
                    return 0
                except FactorsBTError:
                    time.sleep(0.1)
            raise RuntimeError("backend health endpoint did not respond within 10 seconds")
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except RuntimeError as error:
        print(error, file=sys.stderr)
        raise SystemExit(1)
