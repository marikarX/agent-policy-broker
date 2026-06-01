# CLI reference

This document defines the implemented MVP command-line interface for Agent Policy Broker.

The CLI should be local-first, scriptable, and safe by default. Commands should support JSON output for automation and Markdown output for direct agent consumption where useful.

## Global conventions

```bash
agent-policy <command> [flags]
```

Common flags:

```text
--repo <path>             Repository path. Defaults to current directory.
--config <path>           Explicit config file path.
--format <json|markdown>  Output format. Defaults depend on command.
--no-network              Use only local files and cached registries.
--verbose                 Print diagnostic details.
--quiet                   Print only essential output.
```

The MVP writes command output to stdout. Redirect stdout from the shell when a file is needed.

## Exit Codes

Current exit behavior:

```text
0   success
1   general failure
2   invalid arguments reported by clap
```

More granular exit codes are planned, but are not implemented in the MVP.

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

Implemented flags:

```text
--task <text>                  Task summary.
--type <task_type>             Task type such as fix_bug, add_feature, refactor, test, docs.
--files <paths...>             Relevant files.
--risk <flags...>              Risk flags such as auth, payments, migrations, public_api.
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
agent-policy inspect --repo . --format markdown > agent-policy-report.md
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
agent-policy index --repo .
```

The index command may create:

```text
metadata.sqlite
fulltext/
manifest.json
```

`metadata.sqlite` and `fulltext/` are derived index artifacts under the local cache directory. They accelerate lookup but are rebuildable from local policies, a configured cached registry, and configured `index.include` documentation paths.

## `agent-policy registry sync`

Validate and use a local cached Git-backed policy registry.

```bash
agent-policy registry sync --repo .
```

The MVP does not clone, fetch, or pull remote registries. It accepts local filesystem registries, `file://` registries, or already-cloned cache directories configured in `.agent-policy.yaml`. `--no-network`, `offline`, and `pinned` modes use only the local cache.

## `agent-policy serve`

Run a local service for faster repeated lookups or editor integrations.

```bash
agent-policy serve --host 127.0.0.1 --port 8765
```

The service binds to `127.0.0.1` by default. Binding to any other host is explicit via `--host`.

Implemented endpoints:

```text
GET  /health
POST /instructions
POST /discover
POST /inspect
```

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
