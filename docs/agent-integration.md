# Agent integration

Agent Policy Broker should integrate with coding agents through the simplest possible mechanism first: command execution.

MCP support may be useful later, but it should not be required for the open-source core.

## Basic pattern

```text
1. Add a small bootstrap instruction file to the repository.
2. Tell the coding agent to run `agent-policy get` before editing code.
3. The command prints task-specific instructions.
4. The agent follows those instructions.
```

## Codex / AGENTS.md

Example `AGENTS.md`:

```md
# Agent instructions

Before changing code, run:

```bash
agent-policy get --repo . --task "$USER_TASK"
```

If relevant files are known, include them with `--files`.

Follow the returned instructions unless they conflict with higher-priority system, developer, or user instructions.

If the command fails, follow the fallback rules in this file and report that policy lookup was unavailable.
```

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
Applied Agent Policy Broker bundle 2026-05-31.1. Ran npm run lint and npm test -- tests/payments.
```

This makes agent behavior easier to audit.
