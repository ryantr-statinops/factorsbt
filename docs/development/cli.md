# FactorsBT CLI

**Status:** Implemented for local backtest and recorded replay workflows. This CLI is a thin wrapper over the Python SDK; it does not start the Rust backend or model fixture.

For a first runnable demo, use the [quickstart](quickstart.md): its installed CLI smoke starts controlled services and cleans them up. The commands below are for your own separately running backend and compatible stateless model service. Backend launch/settings and troubleshooting are in [local operations](local-operations.md). The package has no standalone model-fixture launcher.

## Install

Use Python 3.13 or later. From the repository root in PowerShell, install the source package into an isolated environment under the temporary directory:

~~~powershell
$cliEnv = Join-Path $env:TEMP 'factorsbt-cli-venv'
python -m venv $cliEnv
& (Join-Path $cliEnv 'Scripts\python.exe') -m pip install .\python-sdk
$cliExe = Join-Path $cliEnv 'Scripts\factorsbt.exe'
& $cliExe --help
~~~

The package has no runtime dependency beyond the Python standard library. The install uses setuptools and exposes the `factorsbt` command.
If `$cliEnv` already exists and contains work you need, choose a different temporary directory before creating the environment.

## Backend address

Start the Rust backend separately. The CLI accepts HTTP loopback addresses only: `localhost`, `127.0.0.1`, or `[::1]`, optionally with a port. Address selection is `--base-url`, then `FACTORSBT_BASE_URL`, then `http://127.0.0.1:8080`. Put the global option before the command:

~~~powershell
$env:FACTORSBT_BASE_URL = 'http://127.0.0.1:8080'
& $cliExe health
& $cliExe --base-url http://localhost:8080 health
~~~

Each HTTP request uses the SDK's 5 second timeout. `run wait` and `replay wait` default to a 60 second local polling deadline and a 0.25 second polling interval. `--timeout` must be a positive finite number. Expiration and Ctrl+C stop local waiting only; they do not cancel backend work.

## Submit and inspect a run

Copy the shared valid configuration and adjust its local model endpoint to the model fixture you started alongside the backend:

~~~powershell
$cliConfig = Join-Path $env:TEMP 'factorsbt-run.json'
Copy-Item contracts/examples/instruments/minimal-valid-spot-run.json $cliConfig
& $cliExe run submit $cliConfig
~~~

The response includes `submission_id`, `run_id`, and the queued status. Use the IDs to recover or follow the job:

~~~powershell
& $cliExe run lookup <submission-id>
& $cliExe run status <run-id>
& $cliExe run wait <run-id> --timeout 60
& $cliExe run result <run-id>
~~~

If a submit acknowledgment is lost, the CLI prints the ID and lookup command to stderr. It never retries the POST automatically. Save the run ID from the acknowledgment; lookup by submission ID returns the associated run ID.

Keep the submission ID before recovery and use the same running backend process. Deduplication/status metadata does not survive restart. The [submission contract](../interfaces/mvp-submissions.md) defines when a caller may explicitly repeat the same body/key; a new key creates independent work.

## Verify with recorded replay

After a successful or failed run has sealed its evidence, submit a recorded replay. Replay reads the recorded model exchange and does not invoke fresh inference:

~~~powershell
& $cliExe replay submit <source-run-id>
& $cliExe replay status <replay-id>
& $cliExe replay wait <replay-id> --timeout 60
& $cliExe replay report <replay-id>
~~~

Replay keeps the Phase C compatibility checks: the same executable digest and contract version are required. A report with `matched: false` is still a successfully retrieved report and exits with code 0; inspect `first_mismatch` to see where it diverged. A wait deadline does not cancel replay; if wait exits with code 4, use the retained replay ID with `replay status` or `replay report` later.

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

Run evidence enumeration and verified artifact reads are available through the [Python SDK](python-sdk.md) and [HTTP API](../interfaces/results-evidence.md), despite being absent from this CLI. Use the SDK for [comparison](comparison.md) and the separate [research interface](../interfaces/research-jobs.md).
