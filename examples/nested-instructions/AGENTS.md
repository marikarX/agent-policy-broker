# Workspace Instructions

Use Agent Policy Broker before changes:

```bash
cargo run -p agent-policy-cli -- --repo examples/nested-instructions get --instruction-mode codex --task "$USER_TASK"
```

Shared rule: keep examples small and avoid generated files.
