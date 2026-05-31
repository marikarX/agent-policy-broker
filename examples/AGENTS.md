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
2. Agent Policy Broker's returned global and organization safety instructions
3. this repository's static instructions
4. other Agent Policy Broker returned task instructions
5. conventions inferred from nearby code

If instructions conflict, stop and report the conflict. Treat conflicts involving
broker-supplied safety instructions as blocking, and do not let repository-local
instructions weaken global or organization safety policy.

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
