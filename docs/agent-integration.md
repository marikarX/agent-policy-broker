# Agent integration

Agent Policy Broker should integrate with coding agents through the simplest possible mechanism first: command execution.

MCP support may be useful later, but it should not be required for the open-source core.

## Integration modes

There are two integration styles:

1. **Manual bootstrap**: a user or repository maintainer adds a small instruction file that tells the agent to run `agent-policy get` before editing code.
2. **Broker activation**: `agent-policy activate` archives existing instruction files, imports or migrates useful guidance, and replaces the active instruction file with a small broker bootstrap.

Lookup commands do not mutate instruction files. Activation and deactivation are explicit lifecycle operations. See [Activation lifecycle](activation-lifecycle.md).

## Basic runtime pattern

```text
1. The coding agent reads a bootstrap instruction file.
2. The bootstrap tells the agent to classify the task and run `agent-policy get` before editing code.
3. The command prints task-specific instructions.
4. The agent follows those instructions.
5. The agent reports the policy version and checks run.
```

## Bootstrap guidance

A useful bootstrap should ask the agent to classify the user task as one of:

```text
fix_bug
add_feature
refactor
test
docs
```

It should include relevant files whenever they are known, and risk flags only when obvious.

Example:

````md
# Agent instructions

Before changing code, classify the user task as one of:

- `fix_bug` — fix incorrect behavior, typo, broken build, failing test, or regression
- `add_feature` — add new user-visible or API behavior
- `refactor` — restructure code without intended behavior change
- `test` — add or update tests only
- `docs` — documentation-only change

Then request task-specific policy guidance:

```bash
agent-policy get --repo . --task "$USER_TASK" --type "<task_type>"
```

If relevant files are known, include them:

```bash
agent-policy get \
  --repo . \
  --task "$USER_TASK" \
  --type "<task_type>" \
  --files path/to/file1 path/to/file2
```

If risk is obvious, include one or more risk flags:

```bash
agent-policy get \
  --repo . \
  --task "$USER_TASK" \
  --type "<task_type>" \
  --files path/to/file1 \
  --risk auth public_api migrations secrets
```

Use only applicable risk flags. Do not invent risk flags just to fill the argument.

Follow the returned instruction bundle. If lookup fails, make the smallest safe change, inspect nearby code and tests, avoid risky areas unless explicitly requested, and report that policy lookup was unavailable.

In the final response, mention the policy version used and checks run.
````

## Global Codex activation

Global Codex activation is useful when users want Codex to ask the broker for instructions across repositories, or when many repositories ignore or do not commit `AGENTS.md`.

Planned commands:

```bash
agent-policy activate codex --global --dry-run
agent-policy activate codex --global --write
```

Global activation should inspect active Codex instruction files such as `AGENTS.override.md` and `AGENTS.md` under the configured Codex home, archive the original files, import reusable guidance into broker-managed global policy or supporting knowledge, and replace the active global instruction file with a small broker bootstrap.

Rollback should be available through:

```bash
agent-policy deactivate codex --global --dry-run
agent-policy deactivate codex --global --restore
```

## Repo activation

Repo activation is useful when a repository should carry its own broker bootstrap and policy files.

Planned commands:

```bash
agent-policy activate repo --repo . --dry-run
agent-policy activate repo --repo . --write
```

Repo activation should inspect tracked repo instruction files, generate or update policy drafts when requested, archive originals, and replace active repo instruction files with a broker bootstrap.

If `AGENTS.md` is ignored by Git, repo activation must not silently create a local-only bootstrap. It should require one of:

```text
--local                   create a local ignored bootstrap
--force-track-bootstrap   create and force-add a tracked bootstrap
--global                  use global Codex activation instead
```

Rollback should be available through:

```bash
agent-policy deactivate repo --repo . --dry-run
agent-policy deactivate repo --repo . --restore
```

## Codex-compatible discovery

For Codex-compatible discovery, configure the broker and call:

```bash
agent-policy get --repo . --instruction-mode codex --task "$USER_TASK"
```

In this mode the broker mirrors Codex `AGENTS.md` loading: optional global instructions from `CODEX_HOME`, `AGENTS.override.md` before `AGENTS.md`, fallback project filenames only when neither standard file exists, and one active file per directory from project root to the configured current directory. Empty files are skipped, and truncated or omitted files are reported in JSON discovery metadata.

## Claude Code / CLAUDE.md

Example `CLAUDE.md`:

```md
# Project guidance

Before editing code, request task-specific policy guidance:

```bash
agent-policy get --repo . --task "$USER_TASK"
```

Apply the returned instruction bundle for the current task.
```

Claude integrations can use manual bootstrap first. A future activation flow may support archiving and replacing `CLAUDE.md` with a broker bootstrap in the same reversible way as repo activation.

## Cursor rules

Cursor rules can use the same pattern: instruct the agent to run the CLI before making task-specific edits.

## GitHub Copilot coding agent

For Copilot-style workflows, the broker can be used in one of two ways:

1. bootstrap instructions ask the agent to run the CLI;
2. a GitHub Action or PR check calls the broker and comments with required policies/checks.

The second option is useful when the coding agent cannot reliably run the command itself.

## MCP mode

MCP is optional. A future MCP server could expose operations such as:

- `get_task_instructions`
- `explain_policy_selection`
- `list_matching_policies`
- `validate_policy_file`

MCP can improve native tool integration, but a CLI should remain the baseline because it works across many coding-agent environments.

## Output formats

The CLI should support at least:

```bash
agent-policy get --format json
agent-policy get --format markdown
```

JSON is best for automation. Markdown is best for direct agent consumption.

## Final-response convention

Agents should be encouraged to mention the applied policy version in their final response, for example:

```text
Applied Agent Policy Broker bundle 2026-05-31.1. Ran trusted checks typescript.lint and payments.unit_tests.
```

This makes agent behavior easier to audit.
