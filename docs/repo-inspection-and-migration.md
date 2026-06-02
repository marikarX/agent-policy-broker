# Repository inspection and migration

Agent Policy Broker should help teams inspect existing repositories, understand current agent instructions, and migrate repeated guidance into the broker over time.

The goal is not to force a big-bang migration. The broker should support gradual adoption, and replacing active instruction files should happen only through the explicit activation lifecycle. See [Activation lifecycle](activation-lifecycle.md).

## Problem

Many repositories already contain scattered instruction files:

```text
repo/
  AGENTS.md
  CLAUDE.md
  .github/copilot-instructions.md
  .cursor/rules/backend.md
  frontend/AGENTS.md
  backend/AGENTS.md
  backend/payments/AGENTS.md
  packages/ui/CLAUDE.md
```

These files may contain useful project knowledge, but they often become:

- duplicated;
- stale;
- inconsistent;
- too broad;
- hard to audit;
- hard to share across repositories;
- overloaded with context that is irrelevant to many tasks.

Agent Policy Broker should inspect these files, preserve useful path-scoped guidance, and help teams move repeated or shared guidance into broker-managed policies or supporting knowledge.

## Inspection command

The CLI should expose an inspection command:

```bash
agent-policy inspect --repo .
```

For a migration-oriented report:

```bash
agent-policy inspect --repo . --format markdown > agent-policy-report.md
```

For machine-readable output:

```bash
agent-policy inspect --repo . --format json
```

To inspect using Codex-compatible `AGENTS.md` discovery:

```bash
agent-policy inspect --repo . --mode codex --format json
```

## What inspection should do

Inspection should:

1. discover root and nested instruction files;
2. infer directory scopes;
3. classify instruction files by Git state: tracked, untracked, ignored, or missing;
4. extract candidate instructions;
5. classify instructions by topic;
6. detect duplicates;
7. detect conflicts;
8. identify stale or overly broad guidance;
9. identify instructions that should stay local;
10. identify instructions that should move to repo or registry policies;
11. identify content that should become supporting knowledge;
12. propose a migration and activation plan.

Inspection is read-only.

## Instruction source discovery

Inspection uses generic discovery by default. Generic mode scans all supported instruction source types and nested paths.

Codex-compatible mode follows the active Codex project chain instead: project root to `codex.current_dir`, one active file per directory, `AGENTS.override.md` before `AGENTS.md`, configured fallback filenames only when both standard files are absent, and optional global instructions from `codex.home` or `CODEX_HOME`. Empty files are skipped; skipped and truncated files are reported in metadata.

Supported sources include:

```text
AGENTS.override.md
AGENTS.md
CLAUDE.md
.github/copilot-instructions.md
.cursor/rules/**
.agent-policy.yaml
.agent-policy/policies/**
```

The scanner should record:

- file path;
- directory scope;
- source type;
- Git state;
- last modified commit when tracked;
- owner if known through CODEOWNERS;
- extracted instruction count;
- detected language/framework/domain labels.

## Example inspection output

```json
{
  "repo": "billing-api",
  "instruction_sources": [
    {
      "path": "AGENTS.md",
      "scope": ".",
      "type": "agents_md",
      "git_state": "tracked",
      "instruction_count": 12
    },
    {
      "path": "backend/payments/AGENTS.md",
      "scope": "backend/payments/**",
      "type": "agents_md",
      "git_state": "tracked",
      "instruction_count": 9
    }
  ],
  "duplicates": [
    {
      "instruction": "Do not edit generated OpenAPI files directly.",
      "sources": [
        "AGENTS.md",
        "backend/AGENTS.md"
      ],
      "suggestion": "Move to registry policy `org.generated-files`."
    }
  ],
  "conflicts": [
    {
      "topic": "package_manager",
      "sources": [
        "AGENTS.md",
        "frontend/AGENTS.md"
      ],
      "summary": "Root guidance says pnpm; frontend guidance says npm.",
      "suggestion": "Keep frontend override scoped to `frontend/**`."
    }
  ],
  "migration_candidates": [
    {
      "target_policy": "domain.payments.refunds",
      "source": "backend/payments/AGENTS.md",
      "instructions": [
        "Preserve refund idempotency.",
        "Add tests for provider retry behavior."
      ]
    }
  ],
  "activation_recommendation": {
    "mode": "repo",
    "requires_archive": true,
    "ignored_bootstrap_warning": false
  }
}
```

## Migration command

A future CLI may generate proposed policy files without modifying the repo by default:

```bash
agent-policy migrate --repo . --dry-run
```

Possible output:

```text
Proposed files:
  .agent-policy/migration/domain.payments.refunds.yaml
  .agent-policy/migration/org.generated-files.yaml
  .agent-policy/migration/frontend.package-manager.yaml
```

To write proposed files locally:

```bash
agent-policy migrate --repo . --write
```

Migration should create proposed policy files for review. It should not delete or rewrite existing instruction files. Replacing instruction files belongs to `agent-policy activate ... --write`.

## Migration targets

The broker should classify existing instructions into migration targets.

### Keep local

Keep instructions local when they are specific to one package or directory.

Examples:

- package-specific test commands;
- local build quirks;
- directory-specific generated-file locations;
- package-specific framework conventions.

### Move to repo policy

Move instructions to a repo policy when they apply across one repository but not broadly across the organization.

Examples:

- repo-specific package manager;
- repo-specific CI commands;
- repo-specific layout conventions.

### Move to shared registry policy

Move instructions to registry policies when they repeat across repositories or represent organizational standards.

Examples:

- security rules;
- generated-file rules;
- public API compatibility rules;
- migration safety rules;
- language and framework standards;
- domain rules for auth, payments, billing, or data export.

### Keep as supporting knowledge

Some content should not become direct instructions. It should be indexed as supporting knowledge instead.

Examples:

- long architecture explanations;
- historical rationale;
- incident notes;
- examples that are useful for retrieval but too verbose for agent context.

## Proposed policy generation

Generated policy drafts should include provenance.

Example:

```yaml
id: domain.payments.refunds
version: 1
status: draft
owner: payments-platform
priority: 90

applies_when:
  paths:
    - backend/payments/**
  risk_flags:
    - payments

instructions:
  - Preserve refund idempotency.
  - Add tests for provider retry behavior.

metadata:
  generated_from:
    - backend/payments/AGENTS.md
  migration_status: proposed
```

Generated policies should start as `status: draft` so humans can review and approve them before they become active.

## Activation after migration

After inspection and migration, users may activate broker-managed instruction delivery.

Planned commands:

```bash
agent-policy activate repo --repo . --dry-run
agent-policy activate repo --repo . --write
```

Activation should:

1. archive instruction files that will be replaced;
2. write an activation manifest;
3. replace active instruction files with a small broker bootstrap;
4. validate and index the resulting state;
5. print a restore command.

Activation should not proceed silently when a repo `AGENTS.md` is ignored by Git. It should require an explicit choice such as local-only activation, force-tracked bootstrap, or global Codex activation.

## Bootstrap reduction

After migration and activation, an existing instruction file can be reduced to a thin bootstrap.

Before:

```text
backend/payments/AGENTS.md
  - 40 lines of payment rules
  - test commands
  - generated file warnings
  - style rules
```

After:

```md
# Backend payments guidance

Before changing code under this directory, run:

```bash
agent-policy get --repo . --task "$USER_TASK" --files <relevant-files>
```

Follow the returned bundle. Keep local notes here only when they are specific to this directory and not represented in the policy registry.
```

## Deactivation and restore

Activation must be reversible.

Planned commands:

```bash
agent-policy deactivate repo --repo . --dry-run
agent-policy deactivate repo --repo . --restore
agent-policy deactivate repo --repo . --activation <activation-id> --restore
```

Deactivation should restore archived instruction files to their previous paths, remove broker-created bootstrap files when safe, and refuse to overwrite files that changed after activation unless `--force` is supplied.

Generated policy drafts and indexes should be left in place unless explicit cleanup flags are supplied.

## Audit report

The inspection report should help teams answer:

- Which instruction files exist?
- Which paths do they apply to?
- Are they tracked, ignored, untracked, or local-only?
- Which instructions are duplicated?
- Which instructions conflict?
- Which files are stale?
- Which instructions should move to shared policy?
- Which instructions should remain local?
- Which instructions are too verbose and should become supporting knowledge?
- Which activation path is safest?

## Safety requirements

Inspection and migration should be conservative.

The tool should not:

- delete existing instruction files by default;
- rewrite files without explicit `--write` or equivalent confirmation;
- replace active instruction files outside activation;
- mark generated policies as active automatically;
- upload private repository instructions to a remote service unless explicitly configured;
- index source code by default.

## Adoption flow

Recommended adoption path:

```text
1. agent-policy inspect --repo .
2. review generated report
3. agent-policy migrate --repo . --dry-run
4. review proposed draft policies
5. approve or edit generated policies
6. agent-policy activate repo --repo . --dry-run
7. review archive and bootstrap plan
8. agent-policy activate repo --repo . --write
9. run agent-policy validate
10. run agent-policy index
11. enable PR checks for policy drift and conflicts
```

For users who prefer global Codex activation or whose repos ignore `AGENTS.md`, use `agent-policy activate codex --global --dry-run` instead of repo activation.
