# Configuration

Agent Policy Broker should support repository-local configuration through `.agent-policy.yaml`.

Configuration should be explicit, local-first, and safe by default.

## Example

```yaml
registry:
  type: git
  url: git@github.com:company/agent-policy-registry.git
  ref: main
  cache_dir: ~/.cache/agent-policy/registries/company
  sync:
    mode: auto
    max_age_minutes: 15

local_policies:
  - .agent-policy/policies

instruction_sources:
  include:
    - AGENTS.md
    - CLAUDE.md
    - .github/copilot-instructions.md
    - .cursor/rules/**
    - "**/AGENTS.md"
    - "**/CLAUDE.md"
  exclude:
    - node_modules/**
    - vendor/**

index:
  include:
    - .agent-policy/policies
    - docs
    - README.md
  exclude:
    - src
    - secrets
    - node_modules

output_budget:
  max_tokens: 900
  max_instructions: 8
  max_required_checks: 4
  max_blocked_actions: 4
  include_examples: false
  include_explanations: compact
```

## `registry`

Configures a shared policy registry.

```yaml
registry:
  type: git
  url: git@github.com:company/agent-policy-registry.git
  ref: main
  cache_dir: ~/.cache/agent-policy/registries/company
```

Fields:

```text
type        Registry backend. Initial supported value: git.
url         Git remote URL.
ref         Branch, tag, or commit SHA.
cache_dir   Local cache path.
```

## `registry.sync`

Controls registry update behavior.

```yaml
registry:
  sync:
    mode: auto
    max_age_minutes: 15
```

Supported modes:

```text
manual   Update only when `agent-policy registry sync` is run.
auto     Fetch or pull when cache is older than max_age_minutes.
pinned   Use an exact commit SHA and do not auto-update.
offline  Use local cache only.
```

## `local_policies`

Paths to repository-local policy files.

```yaml
local_policies:
  - .agent-policy/policies
```

Local policies can extend registry policies. They should not weaken reviewed registry policies unless the local source is explicitly configured as trusted.

## `instruction_sources`

Controls discovery of existing agent instruction files.

```yaml
instruction_sources:
  include:
    - AGENTS.md
    - "**/AGENTS.md"
    - CLAUDE.md
    - "**/CLAUDE.md"
    - .github/copilot-instructions.md
    - .cursor/rules/**
  exclude:
    - node_modules/**
    - vendor/**
```

Instruction sources are path-scoped. A file at `backend/AGENTS.md` applies to `backend/**`.

## `index`

Controls which files are indexed for local retrieval.

```yaml
index:
  include:
    - .agent-policy/policies
    - docs
    - README.md
  exclude:
    - src
    - secrets
    - node_modules
```

Source code should not be indexed by default. Users may explicitly include source paths if they understand the privacy and performance tradeoffs.

## `output_budget`

Controls the size of instruction bundles returned to agents.

```yaml
output_budget:
  max_tokens: 900
  max_instructions: 8
  max_required_checks: 4
  max_blocked_actions: 4
  include_examples: false
  include_explanations: compact
```

The broker should omit lower-priority non-mandatory candidates instead of exceeding the budget. Budget values that can be influenced by a repository branch, pull request, issue, task prompt, or other untrusted source must be authorized or clamped to safe operator-defined minimums before use. They must not reduce `max_required_checks`, `max_blocked_actions`, or token limits enough to hide global safety rules, mandatory validation commands, blocked actions, or other safety-critical controls; if mandatory controls cannot fit, the broker should fail closed.

## Configuration precedence

Recommended precedence, from highest to lowest:

1. CLI flags;
2. explicit `--config` file;
3. repository `.agent-policy.yaml`;
4. registry `config.yaml`;
5. built-in defaults.

Higher-precedence trusted configuration may narrow output or choose a specific registry ref. Configuration from untrusted repository branches or task-controlled inputs must not weaken global safety or reviewed registry policy behavior, and any accepted budget hints must be clamped to safe operator-defined minimums.

## Safe defaults

Default behavior should be conservative:

- no telemetry;
- no remote service calls unless configured;
- no source-code indexing by default;
- no deletion or rewriting of existing instruction files;
- local-only registry and index caches;
- localhost binding for local service mode.
