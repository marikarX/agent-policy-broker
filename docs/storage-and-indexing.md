# Storage and indexing model

Agent Policy Broker separates authoritative policy storage from derived retrieval indexes.

The short version:

```text
Git policy repo = source of truth
Metadata index = exact filtering layer
BM25 index = keyword retrieval layer
Vector index = semantic recall layer
Reranker/compiler = runtime decision layer
Instruction bundle = generated output
```

These layers complement each other. Storing instructions in a Git repository does not conflict with vector search, BM25, metadata filters, path/risk rules, reranking, or deterministic policy priority.

## Source of truth

The policy registry repository is the canonical source of truth.

Example:

```text
agent-policy-registry/
  config.yaml
  ownership.yaml
  policies/
    org/security.yaml
    domains/payments.yaml
    languages/typescript.yaml
    repos/billing-api.yaml
  docs/
    payments/webhooks.md
    testing/jest.md
```

The Git repository provides:

- human-readable policy files;
- pull-request review;
- CODEOWNERS-based ownership;
- history and rollback;
- version pinning;
- reproducible registry commits.

Policy authors edit this repository, not the generated indexes.

## Derived indexes

Indexes are generated artifacts. They are built from the policy registry and selected documentation.

Example local cache:

```text
~/.cache/agent-policy/
  registries/
    company/
      .git/
      policies/
      docs/
  indexes/
    company/
      manifest.json
      metadata.sqlite
      bm25.tantivy/
      vectors/
```

Indexes can be deleted and rebuilt. They should not be treated as the source of truth.

## Metadata index

The metadata index stores exact fields used for filtering and deterministic scoring.

Examples:

- policy ID;
- policy version;
- policy status;
- owner;
- priority;
- repository matchers;
- path globs;
- language;
- framework;
- task type;
- risk flag;
- required checks;
- blocked actions.

The metadata index helps enforce governance rules such as `status: active` and policy precedence.

## BM25 / keyword index

The BM25 index supports keyword-based retrieval.

The recommended OSS implementation is a Tantivy index stored as a directory, for example `bm25.tantivy/`. The docs should not imply SQLite FTS unless a future implementation intentionally chooses that backend.

It is useful for exact or near-exact words such as:

- framework names;
- provider names;
- command names;
- package names;
- domain terms;
- error names.

BM25 complements vector search because exact words still matter.

## Vector index

The vector index supports semantic retrieval.

It is useful when task wording differs from policy wording.

Example:

```text
Task wording:
  repeated Stripe refund callback

Relevant policy wording:
  webhook handlers must preserve idempotency
```

A vector index can help connect the task to the relevant policy even when the exact words differ.

The vector index should provide candidate guidance. It should not directly decide which instructions the coding agent receives.

## Runtime decision layer

At runtime, the broker combines signals:

```text
metadata filters
+ path/risk rules
+ BM25 keyword matches
+ vector semantic matches
+ deterministic policy priority
+ reranking
+ context budget
= final instruction bundle
```

The final instruction bundle should cite policy IDs, versions, and the registry commit used.

## Example pipeline

```text
1. User edits policy in Git
2. Pull request reviews and approves the policy
3. Registry commit changes
4. `agent-policy registry sync` fetches the commit
5. `agent-policy index` rebuilds derived indexes
6. `agent-policy get` queries metadata, BM25, and vector indexes
7. Broker reranks and compiles concise instructions
8. Output cites source policies and registry commit
```

## Staleness handling

The index manifest should record the registry commit it was built from.

Example `manifest.json`:

```json
{
  "registry": {
    "url": "git@github.com:company/agent-policy-registry.git",
    "ref": "main",
    "commit": "9d3c5f1"
  },
  "indexes": {
    "metadata": "metadata.sqlite",
    "bm25": "bm25.tantivy/",
    "vector": "vectors/"
  },
  "created_at": "2026-05-31T18:00:00Z"
}
```

If the registry commit changes, the broker should warn or rebuild the indexes.

## Conflict handling

The main conflicts are operational, not architectural.

| Risk | Expected handling |
|---|---|
| Index is stale | Store registry commit in manifest and rebuild when it changes. |
| Vector search finds deprecated policy | Filter by metadata such as `status: active`. |
| Semantic result is plausible but low authority | Rerank lower unless metadata also matches. |
| Repo-local policy reduces reviewed registry policy | Precedence rules prevent branch-controlled local policy from reducing registry policy unless explicitly trusted. |
| Sensitive docs get indexed | Require explicit include paths and safe defaults. |
| Indexes disagree | Deterministic compiler decides final bundle. |

## Design rule

Treat the policy registry like source code and indexes like build artifacts.

```text
Git policy repo : source
Indexes         : compiled artifacts
Bundle          : runtime output
```

That separation keeps the system auditable while still enabling semantic retrieval and concise task-specific instruction bundles.
