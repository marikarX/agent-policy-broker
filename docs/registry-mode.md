# Registry mode and WSL workflow

Agent Policy Broker supports two MVP storage modes:

1. single-repo mode, where policies live inside the application repository;
2. registry mode, where shared policies live in a separate Git repository.

Registry mode is the recommended setup for teams with multiple repositories.

The policy registry is the source of truth. Metadata and Tantivy full-text indexes are derived artifacts built from the registry. Vector indexes are planned but are not part of the MVP CLI indexing path. For the detailed model, see [Storage and indexing model](storage-and-indexing.md).

## Why use a separate policy registry?

A separate policy registry gives teams:

- one source of truth for shared agent instructions;
- normal pull-request review for policy changes;
- CODEOWNERS-based ownership;
- version pinning and rollbacks;
- cross-repository consistency;
- less duplication across `AGENTS.md`, `CLAUDE.md`, Cursor rules, and Copilot instructions.

Application repositories can keep only a small bootstrap file and local policy hints. The MVP can read full registry settings from `.agent-policy.yaml` or `--config`, but hardened deployments should provide registry URLs, refs, cache locations, and sync behavior from trusted operator-controlled configuration rather than branch-controlled repository files.

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

Example repository-local `.agent-policy.yaml`:

```yaml
registry:
  type: git
  url: ~/.cache/agent-policy/registries/company
  ref: main
  cache_dir: ~/.cache/agent-policy/registries/company
  sync:
    mode: manual

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

The MVP uses local filesystem registries or already-cloned cache directories. It does not clone, fetch, or pull remote registries. Future hardened deployments may let repository-local files select only an operator-defined registry `id`, with the broker resolving that `id` through trusted configuration such as `/etc/agent-policy/registries.yaml`, `$XDG_CONFIG_HOME/agent-policy/registries.yaml`, or CI-managed protected settings.

Trusted operator configuration example:

```yaml
registries:
  company:
    type: git
    url: git@github.com:company/agent-policy-registry.git
    ref: main
    cache_dir: ~/.cache/agent-policy/registries/company
    sync:
      mode: auto
      max_age_minutes: 15
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
      fulltext/
  bundles/
    apb_2026-05-31_001.json
```

One-time setup:

```bash
mkdir -p ~/.cache/agent-policy/registries

git clone git@github.com:company/agent-policy-registry.git \
  ~/.cache/agent-policy/registries/company

agent-policy index --repo ~/code/billing-api
```

Per application repository:

```bash
cd ~/code/billing-api
edit .agent-policy.yaml to point at the local registry cache
```

The coding agent then runs:

```bash
agent-policy get --repo . --task "$USER_TASK"
```

## Request flow

When `agent-policy get` runs inside WSL, the MVP broker:

1. read the application repository path;
2. load `.agent-policy.yaml` or the explicit `--config` file;
3. resolve local policy and registry cache paths;
4. reject remote registry fetch behavior because clone, fetch, and pull are not implemented;
5. read policy modules and configured local documentation;
6. discover path-scoped instruction files;
7. query metadata and full-text indexes when available;
8. rank candidates and compile a concise instruction bundle;
9. print JSON or Markdown to stdout.

## Index lifecycle

Indexes should be rebuilt from the registry, not edited by hand.

Expected lifecycle:

```text
policy registry commit
  -> agent-policy registry sync
  -> agent-policy index
  -> metadata/full-text indexes
  -> agent-policy get
  -> concise instruction bundle
```

The index manifest should record the registry commit used. If the registry changes, the broker should warn or rebuild the affected indexes.

## Sync modes

Trusted registry configuration should support explicit sync behavior. Branch-controlled repository files must not be allowed to choose `auto` sync or change the registry source.

```yaml
registries:
  company:
    sync:
      mode: auto
      max_age_minutes: 15
```

Recommended modes:

- `manual`: use the configured local cache when the user runs `agent-policy registry sync`;
- `auto`: MVP behavior is cached-only; remote fetch or pull is not implemented;
- `pinned`: use an exact commit SHA and do not auto-update;
- `offline`: use local cache only.

Pinned trusted-configuration example:

```yaml
registries:
  company:
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

Recommended policy precedence, after registry identity and source have been resolved from trusted configuration:

1. global safety policy;
2. organization policies from the registry;
3. domain policies from the registry;
4. repository-specific policies from the registry;
5. repository-local policies;
6. conventions inferred from nearby code.

Repository-local policies should be able to extend shared policy from the trusted registry. They should not weaken global safety policies or cause an untrusted registry to be treated as shared policy.
