# Retrieval and ranking

Agent Policy Broker should retrieve broadly, rank aggressively, and compile narrowly.

The goal is to produce a small instruction bundle that is more useful than a large pile of raw context.

## Inputs

Runtime inputs may include:

- task summary;
- task type;
- changed or relevant files;
- repository metadata;
- discovered nested instruction files;
- registry policies;
- local policies;
- metadata index;
- BM25 index;
- vector index;
- configured output budget.

## Candidate sources

The broker can collect candidates from:

1. exact policy matches;
2. path-scoped instruction files;
3. BM25 keyword search;
4. vector semantic search;
5. local repository metadata;
6. explicit CLI-provided risk flags.

## Recommended pipeline

```text
intent
  -> normalize task labels
  -> collect repository metadata
  -> discover scoped instruction sources
  -> exact metadata filtering
  -> BM25 retrieval
  -> vector retrieval
  -> candidate normalization
  -> scoring
  -> deduplication
  -> conflict resolution
  -> context budget trimming
  -> bundle rendering
```

## Candidate normalization

Every candidate should be normalized into a common shape.

```json
{
  "text": "Preserve refund idempotency.",
  "source": "domain.payments.refunds@7",
  "source_type": "policy",
  "scope": "backend/payments/**",
  "priority": 90,
  "status": "active",
  "signals": {
    "path_match": true,
    "risk_match": true,
    "bm25_score": 4.2,
    "vector_score": 0.81
  }
}
```

## Filtering

Before ranking, filter out candidates that are clearly invalid.

Examples:

- `status: deprecated`;
- `status: disabled`;
- source path excluded by config;
- policy outside the current repository scope;
- instruction source under ignored directories such as `node_modules`;
- unsafe or untrusted remote source unless explicitly configured.

## Scoring signals

Suggested scoring factors:

```text
+100 global safety rule
+90 exact path match
+80 risk flag match
+70 domain match
+60 task type match
+50 language/framework match
+40 high semantic similarity
+35 nested instruction source applies to touched path
+30 repeated prior review feedback
+20 recent incident or regression history
+10 keyword match
-50 duplicate guidance
-80 generic advice
-100 deprecated or disabled policy
```

The implementation may choose different numbers, but scoring should remain deterministic and explainable.

## Reranking

Reranking should prefer candidates that are:

- specific to the touched path;
- safety-relevant;
- task-relevant;
- owned and active;
- short and actionable;
- backed by explicit policy metadata;
- compatible with higher-priority instructions.

Semantic similarity alone should not be enough to make a candidate high priority.

## Deduplication

Candidates with overlapping meaning should be merged.

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

The compiled instruction should keep provenance from all meaningful sources when useful.

## Budget trimming

The broker should apply budget limits after scoring and deduplication.

Example:

```yaml
output_budget:
  max_tokens: 900
  max_instructions: 8
  max_required_checks: 4
  max_blocked_actions: 4
```

When over budget, omit lower-scoring or redundant candidates.

The broker should optionally report omission statistics:

```json
{
  "candidate_policies_considered": 14,
  "candidate_policies_omitted": 9,
  "reason": "Lower priority or duplicate guidance excluded by context budget."
}
```

## Required checks

Required checks should be ranked too.

Prefer:

1. checks from exact path/domain policies;
2. checks from package-specific instructions;
3. checks from repo metadata;
4. broad checks only when needed.

Avoid returning every possible test command when a narrower command is available.

## Explainability

Instruction bundles should expose enough detail to debug selection.

Example:

```json
{
  "text": "Do not edit generated OpenAPI files directly.",
  "source": "org.generated-files@2",
  "reason": "Matched generated-file policy and path `openapi/generated/**`."
}
```

## Determinism

Given the same registry commit, repository state, config, and task intent, the broker should produce the same instruction bundle.

If vector retrieval is nondeterministic, the final compiler should still stabilize ordering through deterministic tie breakers such as:

- priority;
- source path;
- policy ID;
- version;
- lexical order as a final fallback.
