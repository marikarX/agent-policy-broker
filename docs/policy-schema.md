# Policy schema

This document defines the draft policy schema for Agent Policy Broker.

Policies should be small, versioned modules. A policy should describe when it applies, what instructions it contributes, what checks it requires, and which actions it blocks.

## Example

```yaml
id: lang.typescript.base
version: 1
status: active
owner: platform-engineering
priority: 50

applies_when:
  languages:
    - typescript
  task_types:
    - fix_bug
    - add_feature
    - refactor

instructions:
  - Prefer existing project types over introducing new abstractions.
  - Do not use `any` unless the existing boundary is intentionally untyped.
  - Keep changes focused on the requested task.

required_checks:
  - npm run typecheck

blocked_actions:
  - Do not edit generated files directly.

metadata:
  tags:
    - language
    - typescript
```

## Top-level fields

### `id`

Required string. Stable policy identifier.

Example:

```yaml
id: domain.payments.refunds
```

### `version`

Required integer or string. Increment when policy behavior changes.

Example:

```yaml
version: 7
```

### `status`

Required string.

Allowed values:

- `draft`
- `active`
- `deprecated`
- `disabled`

Only `active` policies should be selected by default.

### `owner`

Optional string. Team or person responsible for the policy.

Example:

```yaml
owner: payments-platform
```

### `priority`

Optional integer. Higher priority policies should be considered before lower priority policies when specificity is equal.

Example:

```yaml
priority: 90
```

### `applies_when`

Required object. Describes when this policy applies.

Supported draft fields:

```yaml
applies_when:
  repos:
    - billing-api
  paths:
    - src/payments/**
    - tests/payments/**
  languages:
    - typescript
  frameworks:
    - jest
  package_managers:
    - npm
  task_types:
    - fix_bug
    - add_feature
  risk_flags:
    - payments
    - public_api
```

All fields are optional inside `applies_when`, but a policy with no match criteria is effectively global and should be used carefully.

### `instructions`

Required list of strings. These are the instructions returned to the coding agent when the policy applies.

Instructions should be:

- specific
- actionable
- short
- testable when possible
- scoped to the policy's domain

Avoid generic advice such as "write clean code".

### `required_checks`

Optional list of commands or check identifiers.

Example:

```yaml
required_checks:
  - npm run lint
  - npm test -- tests/payments
```

### `blocked_actions`

Optional list of actions that the agent should not perform.

Example:

```yaml
blocked_actions:
  - Do not run destructive database commands.
  - Do not edit production credentials.
```

### `metadata`

Optional object for labels, documentation links, or implementation-specific fields.

Example:

```yaml
metadata:
  tags:
    - security
    - auth
  docs:
    - docs/security/auth.md
```

## Intent schema

The broker should accept an intent object like this:

```json
{
  "repo": "billing-api",
  "branch": "feature/refund-retries",
  "task": {
    "summary": "Fix refund retry handling",
    "type": "fix_bug"
  },
  "files": [
    "src/payments/refunds.ts",
    "tests/payments/refunds.test.ts"
  ],
  "detected": {
    "languages": ["typescript"],
    "frameworks": ["jest"],
    "package_manager": "npm"
  },
  "risk_flags": ["payments"],
  "expected_commands": ["npm test"]
}
```

## Instruction bundle schema

The broker should return an instruction bundle like this:

```json
{
  "status": "ok",
  "policy_version": "2026-05-31.1",
  "summary": "Instructions for a TypeScript payment bug fix.",
  "instructions": [
    "Preserve refund idempotency semantics.",
    "Add tests for provider retry and duplicate refund request."
  ],
  "required_checks": [
    "npm run lint",
    "npm test -- tests/payments"
  ],
  "blocked_actions": [
    "Do not edit production payment credentials."
  ],
  "sources": [
    "domain.payments.refunds@7",
    "lang.typescript.base@1"
  ],
  "explanations": [
    {
      "instruction": "Preserve refund idempotency semantics.",
      "source": "domain.payments.refunds@7",
      "reason": "Matched risk flag `payments` and path `src/payments/**`."
    }
  ]
}
```

## Open questions

- Should `required_checks` be plain shell commands, named check IDs, or both?
- Should policies support templating such as `{{test_path}}`?
- Should policy conflicts fail closed or return warnings?
- How should inherited organization-level policies be represented?
