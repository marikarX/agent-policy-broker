# Registry mode and WSL workflow

Agent Policy Broker should support two storage modes:

1. single-repo mode, where policies live inside the application repository;
2. registry mode, where shared policies live in a separate Git repository.

Registry mode is the recommended setup for teams with multiple repositories.

The policy registry is the source of truth. Metadata, BM25, and vector indexes are derived artifacts built from the registry. For the detailed model, see [Storage and indexing model](storage-and-indexing.md).

## Why use a separate policy registry?

A separate policy registry gives teams:

- one source of truth for shared agent instructions;
- normal pull-request review for policy changes;
- CODEOWNERS-based ownership;
- version pinning and rollbacks;
- cross-repository consistency;
- less duplication across `AGENTS.md`, `CLAUDE.md`, Cursor rules, and Copilot instructions.

Application repositories keep only a small bootstrap file and a pointer to the registry.

## Repository layout

Application repository:

```text
billing-api/
  AGENTS.md
  .agent-policy.yaml
  src/
  tests/
```

Policy registry repository:

```text
agent-policy-registry/
  config.yaml
  ownership.yaml
  policies/
    org/
      security.yaml
      testing.yaml
      generated-files.yaml
    languages/
      typescript.yaml
      python.yaml
      go.yaml
    frameworks/
      jest.yaml
      pytest.yaml
      react.yaml
    domains/
      payments.yaml
      auth.yaml
      billing.yaml
    repos/
      billing-api.yaml
      web-app.yaml
  docs/
    payments/
    auth/
    migrations/
```

## App repo bootstrap

Example `AGENTS.md`:

```md
# Dynamic coding-agent instructions

Before changing code, run:

```bash
agent-policy get --repo . --task "$USER_TASK"
```

Follow the returned instruction bundle unless it conflicts with higher-priority instructions.
```

Example `.agent-policy.yaml`:

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

index:
  include:
    - docs
    - README.md
  exclude:
    - src
    - secrets
    - node_modules
```

## WSL setup

When used from WSL, the broker can interact with the policy registry through normal Git.

Recommended local layout:

```text
~/code/billing-api/
  AGENTS.md
  .agent-policy.yaml

~/.cache/agent-policy/
  registries/
    company/
      .git/
      policies/
      docs/
      config.yaml
  indexes/
    company/
      manifest.json
      metadata.sqlite
      bm25.sqlite
      vectors/
  bundles/
    apb_2026-05-31_001.json
```

One-time setup:

```bash
mkdir -p ~/.cache/agent-policy/registries

git clone git@github.com:company/agent-policy-registry.git \
  ~/.cache/agent-policy/registries/company

agent-policy index --registry company
```

Per application repository:

```bash
cd ~/code/billing-api
agent-policy init
```

The coding agent then runs:

```bash
agent-policy get --repo . --task "$USER_TASK"
```

## Request flow

When `agent-policy get` runs inside WSL, the broker should:

1. read the application repository path;
2. load `.agent-policy.yaml`;
3. resolve the registry URL and ref;
4. ensure the registry is cloned locally;
5. update or reuse the cached registry according to sync settings;
6. read policy modules and registry docs;
7. read application repository metadata;
8. query metadata, BM25, and vector indexes when available;
9. rerank candidates and compile a concise instruction bundle;
10. print JSON or Markdown to stdout.

## Index lifecycle

Indexes should be rebuilt from the registry, not edited by hand.

Expected lifecycle:

```text
policy registry commit
  -> agent-policy registry sync
  -> agent-policy index
  -> metadata/BM25/vector indexes
  -> agent-policy get
  -> concise instruction bundle
```

The index manifest should record the registry commit used. If the registry changes, the broker should warn or rebuild the affected indexes.

## Sync modes

The registry should support explicit sync behavior.

```yaml
registry:
  sync:
    mode: auto
    max_age_minutes: 15
```

Recommended modes:

- `manual`: update only when the user runs `agent-policy registry sync`;
- `auto`: fetch or pull when the local cache is older than `max_age_minutes`;
- `pinned`: use an exact commit SHA and do not auto-update;
- `offline`: use local cache only.

Pinned example:

```yaml
registry:
  type: git
  url: git@github.com:company/agent-policy-registry.git
  ref: 9d3c5f1a7b2e
  sync:
    mode: pinned
```

Instruction bundles should report the registry commit used:

```json
{
  "registry": {
    "url": "git@github.com:company/agent-policy-registry.git",
    "ref": "main",
    "commit": "9d3c5f1"
  }
}
```

## CLI-only mode

CLI-only mode is the recommended first implementation.

```text
Coding agent
  -> runs agent-policy get
  -> CLI reads app repo
  -> CLI reads cached policy registry
  -> CLI queries derived indexes when available
  -> CLI outputs instruction bundle
```

Benefits:

- simple;
- portable;
- easy to debug;
- no daemon lifecycle;
- works naturally from `AGENTS.md` and similar instruction files.

## Local service mode

A later implementation may run a local service inside WSL:

```bash
agent-policy serve --host 127.0.0.1 --port 8765
```

Then the CLI can call the local service:

```text
agent-policy get
  -> http://127.0.0.1:8765/instructions
```

Benefits:

- keeps the registry warm;
- keeps the vector index loaded;
- improves repeated-call latency;
- gives VS Code extensions a stable local endpoint.

The service should bind to localhost by default and should not expose repository data to the network unless explicitly configured.

## Authentication

For a Git-backed registry, use normal Git authentication inside WSL.

SSH is the recommended default:

```bash
ssh-keygen -t ed25519 -C "agent-policy"
ssh-add ~/.ssh/id_ed25519
git clone git@github.com:company/agent-policy-registry.git
```

HTTPS with a Git credential manager can also work, but SSH is usually simpler in WSL.

## Precedence

Recommended policy precedence:

1. global safety policy;
2. organization policies from the registry;
3. domain policies from the registry;
4. repository-specific policies from the registry;
5. repository-local policies;
6. conventions inferred from nearby code.

Repository-local policies should be able to extend shared policy. They should not weaken reviewed registry policies unless the local source is explicitly configured as trusted.
