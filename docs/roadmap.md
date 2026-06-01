# Roadmap

This roadmap is intentionally small and implementation-oriented.

## Phase 0: Documentation scaffold

- Define project purpose
- Draft architecture
- Draft policy schema
- Document agent integration patterns
- Add example policies and bootstrap files
- Document context budgeting and retrieval strategy
- Document nested instruction discovery and layered guidance
- Document implementation stack decision

## Phase 1: Rust workspace and local CLI core

Goal: create a small, testable Rust foundation without indexing, migration, networking, or local service mode.

Planned capabilities:

- create Rust workspace and crate layout;
- implement shared policy, intent, bundle, source, warning, and budget data models;
- implement `.agent-policy.yaml` loading with safe defaults;
- load YAML policies from `.agent-policy/policies` or a configured local directory;
- implement `agent-policy discover` for root and nested instruction files;
- implement basic `agent-policy get` using direct policy loading and deterministic matching;
- match policies by path, task type, risk flag, language, framework, and package manager;
- apply a simple output budget such as max instructions and max checks;
- return JSON or Markdown instruction bundles;
- include source policy IDs and versions;
- provide safe failure behavior.

Explicitly defer from Phase 1:

- `agent-policy index`;
- BM25;
- vector retrieval;
- `agent-policy serve`;
- `agent-policy inspect`;
- `agent-policy migrate`;
- network registry fetch or clone.

## Phase 2: Policy and instruction validation

Planned capabilities:

- `agent-policy validate`;
- schema validation for policy files;
- warnings for vague instructions;
- warnings for overly broad policies;
- detection of duplicate policy IDs;
- detection of duplicated guidance across nested instruction files;
- detection of conflicting package-manager or test-command guidance;
- basic conflict checks;
- warnings when policies are likely to exceed context budgets.

## Phase 3: Better context detection

Planned capabilities:

- detect package manager;
- detect test framework;
- detect changed files from Git;
- read CODEOWNERS;
- map files to likely language/framework/domain;
- map nested instruction files to directory scopes;
- support generated-file and sensitive-path configuration.

## Phase 4: Repository inspection and migration reports

Goal: help teams understand existing instruction files before moving them into broker policies.

Planned capabilities:

- `agent-policy inspect`;
- report discovered instruction files and path scopes;
- extract candidate instructions from Markdown;
- detect stale, duplicated, or conflicting nested instruction files;
- classify migration candidates;
- `agent-policy migrate --dry-run`;
- generate draft policies with provenance;
- keep generated policies as `status: draft`;
- avoid modifying existing instruction files by default.

## Phase 5: Local retrieval index

Goal: improve recall without sending repository data to a hosted service.

Planned capabilities:

- `agent-policy index`;
- local SQLite metadata index;
- Tantivy BM25 index stored as `bm25.tantivy/`;
- index manifest with registry commit;
- local index over policy files, nested instruction files, and selected documentation;
- stale-index detection;
- deduplication of overlapping instructions;
- omission reporting for candidates excluded by the context budget.

## Phase 6: Registry mode

Goal: support shared policy registries while keeping the local CLI useful without hosted infrastructure.

Planned capabilities:

- local filesystem registry support;
- cached Git registry support;
- registry commit provenance;
- `agent-policy registry sync` skeleton;
- offline and pinned modes;
- no network access when `--no-network` is set.

Network clone/fetch can remain limited until registry cache behavior and provenance are stable.

## Phase 7: BM25-assisted retrieval

Planned capabilities:

- collect BM25 candidates from the Tantivy index;
- normalize BM25 candidates with policy and discovered-instruction candidates;
- preserve exact metadata and policy priority as the authority;
- avoid raw search-result dumps into agent context;
- report candidate counts and omissions.

## Phase 8: Vector-assisted retrieval

Planned capabilities:

- optional local vector retrieval;
- evaluate `sqlite-vec` first, with a deterministic in-memory prototype used until the integration is stable;
- keep vector retrieval behind a feature flag if needed;
- preserve metadata and policy priority as the authority;
- avoid raw vector-search dumps into agent context.

## Phase 9: PR and CI integration

Planned capabilities:

- GitHub Action example;
- PR comment showing selected policies;
- required checks derived from selected policy bundle;
- policy drift detection for repos missing bootstrap files;
- detection of stale or conflicting nested instruction files;
- report candidate policies considered and omitted.

## Phase 10: Optional local service and MCP server

Planned capabilities:

- `agent-policy serve --host 127.0.0.1 --port 8765`;
- local HTTP API for editor integrations;
- expose policy lookup as MCP tools;
- expose selected policies as MCP resources;
- support coding agents that prefer native tool calls over command execution.

## Phase 11: Organization deployment patterns

Future organization deployments may support:

- central policy registry;
- organization-wide vector or semantic index;
- policy approvals;
- audit logs;
- SSO and RBAC;
- multi-repo rollout;
- analytics;
- compliance reports;
- duplicated instruction cleanup workflow.

These are deployment capabilities, not a commitment about packaging or pricing.

## Out of scope for early versions

- fully autonomous policy-writing agents;
- raw vector-search dumps into agent context;
- hosted-only operation;
- replacing CI, tests, code review, or security review;
- general-purpose agent orchestration.
