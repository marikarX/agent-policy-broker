# Single Repo Example

This example shows a small repository with local policies and a root `AGENTS.md` bootstrap file.

From the Agent Policy Broker repository root, run:

```bash
cargo run -p agent-policy-cli -- --repo examples/single-repo validate
cargo run -p agent-policy-cli -- --repo examples/single-repo discover --format json
cargo run -p agent-policy-cli -- --repo examples/single-repo get --format markdown --task "fix duplicate refund retry" --type fix_bug --risk payments --files src/payments/refunds.ts tests/payments/refunds.test.ts
```

Workflow demonstrated:

- `validate` checks the local `.agent-policy.yaml` and policy schema.
- `discover` finds the repository instruction file.
- `get` combines local policies with discovered instructions for a payment bug fix.
