//! Core data models for Agent Policy Broker.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use globset::{Glob, GlobSetBuilder};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use walkdir::WalkDir;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Policy {
    pub id: String,
    pub version: PolicyVersion,
    pub status: PolicyStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<u32>,
    pub applies_when: AppliesWhen,
    pub instructions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_checks: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_actions: Vec<BlockedAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieval: Option<PolicyRetrieval>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, Value>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedPolicy {
    pub policy: Policy,
    pub source_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PolicyVersion {
    Integer(u64),
    Text(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyStatus {
    Draft,
    Active,
    Deprecated,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AppliesWhen {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub repos: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frameworks: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub package_managers: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub task_types: Vec<TaskType>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risk_flags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskIntent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<TaskDetails>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detected: Option<DetectedContext>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risk_flags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected_commands: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected_check_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_budget: Option<OutputBudget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskDetails {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub task_type: Option<TaskType>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DetectedContext {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frameworks: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_manager: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskType(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct OutputBudget {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_instructions: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_required_checks: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_blocked_actions: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_examples: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_explanations: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstructionCandidate {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstructionBundle {
    pub status: String,
    pub bundle_id: String,
    pub policy_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub context_budget: ContextBudgetReport,
    pub instructions: Vec<BundleInstruction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_checks: Vec<RequiredCheck>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_actions: Vec<BlockedAction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<SourceRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub explanations: Vec<BundleExplanation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleInstruction {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequiredCheck {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BlockedAction(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SourceRef(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ContextBudgetReport {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate_method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_policies_considered: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_policies_omitted: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PolicyRetrieval {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub semantic_terms: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_docs: Vec<String>,
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleExplanation {
    pub instruction: String,
    pub source: SourceRef,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleBuildOptions {
    pub max_tokens: Option<u32>,
    pub max_instructions: Option<u32>,
    pub max_required_checks: Option<u32>,
    pub max_blocked_actions: Option<u32>,
}

pub fn build_instruction_bundle(
    intent: &TaskIntent,
    policies: &[LoadedPolicy],
    options: BundleBuildOptions,
) -> Result<InstructionBundle> {
    let mut matched = policies
        .iter()
        .filter_map(|loaded| match_policy(intent, loaded).transpose())
        .collect::<Result<Vec<_>>>()?;

    matched.sort_by(|left, right| {
        right
            .loaded
            .policy
            .priority
            .unwrap_or(0)
            .cmp(&left.loaded.policy.priority.unwrap_or(0))
            .then_with(|| left.loaded.policy.id.cmp(&right.loaded.policy.id))
            .then_with(|| left.loaded.source_path.cmp(&right.loaded.source_path))
    });

    let candidate_count = matched.len();
    let instruction_limit = options.max_instructions.map(|value| value as usize);
    let required_check_limit = options.max_required_checks.map(|value| value as usize);
    let blocked_action_limit = options.max_blocked_actions.map(|value| value as usize);

    let mut instructions: Vec<BundleInstruction> = Vec::new();
    let mut required_checks: Vec<RequiredCheck> = Vec::new();
    let mut blocked_actions: Vec<BlockedAction> = Vec::new();
    let mut sources: Vec<SourceRef> = Vec::new();
    let mut explanations: Vec<BundleExplanation> = Vec::new();

    for matched_policy in &matched {
        let policy = &matched_policy.loaded.policy;
        let source = policy_source_ref(policy);
        push_unique(&mut sources, source.clone());

        for instruction in &policy.instructions {
            if instruction_limit.is_some_and(|limit| instructions.len() >= limit) {
                continue;
            }

            instructions.push(BundleInstruction {
                text: instruction.clone(),
                priority: policy.priority.map(priority_label),
                source: Some(source.clone()),
            });
            explanations.push(BundleExplanation {
                instruction: instruction.clone(),
                source: source.clone(),
                reason: matched_policy.reason.clone(),
            });
        }

        for check in &policy.required_checks {
            if required_check_limit.is_some_and(|limit| required_checks.len() >= limit) {
                continue;
            }

            let candidate = RequiredCheck {
                id: check.clone(),
                source: Some(source.clone()),
                resolved: Some(false),
            };
            if !required_checks
                .iter()
                .any(|existing| existing.id == candidate.id)
            {
                required_checks.push(candidate);
            }
        }

        for action in &policy.blocked_actions {
            if blocked_action_limit.is_some_and(|limit| blocked_actions.len() >= limit) {
                continue;
            }
            push_unique(&mut blocked_actions, action.clone());
        }
    }

    let estimated_tokens =
        estimate_bundle_tokens(&instructions, &required_checks, &blocked_actions, &sources);
    let omitted = matched
        .iter()
        .filter(|matched_policy| {
            !matched_policy
                .loaded
                .policy
                .instructions
                .iter()
                .any(|text| {
                    instructions
                        .iter()
                        .any(|instruction| &instruction.text == text)
                })
        })
        .count();

    Ok(InstructionBundle {
        status: "ok".into(),
        bundle_id: stable_bundle_id(intent, &sources),
        policy_version: stable_policy_version(&sources),
        summary: intent.task.as_ref().and_then(|task| task.summary.clone()),
        context_budget: ContextBudgetReport {
            max_tokens: options.max_tokens,
            estimated_tokens: Some(estimated_tokens),
            estimate_method: Some("approx_words".into()),
            candidate_policies_considered: Some(candidate_count as u32),
            candidate_policies_omitted: Some(omitted as u32),
            reason: if omitted > 0 {
                Some("Lower priority policy instructions excluded by context budget.".into())
            } else {
                None
            },
        },
        instructions,
        required_checks,
        blocked_actions,
        sources,
        explanations,
    })
}

pub fn render_bundle_markdown(bundle: &InstructionBundle) -> String {
    let mut out = String::new();
    out.push_str("# Agent Policy Instructions\n\n");

    if let Some(summary) = &bundle.summary {
        out.push_str("Task: ");
        out.push_str(summary);
        out.push_str("\n\n");
    }

    out.push_str("## Instructions\n\n");
    if bundle.instructions.is_empty() {
        out.push_str("- No matching policy instructions.\n");
    } else {
        for instruction in &bundle.instructions {
            out.push_str("- ");
            out.push_str(&instruction.text);
            if let Some(source) = &instruction.source {
                out.push_str(" (");
                out.push_str(&source.0);
                out.push(')');
            }
            out.push('\n');
        }
    }

    if !bundle.required_checks.is_empty() {
        out.push_str("\n## Required Checks\n\n");
        for check in &bundle.required_checks {
            out.push_str("- ");
            out.push_str(&check.id);
            if let Some(source) = &check.source {
                out.push_str(" (");
                out.push_str(&source.0);
                out.push(')');
            }
            out.push('\n');
        }
    }

    if !bundle.blocked_actions.is_empty() {
        out.push_str("\n## Blocked Actions\n\n");
        for action in &bundle.blocked_actions {
            out.push_str("- ");
            out.push_str(&action.0);
            out.push('\n');
        }
    }

    if !bundle.sources.is_empty() {
        out.push_str("\n## Sources\n\n");
        for source in &bundle.sources {
            out.push_str("- ");
            out.push_str(&source.0);
            out.push('\n');
        }
    }

    out
}

pub fn render_bundle_json(bundle: &InstructionBundle) -> Result<String> {
    serde_json::to_string_pretty(bundle).context("failed to serialize instruction bundle")
}

#[derive(Debug)]
struct MatchedPolicy<'a> {
    loaded: &'a LoadedPolicy,
    reason: String,
}

fn match_policy<'a>(
    intent: &TaskIntent,
    loaded: &'a LoadedPolicy,
) -> Result<Option<MatchedPolicy<'a>>> {
    let policy = &loaded.policy;
    if policy.status != PolicyStatus::Active {
        return Ok(None);
    }

    let applies = &policy.applies_when;
    let mut reasons = Vec::new();

    if !matches_task_type(applies, intent, &mut reasons) {
        return Ok(None);
    }
    if !matches_risk_flags(applies, intent, &mut reasons) {
        return Ok(None);
    }
    if !matches_paths(applies, intent, &mut reasons)? {
        return Ok(None);
    }
    if !matches_detected(applies, intent, &mut reasons) {
        return Ok(None);
    }
    if !matches_repo(applies, intent, &mut reasons) {
        return Ok(None);
    }

    if reasons.is_empty() && !is_global_policy(applies) {
        return Ok(None);
    }

    let reason = if reasons.is_empty() {
        "Matched global active policy.".to_string()
    } else {
        format!("Matched {}.", reasons.join(", "))
    };

    Ok(Some(MatchedPolicy { loaded, reason }))
}

fn matches_task_type(
    applies: &AppliesWhen,
    intent: &TaskIntent,
    reasons: &mut Vec<String>,
) -> bool {
    let Some(task_type) = intent
        .task
        .as_ref()
        .and_then(|task| task.task_type.as_ref())
    else {
        return true;
    };
    if applies.task_types.is_empty() {
        return true;
    }
    if applies
        .task_types
        .iter()
        .any(|candidate| candidate == task_type)
    {
        reasons.push(format!("task type `{}`", task_type.0));
        true
    } else {
        false
    }
}

fn matches_risk_flags(
    applies: &AppliesWhen,
    intent: &TaskIntent,
    reasons: &mut Vec<String>,
) -> bool {
    if applies.risk_flags.is_empty() || intent.risk_flags.is_empty() {
        return true;
    }
    if let Some(flag) = first_intersection(&applies.risk_flags, &intent.risk_flags) {
        reasons.push(format!("risk flag `{flag}`"));
        true
    } else {
        false
    }
}

fn matches_paths(
    applies: &AppliesWhen,
    intent: &TaskIntent,
    reasons: &mut Vec<String>,
) -> Result<bool> {
    if applies.paths.is_empty() || intent.files.is_empty() {
        return Ok(true);
    }

    let mut builder = GlobSetBuilder::new();
    for pattern in &applies.paths {
        builder.add(
            Glob::new(pattern).with_context(|| format!("invalid policy path glob `{pattern}`"))?,
        );
    }
    let globset = builder
        .build()
        .context("failed to build policy path glob set")?;

    for file in &intent.files {
        if globset.is_match(file) {
            reasons.push(format!("path `{file}`"));
            return Ok(true);
        }
    }

    Ok(false)
}

fn matches_detected(applies: &AppliesWhen, intent: &TaskIntent, reasons: &mut Vec<String>) -> bool {
    let Some(detected) = &intent.detected else {
        return true;
    };

    if !applies.languages.is_empty() && !detected.languages.is_empty() {
        if let Some(language) = first_intersection(&applies.languages, &detected.languages) {
            reasons.push(format!("language `{language}`"));
        } else {
            return false;
        }
    }

    if !applies.frameworks.is_empty() && !detected.frameworks.is_empty() {
        if let Some(framework) = first_intersection(&applies.frameworks, &detected.frameworks) {
            reasons.push(format!("framework `{framework}`"));
        } else {
            return false;
        }
    }

    if !applies.package_managers.is_empty() {
        if let Some(package_manager) = &detected.package_manager {
            if applies
                .package_managers
                .iter()
                .any(|candidate| candidate == package_manager)
            {
                reasons.push(format!("package manager `{package_manager}`"));
            } else {
                return false;
            }
        }
    }

    true
}

fn matches_repo(applies: &AppliesWhen, intent: &TaskIntent, reasons: &mut Vec<String>) -> bool {
    let Some(repo) = &intent.repo else {
        return true;
    };
    if applies.repos.is_empty() {
        return true;
    }
    if applies.repos.iter().any(|candidate| candidate == repo) {
        reasons.push(format!("repo `{repo}`"));
        true
    } else {
        false
    }
}

fn is_global_policy(applies: &AppliesWhen) -> bool {
    applies.repos.is_empty()
        && applies.paths.is_empty()
        && applies.languages.is_empty()
        && applies.frameworks.is_empty()
        && applies.package_managers.is_empty()
        && applies.task_types.is_empty()
        && applies.risk_flags.is_empty()
}

fn first_intersection<'a>(left: &'a [String], right: &[String]) -> Option<&'a str> {
    left.iter()
        .find(|candidate| right.iter().any(|value| value == *candidate))
        .map(String::as_str)
}

fn priority_label(priority: u32) -> String {
    match priority {
        90.. => "critical",
        70..=89 => "high",
        40..=69 => "normal",
        _ => "low",
    }
    .to_string()
}

fn policy_source_ref(policy: &Policy) -> SourceRef {
    SourceRef(format!(
        "{}@{}",
        policy.id,
        policy_version_to_string(&policy.version)
    ))
}

fn policy_version_to_string(version: &PolicyVersion) -> String {
    match version {
        PolicyVersion::Integer(value) => value.to_string(),
        PolicyVersion::Text(value) => value.clone(),
    }
}

fn stable_bundle_id(intent: &TaskIntent, sources: &[SourceRef]) -> String {
    let mut seed = String::new();
    if let Some(summary) = intent.task.as_ref().and_then(|task| task.summary.as_ref()) {
        seed.push_str(summary);
    }
    for file in &intent.files {
        seed.push_str(file);
    }
    for source in sources {
        seed.push_str(&source.0);
    }

    let hash = seed.bytes().fold(0xcbf29ce484222325u64, |acc, byte| {
        (acc ^ byte as u64).wrapping_mul(0x100000001b3)
    });
    format!("apb_{hash:016x}")
}

fn stable_policy_version(sources: &[SourceRef]) -> String {
    if sources.is_empty() {
        return "none".into();
    }

    sources
        .iter()
        .map(|source| source.0.as_str())
        .collect::<Vec<_>>()
        .join("+")
}

fn estimate_bundle_tokens(
    instructions: &[BundleInstruction],
    checks: &[RequiredCheck],
    actions: &[BlockedAction],
    sources: &[SourceRef],
) -> u32 {
    let words = instructions
        .iter()
        .flat_map(|instruction| instruction.text.split_whitespace())
        .count()
        + checks
            .iter()
            .flat_map(|check| check.id.split_whitespace())
            .count()
        + actions
            .iter()
            .flat_map(|action| action.0.split_whitespace())
            .count()
        + sources
            .iter()
            .flat_map(|source| source.0.split_whitespace())
            .count();

    words as u32
}

fn push_unique<T>(items: &mut Vec<T>, item: T)
where
    T: PartialEq,
{
    if !items.contains(&item) {
        items.push(item);
    }
}

pub fn load_policies_from_dirs<I, P>(
    repo_root: impl AsRef<Path>,
    policy_dirs: I,
) -> Result<Vec<LoadedPolicy>>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let repo_root = repo_root.as_ref();
    let mut policy_files = Vec::new();

    for policy_dir in policy_dirs {
        let policy_dir = resolve_policy_dir(repo_root, policy_dir.as_ref());

        if !policy_dir.exists() {
            continue;
        }

        if !policy_dir.is_dir() {
            bail!(
                "policy directory {} is not a directory",
                policy_dir.display()
            );
        }

        for entry in WalkDir::new(&policy_dir) {
            let entry = entry.with_context(|| {
                format!("failed to walk policy directory {}", policy_dir.display())
            })?;

            if entry.file_type().is_file() && is_policy_file(entry.path()) {
                policy_files.push(entry.into_path());
            }
        }
    }

    policy_files.sort();

    policy_files
        .into_iter()
        .map(|path| {
            let policy = read_yaml_file::<Policy>(&path)?;
            Ok(LoadedPolicy {
                policy,
                source_path: path,
            })
        })
        .collect()
}

fn resolve_policy_dir(repo_root: &Path, policy_dir: &Path) -> PathBuf {
    if policy_dir.is_absolute() {
        policy_dir.to_path_buf()
    } else {
        repo_root.join(policy_dir)
    }
}

fn is_policy_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("yaml" | "yml")
    )
}

fn read_yaml_file<T>(path: &Path) -> Result<T>
where
    T: DeserializeOwned,
{
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read policy file {}", path.display()))?;
    serde_yaml::from_str::<T>(&raw)
        .with_context(|| format!("failed to parse policy file {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_POLICY_YAML: &str = include_str!("../../../examples/policies/typescript.yaml");
    const SAMPLE_PAYMENTS_POLICY_YAML: &str =
        include_str!("../../../examples/policies/payments.yaml");
    const SAMPLE_INTENT_JSON: &str = r#"{
  "repo": "billing-api",
  "branch": "feature/refund-retries",
  "task": {
    "summary": "Fix refund retry handling",
    "type": "fix_bug"
  },
  "files": [
    "src/payments/refunds.ts",
    "tests/payments/refunds.test.ts"
  ],
  "detected": {
    "languages": ["typescript"],
    "frameworks": ["jest"],
    "package_manager": "npm"
  },
  "risk_flags": ["payments"],
  "expected_commands": ["npm test"],
  "expected_check_ids": ["typescript.unit_tests"],
  "output_budget": {
    "max_tokens": 900,
    "max_instructions": 8,
    "max_required_checks": 4,
    "max_blocked_actions": 4,
    "include_explanations": "compact"
  }
}"#;

    const SAMPLE_BUNDLE_JSON: &str = r#"{
  "status": "ok",
  "bundle_id": "apb_2026-05-31_001",
  "policy_version": "2026-05-31.1",
  "summary": "Instructions for a TypeScript payment bug fix.",
  "context_budget": {
    "max_tokens": 900,
    "estimated_tokens": 420,
    "estimate_method": "approx_words",
    "candidate_policies_considered": 14,
    "candidate_policies_omitted": 9,
    "reason": "Lower priority or duplicate non-mandatory guidance excluded by context budget."
  },
  "instructions": [
    {
      "text": "Preserve refund idempotency semantics.",
      "priority": "critical",
      "source": "domain.payments.refunds@7"
    },
    {
      "text": "Add tests for provider retry and repeated refund request handling.",
      "priority": "high",
      "source": "domain.payments.testing@2"
    }
  ],
  "required_checks": [
    {
      "id": "typescript.lint",
      "source": "lang.typescript.base@1",
      "resolved": false
    },
    {
      "id": "payments.unit_tests",
      "source": "domain.payments.testing@2",
      "resolved": false
    }
  ],
  "blocked_actions": [
    "Do not edit production payment credentials."
  ],
  "sources": [
    "domain.payments.refunds@7",
    "domain.payments.testing@2",
    "lang.typescript.base@1"
  ],
  "explanations": [
    {
      "instruction": "Preserve refund idempotency semantics.",
      "source": "domain.payments.refunds@7",
      "reason": "Matched risk flag `payments`, path `src/payments/**`, and semantic terms related to refund retries."
    }
  ]
}"#;

    #[test]
    fn version_is_non_empty() {
        assert!(!version().is_empty());
    }

    #[test]
    fn policy_yaml_deserializes() {
        let policy: Policy = serde_yaml::from_str(SAMPLE_POLICY_YAML).unwrap();

        assert_eq!(policy.id, "lang.typescript.base");
        assert_eq!(policy.version, PolicyVersion::Integer(1));
        assert_eq!(policy.status, PolicyStatus::Active);
        assert_eq!(
            policy.applies_when.task_types,
            vec![
                TaskType("fix_bug".into()),
                TaskType("add_feature".into()),
                TaskType("refactor".into()),
                TaskType("test".into()),
            ]
        );
        assert_eq!(policy.required_checks.len(), 2);
        assert_eq!(
            policy.blocked_actions,
            vec![BlockedAction(
                "Do not edit generated files directly.".into()
            )]
        );
    }

    #[test]
    fn policy_yaml_serializes() {
        let policy: Policy = serde_yaml::from_str(SAMPLE_POLICY_YAML).unwrap();
        let rendered = serde_yaml::to_string(&policy).unwrap();

        assert!(rendered.contains("id: lang.typescript.base"));
        assert!(rendered.contains("status: active"));
        assert!(rendered.contains("- typescript.typecheck"));
        assert!(rendered.contains("- Do not edit generated files directly."));
    }

    #[test]
    fn payments_policy_yaml_deserializes() {
        let policy: Policy = serde_yaml::from_str(SAMPLE_PAYMENTS_POLICY_YAML).unwrap();

        assert_eq!(policy.id, "domain.payments.refunds");
        assert_eq!(policy.status, PolicyStatus::Active);
        assert_eq!(
            policy.applies_when.paths,
            vec![
                "src/payments/**".to_string(),
                "src/refunds/**".to_string(),
                "tests/payments/**".to_string(),
            ]
        );
        assert_eq!(
            policy.applies_when.task_types,
            vec![
                TaskType("fix_bug".into()),
                TaskType("add_feature".into()),
                TaskType("test".into()),
            ]
        );
        assert_eq!(policy.blocked_actions.len(), 2);
    }

    #[test]
    fn yaml_extension_detection_accepts_yaml_and_yml() {
        assert!(is_policy_file(Path::new("policy.yaml")));
        assert!(is_policy_file(Path::new("policy.yml")));
        assert!(!is_policy_file(Path::new("policy.json")));
    }

    #[test]
    fn task_intent_json_deserializes() {
        let intent: TaskIntent = serde_json::from_str(SAMPLE_INTENT_JSON).unwrap();

        assert_eq!(intent.repo.as_deref(), Some("billing-api"));
        assert_eq!(intent.branch.as_deref(), Some("feature/refund-retries"));
        assert_eq!(
            intent
                .task
                .as_ref()
                .and_then(|task| task.task_type.as_ref()),
            Some(&TaskType("fix_bug".into()))
        );
        assert_eq!(intent.files.len(), 2);
        assert_eq!(
            intent
                .detected
                .as_ref()
                .and_then(|detected| detected.package_manager.as_deref()),
            Some("npm")
        );
        assert_eq!(
            intent
                .output_budget
                .as_ref()
                .and_then(|budget| budget.max_tokens),
            Some(900)
        );
    }

    #[test]
    fn bundle_json_deserializes() {
        let bundle: InstructionBundle = serde_json::from_str(SAMPLE_BUNDLE_JSON).unwrap();

        assert_eq!(bundle.status, "ok");
        assert_eq!(bundle.instructions.len(), 2);
        assert_eq!(bundle.required_checks.len(), 2);
        assert_eq!(
            bundle.sources[0],
            SourceRef("domain.payments.refunds@7".into())
        );
        assert_eq!(bundle.context_budget.max_tokens, Some(900));
    }

    #[test]
    fn bundle_json_serializes() {
        let bundle = InstructionBundle {
            status: "ok".into(),
            bundle_id: "apb_2026-05-31_001".into(),
            policy_version: "2026-05-31.1".into(),
            summary: Some("Instructions for a TypeScript payment bug fix.".into()),
            context_budget: ContextBudgetReport {
                max_tokens: Some(900),
                estimated_tokens: Some(420),
                estimate_method: Some("approx_words".into()),
                candidate_policies_considered: Some(14),
                candidate_policies_omitted: Some(9),
                reason: Some(
                    "Lower priority or duplicate non-mandatory guidance excluded by context budget."
                        .into(),
                ),
            },
            instructions: vec![
                BundleInstruction {
                    text: "Preserve refund idempotency semantics.".into(),
                    priority: Some("critical".into()),
                    source: Some(SourceRef("domain.payments.refunds@7".into())),
                },
                BundleInstruction {
                    text: "Add tests for provider retry and repeated refund request handling.".into(),
                    priority: Some("high".into()),
                    source: Some(SourceRef("domain.payments.testing@2".into())),
                },
            ],
            required_checks: vec![
                RequiredCheck {
                    id: "typescript.lint".into(),
                    source: Some(SourceRef("lang.typescript.base@1".into())),
                    resolved: Some(false),
                },
                RequiredCheck {
                    id: "payments.unit_tests".into(),
                    source: Some(SourceRef("domain.payments.testing@2".into())),
                    resolved: Some(false),
                },
            ],
            blocked_actions: vec![BlockedAction(
                "Do not edit production payment credentials.".into(),
            )],
            sources: vec![
                SourceRef("domain.payments.refunds@7".into()),
                SourceRef("domain.payments.testing@2".into()),
                SourceRef("lang.typescript.base@1".into()),
            ],
            explanations: vec![BundleExplanation {
                instruction: "Preserve refund idempotency semantics.".into(),
                source: SourceRef("domain.payments.refunds@7".into()),
                reason: "Matched risk flag `payments`, path `src/payments/**`, and semantic terms related to refund retries.".into(),
            }],
        };

        let value = serde_json::to_value(&bundle).unwrap();

        assert_eq!(value["status"], "ok");
        assert_eq!(value["bundle_id"], "apb_2026-05-31_001");
        assert_eq!(value["instructions"][0]["priority"], "critical");
        assert_eq!(value["required_checks"][0]["resolved"], false);
        assert_eq!(
            value["blocked_actions"][0],
            "Do not edit production payment credentials."
        );
    }

    #[test]
    fn builds_bundle_from_active_matching_policies() {
        let policies = load_policies_from_dirs(fixture_simple_repo(), [".agent-policy/policies"])
            .expect("load fixture policies");
        let intent = TaskIntent {
            repo: Some("simple-repo".into()),
            branch: None,
            task: Some(TaskDetails {
                summary: Some("fix refund retry handling".into()),
                task_type: Some(TaskType("fix_bug".into())),
            }),
            files: vec!["src/payments/refunds.ts".into()],
            detected: Some(DetectedContext {
                languages: vec!["typescript".into()],
                frameworks: Vec::new(),
                package_manager: None,
            }),
            risk_flags: Vec::new(),
            expected_commands: Vec::new(),
            expected_check_ids: Vec::new(),
            output_budget: None,
        };

        let bundle = build_instruction_bundle(
            &intent,
            &policies,
            BundleBuildOptions {
                max_tokens: Some(900),
                max_instructions: Some(8),
                max_required_checks: Some(4),
                max_blocked_actions: Some(4),
            },
        )
        .expect("build bundle");

        assert_eq!(bundle.status, "ok");
        assert_eq!(
            bundle
                .sources
                .iter()
                .map(|source| source.0.as_str())
                .collect::<Vec<_>>(),
            vec!["domain.payments.refunds@1", "lang.typescript.base@1"]
        );
        assert!(bundle.instructions.iter().any(|instruction| {
            instruction
                .text
                .contains("Preserve idempotency for refund creation")
        }));
        assert!(bundle
            .required_checks
            .iter()
            .any(|check| check.id == "payments.unit_tests"));
    }

    #[test]
    fn renders_bundle_markdown_with_sources() {
        let bundle: InstructionBundle = serde_json::from_str(SAMPLE_BUNDLE_JSON).unwrap();
        let markdown = render_bundle_markdown(&bundle);

        assert!(markdown.contains("# Agent Policy Instructions"));
        assert!(markdown.contains("domain.payments.refunds@7"));
        assert!(markdown.contains("## Required Checks"));
    }

    fn fixture_simple_repo() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/simple-repo")
    }
}
