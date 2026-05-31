# Threat model

Agent Policy Broker can influence coding-agent behavior. The threat model should focus on preventing unsafe or misleading instructions from reaching agents.

## Assets

Important assets include:

- source repositories;
- policy registry contents;
- local retrieval indexes;
- generated instruction bundles;
- task intent data;
- repository metadata;
- credentials used for Git access;
- local service endpoints.

## Trust boundaries

Key boundaries:

```text
coding agent
  -> CLI
  -> local config
  -> app repo
  -> policy registry
  -> local indexes
  -> optional local service
  -> optional remote service
```

Each boundary should be explicit and auditable.

## Threats and mitigations

### Malicious or compromised policy registry

Risk: a registry policy instructs agents to run unsafe commands or leak data.

Mitigations:

- require Git review for registry changes;
- record registry commit in bundles;
- support CODEOWNERS;
- validate policies;
- fail closed for unsafe commands;
- keep generated policies in `draft` status until reviewed.

### Prompt injection in documentation

Risk: indexed docs contain text that tries to override agent behavior.

Mitigations:

- do not pass raw retrieved chunks directly to agents;
- compile retrieved content into concise instructions;
- preserve source provenance;
- treat docs as supporting knowledge, not authority;
- prefer explicit policy files over unstructured docs.

### Stale indexes

Risk: the broker uses old policy data after registry changes.

Mitigations:

- store registry commit in index manifest;
- warn when index commit differs from registry commit;
- rebuild indexes when registry changes;
- include registry commit in instruction bundles.

### Sensitive data in indexes

Risk: secrets or private data are indexed locally or sent remotely.

Mitigations:

- do not index source code by default;
- use explicit include paths;
- exclude common sensitive paths;
- keep indexes local by default;
- document remote mode clearly before sending any data.

### Unsafe local service exposure

Risk: a local service exposes policy or repository data over the network.

Mitigations:

- bind to `127.0.0.1` by default;
- require explicit config for non-localhost binding;
- avoid unauthenticated remote access;
- keep local service optional.

### Conflicting policies

Risk: local instructions weaken reviewed organization, domain, risk, or task policies.

Mitigations:

- apply deterministic precedence;
- make reviewed registry policies non-weakenable by branch-controlled local files unless explicitly trusted;
- report conflicts;
- fail closed for safety or authority conflicts.

### Overloaded context

Risk: agents receive too much context and ignore important instructions.

Mitigations:

- enforce context budgets;
- deduplicate candidate guidance;
- omit low-priority candidates;
- return concise instruction bundles instead of raw docs.

## Security posture

The safest default posture is:

```text
local-only
explicit registry configuration
explicit indexing paths
no telemetry
no raw source-code indexing
no raw vector-search dumps
human-reviewed policy activation
```
