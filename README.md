# Agent Policy Broker

Agent Policy Broker is an open-source CLI and service pattern for delivering concise, task-specific, versioned instructions to coding agents.

Instead of placing large, duplicated instruction files in every repository, teams can keep a small bootstrap file such as `AGENTS.md`, `CLAUDE.md`, Cursor rules, or Copilot instructions that tells the coding agent to request the right policy bundle for the current task. The broker can also help activate this pattern by archiving existing instructions, importing or migrating useful guidance, and replacing active instruction files with a small broker bootstrap.

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

This repository is in active early implementation. The open-source core now includes a Rust CLI and local service prototype for:

- policy schema and validation
- local and registry-backed policy loading
- configuration parsing
- generic and Codex-compatible instruction discovery
- Markdown instruction extraction from existing repo guidance
- task-specific instruction bundle compilation
- deterministic policy matching, conflict handling, and context budgeting
- SQLite metadata indexes and Tantivy full-text retrieval
- optional local vector-retrieval abstractions
- repository inspection and migration draft generation
- GitHub Actions PR reporting example
- localhost service endpoints for repeated lookups and editor integrations

Planned lifecycle work includes explicit activation and deactivation commands that archive existing instruction files, write small broker bootstraps, and restore the previous state when requested.

The project is still pre-release. Interfaces, schemas, and command behavior may change while the MVP is hardened.

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

Activation mode adds a preparation step:

```text
existing global or repo instructions
        -> inspect / migrate / archive
        -> replace active instruction file with broker bootstrap
        -> validate and index
        -> agent-policy get becomes the runtime instruction path
```

Instruction discovery has two modes:

- **Generic mode** scans existing repository guidance such as nested `AGENTS.md`, `CLAUDE.md`, Cursor rules, and Copilot instructions.
- **Codex-compatible mode** mirrors Codex `AGENTS.md` loading: optional global `CODEX_HOME` guidance, `AGENTS.override.md` before `AGENTS.md`, fallback filenames configured through `codex.project_doc_fallback_filenames`, one active file per directory, and a project-root-to-current-directory chain.

Codex mode skips empty files and reports omitted or truncated files. Project instruction reads default to `32768` bytes via `codex.project_doc_max_bytes`.

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

The Git policy registry remains the source of truth. `metadata.sqlite`, `fulltext/`, and vector indexes are derived artifacts built from the registry and selected documentation.

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
- [Public demo scenario](docs/demo.md)
- [Architecture](docs/architecture.md)
- [Implementation stack](docs/implementation-stack.md)
- [CLI reference](docs/cli-reference.md)
- [Configuration](docs/configuration.md)
- [Instruction discovery and layered guidance](docs/instruction-discovery.md)
- [Repository inspection and migration](docs/repo-inspection-and-migration.md)
- [Activation lifecycle](docs/activation-lifecycle.md)
- [Retrieval and ranking](docs/retrieval-and-ranking.md)
- [Conflict resolution](docs/conflict-resolution.md)
- [Context budgeting and retrieval](docs/context-budgeting.md)
- [Storage and indexing model](docs/storage-and-indexing.md)
- [Registry mode and WSL workflow](docs/registry-mode.md)
- [Policy schema](docs/policy-schema.md)
- [Agent integration](docs/agent-integration.md)
- [GitHub Actions PR example](docs/github-action.md)
- [Project scope](docs/project-scope.md)
- [Threat model](docs/threat-model.md)
- [Roadmap](docs/roadmap.md)
- [Privacy](PRIVACY.md)
- [Security](SECURITY.md)
- [Contributing](CONTRIBUTING.md)

## Install

Agent Policy Broker is currently installed from source. From the repository root:

```bash
cargo install --path crates/agent-policy-cli
```

This installs the `agent-policy` binary into Cargo's bin directory, typically `~/.cargo/bin`.

For local development without installing:

```bash
cargo run -p agent-policy-cli -- --help
```

## Build and release

Build an optimized local binary with:

```bash
cargo build --release -p agent-policy-cli
```

The binary is written to `target/release/agent-policy`.

Before tagging or sharing a build, run the same checks used by CI:

```bash
cargo check --workspace --all-targets
cargo test --workspace --all-targets
```
