# Storage and indexing model

Agent Policy Broker separates authoritative policy storage from derived retrieval indexes.

The short version:

```text
Git policy repo = source of truth
Metadata index = exact filtering layer
Full-text index = keyword retrieval layer
Vector index = future semantic recall layer
Reranker/compiler = runtime decision layer
Instruction bundle = generated output
```

These layers complement each other. Storing instructions in a Git repository does not conflict with full-text search, future vector search, metadata filters, path/risk rules, reranking, or deterministic policy priority.

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

Indexes are generated artifacts. They are built from local policies or the configured local registry cache, plus selected documentation configured through `index.include`.

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
      fulltext/
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

## Full-Text / Keyword Index

The full-text index supports keyword-based retrieval.

The MVP implementation uses a Tantivy index stored in a `fulltext/` directory.

It is useful for exact or near-exact words such as:

- framework names;
- provider names;
- command names;
- package names;
- domain terms;
- error names.

Full-text search complements future vector search because exact words still matter.

## Vector Index

Vector indexing is planned but is not part of the MVP CLI indexing path. It should remain local-only unless future remote behavior is explicitly configured and documented.

It is useful when task wording differs from policy wording.

Example:

```text
Task wording:
  repeated Stripe refund callback

Relevant policy wording:
  webhook handlers must preserve idempotency
```

Future vector retrieval can help connect the task to the relevant policy even when the exact words differ.

Vector retrieval should provide candidate guidance. It should not directly decide which instructions the coding agent receives.

## Runtime decision layer

At runtime, the broker combines signals:

```text
metadata filters
+ path/risk rules
+ full-text keyword matches
+ future vector semantic matches
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
4. The local registry cache is updated outside the MVP, or `agent-policy registry sync` validates the existing cache
5. `agent-policy index` rebuilds derived indexes
6. `agent-policy get` queries metadata and full-text indexes
7. Broker reranks and compiles concise instructions
8. Output cites source policies and registry commit
```

## Staleness handling

The index manifest should record the registry commit it was built from.

Example `manifest.json`:

```json
{
  "schema_version": 2,
  "source": {
    "kind": "registry",
    "name": "company",
    "path": "/home/user/.cache/agent-policy/registries/company",
    "url": "/home/user/.cache/agent-policy/registries/company",
    "ref": "main",
    "commit": "9d3c5f1"
  },
  "indexes": {
    "metadata": "metadata.sqlite",
    "fulltext": "fulltext"
  },
  "created_at_unix": 1780279200
}
```

If the registry commit changes, the broker should warn or rebuild the indexes.

## Conflict handling

The main conflicts are operational, not architectural.

| Risk | Expected handling |
|---|---|
| Index is stale | Store registry commit in manifest and rebuild when it changes. |
| Future vector search finds deprecated policy | Filter by metadata such as `status: active`. |
| Future semantic result is plausible but low authority | Rerank lower unless metadata also matches. |
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
