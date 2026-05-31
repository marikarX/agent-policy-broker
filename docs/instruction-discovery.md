# Instruction discovery and layered guidance

Many repositories already contain agent instruction files in subdirectories. Agent Policy Broker should work with that structure instead of replacing it.

The broker should treat existing instruction files as path-scoped guidance sources that can be discovered, indexed, summarized, and merged with registry policies.

For auditing and migrating existing repositories, see [Repository inspection and migration](repo-inspection-and-migration.md).

## Supported instruction sources

Common sources include:

```text
AGENTS.md
CLAUDE.md
.github/copilot-instructions.md
.cursor/rules/**
.agent-policy.yaml
.agent-policy/policies/**
```

Nested examples:

```text
repo/
  AGENTS.md
  frontend/
    AGENTS.md
    .cursor/rules/react.md
  backend/
    AGENTS.md
  backend/payments/
    AGENTS.md
  packages/ui/
    CLAUDE.md
```

The broker should discover these files and associate each one with the directory scope where it applies.

## Scope model

Instruction files should be scoped by location.

Example:

```text
repo/AGENTS.md                    applies to whole repo
repo/backend/AGENTS.md            applies to backend/**
repo/backend/payments/AGENTS.md   applies to backend/payments/**
```

When a task touches `backend/payments/refunds.ts`, the broker should consider:

1. selected registry policies for repo, domain, risk, package, task, language, and framework;
2. explicitly trusted repository-local guidance that applies to the path;
3. untrusted repository-local guidance and policies as supporting inputs;
4. retrieved supporting knowledge.

More specific path-scoped guidance should usually outrank broader guidance within the same trust level. Branch-controlled repo guidance must not reduce reviewed registry policies unless it is explicitly configured as trusted.

## Trust model

Repository-local instruction files are branch-controlled by default. That makes them useful sources of context, but not automatically authoritative.

A deployment may mark selected paths as trusted through configuration:

```yaml
instruction_sources:
  trusted:
    - AGENTS.md
```

MVP implementations may support exact trusted paths only. Later versions may support trusted globs, signed registry snapshots, or other review-aware trust models.

## Discovery command

A future CLI should expose discovery:

```bash
agent-policy discover --repo .
```

Possible output:

```json
{
  "instruction_sources": [
    {
      "path": "AGENTS.md",
      "scope": ".",
      "type": "agents_md",
      "trusted": true
    },
    {
      "path": "backend/AGENTS.md",
      "scope": "backend/**",
      "type": "agents_md",
      "trusted": false
    },
    {
      "path": "backend/payments/AGENTS.md",
      "scope": "backend/payments/**",
      "type": "agents_md",
      "trusted": false
    }
  ]
}
```

## Runtime behavior

When `agent-policy get` runs, the broker should:

1. identify relevant files for the task;
2. discover applicable instruction files by path scope;
3. classify discovered sources as trusted or untrusted;
4. read only the relevant instruction files;
5. extract candidate guidance;
6. merge with registry and local policies;
7. deduplicate overlapping guidance;
8. apply precedence and context budget;
9. return a concise instruction bundle.

The coding agent should not receive the full contents of every nested instruction file. The broker should compile the relevant parts into the final bundle.

## Precedence

The canonical default precedence is defined in [Conflict resolution](conflict-resolution.md). In short:

1. system, developer, and direct user instructions;
2. global safety policies;
3. organization-wide registry policies;
4. domain- and risk-specific registry policies;
5. repository-specific registry policies;
6. directory- and package-specific registry policies;
7. task-specific registry policies;
8. language, framework, and package-manager registry policies;
9. explicitly trusted repository-local instructions and policies, broad to specific;
10. untrusted repository-local instructions and policies, broad to specific;
11. inferred nearby conventions.

This precedence can be configured, but the broker should prevent branch-controlled local instructions from reducing reviewed registry policies unless the local source is explicitly trusted.

## Conflict examples

### Package manager conflict

```text
Root AGENTS.md: use pnpm
frontend/AGENTS.md: use npm for this package
```

If both sources are at the same trust level and the task only touches `frontend/**`, the more specific frontend instruction should win for package-manager commands.

### Safety conflict

```text
Organization policy: avoid destructive database commands
backend/AGENTS.md: reset the local database before tests
```

The organization policy should win unless the command is explicitly classified as safe and local-only.

## Indexing nested instructions

Nested instruction files should be indexed like other policy knowledge, but with path metadata.

Metadata to store:

- source path;
- directory scope;
- file type;
- last modified commit;
- extracted instructions;
- related language/framework/domain labels;
- whether the source is authoritative, explicitly trusted, or supporting.

This allows the broker to retrieve only the instruction files that matter for the task.

## Migration use case

Agent Policy Broker can help teams migrate from scattered static instruction files to a shared policy registry.

Migration flow:

```text
1. discover existing AGENTS.md / CLAUDE.md / editor rules
2. index them with path scopes and trust metadata
3. detect duplicates and conflicts
4. suggest registry policies for repeated guidance
5. leave thin local bootstrap files in each repo or package
```

The project should support gradual migration. Teams should not need to delete existing instruction files on day one.

For a detailed audit and migration workflow, see [Repository inspection and migration](repo-inspection-and-migration.md).

## Other important use cases

### Monorepos

A monorepo may contain many packages with different languages, package managers, frameworks, and test commands.

The broker should select instructions based on changed paths and package ownership.

### Polyglot repositories

A single repo may contain TypeScript, Python, Go, Terraform, and SQL. The broker should avoid returning irrelevant language guidance.

### Domain-sensitive code

Paths such as auth, payments, billing, data export, and migrations may need stricter instructions and required checks.

### Generated code

The broker should detect generated files and return instructions that point the agent to the source schema or generator instead of editing generated output directly.

### Public API changes

If files suggest API contract changes, the broker can require schema updates, compatibility checks, changelog notes, or migration guidance.

### Test selection

The broker can return task-specific test commands based on package, framework, path, and risk.

### PR review support

A GitHub Action can run the broker on changed files and comment with the policy bundle that should have applied to the PR.

### Onboarding

New developers or agents can ask for focused instructions for a directory or task instead of reading the entire engineering handbook.

### Agent evaluation

The broker can produce expected instruction bundles for historical tasks. These bundles can be used to evaluate whether coding agents follow relevant policies.

### Policy drift detection

The broker can detect repositories or subdirectories with stale, duplicated, or conflicting instruction files.
