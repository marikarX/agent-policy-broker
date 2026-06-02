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

## Command mutability

Read-only commands must not modify instruction files:

```text
agent-policy get
agent-policy discover
agent-policy inspect
agent-policy validate
agent-policy index
agent-policy migrate --dry-run
agent-policy activate ... --dry-run
agent-policy deactivate ... --dry-run
```

Mutating commands must require an explicit write or restore flag:

```text
agent-policy migrate --write
agent-policy activate ... --write
agent-policy deactivate ... --restore
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

Discovery should also classify instruction files by Git state when the repository is available:

```text
tracked      committed shared repo source
untracked    local or draft source
ignored      local-only source
missing      absent source
```

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
- Git state for instruction files;
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

Migration must be conservative. It should not delete or rewrite existing instruction files unless a future explicit activation or cleanup flag is used for that behavior.

Generated policies should default to `status: draft`.

## `agent-policy activate`

Activate broker-managed instruction delivery by archiving existing instruction files, importing or migrating useful guidance, and replacing the active instruction file with a small bootstrap.

Activation is planned, not part of the early MVP command set unless implemented by the current binary.

Repo activation dry run:

```bash
agent-policy activate repo --repo . --dry-run
```

Repo activation write:

```bash
agent-policy activate repo --repo . --write
```

Global Codex activation dry run:

```bash
agent-policy activate codex --global --dry-run
```

Global Codex activation write:

```bash
agent-policy activate codex --global --write
```

Planned flags:

```text
--dry-run                  Print the activation plan without writing files.
--write                    Apply the activation plan.
--global                   Activate global agent instructions such as Codex home instructions.
--archive-existing         Archive existing instruction files before replacement.
--local                    Create a local-only ignored bootstrap when repo AGENTS.md is ignored.
--force-track-bootstrap    Force-add a tracked repo bootstrap even when AGENTS.md is ignored.
--smoke-test               Run a lookup smoke test after activation.
```

Activation should:

- discover existing instruction sources;
- classify each source by path scope, trust, and Git state;
- archive files before replacing them;
- write an activation manifest;
- generate or update broker-managed policy drafts when requested;
- replace active instruction files with a small broker bootstrap;
- validate and index the resulting configuration;
- print a restore command.

If repo `AGENTS.md` is ignored, repo activation must not silently create a local-only bootstrap. It should require `--local`, `--force-track-bootstrap`, or use global activation.

See [Activation lifecycle](activation-lifecycle.md).

## `agent-policy deactivate`

Deactivate broker-managed instruction delivery and restore archived instruction files.

Deactivation is planned, not part of the early MVP command set unless implemented by the current binary.

Repo deactivation dry run:

```bash
agent-policy deactivate repo --repo . --dry-run
```

Repo restore:

```bash
agent-policy deactivate repo --repo . --restore
```

Restore a specific activation:

```bash
agent-policy deactivate repo --repo . --activation act_2026_06_01_223000 --restore
```

Global Codex deactivation:

```bash
agent-policy deactivate codex --global --dry-run
agent-policy deactivate codex --global --restore
```

Planned flags:

```text
--dry-run                    Print the restore plan without writing files.
--restore                    Restore archived instruction files.
--activation <id>            Restore a specific activation archive.
--force                      Overwrite files changed after activation.
--remove-generated-policies  Remove broker-generated policy drafts.
--remove-index               Remove local derived indexes.
```

Deactivation should:

- find the activation manifest;
- verify current files still match broker-managed bootstrap state where possible;
- restore archived files to their original paths;
- remove broker-created bootstrap files when safe;
- leave generated policies and indexes in place unless explicit cleanup flags are supplied;
- refuse to overwrite changed files unless `--force` is supplied.

See [Activation lifecycle](activation-lifecycle.md).

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
- output budget risks;
- activation archive manifest shape when present.

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

`metadata.sqlite` and `fulltext/` are derived index artifacts under the local cache directory. They accelerate lookup but are rebuildable from local policies, a configured cached registry, archived instructions, and configured `index.include` documentation paths.

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
