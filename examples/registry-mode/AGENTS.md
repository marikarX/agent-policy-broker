# Registry Mode Instructions

Sync the configured local registry, then request task-specific guidance:

```bash
cargo run -p agent-policy-cli -- --repo examples/registry-mode registry sync
cargo run -p agent-policy-cli -- --repo examples/registry-mode get --task "$USER_TASK"
```

Local repo policies may add repository-specific guidance, but registry policies remain the shared source of truth.
