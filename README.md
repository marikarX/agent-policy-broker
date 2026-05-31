# Agent Policy Broker

Agent Policy Broker is an open-source CLI and service pattern for delivering task-specific, versioned instructions to coding agents.

Instead of placing large, duplicated instruction files in every repository, teams can keep a small bootstrap file such as `AGENTS.md`, `CLAUDE.md`, Cursor rules, or Copilot instructions that tells the coding agent to request the right policy bundle for the current task.

The broker selects policies based on structured intent and repository context:

- repository and branch
- files and directories involved
- task type
- language and framework
- package manager
- risk area, such as auth, payments, migrations, public APIs, generated code, or security-sensitive paths
- required validation commands

The goal is to help coding agents follow engineering standards, security rules, test requirements, and domain conventions without overloading every task with irrelevant context.

## Status

This repository is in the documentation and design phase. The current focus is defining the open-source core:

- policy schema
- CLI behavior
- agent bootstrap patterns
- deterministic policy selection
- examples for common coding-agent workflows

## Why this exists

Coding agents work better when they receive precise instructions. Static repository instruction files are useful, but they become hard to maintain when policies differ by language, framework, directory, domain, or task risk.

Agent Policy Broker proposes a lightweight control plane:

```text
AGENTS.md / CLAUDE.md / editor rules
        -> run agent-policy get
        -> broker selects relevant policies
        -> agent receives compact instructions
        -> agent applies instructions and reports policy version
```

## Non-goals

Agent Policy Broker is not intended to be:

- a general-purpose AI agent orchestrator
- a replacement for coding agents
- a vector database by default
- a prompt dump or company handbook retriever
- a substitute for CI, tests, code review, or security review

The broker should be deterministic, inspectable, and easy to run locally.

## Example

A repository-level `AGENTS.md` can contain:

```md
# Dynamic coding-agent instructions

Before editing code, run:

```bash
agent-policy get --task "$USER_TASK" --repo .
```

If files are known, include them:

```bash
agent-policy get --task "$USER_TASK" --repo . --files src/payments/refunds.ts tests/payments/refunds.test.ts
```

Follow the returned instructions unless they conflict with higher-priority user, system, or repository instructions.

If policy lookup fails, make the smallest safe change and report that dynamic policy lookup was unavailable.
```

The broker may return:

```json
{
  "status": "ok",
  "policy_version": "2026-05-31.1",
  "instructions": [
    "Use existing MoneyAmount and Currency types; do not introduce raw floating-point money handling.",
    "Preserve refund idempotency semantics.",
    "Add tests for success, provider failure, retry, and duplicate refund request.",
    "Do not edit generated OpenAPI files directly."
  ],
  "required_checks": [
    "npm run lint",
    "npm run typecheck",
    "npm test -- tests/payments"
  ],
  "sources": [
    "repo.billing-api.v3",
    "domain.payments.v7",
    "lang.typescript.v4"
  ]
}
```

## Documentation

- [Getting started](docs/getting-started.md)
- [Architecture](docs/architecture.md)
- [Policy schema](docs/policy-schema.md)
- [Agent integration](docs/agent-integration.md)
- [Roadmap](docs/roadmap.md)
- [Privacy](PRIVACY.md)
- [Contributing](CONTRIBUTING.md)

## Repository layout

```text
.
├── README.md
├── CONTRIBUTING.md
├── LICENSE
├── PRIVACY.md
├── docs/
│   ├── architecture.md
│   ├── agent-integration.md
│   ├── getting-started.md
│   ├── policy-schema.md
│   └── roadmap.md
└── examples/
    ├── AGENTS.md
    └── policies/
        ├── payments.yaml
        └── typescript.yaml
```

## Design principles

1. **Deterministic first**: policy selection should be explainable and reproducible.
2. **Small outputs**: return only the instructions relevant to the current task.
3. **Policy as code**: policies should be versioned, reviewed, and owned.
4. **Local-first**: the open-source core should work without a hosted service.
5. **Vendor-neutral**: support Codex, Claude Code, Copilot, Cursor, and other coding agents through simple command execution first.
6. **Auditable**: every returned instruction should be traceable to a source policy.

## License

Agent Policy Broker is licensed under the [MIT License](LICENSE).
