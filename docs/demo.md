# Public Demo Scenario

This demo is a copy-paste runnable walkthrough of Agent Policy Broker from the repository root. It uses only committed synthetic fixtures under `examples/`, runs offline, and writes derived cache/build artifacts to a temporary directory.

## Setup

```bash
DEMO_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/agent-policy-demo.XXXXXX")"
export XDG_CACHE_HOME="$DEMO_ROOT/cache"
export CARGO_TARGET_DIR="$DEMO_ROOT/target"
export APB="cargo run -q -p agent-policy-cli --"
printf 'Demo temp directory: %s\n' "$DEMO_ROOT"
```

What this demonstrates: the demo keeps Agent Policy Broker indexes under `XDG_CACHE_HOME` and Cargo build output under a temp target directory. It does not require network access or private repositories.

## 1. Validate a Repository

```bash
$APB --repo examples/single-repo --no-network validate --format markdown
```

What this demonstrates: `validate` checks the example repository config and local policy files before an agent relies on them.

## 2. Discover Existing Instructions

```bash
$APB --repo examples/single-repo --no-network discover --format json
```

What this demonstrates: `discover` finds the existing `AGENTS.md` bootstrap and extracts candidate instructions and required checks from committed repository guidance.

## 3. Compile a Task-Specific Bundle

```bash
$APB --repo examples/single-repo --no-network get \
  --format markdown \
  --task "fix duplicate refund retry" \
  --type fix_bug \
  --risk payments \
  --files src/payments/refunds.ts tests/payments/refunds.test.ts
```

What this demonstrates: `get` combines local policies with discovered `AGENTS.md` guidance to return a compact, task-specific instruction bundle for a payment bug fix. Because the temp cache is new, the output should also report that metadata and full-text indexes are missing and direct loading was used.

## 4. Show Codex-Compatible Discovery

```bash
$APB --repo examples/nested-instructions --no-network discover \
  --mode codex \
  --format json
```

What this demonstrates: Codex-compatible discovery follows the configured project-root-to-current-directory `AGENTS.md` chain in `examples/nested-instructions`, including nested guidance for `services/api/src`.

## 5. Build Local Indexes

```bash
$APB --repo examples/single-repo --no-network index --format markdown
```

What this demonstrates: `index` builds rebuildable metadata and full-text indexes under `$XDG_CACHE_HOME/agent-policy`. No committed files are changed.

## 6. Show Indexed `get` Behavior

```bash
$APB --repo examples/single-repo --no-network get \
  --format markdown \
  --task "fix duplicate refund retry" \
  --type fix_bug \
  --risk payments \
  --files src/payments/refunds.ts tests/payments/refunds.test.ts
```

What this demonstrates: after indexing, `get` uses the local index manifest and full-text index for candidate retrieval. The missing-index warnings from the earlier `get` call should no longer appear.

## 7. Preview Migration from Existing `AGENTS.md`

```bash
$APB --repo examples/migration --no-network migrate --dry-run --format markdown
```

What this demonstrates: migration mode conservatively drafts policy YAML from existing `AGENTS.md` files without writing generated migration files. To write drafts for inspection, run `migrate --write` intentionally; the public demo keeps this dry-run only.

## 8. Optional Localhost Service Check

Run this only when binding a local port is allowed in your environment.

```bash
$APB --repo examples/single-repo --no-network serve --host 127.0.0.1 --port 8765
```

In another terminal:

```bash
curl -sS http://127.0.0.1:8765/health
curl -sS http://127.0.0.1:8765/instructions \
  -H 'Content-Type: application/json' \
  -d '{"task":"fix duplicate refund retry","type":"fix_bug","risk":["payments"],"files":["src/payments/refunds.ts","tests/payments/refunds.test.ts"]}'
```

What this demonstrates: the service exposes repeated local lookups through `/health` and `/instructions`. It still uses local fixture policies and the same temp cache when launched with the setup environment.

## Scripted Run

The default scripted demo runs steps 1 through 7 and skips the optional service check:

```bash
bash scripts/demo.sh
```

What this demonstrates: the main public demo path can be smoke-tested as one safe command. It creates a temp directory, sets `XDG_CACHE_HOME`, runs offline, and does not mutate committed files.
