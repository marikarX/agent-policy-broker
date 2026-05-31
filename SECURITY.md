# Security

Agent Policy Broker is intended to influence coding-agent behavior. Security issues should be handled carefully.

## Reporting a vulnerability

If you discover a security issue, please do not publish sensitive details in a public issue.

Until a dedicated private disclosure channel is configured, open a minimal public issue that says you have a security report and request a private contact path. Do not include exploit details, secrets, private repository content, or sensitive logs.

## Security principles

Agent Policy Broker should follow these principles:

- local-first by default;
- no telemetry by default;
- no source-code indexing by default;
- no remote uploads unless explicitly configured;
- deterministic policy selection;
- auditable instruction bundles;
- explicit provenance for generated instructions;
- conservative behavior for unsafe commands and conflicting safety policies.

## Sensitive data

Do not commit secrets, tokens, credentials, customer data, or private repository contents to this repository.

Examples, tests, and documentation should use synthetic data.

## Local service security

If local service mode is implemented, it should bind to `127.0.0.1` by default.

It should not expose repository metadata, policy contents, retrieval indexes, or instruction bundles to the network unless explicitly configured.

## Policy registry security

The Git policy registry should be treated as a trusted input source.

Implementations should record the registry commit used for each instruction bundle and should warn when indexes are stale relative to the registry commit.

## Unsafe instructions

Policy files, nested instruction files, or retrieved docs may contain unsafe commands or prompt-injection-like content.

The broker should not blindly forward raw retrieved text to coding agents. It should compile concise instructions, preserve provenance, and block or warn on unsafe instructions.
