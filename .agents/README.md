# Agent role guides

These guides describe responsibilities and review expectations for work on FactorsBT. Read the repository-wide [AGENTS.md](../AGENTS.md) first.

## Available roles

| Role | Focus |
| --- | --- |
| [Python SDK](roles/python-sdk.md) | Client experience, API communication, errors, and run tracking |
| [Rust engine](roles/rust-engine.md) | Simulation time, portfolio state, execution, and replay |
| [Backend API](roles/backend-api.md) | Job coordination, external adapters, and service boundaries |
| [Reviewer](roles/reviewer.md) | Correctness, compatibility, verification, and documentation consistency |

## How to use the guides

Select the guide matching the task. Consult adjacent roles for cross-boundary changes, and hand off unresolved interface assumptions explicitly. A single contributor may use several guides.

The files are documentation, not registered skills, agent configurations, or an automatic delegation mechanism. Read them explicitly when relevant; do not assume a tool will discover or execute them. Creating or delegating to agents requires authorization under the active session's rules.

Keep project-wide rules in AGENTS.md. Put product and architecture facts in [docs](../docs/README.md), and link to them from role guides so those facts have one primary home.
