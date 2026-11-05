"""Install the SDK and exercise the installed CLI through Rust and recorded replay."""

from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import time

import smoke_backtest as support


ROOT = support.ROOT


def cli_call(executable: Path, arguments: list[str], environment: dict[str, str], *, expected: int = 0):
    result = subprocess.run(
        [str(executable), *arguments],
        cwd=environment["FACTORSBT_CLI_WORKDIR"],
        env=environment,
        capture_output=True,
        text=True,
        timeout=90,
        check=False,
    )
    if result.returncode != expected:
        raise RuntimeError(
            f"factorsbt {' '.join(arguments)} exited {result.returncode}, expected {expected}: "
            f"stdout={result.stdout!r}, stderr={result.stderr!r}"
        )
    if expected == 0:
        if result.stderr:
            raise RuntimeError(f"successful factorsbt command wrote stderr: {result.stderr}")
        try:
            return json.loads(result.stdout)
        except json.JSONDecodeError as exc:
            raise RuntimeError(f"factorsbt did not return one JSON object: {result.stdout!r}") from exc
    if result.stdout:
        raise RuntimeError(f"failed factorsbt command wrote stdout: {result.stdout!r}")
    return json.loads(result.stderr)


def main() -> int:
    cargo = support.shutil.which("cargo")
    if cargo is None:
        raise RuntimeError("cargo was not found on PATH")

    with tempfile.TemporaryDirectory(prefix="factorsbt-cli-smoke-") as temp:
        temporary = Path(temp)
        target = Path(os.environ.get("CARGO_TARGET_DIR", str(temporary / "target")))
        environment = os.environ.copy()
        environment["CARGO_TARGET_DIR"] = str(target)
        environment["FACTORSBT_FIXTURE_DIR"] = str(ROOT / "contracts" / "fixtures" / "data")
        environment["FACTORSBT_EVIDENCE_DIR"] = str(temporary / "evidence")
        build = subprocess.run(
            [cargo, "build", "--offline", "--manifest-path", str(ROOT / "rust" / "Cargo.toml"), "-p", "factorsbt-backend", "--bin", "factorsbt-backend"],
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

        venv = temporary / "venv"
        create_env = subprocess.run([sys.executable, "-m", "venv", str(venv)], capture_output=True, text=True)
        if create_env.returncode:
            raise RuntimeError(f"could not create clean Python environment: {create_env.stderr}")
        python = venv / "Scripts" / "python.exe" if os.name == "nt" else venv / "bin" / "python"
        install = subprocess.run(
            [str(python), "-m", "pip", "install", "--disable-pip-version-check", str(ROOT / "python-sdk")],
            cwd=temporary,
            capture_output=True,
            text=True,
            check=False,
            timeout=180,
        )
        if install.returncode:
            sys.stderr.write(install.stdout)
            sys.stderr.write(install.stderr)
            return install.returncode
        executable = venv / "Scripts" / "factorsbt.exe" if os.name == "nt" else venv / "bin" / "factorsbt"
        if not executable.is_file():
            raise RuntimeError("pip installation did not create the factorsbt console entry point")

        environment["PYTHONPATH"] = ""
        environment.pop("FACTORSBT_BASE_URL", None)
        environment["FACTORSBT_CLI_WORKDIR"] = str(temporary)
        model_server = support.ThreadingHTTPServer(("127.0.0.1", 0), support.ControlledModelHandler)
        model_thread = threading.Thread(target=model_server.serve_forever, daemon=True)
        model_thread.start()
        backend = None
        try:
            model_url = f"http://127.0.0.1:{model_server.server_port}/infer"
            configuration = support.configurations(model_url)[0][1]
            config_path = temporary / "run.json"
            config_path.write_text(json.dumps(configuration.to_wire(), ensure_ascii=False), encoding="utf-8")
            backend, client = support.start_backend(
                target / "debug" / ("factorsbt-backend.exe" if os.name == "nt" else "factorsbt-backend"),
                environment,
            )
            environment["FACTORSBT_BASE_URL"] = client.base_url
            health = cli_call(executable, ["health"], environment)
            if health.get("contract_version") != "1.0.0":
                raise RuntimeError("installed CLI health did not report the expected contract")

            submission_id = "00000000-0000-4000-8000-000000000101"
            accepted = cli_call(executable, ["run", "submit", str(config_path), "--submission-id", submission_id], environment)
            if accepted.get("submission_id") != submission_id:
                raise RuntimeError("CLI run submit did not retain the requested submission ID")
            lookup = cli_call(executable, ["run", "lookup", submission_id], environment)
            if lookup.get("run_id") != accepted.get("run_id"):
                raise RuntimeError("CLI submission lookup returned a different run ID")
            run_id = accepted["run_id"]
            terminal = cli_call(executable, ["run", "wait", run_id, "--timeout", "30"], environment)
            result = cli_call(executable, ["run", "result", run_id], environment)
            if terminal["status"] != "succeeded" or result["status"] != "succeeded" or not result["evidence"]:
                raise RuntimeError("installed CLI run flow did not return a successful evidence-backed result")
            print(f"CLI run: submission={submission_id} run={run_id} evidence={len(result['evidence'])}")

            model_server.shutdown()
            model_server.server_close()
            model_thread.join(timeout=5)
            replay_id = "00000000-0000-4000-8000-000000000102"
            replay = cli_call(executable, ["replay", "submit", run_id, "--replay-id", replay_id], environment)
            if replay.get("replay_id") != replay_id:
                raise RuntimeError("CLI replay submit did not retain the requested replay ID")
            replay_status = cli_call(executable, ["replay", "wait", replay_id, "--timeout", "30"], environment)
            report = cli_call(executable, ["replay", "report", replay_id], environment)
            if replay_status["status"] != "succeeded" or report["matched"] is not True:
                raise RuntimeError("installed CLI recorded replay did not match its source")
            print(f"CLI replay: replay={replay_id} matched=true (model fixture offline)")
        finally:
            support.stop_backend(backend)
            if model_thread.is_alive():
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
