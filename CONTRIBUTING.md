# Contributing

Thanks for your interest in Agent Policy Broker.

The project is early. Contributions are especially useful in these areas:

- policy schema design
- CLI interface design
- examples for different coding agents
- sample policies for common stacks
- deterministic policy selection logic
- validation rules
- documentation improvements

## Project principles

Before contributing, please keep these principles in mind:

1. **Deterministic first**: policy selection should be reproducible and explainable.
2. **Small instruction bundles**: avoid returning large generic handbooks to agents.
3. **Local-first**: the open-source core should work without a hosted service.
4. **Policy as code**: policies should be reviewable, versioned, and owned.
5. **Vendor-neutral**: the project should work with multiple coding agents.
6. **Privacy-conscious by default**: avoid collecting source code, secrets, or unnecessary repository data.

## Ways to contribute

Good first contributions include:

- improving documentation clarity;
- adding examples for Codex, Claude Code, Cursor, GitHub Copilot, or other coding agents;
- proposing policy schema changes;
- adding sample policies for common languages and frameworks;
- documenting tradeoffs and open design questions.

Implementation contributions should preserve the local-first architecture unless the change is explicitly about optional hosted or remote operation.

## Documentation style

Documentation should be:

- direct;
- implementation-oriented;
- free of unnecessary marketing language;
- explicit about tradeoffs and non-goals;
- clear about whether a behavior is implemented, proposed, or future work.

## Policy examples

Good policy instructions are specific and actionable:

```yaml
instructions:
  - Add tests for provider retry and duplicate refund request.
  - Do not edit generated OpenAPI files directly.
```

Avoid vague instructions:

```yaml
instructions:
  - Write clean code.
  - Be careful.
```

## Privacy and sensitive data

Do not include real secrets, tokens, private customer data, proprietary code, or confidential internal policies in examples, issues, tests, or documentation.

When adding examples, use synthetic names and paths such as:

```text
src/payments/refunds.ts
tests/payments/refunds.test.ts
billing-api
```

Do not paste private repository contents into issues or discussions unless you have permission to publish them.

## Pull requests

When opening a pull request, include:

- what changed;
- why it changed;
- examples or tests when relevant;
- any open design questions;
- whether the change affects privacy, telemetry, or remote service behavior.

## Commit style

Prefer clear commit messages with a conventional prefix when practical:

```text
docs: clarify policy schema
feat: add local policy loader
fix: handle missing policy directory
chore: update examples
```

## License

By contributing to this repository, you agree that your contributions are provided under the repository's [MIT License](LICENSE).
