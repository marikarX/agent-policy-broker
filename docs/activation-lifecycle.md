# Activation lifecycle

Agent Policy Broker can be used in two ways:

1. **Lookup only**: an existing `AGENTS.md`, `CLAUDE.md`, editor rule, or user habit calls `agent-policy get` before a coding task.
2. **Activated**: the broker imports existing instruction sources, archives the originals, replaces active instruction files with a small bootstrap, validates the result, builds indexes, and provides a rollback path.

Activation is the lifecycle that turns scattered static instructions into broker-managed instruction delivery. It is intentionally separate from normal lookup commands.

## Goals

Activation should:

- preserve existing instructions before modifying anything;
- convert useful instruction content into broker-managed policy, supporting knowledge, or archived provenance;
- discover references between instruction files before deciding what to replace;
- recommend native, shared, wrapper, global, local-only, or CI/comment activation strategies based on the existing instruction graph;
- replace active instruction files with a small bootstrap that tells the coding agent to call `agent-policy get`;
- validate and index the prepared policy state;
- smoke-test that policy lookup returns a useful bundle;
- provide a first-class deactivation command that restores the previous instruction files.

## Non-goals

Activation should not:

- silently delete instruction files;
- rewrite global agent configuration without an explicit `--write` request;
- treat ignored or untracked repo instruction files as shared repo policy by default;
- flatten an existing hybrid instruction layout without recording its references;
- execute policy-provided shell commands from untrusted repository content;
- make `get`, `discover`, `inspect`, `validate`, or `index` mutate instruction files.

## Modes

### Agent adapters

Activation should be implemented through agent adapters. Codex is one adapter, not the architecture.

Planned command shape:

```bash
agent-policy activate <agent> --global --dry-run
agent-policy activate <agent> --global --write
agent-policy activate <agent> --repo . --dry-run
agent-policy activate <agent> --repo . --write

agent-policy deactivate <agent> --global --restore
agent-policy deactivate <agent> --repo . --restore
```

Initial adapters may include:

```text
codex
claude
copilot
cursor
gemini
generic
```

Each adapter should define native source files, native bootstrap targets, import syntax when available, and whether local command execution can be relied on.

### Global activation

Global activation configures a user's agent environment to ask the broker for task-specific instructions across repositories.

Example:

```bash
agent-policy activate codex --global --dry-run
agent-policy activate codex --global --write
```

A Codex adapter should inspect global Codex instruction sources such as:

```text
~/.codex/AGENTS.override.md
~/.codex/AGENTS.md
$CODEX_HOME/AGENTS.override.md
$CODEX_HOME/AGENTS.md
```

The exact Codex home resolution should match the configured `codex.home`, then `CODEX_HOME`, then the default Codex home when applicable.

A Claude adapter may inspect global Claude sources such as:

```text
~/.claude/CLAUDE.md
~/.claude/rules/**/*.md
```

A Gemini adapter may inspect global Gemini context files such as:

```text
~/.gemini/GEMINI.md
~/.gemini/AGENTS.md
```

Global activation should:

1. discover active global instruction files for the selected adapter;
2. build a reference graph between discovered instruction files;
3. archive the original files and write a manifest;
4. import reusable guidance into broker-managed global policy or supporting knowledge;
5. replace or wrap the global instruction entrypoint with a small broker bootstrap;
6. validate the resulting broker configuration;
7. optionally run an activation smoke test.

Global activation is useful when repositories do not commit `AGENTS.md`, when `AGENTS.md` is ignored by many users, or when a user wants the broker to be the default instruction source across repositories.

### Repo activation

Repo activation configures one repository to use the broker.

Example commands:

```bash
agent-policy activate generic --repo . --dry-run
agent-policy activate generic --repo . --write
```

Repo activation should inspect supported repo instruction sources, including adapter-native files and portable instruction files:

```text
AGENTS.override.md
AGENTS.md
**/AGENTS.override.md
**/AGENTS.md
CLAUDE.md
**/CLAUDE.md
GEMINI.md
**/GEMINI.md
.github/copilot-instructions.md
.github/instructions/**/*.instructions.md
.cursor/rules/**/*.mdc
```

Repo activation should:

1. run discovery and inspection;
2. classify instruction sources by path scope, trust, adapter, and Git state;
3. detect imports, references, symlinks, and duplicate wrapper files;
4. recommend a strategy such as native, shared, import bridge, wrapper, global, local-only, or CI/comment;
5. generate or update broker-managed policy drafts when requested;
6. archive files that will be replaced;
7. replace, wrap, or preserve active repo instruction files according to the selected strategy;
8. validate the policy/configuration state;
9. build or refresh local indexes;
10. print a rollback command.

Repo activation should only mutate files when `--write` is provided. Without `--write`, it should print a plan.

### Local-only activation

Some users intentionally keep `AGENTS.md` ignored or untracked. Local-only activation may create or update an ignored local bootstrap, but it should be explicit.

Example:

```bash
agent-policy activate generic --repo . --local --write
```

The command output should clearly state that the activation affects only the current checkout and is not shared with other users, CI, or remote coding agents.

## Hybrid activation strategies

The broker should not assume that each agent has an isolated instruction file. Many repositories use hybrid layouts where one instruction file imports, references, or wraps another.

Activation should detect these layouts during inspection and recommend the least disruptive strategy.

### Native strategy

Use the selected agent's native instruction file as the broker bootstrap.

Examples:

```text
Codex   -> AGENTS.md
Claude  -> CLAUDE.md
Cursor  -> .cursor/rules/agent-policy-broker.mdc
Gemini  -> GEMINI.md
Copilot -> .github/copilot-instructions.md
```

### Shared canonical strategy

Use one shared canonical instruction file, often `AGENTS.md`, and have agent-native files import or point to it when the agent supports that pattern.

Example:

```text
AGENTS.md         broker bootstrap
CLAUDE.md         imports or delegates to AGENTS.md
GEMINI.md         imports or delegates when supported/configured
```

### Import bridge strategy

Preserve an agent-native entrypoint but make it import or reference a shared broker bootstrap.

Example shape:

```text
CLAUDE.md     -> imports @AGENTS.md
AGENTS.md     -> broker bootstrap
```

The broker should record the import edge in the activation manifest.

### Wrapper strategy

Keep an agent-native file as a small wrapper around a shared APB-owned bootstrap file.

Example:

```text
.agent-policy/bootstrap.md
AGENTS.md
CLAUDE.md
GEMINI.md
.cursor/rules/agent-policy-broker.mdc
```

The wrapper files should stay small and should not duplicate large policy content.

### Global strategy

Use global agent activation and leave the repository to provide only broker config and policies.

Example:

```text
~/.codex/AGENTS.md        calls agent-policy get
repo/.agent-policy.yaml   repo config
repo/.agent-policy/**     repo policies and archive
```

This is useful when repos ignore `AGENTS.md` or when activation is personal.

### CI/comment strategy

For agents that cannot reliably run local commands, activation may be a CI or PR-comment workflow rather than an instruction-file workflow.

Example:

```text
GitHub Action runs agent-policy on changed files
        -> comments selected policy bundle on the PR
        -> coding agent and reviewers see expected policy guidance
```

This strategy is especially useful for cloud coding-agent workflows.

## Instruction reference graph

Inspection should build an instruction reference graph before activation.

The graph should include:

```text
source file
adapter or source type
path scope
Git state
trust level
outbound references
inbound references
symlink target when applicable
activation role: native entrypoint, shared canonical source, wrapper, supporting knowledge, or archive-only
```

Examples of references to detect:

```text
Claude-style imports such as @AGENTS.md
Markdown links to other instruction files
plain mentions of AGENTS.md, CLAUDE.md, GEMINI.md, or .cursor/rules files
symlinks between instruction files
agent-specific include/import syntax when supported
```

The broker should not blindly follow arbitrary remote links. Local file references should be normalized, bounded to configured roots, and recorded with errors or omissions when they cannot be resolved.

Activation plans should use the graph to avoid breaking existing hybrids. If `CLAUDE.md` imports `AGENTS.md`, replacing only `AGENTS.md` may be enough; replacing both files with duplicated bootstraps is usually worse.

## Git state classification

Instruction files must be classified by Git state before activation decisions are made.

```text
tracked      shared repo instruction source
untracked    local or draft instruction source
ignored      local-only instruction source
missing      no instruction source
```

Suggested probes:

```bash
git ls-files --error-unmatch AGENTS.md
git check-ignore -v AGENTS.md
git status --ignored --short AGENTS.md
```

If a repo `AGENTS.md` is ignored, repo activation must not silently create a local-only bootstrap. It should require one of:

```text
--local                   create a local ignored bootstrap
--force-track-bootstrap   create and force-add a tracked bootstrap
--global                  prefer global activation
```

If `AGENTS.md` is already tracked, ignore rules do not affect it.

## Archive layout

Activation archives must preserve the user's sharing intent for each instruction source. In particular, ignored and untracked repo instruction files are local-only or draft material and must not be copied into a repository path by default.

Repo activation archives for tracked instruction sources may use a repository-local archive only after the command ensures the archive path is protected from accidental staging, for example by creating or verifying an ignore rule for `.agent-policy/archive/` before writing archive contents:

```text
.agent-policy/
  archive/                 ignored before any files are written here
    activations/
      2026-06-01T22-30-00Z/
        manifest.json
        files/
          AGENTS.md
          frontend/AGENTS.md
          CLAUDE.md
```

Repo activation archives that contain ignored, untracked, or otherwise local-only instruction sources must live outside the repository by default, for example:

```text
~/.config/agent-policy/
  archive/
    repos/
      example-repo/
        activations/
          2026-06-01T22-30-00Z/
            manifest.json
            files/
              AGENTS.md
```

Global activation archives should also live outside the repository, for example:

```text
~/.config/agent-policy/
  archive/
    global/
      activations/
        2026-06-01T22-30-00Z/
          manifest.json
          files/
            AGENTS.md
            AGENTS.override.md
```

Archive locations should be configurable. A request to place local-only archive contents inside the repository must require an explicit opt-in flag, a warning that the contents may become visible to other users, CI, or remote agents, and a verified ignore rule for the chosen archive path before any archive file is written. Archives are provenance and rollback material; they should not be deleted automatically.

## Manifest schema

Each activation archive should include a manifest similar to:

```json
{
  "activation_id": "act_2026_06_01_223000",
  "mode": "repo",
  "agent": "claude",
  "strategy": "import_bridge",
  "repo": "/home/user/work/example",
  "created_at": "2026-06-01T22:30:00Z",
  "instruction_graph": [
    {
      "path": "CLAUDE.md",
      "role": "native_entrypoint",
      "references": ["AGENTS.md"]
    },
    {
      "path": "AGENTS.md",
      "role": "shared_canonical"
    }
  ],
  "modified_files": [
    {
      "path": "AGENTS.md",
      "state_before": "tracked",
      "action": "replaced_with_bootstrap",
      "archive_path": "files/AGENTS.md",
      "sha256_before": "...",
      "sha256_after": "..."
    }
  ],
  "created_files": [
    {
      "path": ".agent-policy/policies/repo.baseline.yaml",
      "action": "created"
    }
  ],
  "restore_command": "agent-policy deactivate claude --repo . --activation act_2026_06_01_223000 --restore"
}
```

The manifest should record enough information to support dry-run restore, conflict detection, exact rollback, and reconstruction of the instruction graph that activation changed.

## Bootstrap template

A minimal bootstrap should tell the coding agent to request task-specific instructions before editing.

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

If relevant files are known, include them with `--files`. If risk areas are obvious, include applicable `--risk` flags.

Follow the returned instruction bundle. If lookup fails, make the smallest safe change, inspect nearby code and tests, avoid risky areas unless explicitly requested, and report that policy lookup was unavailable.

In the final response, mention the policy version used and checks run.
````

The bootstrap should remain small. Detailed policy belongs in broker-managed policy files or indexed supporting knowledge.

## Deactivation and restore

Activation must be reversible.

Example commands:

```bash
agent-policy deactivate generic --repo . --dry-run
agent-policy deactivate generic --repo . --restore
agent-policy deactivate generic --repo . --activation act_2026_06_01_223000 --restore

agent-policy deactivate codex --global --dry-run
agent-policy deactivate codex --global --restore
```

Deactivation should:

1. find the requested activation archive;
2. verify current files still match the broker-managed state where possible;
3. restore archived files to their original paths;
4. restore or remove wrapper/import files according to the manifest;
5. remove broker-created bootstrap files when safe;
6. leave generated policies and indexes in place unless explicit cleanup flags are supplied;
7. print a summary of restored, removed, skipped, and conflicted files.

Optional cleanup flags may include:

```text
--remove-generated-policies
--remove-index
--force
```

If a file changed after activation, deactivation should refuse to overwrite it unless `--force` is supplied.

## Safety rules

Activation should be transactional:

1. compute the plan;
2. validate inputs;
3. archive originals;
4. write broker-managed files;
5. validate the result;
6. build indexes;
7. run smoke checks when requested.

If a step fails before writes, no files should change. If a step fails after writes, the command should print the restore command and archive location.

Activation and deactivation should support `--dry-run` and show exactly what would be changed.

## Command mutability

Read-only commands:

```text
agent-policy get
agent-policy discover
agent-policy inspect
agent-policy validate
agent-policy index
agent-policy migrate --dry-run
agent-policy activate ... --dry-run
agent-policy deactivate ... --dry-run
```

Mutating commands:

```text
agent-policy migrate --write
agent-policy activate ... --write
agent-policy deactivate ... --restore
```

Lookup commands must not rewrite instruction files.
