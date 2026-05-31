# Project scope

Agent Policy Broker is an open-source, local-first project for compiling concise, task-specific instruction bundles for coding agents.

This document clarifies what belongs in the open-source core and how to discuss future deployment modes without over-specifying commercial plans.

## Open-source core

The open-source core should include:

- local CLI;
- local configuration;
- Git-backed policy registry support;
- policy schema and validation;
- instruction source discovery;
- repository inspection and migration reports;
- local metadata and keyword indexes;
- optional local vector index;
- deterministic retrieval, ranking, and conflict resolution;
- local instruction bundle compilation;
- safe defaults and privacy-conscious behavior;
- examples for common coding-agent workflows.

## Optional deployment modes

The project may support different deployment modes over time:

- CLI-only local mode;
- local service mode for editor integrations;
- self-hosted organization mode;
- remote service mode when explicitly configured.

Public documentation should focus on technical interfaces, safety properties, and local-first behavior.

## What not to over-specify publicly

Avoid documenting pricing, packaging tiers, or proprietary commercial differentiation in the open-source docs.

It is fine to describe generic organization-scale needs such as:

- shared policy registries;
- approval workflows;
- audit logs;
- multi-repository rollout;
- compliance reporting;
- editor and CI integrations.

But these should be framed as possible deployment capabilities, not as paid-feature commitments.

## Documentation rule

Use neutral language:

```text
Future organization deployments may support...
```

Avoid language like:

```text
Paid tier includes...
Enterprise plan includes...
```

This keeps the repository focused on adoption, trust, and implementation clarity.
