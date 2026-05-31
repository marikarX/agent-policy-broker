# Policy schema

This document defines the draft policy schema for Agent Policy Broker.

Policies should be small, versioned modules. A policy should describe when it applies, what instructions it contributes, what checks it requires, and which actions it blocks.

The broker may retrieve many candidate policies and knowledge snippets internally, but it should return only a concise instruction bundle that fits the configured context budget.

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
  - typescript.typecheck

blocked_actions:
  - Do not edit generated files directly.

retrieval:
  semantic_terms:
    - typed boundaries
    - generated files
    - TypeScript project conventions

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

Required list of strings. These are candidate instructions returned to the coding agent when the policy applies and when the instruction survives ranking, deduplication, and the context budget. Instructions from global safety policies or other safety-critical policies are mandatory controls: they must be returned or the broker must fail closed instead of silently omitting them.

Instructions should be:

- specific
- actionable
- short
- testable when possible
- scoped to the policy's domain

Avoid generic advice such as "write clean code".

### `required_checks`

Optional list of named check identifiers. Policy files MUST NOT define free-form shell commands in `required_checks`. Each identifier should resolve to a trusted check definition from organization-level configuration, a pinned registry entry, or another explicitly trusted allowlist outside unreviewed repository-local policy content.

Brokers and agents MUST NOT execute policy-supplied check text as a shell command. If a returned check identifier cannot be resolved by trusted configuration, the agent should report it as unavailable or ask for explicit user confirmation instead of running it.

Example:

```yaml
required_checks:
  - typescript.lint
  - payments.unit_tests
```

### `blocked_actions`

Optional list of actions that the agent should not perform. Blocked actions from selected safety-critical policies are mandatory controls. A context or output budget must not drop them solely to satisfy a caller-provided limit; if the bundle cannot include mandatory blocked actions, the broker should fail closed and report the budget violation.

Example:

```yaml
blocked_actions:
  - Do not run destructive database commands.
  - Do not edit production credentials.
```

### `retrieval`

Optional object. Helps semantic retrieval find this policy when task wording differs from policy wording.

Example:

```yaml
retrieval:
  semantic_terms:
    - repeated provider callback
    - webhook idempotency
    - refund retry handling
  related_docs:
    - docs/payments/webhooks.md
    - docs/payments/refunds.md
```

The `retrieval` section should improve recall. It should not override policy priority, status, or structured matching.

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
  "expected_check_ids": ["typescript.unit_tests"],
  "output_budget": {
    "max_tokens": 900,
    "max_instructions": 8,
    "max_required_checks": 4,
    "max_blocked_actions": 4,
    "include_explanations": "compact"
  }
}
```

### Budget and trust constraints

Intent data is often derived from task text, repository contents, pull requests, issues, or bootstrap instructions, so it should be treated as untrusted unless the deployment explicitly authenticates it as operator-controlled configuration. The broker must not honor caller-provided `output_budget` limits directly from untrusted intent.

Implementations should keep output budgets in trusted broker or operator configuration. If a deployment accepts budget hints in an intent object, it must validate the caller's authority and clamp each value to safe operator-defined minimums before selection or rendering. In particular, `max_required_checks` and `max_blocked_actions` must not be allowed to suppress mandatory checks, blocked actions, global safety policies, or other safety-critical controls. If mandatory controls cannot fit in the configured budget, the broker should fail closed rather than returning a weakened bundle.

## Instruction bundle schema

The broker should return an instruction bundle like this:

```json
{
  "status": "ok",
  "bundle_id": "apb_2026-05-31_001",
  "policy_version": "2026-05-31.1",
  "summary": "Instructions for a TypeScript payment bug fix.",
  "context_budget": {
    "max_tokens": 900,
    "estimated_tokens": 420,
    "candidate_policies_considered": 14,
    "candidate_policies_omitted": 9,
    "reason": "Lower priority or duplicate non-mandatory guidance excluded by context budget."
  },
  "instructions": [
    {
      "text": "Preserve refund idempotency semantics.",
      "priority": "critical",
      "source": "domain.payments.refunds@7"
    },
    {
      "text": "Add tests for provider retry and repeated refund request handling.",
      "priority": "high",
      "source": "domain.payments.testing@2"
    }
  ],
  "required_checks": [
    {
      "id": "typescript.lint",
      "source": "lang.typescript.base@1"
    },
    {
      "id": "payments.unit_tests",
      "source": "domain.payments.testing@2"
    }
  ],
  "blocked_actions": [
    "Do not edit production payment credentials."
  ],
  "sources": [
    "domain.payments.refunds@7",
    "domain.payments.testing@2",
    "lang.typescript.base@1"
  ],
  "explanations": [
    {
      "instruction": "Preserve refund idempotency semantics.",
      "source": "domain.payments.refunds@7",
      "reason": "Matched risk flag `payments`, path `src/payments/**`, and semantic terms related to refund retries."
    }
  ]
}
```

## Open questions

- Should trusted check definitions support parameterized arguments such as test-path selectors?
- How should agents display unresolved check identifiers without encouraging unsafe command execution?
- Should policy conflicts fail closed or return warnings?
- How should inherited organization-level policies be represented?
- Which local vector index backend should the open-source core support first?
- How should the broker measure estimated instruction tokens across providers?
