# Instruction discovery and layered guidance

Many repositories already contain agent instruction files in subdirectories. Agent Policy Broker should work with that structure during lookup, inspection, migration, and activation.

The broker should treat existing instruction files as path-scoped guidance sources that can be discovered, indexed, summarized, migrated, archived, and merged with registry policies.

For auditing and migrating existing repositories, see [Repository inspection and migration](repo-inspection-and-migration.md). For replacing active instruction files with broker bootstraps, see [Activation lifecycle](activation-lifecycle.md).

## Discovery modes

The broker supports two instruction discovery modes:

- **Generic mode** scans the repository for supported agent files such as `AGENTS.md`, `CLAUDE.md`, Copilot instructions, and Cursor rules. This is the default mode and preserves the existing broad discovery behavior.
- **Codex-compatible mode** follows Codex `AGENTS.md` discovery semantics for the active working directory. Use it when the broker should mirror what Codex would load.

Codex-compatible mode:

- optionally reads one global file from `codex.home` or `CODEX_HOME`, checking `AGENTS.override.md` before `AGENTS.md`;
- starts at the project root and walks down to `codex.current_dir`;
- checks one active file per directory in this order: `AGENTS.override.md`, `AGENTS.md`, then names in `codex.project_doc_fallback_filenames`;
- uses fallback names only when no override or `AGENTS.md` exists in that directory;
- skips empty files;
- reads at most `codex.project_doc_max_bytes` bytes per project file, defaulting to `32768`;
- reports omission and truncation metadata in discovery JSON.

Project files are merged from root to current directory. Later, more-specific directory guidance overrides earlier guidance at the same trust level. Sibling directory instructions are not active in Codex-compatible mode.

## Supported instruction sources

Common sources include:

```text
AGENTS.override.md
AGENTS.md
CLAUDE.md
GEMINI.md
.github/copilot-instructions.md
.github/instructions/**/*.instructions.md
.cursor/rules/**/*.mdc
.agent-policy.yaml
.agent-policy/policies/**
```

Nested examples:

```text
repo/
  AGENTS.md
  CLAUDE.md
  frontend/
    AGENTS.md
    .cursor/rules/react.mdc
  backend/
    AGENTS.md
  backend/payments/
    AGENTS.md
  packages/ui/
    CLAUDE.md
```

The broker should discover these files and associate each one with the directory scope where it applies.

## Git state model

Instruction discovery should classify every discovered instruction file by Git state when the repository is available.

```text
tracked      committed shared repo instruction source
untracked    local or draft instruction source
ignored      local-only instruction source
missing      no instruction source
```

Suggested probes:

```bash
git ls-files --error-unmatch AGENTS.md
git check-ignore -v AGENTS.md
git status --ignored --short AGENTS.md
```

If `AGENTS.md` is already tracked, ignore rules do not affect it. If `AGENTS.md` is untracked and ignored, it should not be treated as shared repository policy by default.

Discovery JSON should expose this distinction, for example:

```json
{
  "path": "AGENTS.md",
  "scope": ".",
  "type": "agents_md",
  "git_state": "ignored",
  "source_class": "local_ignored_instruction",
  "trusted": false
}
```

Activation uses this metadata to avoid silently creating local-only repo bootstraps.

## Instruction reference graph

Inspection should build an instruction reference graph across discovered instruction files. This prevents activation from breaking hybrid layouts where one agent instruction file delegates to another file.

The graph should record:

- source path;
- adapter or source type;
- directory scope;
- Git state;
- trust level;
- inbound references;
- outbound references;
- symlink targets;
- unresolved references;
- inferred activation role, such as native entrypoint, shared canonical source, wrapper, supporting knowledge, or archive-only.

References to detect include:

```text
Claude-style imports such as @AGENTS.md
Markdown links to other instruction files
plain mentions of AGENTS.md, CLAUDE.md, GEMINI.md, or .cursor/rules files
symlinks between instruction files
agent-specific include or import syntax when supported
```

The broker should normalize local references relative to the importing file, keep traversal inside configured roots, and record omissions for broken, ignored, unsafe, or unsupported references.

Example graph output:

```json
{
  "instruction_graph": [
    {
      "path": "CLAUDE.md",
      "type": "claude_md",
      "role": "native_entrypoint",
      "references": [
        {
          "target": "AGENTS.md",
          "kind": "import",
          "syntax": "@AGENTS.md",
          "resolved": true
        }
      ]
    },
    {
      "path": "AGENTS.md",
      "type": "agents_md",
      "role": "shared_canonical",
      "referenced_by": ["CLAUDE.md"]
    }
  ]
}
```

Activation should use this graph to recommend hybrid strategies. If `CLAUDE.md` already imports `AGENTS.md`, replacing only `AGENTS.md` with a broker bootstrap may preserve the existing hybrid better than writing duplicate bootstraps into both files.

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

Ignored or untracked instruction files should default to local-only or draft trust, not shared repo trust.

## Discovery command

Generic discovery:

```bash
agent-policy discover --repo .
```

Codex-compatible discovery:

```bash
agent-policy discover --repo . --mode codex --format json
```

Possible output:

```json
{
  "instruction_sources": [
    {
      "path": "AGENTS.md",
      "scope": ".",
      "type": "agents_md",
      "git_state": "tracked",
      "trusted": true
    },
    {
      "path": "backend/AGENTS.md",
      "scope": "backend/**",
      "type": "agents_md",
      "git_state": "tracked",
      "trusted": false
    },
    {
      "path": "backend/payments/AGENTS.md",
      "scope": "backend/payments/**",
      "type": "agents_md",
      "git_state": "ignored",
      "trusted": false
    }
  ],
  "instruction_graph": [
    {
      "path": "CLAUDE.md",
      "role": "native_entrypoint",
      "references": ["AGENTS.md"]
    },
    {
      "path": "AGENTS.md",
      "role": "shared_canonical",
      "referenced_by": ["CLAUDE.md"]
    }
  ]
}
```

Codex-compatible output may also include source byte metadata and an `omissions` list for skipped empty files, shadowed lower-precedence files, or unresolved references.

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

## Activation behavior

Activation is separate from runtime lookup.

During activation, discovered instruction files may be:

```text
imported into broker-managed policy
indexed as supporting knowledge
archived for provenance and rollback
replaced with a small broker bootstrap
wrapped around a shared bootstrap
left untouched when not selected for activation
```

Activation should never delete instruction files without archiving them first. If an instruction file is ignored by Git, repo activation should require an explicit decision:

```text
--local                   create a local ignored bootstrap
--force-track-bootstrap   create and force-add a tracked bootstrap
--global                  use global activation instead
```

See [Activation lifecycle](activation-lifecycle.md).

## Precedence

The canonical default precedence is defined in [Conflict resolution](conflict-resolution.md). In short:

1. system and developer instructions;
2. global safety policies;
3. direct user task instructions;
4. organization-wide registry policies;
5. domain- and risk-specific registry policies;
6. repository-specific registry policies;
7. directory- and package-specific registry policies;
8. task-specific registry policies;
9. language, framework, and package-manager registry policies;
10. explicitly trusted repository-local instructions and policies, broad to specific;
11. untrusted repository-local instructions and policies, broad to specific;
12. inferred nearby conventions.

This precedence can be configured, but the broker should always prevent direct user task text and branch-controlled local instructions from reducing global safety policies. It should also prevent branch-controlled local instructions from reducing reviewed registry policies unless the local source is explicitly trusted.

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
- Git state;
- last modified commit when tracked;
- extracted instructions;
- inbound and outbound instruction references;
- related language/framework/domain labels;
- whether the source is authoritative, explicitly trusted, local-only, wrapper, shared canonical, or supporting.

This allows the broker to retrieve only the instruction files that matter for the task and preserve reference-aware activation plans.

## Migration use case

Agent Policy Broker can help teams migrate from scattered static instruction files to a shared policy registry.

Migration flow:

```text
1. discover existing AGENTS.md / CLAUDE.md / editor rules
2. classify Git state, path scopes, and trust metadata
3. build an instruction reference graph
4. index them with path scopes and trust metadata
5. detect duplicates and conflicts
6. suggest registry policies for repeated guidance
7. recommend native, shared, wrapper, global, local-only, or CI/comment activation
8. activate thin local bootstrap files only when explicitly requested
```

The project should support gradual migration. Teams should not need to delete existing instruction files on day one. Activation provides a separate, reversible path for replacing instruction files with broker bootstraps.

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

The broker can detect repositories or subdirectories with stale, duplicated, conflicting, ignored, locally overridden, or reference-broken instruction files.
