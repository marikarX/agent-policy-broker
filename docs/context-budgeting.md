# Context budgeting and retrieval

Agent Policy Broker is built around one assumption:

> Coding agents are more likely to follow important guidance when they receive less irrelevant context.

The broker should avoid dumping raw docs, handbook pages, old instruction files, or search results into the agent context. Instead, it should retrieve broadly and compile narrowly.

For the storage model behind retrieval indexes, see [Storage and indexing model](storage-and-indexing.md).

## Core model

```text
Raw knowledge
  -> candidate guidance
  -> ranked instruction candidates
  -> concise instruction bundle
```

The coding agent should receive the final instruction bundle, not the raw knowledge layer.

## Three content levels

### 1. Raw knowledge

Raw knowledge is long-form, messy, or historical material.

Examples:

- engineering handbook pages
- architecture docs
- code review comments
- incident notes
- legacy `AGENTS.md` or `CLAUDE.md` files
- domain documentation
- provider-specific docs
- migration guides

This material is useful for retrieval but usually too noisy to send directly to a coding agent.

### 2. Candidate guidance

Candidate guidance is material that may be relevant to the current task.

Examples:

- a paragraph about webhook idempotency
- a review comment about repeated refund callbacks
- a testing convention for payment provider failures
- a policy snippet about generated files

Candidate guidance should be scored, deduplicated, and transformed before it reaches the agent.

### 3. Agent instructions

Agent instructions are short, imperative, and task-specific.

Examples:

```text
Preserve webhook idempotency: repeated provider callbacks must not create repeated refunds.
Add tests for repeated callback, provider retry, success path, and provider failure.
Do not edit generated OpenAPI files directly.
```

These are appropriate to include in the instruction bundle.

## Hybrid retrieval

The broker should combine exact and semantic retrieval.

### Exact retrieval

Exact retrieval is best for structured metadata:

- repository
- file path
- language
- framework
- task type
- risk flag
- package manager
- policy status
- policy version
- policy owner
- priority

### Semantic retrieval

Semantic retrieval is best for meaning-based matching.

Example:

```text
Task:
  Fix repeated Stripe refund callback

Potential semantic matches:
  payment provider retry behavior
  webhook idempotency
  repeated callback handling
  refund reconciliation safety
```

A vector index can help find relevant guidance even when the task uses different words than the source documents.

## Retrieval is not instruction generation

Vector search should provide recall. It should not directly decide what the agent sees.

Bad pattern:

```text
vector search -> top 10 chunks -> coding agent context
```

Preferred pattern:

```text
vector search -> candidate snippets -> scoring -> deduplication -> instruction compiler -> compact bundle
```

## Context budget

Every instruction bundle should have a strict budget.

Example budget:

```yaml
output_budget:
  max_tokens: 900
  max_instructions: 8
  max_required_checks: 4
  max_blocked_actions: 4
  include_examples: false
  include_explanations: compact
```

The budget forces prioritization. Lower-value guidance should be omitted, not appended.

## Scoring model

Candidate scoring can combine exact and semantic signals.

Example scoring factors:

```text
+100 global safety rule
+90 exact path match
+80 risk flag match
+70 domain match
+60 task type match
+50 language/framework match
+40 high semantic similarity
+30 repeated prior review feedback
+20 recent incident or regression history
-50 duplicate guidance
-80 generic advice
-100 deprecated policy
```

The exact numbers are implementation details. The important principle is that specific, safety-relevant, and task-relevant guidance should win the budget.

## Deduplication

Policies and docs often say the same thing in different ways. The compiler should merge duplicate instructions.

Example candidates:

```text
Refund callbacks must be idempotent.
Provider retry webhooks must not create repeated refunds.
Repeated refund provider events should be handled consistently.
```

Compiled instruction:

```text
Preserve refund idempotency: repeated provider callbacks must not create repeated refunds.
```

## Omission reporting

The broker should make compression visible.

Example:

```json
{
  "context_budget": {
    "max_tokens": 900,
    "estimated_tokens": 420,
    "candidate_policies_considered": 14,
    "candidate_policies_omitted": 9,
    "reason": "Lower priority or duplicate guidance excluded by context budget."
  }
}
```

This helps users trust that missing policies were intentionally omitted, not accidentally ignored.

## Local vector index

The open-source core may support a local vector index for privacy-conscious retrieval.

Possible commands:

```bash
agent-policy index .agent-policy docs examples
agent-policy get --task "fix repeated refund callback" --files src/payments/webhooks.ts
```

A local index should avoid sending source code or policy contents to a remote service unless explicitly configured.

## Design rule

The broker should optimize for this sequence:

```text
retrieve broadly
rank aggressively
compile narrowly
return only what matters
```
