# Agent Policy Broker

Agent Policy Broker is an open-source CLI and service pattern for delivering concise, task-specific, versioned instructions to coding agents.

Instead of placing large, duplicated instruction files in every repository, teams can keep a small bootstrap file such as `AGENTS.md`, `CLAUDE.md`, Cursor rules, or Copilot instructions that tells the coding agent to request the right policy bundle for the current task.

The broker is designed as a **context-budgeting policy engine**. It retrieves broadly, ranks aggressively, and returns only the instructions that matter now.

The broker can select policies based on structured intent and repository context:

- repository and branch
- files and directories involved
- task type
- language and framework
- package manager
- risk area, such as auth, payments, migrations, public APIs, generated code, or security-sensitive paths
- existing path-scoped instruction files such as nested `AGENTS.md`, `CLAUDE.md`, Cursor rules, and Copilot instructions
- semantically relevant docs, review comments, postmortems, and policy snippets
- required validation commands

The goal is to help coding agents follow engineering standards, security rules, test requirements, and domain conventions without overloading every task with irrelevant context.

## Status

This repository is in the documentation and design phase. The current focus is defining the open-source core:

- policy schema
- CLI behavior
- configuration model
- Rust implementation stack
- agent bootstrap patterns
- layered instruction discovery for nested repo guidance
- inspection and migration of existing repo instructions
- hybrid retrieval and context budgeting
- deterministic policy selection
- instruction bundle compilation
- registry mode for shared policy repositories
- storage and indexing model for Git-backed policies and derived retrieval indexes
- examples for common coding-agent workflows

## Why this exists

Coding agents work better when they receive precise instructions. Static repository instruction files are useful, but they become hard to maintain when policies differ by language, framework, directory, domain, or task risk.

Long instruction contexts can also reduce compliance: important guidance may compete with irrelevant rules, examples, and documentation. Agent Policy Broker aims to give agents less context, but more useful context.

Agent Policy Broker proposes a lightweight control plane:

```text
AGENTS.md / CLAUDE.md / editor rules
        -> run agent-policy get
        -> broker discovers applicable nested instructions
        -> broker retrieves candidate guidance
        -> broker ranks and compiles concise instructions
        -> agent receives compact policy bundle
        -> agent applies instructions and reports policy version
```

## Core idea

Agent Policy Broker is not just a document retriever. It is an instruction compiler.

```text
raw policies + docs + nested instructions + review knowledge
        -> hybrid retrieval
        -> policy scoring
        -> deduplication
        -> context budget
        -> concise instruction bundle
```

Vector search is useful for finding semantically relevant guidance from messy knowledge sources such as architecture docs, prior review comments, incident notes, and legacy instruction files. Structured metadata remains important for exact matching by repo, path, task type, risk flag, language, and framework.

The intended design is hybrid:

```text
vector retrieval for recall
+ exact metadata filters for precision
+ path-scoped instruction discovery
+ deterministic policy priority
+ output budgets
= small, high-signal agent instructions
```

The Git policy registry remains the source of truth. Metadata, BM25, and vector indexes are derived artifacts built from the registry and selected documentation.

## Non-goals

Agent Policy Broker is not intended to be:

- a general-purpose AI agent orchestrator
- a replacement for coding agents
- a raw vector-search dump into the agent context
- a prompt dump or company handbook retriever
- a substitute for CI, tests, code review, or security review

The broker should be deterministic, inspectable, privacy-conscious, and easy to run locally.

## Example

A repository-level `AGENTS.md` can contain:

````md
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
````

The broker may return:

```json
{
  "status": "ok",
  "bundle_id": "apb_2026-05-31_001",
  "policy_version": "2026-05-31.1",
  "context_budget": {
    "max_tokens": 900,
    "estimated_tokens": 430,
    "candidate_policies_considered": 14,
    "candidate_policies_omitted": 9
  },
  "instructions": [
    {
      "text": "Preserve refund idempotency: duplicate provider callbacks must not create duplicate refunds.",
      "priority": "critical",
      "source": "domain.payments.webhooks@3"
    },
    {
      "text": "Use existing MoneyAmount and Currency types; do not introduce raw floating-point money handling.",
      "priority": "high",
      "source": "domain.payments.money@2"
    },
    {
      "text": "Add tests for success, provider failure, retry, and duplicate refund request.",
      "priority": "high",
      "source": "domain.payments.testing@2"
    }
  ],
  "required_checks": [
    { "id": "typescript.typecheck", "source": "lang.typescript.v4" },
    { "id": "payments.unit_tests", "source": "domain.payments.testing@2" }
  ],
  "sources": [
    "repo.billing-api.v3",
    "domain.payments.webhooks@3",
    "domain.payments.money@2",
    "lang.typescript.v4"
  ]
}
```

## Documentation

- [Getting started](docs/getting-started.md)
- [Architecture](docs/architecture.md)
- [Implementation stack](docs/implementation-stack.md)
- [CLI reference](docs/cli-reference.md)
- [Configuration](docs/configuration.md)
- [Instruction discovery and layered guidance](docs/instruction-discovery.md)
- [Repository inspection and migration](docs/repo-inspection-and-migration.md)
- [Retrieval and ranking](docs/retrieval-and-ranking.md)
- [Conflict resolution](docs/conflict-resolution.md)
- [Context budgeting and retrieval](docs/context-budgeting.md)
- [Storage and indexing model](docs/storage-and-indexing.md)
- [Registry mode and WSL workflow](docs/registry-mode.md)
- [Policy schema](docs/policy-schema.md)
- [Agent integration](docs/agent-integration.md)
- [Project scope](docs/project-scope.md)
- [Threat model](docs/threat-model.md)
- [Roadmap](docs/roadmap.md)
- [Privacy](PRIVACY.md)
- [Security](SECURITY.md)
- [Contributing](CONTRIBUTING.md)

## Repository layout

```text
.
├── README.md
├── CONTRIBUTING.md
├── LICENSE
├── PRIVACY.md
├── SECURITY.md
├── docs/
│   ├── architecture.md
│   ├── agent-integration.md
│   ├── cli-reference.md
│   ├── configuration.md
│   ├── conflict-resolution.md
│   ├── context-budgeting.md
│   ├── getting-started.md
│   ├── implementation-stack.md
│   ├── instruction-discovery.md
│   ├── policy-schema.md
│   ├── project-scope.md
│   ├── registry-mode.md
│   ├── repo-inspection-and-migration.md
│   ├── retrieval-and-ranking.md
│   ├── storage-and-indexing.md
│   ├── threat-model.md
│   └── roadmap.md
└── examples/
    ├── AGENTS.md
    └── policies/
        ├── payments.yaml
        └── typescript.yaml
```

## Design principles

1. **Less context, stronger signal**: return only the guidance that matters for the current task.
2. **Retrieve broadly, compile narrowly**: use semantic retrieval and structured matching internally, but do not dump raw documents into the agent context.
3. **Respect existing repo guidance safely**: discover nested instruction files and treat them as path-scoped inputs without letting untrusted local files weaken reviewed registry policy.
4. **Support gradual migration**: inspect existing instruction files, detect duplicates/conflicts, and generate draft broker policies for human review.
5. **Deterministic first**: policy selection should be explainable and reproducible.
6. **Policy as code**: policies should be versioned, reviewed, and owned.
7. **Indexes are derived artifacts**: metadata, BM25, and vector indexes accelerate retrieval but do not replace the Git policy registry as source of truth.
8. **Local-first**: the open-source core should work without a hosted service.
9. **Vendor-neutral**: support Codex, Claude Code, Copilot, Cursor, and other coding agents through simple command execution first.
10. **Auditable**: every returned instruction should be traceable to a source policy.

## License

Agent Policy Broker is licensed under the [MIT License](LICENSE).
