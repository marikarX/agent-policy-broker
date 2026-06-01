# Privacy

Agent Policy Broker is designed to be local-first and privacy-conscious by default.

This document describes the intended privacy posture for the open-source project. It is not a commercial privacy policy for a hosted service.

## Current Project Status

The repository currently contains a local Rust CLI and localhost service prototype. There is no hosted service implementation in this repository.

## Local-first expectation

The open-source core should be able to run locally without sending repository data, source code, prompts, policy files, task intent, or retrieval indexes to an external service.

A local invocation such as:

```bash
agent-policy get --repo . --task "fix refund retry handling"
```

uses local configuration and local policy files. A configured Git registry is loaded from a local filesystem path or already-cloned cache; remote registry fetch is not implemented in the MVP.

## Data the CLI may process locally

A local broker implementation may inspect local repository metadata such as:

- repository name or path;
- current branch;
- changed file paths;
- configured policy files;
- package metadata such as `package.json`, `pyproject.toml`, `go.mod`, or similar files;
- CI or test command configuration;
- user-provided task summaries;
- explicit file paths passed through CLI flags;
- selected documentation paths explicitly included in a local retrieval index.

The CLI should avoid reading full source files unless a feature explicitly requires it and the behavior is documented.

## Local Retrieval Indexes

The MVP can build local metadata and full-text indexes under the user cache directory, usually `~/.cache/agent-policy/indexes`. The index is built from local policy files, discovered Markdown instruction files, and documentation paths explicitly listed in `index.include`.

Local indexing should follow these rules:

- indexing should be explicit;
- indexed paths should be configurable;
- source code should not be indexed by default;
- vector indexing is disabled by default and is not part of the MVP CLI indexing path;
- generated index files should remain local unless the user explicitly moves or uploads them;
- users should be able to delete and rebuild the index;
- documentation should clearly explain what is indexed.

## Data that should not be collected by default

The open-source core should not collect or transmit by default:

- source code contents;
- secrets, tokens, keys, or credentials;
- private customer data;
- full prompts or chat transcripts;
- environment variables, except those explicitly required for configuration;
- personally identifiable information beyond what is necessary for local operation;
- local vector indexes or retrieval caches.

## Remote service behavior

If a future implementation supports a remote policy service or remote registry fetch, remote behavior must be explicit and documented.

Remote mode should clearly disclose:

- the endpoint being called;
- what request fields are sent;
- whether file contents are sent;
- whether task summaries are sent;
- whether retrieval vectors, snippets, or index metadata are sent;
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
- free of source code, secrets, private policy contents, and local retrieval indexes.

## Logs

Logs should avoid printing secrets or source code. Debug logs should be opt-in and should make it clear when potentially sensitive metadata may be shown.

## Examples and tests

Examples, tests, fixtures, documentation, and issue templates should use synthetic data only.

Do not commit real secrets, customer data, proprietary policies, or private repository contents.

## Security reports

If you find a privacy or security issue, please avoid posting sensitive exploit details publicly. Until a dedicated security policy is added, open a minimal issue describing the area affected and request a private disclosure path.
