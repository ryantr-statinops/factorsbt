# FactorsBT CLI

**Status:** Implemented for local backtest and recorded replay workflows. This CLI is a thin wrapper over the Python SDK; it does not start the Rust backend or model fixture.

## Install

Use Python 3.13 or later and install the source package into the active virtual environment:

~~~powershell
python -m venv .venv
& .\.venv\Scripts\Activate.ps1
python -m pip install .\python-sdk
factorsbt --help
~~~

The package has no runtime dependency beyond the Python standard library. The install uses setuptools and exposes the `factorsbt` command.

## Backend address

Start the Rust backend separately. The CLI accepts HTTP loopback addresses only: `localhost`, `127.0.0.1`, or `[::1]`, optionally with a port. Address selection is `--base-url`, then `FACTORSBT_BASE_URL`, then `http://127.0.0.1:8080`. Put the global option before the command:

~~~powershell
$env:FACTORSBT_BASE_URL = 'http://127.0.0.1:8080'
factorsbt health
factorsbt --base-url http://localhost:8080 health
~~~

Each HTTP request uses the SDK's 5 second timeout. `run wait` and `replay wait` default to a 60 second local polling deadline and a 0.25 second polling interval. `--timeout` must be a positive finite number. Expiration and Ctrl+C stop local waiting only; they do not cancel backend work.

## Submit and inspect a run

Copy the shared valid configuration and adjust its local model endpoint to the model fixture you started alongside the backend:

~~~powershell
Copy-Item contracts/examples/instruments/minimal-valid-spot-run.json .\run.json
factorsbt run submit .\run.json
~~~

The response includes `submission_id`, `run_id`, and the queued status. Use the IDs to recover or follow the job:

~~~powershell
factorsbt run lookup <submission-id>
factorsbt run status <run-id>
factorsbt run wait <run-id> --timeout 60
factorsbt run result <run-id>
~~~

If a submit acknowledgment is lost, the CLI prints the ID and lookup command to stderr. It never retries the POST automatically. Save the run ID from the acknowledgment; lookup by submission ID returns the associated run ID.

## Verify with recorded replay

After a successful or failed run has sealed its evidence, submit a recorded replay. Replay reads the recorded model exchange and does not invoke fresh inference:

~~~powershell
factorsbt replay submit <source-run-id>
factorsbt replay status <replay-id>
factorsbt replay wait <replay-id> --timeout 60
factorsbt replay report <replay-id>
~~~

Replay keeps the Phase C compatibility checks: the same executable digest and contract version are required. A report with `matched: false` is still a successfully retrieved report and exits with code 0; inspect `first_mismatch` to see where it diverged.

## Output and exit codes

Successful commands print one compact UTF-8 JSON object to stdout. Decimal values remain strings, UTC timestamps retain their UTC representation, and evidence references are arrays. `--help` prints text. Errors print a JSON object to stderr and leave stdout empty.

| Exit code | Meaning |
| --- | --- |
| `0` | Command succeeded; status/report reads may describe a failed job or mismatch. |
| `1` | HTTP, transport, protocol, or other backend error. |
| `2` | Invalid arguments, endpoint, or local JSON configuration. |
| `3` | A `wait` command observed a failed run or replay. |
| `4` | Local wait deadline expired; backend work continues. |
| `5` | Submission acknowledgment is unknown; use the printed lookup command. |
| `130` | Ctrl+C interrupted the local CLI. |

The CLI does not implement scenario comparison, artifact downloads, research jobs, an interactive console, or PyPI distribution.
