# Migration Example

This example starts with legacy `AGENTS.md` files and one existing draft policy. It demonstrates inspection and conservative policy draft generation for guidance that has not been migrated yet.

From the Agent Policy Broker repository root, run:

```bash
cargo run -p agent-policy-cli -- --repo examples/migration inspect --format markdown
cargo run -p agent-policy-cli -- --repo examples/migration migrate --dry-run --format markdown
cargo run -p agent-policy-cli -- --repo examples/migration migrate --write --format json
cargo run -p agent-policy-cli -- --repo examples/migration validate --format markdown
```

Workflow demonstrated:

- `inspect` finds legacy instructions and migration candidates.
- `migrate --dry-run` previews generated draft policies.
- `migrate --write` writes drafts under `.agent-policy/migration`.
- `validate` checks the existing draft policy and any generated migration drafts.

The generated migration drafts are intentionally not committed in this example.
