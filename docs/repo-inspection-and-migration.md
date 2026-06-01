# Repository inspection and migration

Agent Policy Broker should help teams inspect existing repositories, understand current agent instructions, and migrate repeated guidance into the broker over time.

The goal is not to force a big-bang migration. The broker should support gradual adoption.

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

Agent Policy Broker should inspect these files, preserve useful path-scoped guidance, and help teams move repeated or shared guidance into a registry.

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
3. extract candidate instructions;
4. classify instructions by topic;
5. detect duplicates;
6. detect conflicts;
7. identify stale or overly broad guidance;
8. identify instructions that should stay local;
9. identify instructions that should move to shared registry policies;
10. propose a migration plan.

## Instruction source discovery

Inspection uses generic discovery by default. Generic mode scans all supported instruction source types and nested paths.

Codex-compatible mode follows the active Codex project chain instead: project root to `codex.current_dir`, one active file per directory, `AGENTS.override.md` before `AGENTS.md`, configured fallback filenames only when both standard files are absent, and optional global instructions from `codex.home` or `CODEX_HOME`. Empty files are skipped; skipped and truncated files are reported in metadata.

Supported sources include:

```text
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
- last modified commit;
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
      "instruction_count": 12
    },
    {
      "path": "backend/payments/AGENTS.md",
      "scope": "backend/payments/**",
      "type": "agents_md",
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
  ]
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

Migration should create proposed policy files for review. It should not delete or rewrite existing instruction files unless explicitly requested.

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

## Bootstrap reduction

After migration, an existing instruction file can be reduced to a thin bootstrap.

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

## Audit report

The inspection report should help teams answer:

- Which instruction files exist?
- Which paths do they apply to?
- Which instructions are duplicated?
- Which instructions conflict?
- Which files are stale?
- Which instructions should move to shared policy?
- Which instructions should remain local?
- Which instructions are too verbose and should become supporting knowledge?

## Safety requirements

Inspection and migration should be conservative.

The tool should not:

- delete existing instruction files by default;
- rewrite files without explicit `--write` or equivalent confirmation;
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
5. move approved shared policies to registry repo
6. reduce local instruction files to bootstrap guidance
7. run agent-policy validate
8. enable PR checks for policy drift and conflicts
```
