# CLI reference

This document defines the intended command-line interface for Agent Policy Broker.

The CLI should be local-first, scriptable, and safe by default. Commands should support JSON output for automation and Markdown output for direct agent consumption where useful.

## Global conventions

Recommended global flags:

```bash
agent-policy <command> [flags]
```

Common flags:

```text
--repo <path>             Repository path. Defaults to current directory.
--config <path>           Explicit config file path.
--registry <name|path>    Registry name or local path.
--format <json|markdown>  Output format. Defaults depend on command.
--output <path>           Write output to a file.
--no-network              Do not fetch remote registries or call remote endpoints.
--verbose                 Print diagnostic details.
--quiet                   Print only essential output.
```

## Exit codes

Suggested exit codes:

```text
0   success
1   general failure
2   invalid arguments
3   configuration error
4   policy validation error
5   registry sync error
6   index error
7   conflict detected
8   unsafe operation blocked
```

## `agent-policy get`

Compile a task-specific instruction bundle.

```bash
agent-policy get --repo . --task "fix refund retry handling"
```

With files:

```bash
agent-policy get \
  --repo . \
  --task "fix refund retry handling" \
  --files src/payments/refunds.ts tests/payments/refunds.test.ts
```

With JSON intent:

```bash
agent-policy get --intent intent.json --format json
```

Recommended flags:

```text
--task <text>                  Task summary.
--type <task_type>             Task type such as fix_bug, add_feature, refactor, test, docs.
--files <paths...>             Relevant files.
--risk <flags...>              Risk flags such as auth, payments, migrations, public_api.
--intent <path>                JSON intent file.
--instruction-mode <mode>      Instruction discovery mode: generic or codex.
--format <json|markdown>       Output bundle format.
--max-tokens <number>          Override output token budget.
--max-instructions <number>    Override instruction count budget.
```

Use Codex-compatible discovery for task bundles:

```bash
agent-policy get --repo . --instruction-mode codex --task "fix refund retry handling"
```

## `agent-policy discover`

Discover existing instruction sources in a repository.

```bash
agent-policy discover --repo . --format json
```

Discovery modes:

```bash
agent-policy discover --repo . --mode generic
agent-policy discover --repo . --mode codex
```

Generic mode scans for files such as:

```text
AGENTS.md
CLAUDE.md
.github/copilot-instructions.md
.cursor/rules/**
.agent-policy/policies/**
```

Codex-compatible mode follows Codex `AGENTS.md` semantics: `AGENTS.override.md` wins over `AGENTS.md`, fallback filenames are used only when both are absent, one file is active per directory, and only the project-root-to-current-directory chain is active. Optional global instructions come from `codex.home` or `CODEX_HOME` when `codex.include_global` is enabled. Empty files are skipped, and max-byte truncation or omissions are reported in JSON metadata.

## `agent-policy inspect`

Inspect an existing repository and produce an audit report.

```bash
agent-policy inspect --repo . --format markdown --output agent-policy-report.md
```

Use Codex-compatible discovery for the report:

```bash
agent-policy inspect --repo . --mode codex --format markdown
```

The report should include:

- discovered instruction files;
- path scopes;
- duplicated guidance;
- conflicts;
- migration candidates;
- stale or overly broad guidance;
- suggested policy targets.

## `agent-policy migrate`

Generate proposed policy files from existing instruction sources.

Dry run:

```bash
agent-policy migrate --repo . --dry-run
```

Write proposed drafts:

```bash
agent-policy migrate --repo . --write
```

Migration must be conservative. It should not delete or rewrite existing instruction files unless a future explicit flag is added for that behavior.

Generated policies should default to `status: draft`.

## `agent-policy validate`

Validate policies, config, and optionally discovered instruction sources.

```bash
agent-policy validate --repo .
```

Validate a registry:

```bash
agent-policy validate --registry company
```

Validation should check:

- schema correctness;
- duplicate policy IDs;
- invalid status values;
- overly broad policies;
- vague instructions;
- conflicts;
- missing owners for high-priority policies;
- output budget risks.

## `agent-policy index`

Build or rebuild local retrieval indexes.

```bash
agent-policy index --registry company
```

With explicit include paths:

```bash
agent-policy index \
  --registry company \
  --include policies \
  --include docs \
  --exclude secrets
```

The index command may create:

```text
metadata.sqlite
fulltext/
vectors/
manifest.json
```

`metadata.sqlite` and `fulltext/` are derived index artifacts. They accelerate lookup but should be rebuildable from the policy registry and configured local sources.

## `agent-policy registry sync`

Fetch or update a Git-backed policy registry.

```bash
agent-policy registry sync --registry company
```

This should obey registry sync settings such as `manual`, `auto`, `pinned`, and `offline`.

## `agent-policy serve`

Run a local service for faster repeated lookups or editor integrations.

```bash
agent-policy serve --host 127.0.0.1 --port 8765
```

The service should bind to localhost by default.

Suggested endpoints are described in a future local service API document.

## Error format

JSON errors should be structured:

```json
{
  "status": "error",
  "code": "registry_not_found",
  "message": "Policy registry `company` is not configured.",
  "details": {
    "config_path": ".agent-policy.yaml"
  }
}
```

Markdown errors should be short and actionable.
