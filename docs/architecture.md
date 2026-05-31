# Architecture

Agent Policy Broker is designed as a deterministic policy composition layer for coding agents.

The broker should answer one question:

> Given this task intent and repository context, which compact instructions should the coding agent follow now?

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
    | loads policy modules and repo metadata
    v
Instruction bundle
    |
    | compact, versioned, auditable guidance
    v
Coding agent applies instructions
```

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
  -> candidate policy lookup
  -> scoring
  -> precedence/conflict resolution
  -> instruction rendering
  -> audit metadata
```

## Optional hosted service

The open-source core should work locally. A hosted or self-hosted service can later add:

- centralized policy registry
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

But the policy source of truth should remain explicit, versioned, and reviewable.
