#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEMO_ROOT="${APB_DEMO_ROOT:-$(mktemp -d "${TMPDIR:-/tmp}/agent-policy-demo.XXXXXX")}"

export XDG_CACHE_HOME="$DEMO_ROOT/cache"
export CARGO_TARGET_DIR="$DEMO_ROOT/target"

APB=(cargo run -q -p agent-policy-cli --)

cd "$ROOT_DIR"

run_step() {
  local title="$1"
  shift
  printf '\n## %s\n\n' "$title"
  printf '+'
  printf ' %q' "$@"
  printf '\n\n'
  "$@"
}

printf 'Agent Policy Broker public demo\n'
printf 'Repository: %s\n' "$ROOT_DIR"
printf 'Demo temp directory: %s\n' "$DEMO_ROOT"
printf 'XDG_CACHE_HOME: %s\n' "$XDG_CACHE_HOME"
printf 'CARGO_TARGET_DIR: %s\n' "$CARGO_TARGET_DIR"

run_step "1. Validate a repository" \
  "${APB[@]}" --repo examples/single-repo --no-network validate --format markdown

run_step "2. Discover existing instructions" \
  "${APB[@]}" --repo examples/single-repo --no-network discover --format json

run_step "3. Compile a task-specific bundle before indexes exist" \
  "${APB[@]}" --repo examples/single-repo --no-network get \
    --format markdown \
    --task "fix duplicate refund retry" \
    --type fix_bug \
    --risk payments \
    --files src/payments/refunds.ts tests/payments/refunds.test.ts

run_step "4. Show Codex-compatible discovery mode" \
  "${APB[@]}" --repo examples/nested-instructions --no-network discover \
    --mode codex \
    --format json

run_step "5. Build local indexes" \
  "${APB[@]}" --repo examples/single-repo --no-network index --format markdown

run_step "6. Compile the same bundle after indexes exist" \
  "${APB[@]}" --repo examples/single-repo --no-network get \
    --format markdown \
    --task "fix duplicate refund retry" \
    --type fix_bug \
    --risk payments \
    --files src/payments/refunds.ts tests/payments/refunds.test.ts

run_step "7. Preview migration from existing AGENTS.md files" \
  "${APB[@]}" --repo examples/migration --no-network migrate --dry-run --format markdown

printf '\nDemo complete. Optional localhost service commands are documented in docs/demo.md.\n'
