# Getting started

Agent Policy Broker is an early local-first Rust CLI and service prototype. This guide describes the implemented MVP workflow.

## Core workflow

```text
1. A coding agent reads AGENTS.md, CLAUDE.md, Cursor rules, or Copilot instructions.
2. The instruction file tells the agent to run `agent-policy get` before editing code.
3. The CLI collects task and repository context.
4. The broker selects relevant policy modules.
5. The CLI prints a compact instruction bundle.
6. The coding agent follows the returned instructions and reports the policy version.
```

## CLI

Compile an instruction bundle:

```bash
agent-policy get --repo . --task "fix refund retry handling"
```

When files are known:

```bash
agent-policy get \
  --repo . \
  --task "fix refund retry handling" \
  --files src/payments/refunds.ts tests/payments/refunds.test.ts
```

Expected output:

```json
{
  "status": "ok",
  "policy_version": "2026-05-31.1",
  "summary": "Instructions for a TypeScript payment change.",
  "instructions": [
    "Preserve refund idempotency semantics.",
    "Add tests for provider retry and duplicate refund request.",
    "Run required check IDs only when they resolve through trusted configuration."
  ],
  "required_checks": [
    { "id": "typescript.lint", "source": "lang.typescript.v4" },
    { "id": "payments.unit_tests", "source": "domain.payments.v7" }
  ],
  "sources": [
    "domain.payments.v7",
    "lang.typescript.v4"
  ]
}
```

## Minimal policy directory

A repository or organization can keep policy files in a directory such as:

```text
.agent-policy/
├── policies/
│   ├── typescript.yaml
│   ├── testing.yaml
│   └── payments.yaml
└── config.yaml
```

## Example bootstrap file

See [`../examples/AGENTS.md`](../examples/AGENTS.md) for a starter `AGENTS.md`.

## MVP checklist

The first implementation should be able to:

- load local YAML policy files
- accept task intent through CLI flags
- detect basic repository metadata
- match policies by repo, path, language, framework, task type, and risk flag
- return compact JSON or Markdown instructions
- include source policy IDs and versions
- fail safely with a clear message

JSON intent input is not implemented in the MVP.

## Fallback behavior

If policy lookup fails, the coding agent should:

1. make the smallest safe change;
2. avoid risky areas unless explicitly instructed;
3. inspect nearby code and tests;
4. run the narrowest relevant checks;
5. report that dynamic policy lookup was unavailable.
