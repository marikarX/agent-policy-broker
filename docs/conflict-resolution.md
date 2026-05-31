# Conflict resolution

Agent Policy Broker should resolve or report conflicts between policies, nested instruction files, registry guidance, and inferred repository conventions.

Conflict handling must be deterministic and auditable.

This document is the canonical source for default precedence. Other docs should refer to this ordering instead of defining a separate one.

## Conflict types

Common conflicts include:

- package manager commands;
- test commands;
- formatting commands;
- generated-file handling;
- migration safety rules;
- public API compatibility rules;
- local package guidance versus organization policy;
- broad root instructions versus nested path-specific instructions.

## Precedence model

Recommended default precedence:

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

Global safety policies are mandatory broker controls and must not be weakened by direct user task text, branch-controlled local instructions, or any lower-precedence policy source. Reviewed registry policies should not be reduced by branch-controlled local instructions. Local instructions may take precedence over registry policy only when the source is explicitly configured as trusted.

## Specificity rule

When two non-safety instructions with the same trust level conflict, the more specific instruction usually wins. Trust level is evaluated before path specificity.

Example:

```text
Root AGENTS.md: use pnpm
frontend/AGENTS.md: use npm for frontend/**
```

For a task touching only `frontend/**`, the frontend instruction should win if both files have the same trust level and no reviewed registry policy requires a different package manager.

## Safety rule

Safety constraints should win over convenience or local workflow instructions.

Example:

```text
Organization policy: avoid destructive database commands
backend/AGENTS.md: reset database before tests
```

The broker should either omit the reset instruction or return a warning unless the command is explicitly configured as safe and local-only.

## Conflict outcomes

The broker can handle conflicts in three ways.

### Resolve

Use deterministic precedence and include the winning instruction.

### Warn

Include a warning when the task can continue safely but the user should know about the conflict.

### Fail closed

Return an error when the conflict affects safety, credentials, destructive commands, or policy authority.

Example error:

```json
{
  "status": "error",
  "code": "policy_conflict",
  "message": "Conflicting migration policies require human review.",
  "conflicts": [
    {
      "topic": "database_migration",
      "sources": ["org.migrations@3", "backend/AGENTS.md"],
      "reason": "Local instruction reduces organization migration safety policy."
    }
  ]
}
```

## Conflict report

Instruction bundles should optionally include conflict metadata.

```json
{
  "warnings": [
    {
      "type": "conflict_resolved",
      "topic": "package_manager",
      "winner": "frontend/AGENTS.md",
      "loser": "AGENTS.md",
      "reason": "Nested instruction is more specific for touched path."
    }
  ]
}
```

## Validation

`agent-policy validate` should detect common conflicts before runtime.

Examples:

- multiple active policies with same ID;
- two active policies with mutually exclusive required commands for the same path;
- local policies that reduce reviewed registry policies;
- nested instruction files with contradictory package-manager commands;
- broad policies with missing path or risk constraints.

## Human review

Generated policies from migration should default to `status: draft`. Conflict resolution should not automatically activate generated policies or delete old instructions.
