# Monorepo Example

This example shows package-scoped policies and package-scoped `AGENTS.md` files in one repository.

From the Agent Policy Broker repository root, run:

```bash
cargo run -p agent-policy-cli -- --repo examples/monorepo validate
cargo run -p agent-policy-cli -- --repo examples/monorepo discover --format json
cargo run -p agent-policy-cli -- --repo examples/monorepo get --format markdown --task "change the web greeting" --type add_feature --files packages/web/src/App.tsx
cargo run -p agent-policy-cli -- --repo examples/monorepo get --format markdown --task "handle blank API names" --type fix_bug --files packages/api/src/lib.rs
```

Workflow demonstrated:

- `validate` checks both package policies.
- `discover` finds root and package instruction files.
- `get` selects web or API policies based on the changed file path.
