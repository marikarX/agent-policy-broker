# Single Repo Agent Instructions

Before editing, request task-specific policy guidance:

```bash
cargo run -p agent-policy-cli -- --repo examples/single-repo get --task "$USER_TASK"
```

For payment files, include the relevant paths:

```bash
cargo run -p agent-policy-cli -- --repo examples/single-repo get --task "$USER_TASK" --type fix_bug --risk payments --files src/payments/refunds.ts tests/payments/refunds.test.ts
```

Follow the returned policy bundle and run the required checks that apply to the change.
