# Architecture

Agent Policy Broker is designed as a context-budgeting policy engine for coding agents.

The broker should answer one runtime question:

> Given this task intent and repository context, which compact instructions should the coding agent follow now?

The broker should retrieve broadly, rank aggressively, and compile narrowly. The coding agent should receive concise instructions, not raw documentation dumps.

The broker also has an activation-time responsibility: it can help users move from scattered static instruction files to broker-managed instruction delivery by archiving existing instructions, importing or migrating useful guidance, replacing active instruction files with a small bootstrap, and providing a restore path.

## High-level runtime design

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
    | discovers path-scoped instruction files
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

## Activation-time design

Activation is explicit and separate from runtime lookup.

```text
Existing global or repo instruction files
    |
    | agent-policy inspect / migrate / activate
    v
Archive originals + manifest
    |
    v
Broker-managed policy and supporting knowledge
    |
    v
Small bootstrap instruction file
    |
    v
agent-policy validate + index + smoke test
```

Activation commands should archive any instruction files they replace and write a manifest with enough information to restore the previous state. Deactivation uses that manifest to restore archived files and remove broker-created bootstraps when safe.

Normal lookup commands such as `get`, `discover`, `inspect`, `validate`, and `index` must not rewrite instruction files. Mutations belong behind explicit commands such as `activate ... --write`, `migrate --write`, and `deactivate ... --restore`.

See [Activation lifecycle](activation-lifecycle.md).

## Retrieval model

Agent Policy Broker should use hybrid retrieval.

```text
Structured policy store
  - exact repo/path/language/framework/risk matching
  - policy priority
  - policy version
  - owner and status

Layered instruction sources
  - root AGENTS.md / CLAUDE.md
  - nested AGENTS.md / CLAUDE.md files
  - editor-specific rules
  - repo-local .agent-policy policies
  - trust metadata that distinguishes reviewed sources from branch-controlled sources
  - Git state metadata that distinguishes tracked, untracked, ignored, and local-only files

Vector or semantic index
  - architecture docs
  - engineering handbook pages
  - old AGENTS.md / CLAUDE.md files
  - archived instruction files
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

Vector retrieval is useful for recall. Structured matching is useful for precision and governance. Path-scoped instruction discovery preserves existing repo guidance, but repository-local files are branch-controlled by default and should be treated as lower-authority inputs unless explicitly configured as trusted. The final instruction bundle should be produced by the broker, not by dumping vector-search results or every nested instruction file into the agent context.

## Components

### CLI

The CLI is the first integration point. It should be simple enough for any coding agent to run from a repository instruction file.

Responsibilities:

- parse task intent from flags or JSON;
- detect repository metadata;
- detect changed or relevant files when possible;
- discover applicable instruction files;
- classify instruction files by Git state when available;
- call the local selector or remote service;
- print JSON or Markdown output;
- cache safe results when appropriate.

Activation-specific responsibilities:

- produce activation and deactivation dry-run plans;
- archive instruction files before replacement;
- write activation manifests;
- generate broker bootstrap files;
- restore archived files during deactivation;
- refuse unsafe overwrites unless explicitly forced.

### Policy store

The policy store contains small, versioned policy modules.

Policies should be stored as plain text files, usually YAML, so teams can review them through normal code review.

Examples:

- language policy: TypeScript, Python, Go
- framework policy: Jest, Pytest, React, FastAPI
- domain policy: payments, auth, billing, search
- risk policy: migrations, generated code, public API, secrets
- repo policy: package manager, commands, directory layout
- global user or operator policy imported during global activation

### Instruction source discovery

The broker should discover existing path-scoped instruction files such as:

- `AGENTS.override.md`
- `AGENTS.md`
- `CLAUDE.md`
- `.github/copilot-instructions.md`
- `.cursor/rules/**`
- `.agent-policy/policies/**`

Nested instruction files should be associated with their directory scope. For example, `backend/payments/AGENTS.md` applies to `backend/payments/**` and should be considered when the task touches files under that path. Because these files can be changed by untrusted branches or pull requests, they should not override reviewed registry policy unless the source is explicitly marked trusted.

Instruction discovery should also report whether sources are tracked, untracked, ignored, or local-only. Activation uses that metadata to avoid silently treating ignored local files as shared repo instructions.

See [Instruction discovery and layered guidance](instruction-discovery.md) for details.

### Activation manager

The activation manager is responsible for planned mutating lifecycle operations.

Responsibilities:

- plan global Codex and repo activation;
- import or migrate existing instruction content;
- archive files before replacement;
- write activation manifests;
- create small broker bootstraps;
- validate and index after activation;
- restore archived files during deactivation;
- report conflicts when current files changed after activation.

Activation should be transactional where possible. If a step fails before writes, no files should change. If a step fails after writes, the command should print the restore command and archive location.

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
- archived instruction files

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
- activation manifests and archive metadata
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

- combine structured policy matches, path-scoped instructions, and semantic retrieval candidates
- remove duplicate or generic guidance
- prefer specific guidance over broad guidance within the same trust level
- apply priority and safety rules
- fit the result into a strict output budget
- return source IDs for auditability

### Conflict resolver

When selected policies conflict, the broker should resolve or report the conflict.

The canonical default precedence is defined in [Conflict resolution](conflict-resolution.md). Architecture, discovery, and retrieval docs should refer to that ordering instead of defining competing precedence models.

Summary:

1. system and developer instructions
2. global safety policies
3. direct user task instructions
4. organization-wide registry policies
5. domain- and risk-specific registry policies
6. repository-specific registry policies
7. directory- and package-specific registry policies
8. task-specific registry policies
9. language, framework, and package-manager registry policies
10. explicitly trusted repository-local instructions and policies, broad to specific
11. untrusted repository-local instructions and policies, broad to specific
12. inferred nearby conventions

Global safety rules are mandatory broker controls and should always win over direct user task text, branch-controlled repository instructions, and lower-precedence policy sources. Reviewed registry policies should not be reduced by branch-controlled repository instructions. Repository-local instructions may refine workflow details when they do not conflict with higher-authority policy, and may outrank registry policy only when the source is explicitly configured as trusted. Otherwise, more specific policy usually wins over general policy within the same trust level.

### Renderer

The renderer converts selected policies into a compact instruction bundle.

Supported output formats should include:

- JSON for tools and automation
- Markdown for direct agent consumption

## Recommended runtime data flow

```text
Intent input
  -> validation
  -> context enrichment
  -> instruction source discovery
  -> exact policy lookup
  -> semantic candidate retrieval
  -> scoring and reranking
  -> deduplication
  -> precedence/conflict resolution
  -> context budget application using trusted or safely clamped budget values
  -> instruction rendering
  -> audit metadata
```

## Recommended activation data flow

```text
Activation request
  -> discover global or repo instruction sources
  -> classify Git state and trust
  -> compute activation plan
  -> dry-run output or confirmation gate
  -> archive originals and write manifest
  -> import/migrate guidance into broker-managed policy or knowledge
  -> write bootstrap files
  -> validate
  -> index
  -> smoke-test lookup
  -> print restore command
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

Candidates that do not fit the budget should be omitted, not appended, only when they are lower-priority, non-mandatory guidance. Global safety rules, mandatory required checks, blocked actions, and other safety-critical controls must not be trimmed solely because an untrusted caller supplied a small budget. Budgets should come from trusted broker or operator configuration; any budget hints accepted in intent input must be authorized or clamped to safe operator-defined minimums before use. If mandatory controls cannot fit, the broker should fail closed and report the budget violation. The response should optionally report how many non-mandatory candidate policies were considered and omitted.

## Optional organization deployment

The open-source core should work locally. Future organization deployment patterns may add centralized registry, approval, audit, rollout, and integration capabilities. Public OSS docs should describe these as deployment patterns rather than pricing or packaging commitments.

## Why not make this fully agentic?

The broker should not rely on an unconstrained agent to decide policy from scratch. Policy selection needs to be reproducible.

A language model may be useful for optional tasks such as:

- summarizing already-selected policies
- classifying vague task intent
- mapping natural language to canonical risk labels
- transforming retrieved evidence into concise instruction candidates

But the policy source of truth should remain explicit, versioned, and reviewable.
