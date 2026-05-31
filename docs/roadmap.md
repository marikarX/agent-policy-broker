# Roadmap

This roadmap is intentionally small and implementation-oriented.

## Phase 0: Documentation scaffold

- Define project purpose
- Draft architecture
- Draft policy schema
- Document agent integration patterns
- Add example policies and bootstrap files

## Phase 1: Local CLI MVP

Goal: make the project useful without any hosted service.

Planned capabilities:

- `agent-policy get`
- load YAML policies from `.agent-policy/policies` or a configured directory
- accept task intent through flags
- accept task intent through JSON input
- detect basic repository metadata
- match policies by language, framework, path, task type, and risk flag
- return JSON or Markdown instruction bundles
- include source policy IDs and versions
- provide safe failure behavior

## Phase 2: Policy validation

Planned capabilities:

- `agent-policy validate`
- schema validation for policy files
- warnings for vague instructions
- warnings for overly broad policies
- detection of duplicate policy IDs
- basic conflict checks

## Phase 3: Better context detection

Planned capabilities:

- detect package manager
- detect test framework
- detect changed files from Git
- read CODEOWNERS
- map files to likely language/framework/domain
- support generated-file and sensitive-path configuration

## Phase 4: PR and CI integration

Planned capabilities:

- GitHub Action example
- PR comment showing selected policies
- required checks derived from selected policy bundle
- policy drift detection for repos missing bootstrap files

## Phase 5: Optional MCP server

Planned capabilities:

- expose policy lookup as MCP tools
- expose selected policies as MCP resources
- support coding agents that prefer native tool calls over command execution

## Phase 6: Organization control plane

Potential paid or separately deployed capabilities:

- central policy registry
- policy approvals
- audit logs
- SSO and RBAC
- multi-repo rollout
- analytics
- compliance reports

## Out of scope for early versions

- fully autonomous policy-writing agents
- vector database as a default dependency
- hosted-only operation
- replacing CI, tests, code review, or security review
- general-purpose agent orchestration
