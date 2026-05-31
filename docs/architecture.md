# Architecture

Agent Policy Broker is designed as a context-budgeting policy engine for coding agents.

The broker should answer one question:

> Given this task intent and repository context, which compact instructions should the coding agent follow now?

The broker should retrieve broadly, rank aggressively, and compile narrowly. The coding agent should receive concise instructions, not raw documentation dumps.

## High-level design

```text
Coding agent
    |
    | runs command from AGENTS.md / CLAUDE.md / editor rules
    v
agent-policy CLI
    |
    | sends structured intent
    v
Policy broker
    |
    | retrieves candidate guidance
    | ranks policies and knowledge snippets
    | applies context budget
    v
Instruction bundle
    |
    | compact, versioned, auditable guidance
    v
Coding agent applies instructions
```

## Retrieval model

Agent Policy Broker should use hybrid retrieval.

```text
Structured policy store
  - exact repo/path/language/framework/risk matching
  - policy priority
  - policy version
  - owner and status

Vector or semantic index
  - architecture docs
  - engineering handbook pages
  - old AGENTS.md / CLAUDE.md files
  - code review comments
  - incident postmortems
  - domain notes
  - provider-specific docs

Instruction compiler
  - merges candidates
  - deduplicates overlapping guidance
  - applies priority and context budget
  - returns only concise instructions
```

Vector retrieval is useful for recall. Structured matching is useful for precision and governance. The final instruction bundle should be produced by the broker, not by dumping vector-search results into the agent context.

## Components

### CLI

The CLI is the first integration point. It should be simple enough for any coding agent to run from a repository instruction file.

Responsibilities:

- parse task intent from flags or JSON
- detect repository metadata
- detect changed or relevant files when possible
- call the local selector or remote service
- print JSON or Markdown output
- cache safe results when appropriate

### Policy store

The policy store contains small, versioned policy modules.

Policies should be stored as plain text files, usually YAML, so teams can review them through normal code review.

Examples:

- language policy: TypeScript, Python, Go
- framework policy: Jest, Pytest, React, FastAPI
- domain policy: payments, auth, billing, search
- risk policy: migrations, generated code, public API, secrets
- repo policy: package manager, commands, directory layout

### Knowledge index

The knowledge index stores semantically searchable supporting material.

Examples:

- architecture docs
- migration guides
- prior review feedback
- production incident notes
- domain explanations
- API provider notes
- legacy instruction files

The knowledge index is not the source of truth for final policy. It provides candidate evidence and context for the instruction compiler.

### Context resolver

The context resolver enriches the incoming intent with known repository facts.

Potential context sources:

- `package.json`
- `pyproject.toml`
- `go.mod`
- CI configuration
- CODEOWNERS
- existing `AGENTS.md` or `CLAUDE.md`
- configured sensitive paths
- generated-file maps
- policy config files

### Policy selector

The selector chooses candidate policies using structured matching and scoring.

Typical matching fields:

- repository
- path glob
- language
- framework
- package manager
- task type
- risk flag
- owner
- priority

Selection should be deterministic and explainable.

### Semantic retriever

The semantic retriever finds additional candidate guidance from less-structured material.

Example:

```text
Task: "Fix duplicated Stripe refund callback"

Semantic retrieval may find:
  - refund webhook idempotency notes
  - payment provider retry docs
  - prior review comments about duplicate callbacks
  - payment testing conventions
```

The agent should not receive all retrieved chunks. Retrieved material should be converted into short, source-backed instructions or omitted.

### Instruction compiler

The instruction compiler is the core value of the broker.

Responsibilities:

- combine structured policy matches and semantic retrieval candidates
- remove duplicate or generic guidance
- prefer specific guidance over broad guidance
- apply priority and safety rules
- fit the result into a strict output budget
- return source IDs for auditability

### Conflict resolver

When selected policies conflict, the broker should resolve or report the conflict.

Suggested precedence:

1. global safety rules
2. organization-wide rules
3. repository-specific rules
4. directory or package-specific rules
5. domain-specific rules
6. task-specific rules
7. language and framework rules
8. inferred conventions

Global safety rules should always win. Otherwise, more specific policy usually wins over general policy.

### Renderer

The renderer converts selected policies into a compact instruction bundle.

Supported output formats should include:

- JSON for tools and automation
- Markdown for direct agent consumption

## Recommended data flow

```text
Intent input
  -> validation
  -> context enrichment
  -> exact policy lookup
  -> semantic candidate retrieval
  -> scoring and reranking
  -> deduplication
  -> precedence/conflict resolution
  -> context budget application
  -> instruction rendering
  -> audit metadata
```

## Context budget

Every instruction bundle should have a budget.

Example:

```yaml
output_budget:
  max_tokens: 900
  max_instructions: 8
  max_required_checks: 4
  max_blocked_actions: 4
  include_examples: false
  include_explanations: compact
```

Candidates that do not fit the budget should be omitted, not appended. The response should optionally report how many candidate policies were considered and omitted.

## Optional hosted service

The open-source core should work locally. A hosted or self-hosted service can later add:

- centralized policy registry
- organization-wide vector index
- organization-wide rollout
- approval workflows
- audit logs
- analytics
- SSO and RBAC
- GitHub/GitLab checks
- compliance exports

## Why not make this fully agentic?

The broker should not rely on an unconstrained agent to decide policy from scratch. Policy selection needs to be reproducible.

A language model may be useful for optional tasks such as:

- summarizing already-selected policies
- classifying vague task intent
- mapping natural language to canonical risk labels
- transforming retrieved evidence into concise instruction candidates

But the policy source of truth should remain explicit, versioned, and reviewable.
