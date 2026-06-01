# Nested Instructions Example

This example demonstrates Codex-compatible nested `AGENTS.md` discovery. The config sets `codex.current_dir` to `services/api/src`, so Codex mode follows the root-to-current-directory instruction chain.

From the Agent Policy Broker repository root, run:

```bash
cargo run -p agent-policy-cli -- --repo examples/nested-instructions discover --mode codex --format json
cargo run -p agent-policy-cli -- --repo examples/nested-instructions inspect --mode codex --format markdown
cargo run -p agent-policy-cli -- --repo examples/nested-instructions get --instruction-mode codex --format markdown --task "add an API health field" --type add_feature --files services/api/src/handler.ts
```

Workflow demonstrated:

- `discover --mode codex` returns only the active `AGENTS.md` chain for the configured current directory.
- `inspect --mode codex` audits the layered instructions.
- `get --instruction-mode codex` includes applicable nested instruction candidates with matching policies.
