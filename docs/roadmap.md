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

## Phase 1: Rust workspace and local CLI MVP

Goal: make the project useful without any hosted service.

Planned capabilities:

- create Rust workspace and crate layout
- implement `agent-policy get`
- implement `agent-policy discover`
- load YAML policies from `.agent-policy/policies` or a configured directory
- load `.agent-policy.yaml`
- accept task intent through flags
- accept task intent through JSON input
- detect basic repository metadata
- discover root and nested instruction files such as `AGENTS.md`, `CLAUDE.md`, and editor rules
- match policies by language, framework, path, task type, and risk flag
- apply a simple output budget such as max instructions and max checks
- return JSON or Markdown instruction bundles
- include source policy IDs and versions
- provide safe failure behavior

## Phase 2: Policy and instruction validation

Planned capabilities:

- `agent-policy validate`
- schema validation for policy files
- warnings for vague instructions
- warnings for overly broad policies
- detection of duplicate policy IDs
- detection of duplicated guidance across nested instruction files
- detection of conflicting package-manager or test-command guidance
- basic conflict checks
- warnings when policies are likely to exceed context budgets

## Phase 3: Better context detection

Planned capabilities:

- detect package manager
- detect test framework
- detect changed files from Git
- read CODEOWNERS
- map files to likely language/framework/domain
- map nested instruction files to directory scopes
- support generated-file and sensitive-path configuration

## Phase 4: Local retrieval index

Goal: improve recall without sending repository data to a hosted service.

Planned capabilities:

- `agent-policy index`
- local SQLite metadata index
- Tantivy BM25 index
- local index over policy files, nested instruction files, and selected documentation
- semantic lookup for policy `retrieval.semantic_terms`
- hybrid retrieval combining exact metadata and semantic similarity
- deduplication of overlapping instructions
- omission reporting for candidates excluded by the context budget

## Phase 5: Repository inspection and migration

Planned capabilities:

- `agent-policy inspect`
- `agent-policy migrate --dry-run`
- report discovered instruction files and path scopes
- detect stale, duplicated, or conflicting nested instruction files
- generate draft policies with provenance
- support gradual migration to thin bootstrap files

## Phase 6: Vector-assisted retrieval

Planned capabilities:

- optional local vector retrieval
- evaluate `sqlite-vec` first
- keep vector retrieval behind a feature flag if needed
- preserve metadata and policy priority as the authority
- avoid raw vector-search dumps into agent context

## Phase 7: PR and CI integration

Planned capabilities:

- GitHub Action example
- PR comment showing selected policies
- required checks derived from selected policy bundle
- policy drift detection for repos missing bootstrap files
- detection of stale or conflicting nested instruction files
- report candidate policies considered and omitted

## Phase 8: Optional local service and MCP server

Planned capabilities:

- `agent-policy serve --host 127.0.0.1 --port 8765`
- local HTTP API for editor integrations
- expose policy lookup as MCP tools
- expose selected policies as MCP resources
- support coding agents that prefer native tool calls over command execution

## Phase 9: Organization deployment patterns

Future organization deployments may support:

- central policy registry
- organization-wide vector or semantic index
- policy approvals
- audit logs
- SSO and RBAC
- multi-repo rollout
- analytics
- compliance reports
- duplicated instruction cleanup workflow

These are deployment capabilities, not a commitment about packaging or pricing.

## Out of scope for early versions

- fully autonomous policy-writing agents
- raw vector-search dumps into agent context
- hosted-only operation
- replacing CI, tests, code review, or security review
- general-purpose agent orchestration
