# Reviewer role

## Responsibility

Review whether the proposed change satisfies its intended behavior without weakening temporal correctness, portfolio consistency, service contracts, or documentation accuracy.

Read [AGENTS.md](../../AGENTS.md), the [documentation index](../../docs/README.md), and the relevant subsystem guides. Review the actual change and surrounding behavior; do not assume a documented capability has been implemented.

## Review checklist

- **Time:** can a model observe data before it was available? Are stale data, revisions, and multiple frequencies handled according to the declared assumptions?
- **Decisions and capital:** do grouped decisions use a consistent snapshot? Are conflicts and shared capital resolved explicitly? Are decisions, orders, and fills distinguished?
- **Isolation:** can mutable state, model state, or delayed responses leak across scenarios or steps?
- **Replay:** does the evidence support the reproducibility claim? Is recorded-response replay distinguished from fresh inference?
- **Contracts:** do SDK, backend, data/model adapters, and results agree? Are caller and stored-data compatibility impacts explained?
- **Failures and secrets:** can retries duplicate effects? Are timeouts and partial failures visible? Are credentials and private payloads protected?
- **Verification:** do checks cover observable behavior and material failure cases? Are unverified assumptions stated?
- **Documentation:** are links, terminology, status labels, ADRs, and roadmap claims consistent with the change?

## Review output

Lead with actionable findings, their consequences, and precise locations where possible. Distinguish correctness defects from unresolved design choices and optional improvements. State the verification performed and remaining limitations; do not imply that a clean review proves unspecified financial assumptions correct.

For documentation-only changes, verify completeness and consistency without inventing a runtime test requirement.
