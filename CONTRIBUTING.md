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

## Documentation style

Documentation should be:

- direct
- implementation-oriented
- free of unnecessary marketing language
- explicit about tradeoffs and non-goals

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

## Pull requests

When opening a pull request, include:

- what changed
- why it changed
- examples or tests when relevant
- any open design questions

## License note

No license has been selected yet. Until a license is added, external reuse rights are not formally granted. Contributors should wait for a license decision before submitting substantial code.
