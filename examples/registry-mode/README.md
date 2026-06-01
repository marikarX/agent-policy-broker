# Registry Mode Example

This example uses a local directory as a synthetic policy registry. It does not fetch from the network. The configured `registry.url` and `registry.cache_dir` both point at `./local-registry`, which the current CLI reports as a local path registry.

From the Agent Policy Broker repository root, run:

```bash
cargo run -p agent-policy-cli -- --repo examples/registry-mode registry sync --format markdown
cargo run -p agent-policy-cli -- --repo examples/registry-mode index --format markdown
cargo run -p agent-policy-cli -- --repo examples/registry-mode get --format markdown --task "fix auth token handling" --type fix_bug --risk auth --files src/auth/session.ts
```

Workflow demonstrated:

- `registry sync` verifies the configured local registry mode.
- `index` builds local metadata and full-text indexes for registry policies.
- `get` combines registry policies with repo-local policies.
