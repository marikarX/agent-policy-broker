# Example AGENTS.md

This file demonstrates how a repository can bootstrap Agent Policy Broker.

## Dynamic task instructions

Before making code changes, run:

```bash
agent-policy get --repo . --task "$USER_TASK"
```

If the relevant files are known, include them:

```bash
agent-policy get \
  --repo . \
  --task "$USER_TASK" \
  --files src/example.ts tests/example.test.ts
```

Use the returned instruction bundle as task-specific guidance.

## Precedence

Follow instructions in this order:

1. system, developer, and user instructions
2. this repository's static instructions
3. Agent Policy Broker's returned task instructions
4. conventions inferred from nearby code

If instructions conflict, stop and report the conflict.

## Fallback

If `agent-policy get` fails:

- make the smallest safe change;
- inspect nearby code and tests;
- avoid migrations, generated files, public API changes, credentials, and security-sensitive paths unless explicitly requested;
- run the narrowest relevant checks;
- report that dynamic policy lookup was unavailable.

## Final response

When possible, mention:

- policy bundle version used;
- checks run;
- any unavailable policy lookup or skipped checks.
