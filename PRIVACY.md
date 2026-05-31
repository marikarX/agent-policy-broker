# Privacy

Agent Policy Broker is designed to be local-first and privacy-conscious by default.

This document describes the intended privacy posture for the open-source project. It is not a commercial privacy policy for a hosted service.

## Current project status

The repository is currently in the documentation and design phase. There is no implemented hosted service in this repository at this time.

## Local-first expectation

The open-source core should be able to run locally without sending repository data, source code, prompts, policy files, or task intent to an external service.

A local invocation such as:

```bash
agent-policy get --repo . --task "fix refund retry handling"
```

should use local configuration and local policy files unless the user explicitly configures a remote endpoint.

## Data the CLI may process locally

A local broker implementation may inspect local repository metadata such as:

- repository name or path;
- current branch;
- changed file paths;
- configured policy files;
- package metadata such as `package.json`, `pyproject.toml`, `go.mod`, or similar files;
- CI or test command configuration;
- user-provided task summaries;
- explicit file paths passed through CLI flags.

The CLI should avoid reading full source files unless a feature explicitly requires it and the behavior is documented.

## Data that should not be collected by default

The open-source core should not collect or transmit by default:

- source code contents;
- secrets, tokens, keys, or credentials;
- private customer data;
- full prompts or chat transcripts;
- environment variables, except those explicitly required for configuration;
- personally identifiable information beyond what is necessary for local operation.

## Remote service behavior

If a future implementation supports a remote policy service, remote behavior must be explicit and documented.

Remote mode should clearly disclose:

- the endpoint being called;
- what request fields are sent;
- whether file contents are sent;
- whether task summaries are sent;
- what metadata is logged;
- how long logs are retained;
- how users can disable remote mode;
- whether telemetry is collected.

Remote mode should prefer sending minimal structured intent rather than source code.

## Telemetry

The open-source core should not enable telemetry by default.

If telemetry is introduced later, it should be:

- opt-in for local development use;
- documented in this file;
- configurable through environment variables or config files;
- limited to operational metadata;
- free of source code, secrets, and private policy contents.

## Logs

Logs should avoid printing secrets or source code. Debug logs should be opt-in and should make it clear when potentially sensitive metadata may be shown.

## Examples and tests

Examples, tests, fixtures, documentation, and issue templates should use synthetic data only.

Do not commit real secrets, customer data, proprietary policies, or private repository contents.

## Security reports

If you find a privacy or security issue, please avoid posting sensitive exploit details publicly. Until a dedicated security policy is added, open a minimal issue describing the area affected and request a private disclosure path.
