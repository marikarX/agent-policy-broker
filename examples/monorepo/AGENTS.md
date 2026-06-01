# Monorepo Instructions

Use Agent Policy Broker with the files being changed:

```bash
cargo run -p agent-policy-cli -- --repo examples/monorepo get --task "$USER_TASK" --files packages/web/src/App.tsx
```

Keep package-specific conventions in the nearest package `AGENTS.md`.
