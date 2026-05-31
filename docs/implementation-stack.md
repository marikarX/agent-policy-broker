# Implementation stack

Agent Policy Broker should be implemented as a local-first developer tool.

The recommended stack is:

```text
Core CLI and local service: Rust
Future VS Code extension: TypeScript
Research/prototyping scripts: Python, optional
```

## Decision

Use Rust for the open-source core.

Rust is a good fit because Agent Policy Broker needs:

- fast filesystem scanning;
- deterministic policy selection;
- Git-backed registry sync;
- local metadata, BM25, and vector indexes;
- JSON and Markdown output;
- cross-platform CLI distribution;
- optional local service mode;
- privacy-conscious local execution.

## Recommended Rust components

| Need | Recommended component |
|---|---|
| CLI | `clap` |
| JSON/YAML/TOML parsing | `serde`, `serde_json`, `serde_yaml`, `toml` |
| Filesystem walking | `ignore`, `walkdir` |
| Glob and path matching | `globset` |
| Git registry sync | `gix` or `git2` |
| Metadata store | SQLite through `rusqlite` |
| BM25 / full-text search | `tantivy` |
| Vector search | `sqlite-vec` first; evaluate LanceDB later |
| Local service | `axum` |
| Logging and diagnostics | `tracing` |
| CLI tests | `assert_cmd`, `tempfile` |
| Snapshot tests | `insta` |

## Proposed crate layout

```text
crates/
  agent-policy-cli/       command-line interface
  agent-policy-core/      policy model, bundle compiler, scoring, conflicts
  agent-policy-config/    config loading and validation
  agent-policy-discover/  instruction source discovery
  agent-policy-git/       registry sync and commit metadata
  agent-policy-index/     metadata, BM25, and vector indexes
  agent-policy-server/    optional localhost service
```

The CLI should be thin. Most behavior should live in reusable library crates so the same engine can power:

- CLI commands;
- local service mode;
- GitHub Action integration;
- future MCP server;
- future VS Code extension bridge.

## Phased implementation

### Phase 1: Local CLI core

Implement:

- `agent-policy get`;
- `agent-policy discover`;
- policy loading;
- `.agent-policy.yaml` loading;
- nested instruction discovery;
- path/risk/language/framework matching;
- JSON and Markdown bundle output;
- basic validation.

Avoid heavy vector complexity in the first implementation.

### Phase 2: Local indexes

Implement:

- SQLite metadata index;
- Tantivy BM25 index;
- index manifest with registry commit;
- stale-index detection;
- `agent-policy index`.

### Phase 3: Inspection and migration

Implement:

- `agent-policy inspect`;
- duplicated instruction detection;
- conflict reports;
- migration candidate reports;
- draft policy generation through `agent-policy migrate --dry-run`.

### Phase 4: Vector-assisted retrieval

Add optional local vector retrieval after exact matching and BM25 are working.

Recommended starting point:

- `sqlite-vec` for local-first vector search;
- keep vector retrieval optional;
- keep policy registry and metadata as source of truth;
- never dump raw vector results directly into agent context.

Evaluate LanceDB later if richer local vector and hybrid search capabilities become necessary.

### Phase 5: Local service and integrations

Implement:

- `agent-policy serve --host 127.0.0.1 --port 8765`;
- localhost API for editor integrations;
- optional MCP server;
- TypeScript VS Code extension bridge.

## Why not all TypeScript?

TypeScript is a good fit for editor integrations and VS Code extensions, but less ideal for the local-first core because the broker needs native filesystem performance, local indexing, easy binary distribution, and deterministic CLI behavior.

## Why not all Python?

Python is useful for retrieval experiments, migration heuristics, and notebooks. It is not the best default for a polished cross-platform CLI because packaging and runtime dependencies are more complex for end users.

## Why not Go?

Go is a strong alternative for simple static binaries and local services. Rust is preferred here because of strong type modeling, excellent filesystem tooling, and the availability of Tantivy for local full-text search.

## Design rule

The implementation should optimize for:

```text
single local binary
safe defaults
deterministic output
rebuildable indexes
clear provenance
future editor/service reuse
```
