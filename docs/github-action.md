# GitHub Actions PR example

The workflow at `.github/workflows/agent-policy-example.yml` is an example for running Agent Policy Broker during pull requests. It is not a published marketplace action and does not require secrets.

The example:

- runs on `pull_request`;
- checks out the repository;
- installs a stable Rust toolchain;
- builds the local `agent-policy` binary with Cargo;
- finds files changed against the pull request base branch;
- runs `agent-policy get` with the changed file paths;
- falls back to `agent-policy inspect` when no changed files are found;
- writes a Markdown report to the GitHub Actions job summary;
- does not fail the pull request by default.

## Adapting the workflow

Copy `.github/workflows/agent-policy-example.yml` into a repository that contains Agent Policy Broker, or use it as a starting point for your own CI job.

Common changes:

- Change the build step if `agent-policy` is installed another way. For example, replace `cargo build --release -p agent-policy-cli` with an internal package install command, then set `binary` in the report step to the installed executable.
- Change the `task` value in the report step to describe the kind of pull request review you want policy guidance for.
- Change `--type review` to another task type if your policies distinguish between documentation, refactoring, bug fixes, migrations, or other work.
- Add `--risk` flags when the workflow can infer sensitive areas from paths, labels, or branch names.
- Replace `agent-policy get` with `agent-policy inspect` if you want an instruction-source audit instead of task-specific guidance.
- Set `AGENT_POLICY_FAIL_ON_REPORT` to `"true"` only after the report is stable enough to block pull requests.

## Changed files

The example fetches the pull request base branch and computes:

```bash
git diff --name-only --diff-filter=ACMR "$base_commit" HEAD
```

Only added, copied, modified, and renamed paths are passed to `agent-policy get`. Deleted files are omitted because they may no longer exist in the checkout.

## Summary output

The workflow appends a Markdown section to `$GITHUB_STEP_SUMMARY`. This makes the policy report visible on the workflow run without requiring comments, tokens, or API calls.

If `agent-policy` exits with a non-zero status, the example records the error in the summary. By default, the job still succeeds. To make policy failures block the pull request, change:

```yaml
AGENT_POLICY_FAIL_ON_REPORT: "true"
```

## Permissions and secrets

The example uses only:

```yaml
permissions:
  contents: read
  pull-requests: read
```

No repository secrets, GitHub token writes, or marketplace action publishing are required.
