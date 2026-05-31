# Conflict resolution

Agent Policy Broker should resolve or report conflicts between policies, nested instruction files, registry guidance, and inferred repository conventions.

Conflict handling must be deterministic and auditable.

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

1. system, developer, and direct user instructions;
2. global safety policies;
3. organization policies from the registry;
4. domain and risk policies from the registry;
5. repository-specific registry policies;
6. directory, language, framework, and task registry policies;
7. explicitly trusted repository instructions, broad to specific;
8. untrusted repository-local instructions and `.agent-policy` policies, broad to specific;
9. inferred nearby conventions.

Reviewed registry policies should not be weakened by branch-controlled local instructions. Local instructions may override registry policy only when the source is explicitly configured as trusted.

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
Org policy: do not run destructive database commands
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
      "reason": "Local instruction weakens organization migration safety policy."
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
- local policies that weaken reviewed registry policies;
- nested instruction files with contradictory package-manager commands;
- broad policies with missing path or risk constraints.

## Human review

Generated policies from migration should default to `status: draft`. Conflict resolution should not automatically activate generated policies or delete old instructions.
