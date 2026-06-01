//! Core data models for Agent Policy Broker.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use globset::{Glob, GlobSetBuilder};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use serde_yaml::{Mapping as YamlMapping, Value as YamlValue};
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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
            .then_with(|| right.score.rank.cmp(&left.score.rank))
            .then_with(|| {
                right
                    .score
                    .path_specificity
                    .cmp(&left.score.path_specificity)
            })
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
    let mut seen_instructions: BTreeSet<String> = BTreeSet::new();
    let mut seen_required_checks: BTreeSet<String> = BTreeSet::new();
    let mut seen_blocked_actions: BTreeSet<BlockedAction> = BTreeSet::new();
    let mut omitted = 0usize;

    for matched_policy in &matched {
        let policy = &matched_policy.loaded.policy;
        let source = policy_source_ref(policy);
        let mut included_policy_content = false;

        for instruction in &policy.instructions {
            if !seen_instructions.insert(instruction.clone()) {
                continue;
            }
            if instruction_limit.is_some_and(|limit| instructions.len() >= limit) {
                continue;
            }

            push_unique(&mut sources, source.clone());
            included_policy_content = true;
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
            if !seen_required_checks.insert(check.clone()) {
                continue;
            }
            if required_check_limit.is_some_and(|limit| required_checks.len() >= limit) {
                continue;
            }

            push_unique(&mut sources, source.clone());
            included_policy_content = true;
            let candidate = RequiredCheck {
                id: check.clone(),
                source: Some(source.clone()),
                resolved: Some(false),
            };
            required_checks.push(candidate);
        }

        for action in &policy.blocked_actions {
            if !seen_blocked_actions.insert(action.clone()) {
                continue;
            }
            if blocked_action_limit.is_some_and(|limit| blocked_actions.len() >= limit) {
                continue;
            }
            push_unique(&mut sources, source.clone());
            included_policy_content = true;
            blocked_actions.push(action.clone());
        }

        if !included_policy_content {
            omitted += 1;
        }
    }

    let estimated_tokens =
        estimate_bundle_tokens(&instructions, &required_checks, &blocked_actions, &sources);

    let context_budget_reason = if omitted > 0 {
        Some(
            "Lower priority or duplicate non-mandatory guidance excluded by context budget.".into(),
        )
    } else {
        None
    };
    let warnings = if omitted > 0 {
        vec![format!(
            "Context budget omitted {omitted} candidate {}.",
            pluralize(omitted, "policy", "policies")
        )]
    } else {
        Vec::new()
    };

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
            reason: context_budget_reason,
        },
        warnings,
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
    out.push_str("- Bundle ID: `");
    out.push_str(&markdown_inline(&bundle.bundle_id));
    out.push_str("`\n");
    out.push_str("- Policy version: `");
    out.push_str(&markdown_inline(&bundle.policy_version));
    out.push_str("`\n");
    out.push_str("- Status: `");
    out.push_str(&markdown_inline(&bundle.status));
    out.push_str("`\n\n");

    out.push_str("## Task Summary\n\n");
    if let Some(summary) = &bundle.summary {
        out.push_str(&markdown_paragraph(summary));
        out.push_str("\n\n");
    } else {
        out.push_str("No task summary provided.\n\n");
    }

    out.push_str("## Instructions\n\n");
    if bundle.instructions.is_empty() {
        out.push_str("- No matching policy instructions.\n");
    } else {
        for instruction in &bundle.instructions {
            out.push_str("- ");
            out.push_str(&markdown_list_text(&instruction.text));
            let mut details = Vec::new();
            if let Some(priority) = &instruction.priority {
                details.push(format!("priority: {}", markdown_inline(priority)));
            }
            if let Some(source) = &instruction.source {
                details.push(format!("source: `{}`", markdown_inline(&source.0)));
            }
            push_details(&mut out, &details);
            out.push('\n');
        }
    }

    out.push_str("\n## Required Checks\n\n");
    if bundle.required_checks.is_empty() {
        out.push_str("- None.\n");
    } else {
        for check in &bundle.required_checks {
            out.push_str("- `");
            out.push_str(&markdown_inline(&check.id));
            out.push('`');
            let mut details = Vec::new();
            if let Some(source) = &check.source {
                details.push(format!("source: `{}`", markdown_inline(&source.0)));
            }
            if let Some(resolved) = check.resolved {
                details.push(format!("resolved: {}", if resolved { "yes" } else { "no" }));
            }
            push_details(&mut out, &details);
            out.push('\n');
        }
    }

    out.push_str("\n## Blocked Actions\n\n");
    if bundle.blocked_actions.is_empty() {
        out.push_str("- None.\n");
    } else {
        for action in &bundle.blocked_actions {
            out.push_str("- ");
            out.push_str(&markdown_list_text(&action.0));
            out.push('\n');
        }
    }

    out.push_str("\n## Sources\n\n");
    if bundle.sources.is_empty() {
        out.push_str("- None.\n");
    } else {
        for source in &bundle.sources {
            out.push_str("- `");
            out.push_str(&markdown_inline(&source.0));
            out.push_str("`\n");
        }
    }

    out.push_str("\n## Context Budget\n\n");
    out.push_str("- ");
    out.push_str(&render_budget_summary(&bundle.context_budget));
    out.push('\n');
    if let Some(reason) = &bundle.context_budget.reason {
        out.push_str("- ");
        out.push_str(&markdown_list_text(reason));
        out.push('\n');
    }

    if !bundle.warnings.is_empty() {
        out.push_str("\n## Warnings\n\n");
        for warning in &bundle.warnings {
            out.push_str("- ");
            out.push_str(&markdown_list_text(warning));
            out.push('\n');
        }
    }

    out
}

fn render_budget_summary(context_budget: &ContextBudgetReport) -> String {
    let mut parts = Vec::new();

    match (
        context_budget.estimated_tokens,
        context_budget.max_tokens,
        context_budget.estimate_method.as_deref(),
    ) {
        (Some(estimated), Some(max), Some(method)) => {
            parts.push(format!(
                "tokens: {estimated}/{max} ({})",
                markdown_inline(method)
            ));
        }
        (Some(estimated), Some(max), None) => {
            parts.push(format!("tokens: {estimated}/{max}"));
        }
        (Some(estimated), None, Some(method)) => {
            parts.push(format!(
                "estimated tokens: {estimated} ({})",
                markdown_inline(method)
            ));
        }
        (Some(estimated), None, None) => {
            parts.push(format!("estimated tokens: {estimated}"));
        }
        (None, Some(max), _) => {
            parts.push(format!("max tokens: {max}"));
        }
        (None, None, _) => {}
    }

    if let Some(considered) = context_budget.candidate_policies_considered {
        parts.push(format!("policies considered: {considered}"));
    }
    if let Some(omitted) = context_budget.candidate_policies_omitted {
        parts.push(format!("policies omitted: {omitted}"));
    }

    if parts.is_empty() {
        "No budget details reported.".into()
    } else {
        parts.join("; ")
    }
}

fn push_details(out: &mut String, details: &[String]) {
    if details.is_empty() {
        return;
    }

    out.push_str(" (");
    out.push_str(&details.join(", "));
    out.push(')');
}

fn markdown_paragraph(text: &str) -> String {
    normalize_markdown_text(text)
}

fn markdown_list_text(text: &str) -> String {
    normalize_markdown_text(text)
}

fn markdown_inline(text: &str) -> String {
    normalize_markdown_text(text).replace('`', "\\`")
}

fn normalize_markdown_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn pluralize<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 {
        singular
    } else {
        plural
    }
}

pub fn render_bundle_json(bundle: &InstructionBundle) -> Result<String> {
    serde_json::to_string_pretty(bundle).context("failed to serialize instruction bundle")
}

#[derive(Debug)]
struct MatchedPolicy<'a> {
    loaded: &'a LoadedPolicy,
    reason: String,
    score: MatchScore,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct MatchScore {
    rank: u32,
    path_specificity: u32,
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
    let mut score = MatchScore::default();

    if !matches_task_type(applies, intent, &mut reasons, &mut score) {
        return Ok(None);
    }
    if !matches_risk_flags(applies, intent, &mut reasons, &mut score) {
        return Ok(None);
    }
    if !matches_paths(applies, intent, &mut reasons, &mut score)? {
        return Ok(None);
    }
    if !matches_detected(applies, intent, &mut reasons, &mut score) {
        return Ok(None);
    }
    if !matches_repo(applies, intent, &mut reasons, &mut score) {
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

    Ok(Some(MatchedPolicy {
        loaded,
        reason,
        score,
    }))
}

fn matches_task_type(
    applies: &AppliesWhen,
    intent: &TaskIntent,
    reasons: &mut Vec<String>,
    score: &mut MatchScore,
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
        score.rank += 100;
        true
    } else {
        false
    }
}

fn matches_risk_flags(
    applies: &AppliesWhen,
    intent: &TaskIntent,
    reasons: &mut Vec<String>,
    score: &mut MatchScore,
) -> bool {
    if applies.risk_flags.is_empty() || intent.risk_flags.is_empty() {
        return true;
    }
    if let Some(flag) = first_intersection(&applies.risk_flags, &intent.risk_flags) {
        reasons.push(format!("risk flag `{flag}`"));
        score.rank += 500;
        true
    } else {
        false
    }
}

fn matches_paths(
    applies: &AppliesWhen,
    intent: &TaskIntent,
    reasons: &mut Vec<String>,
    score: &mut MatchScore,
) -> Result<bool> {
    if applies.paths.is_empty() || intent.files.is_empty() {
        return Ok(true);
    }

    let mut builder = GlobSetBuilder::new();
    for pattern in &applies.paths {
        let normalized_pattern = normalize_policy_path(pattern);
        builder.add(
            Glob::new(&normalized_pattern)
                .with_context(|| format!("invalid policy path glob `{pattern}`"))?,
        );
    }
    let globset = builder
        .build()
        .context("failed to build policy path glob set")?;

    for file in &intent.files {
        let normalized_file = normalize_policy_path(file);
        let matches = globset.matches(&normalized_file);
        if let Some(best_match) = matches
            .iter()
            .map(|index| &applies.paths[*index])
            .max_by_key(|pattern| path_pattern_specificity(pattern))
        {
            let specificity = path_pattern_specificity(best_match);
            score.rank += specificity;
            score.path_specificity = score.path_specificity.max(specificity);
            reasons.push(format!("path `{file}` matched `{best_match}`"));
            return Ok(true);
        }
    }

    Ok(false)
}

fn matches_detected(
    applies: &AppliesWhen,
    intent: &TaskIntent,
    reasons: &mut Vec<String>,
    score: &mut MatchScore,
) -> bool {
    let Some(detected) = &intent.detected else {
        return true;
    };

    if !applies.languages.is_empty() && !detected.languages.is_empty() {
        if let Some(language) = first_intersection(&applies.languages, &detected.languages) {
            reasons.push(format!("language `{language}`"));
            score.rank += 100;
        } else {
            return false;
        }
    }

    if !applies.frameworks.is_empty() && !detected.frameworks.is_empty() {
        if let Some(framework) = first_intersection(&applies.frameworks, &detected.frameworks) {
            reasons.push(format!("framework `{framework}`"));
            score.rank += 100;
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
                score.rank += 100;
            } else {
                return false;
            }
        }
    }

    true
}

fn matches_repo(
    applies: &AppliesWhen,
    intent: &TaskIntent,
    reasons: &mut Vec<String>,
    score: &mut MatchScore,
) -> bool {
    let Some(repo) = &intent.repo else {
        return true;
    };
    if applies.repos.is_empty() {
        return true;
    }
    if applies.repos.iter().any(|candidate| candidate == repo) {
        reasons.push(format!("repo `{repo}`"));
        score.rank += 50;
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

fn normalize_policy_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string()
}

fn path_pattern_specificity(pattern: &str) -> u32 {
    let normalized = normalize_policy_path(pattern);
    let literal_chars = normalized
        .chars()
        .filter(|character| !matches!(character, '*' | '?' | '[' | ']' | '{' | '}' | ','))
        .count() as u32;
    let components = normalized
        .split('/')
        .filter(|component| !component.is_empty())
        .count() as u32;
    let wildcard_count = normalized
        .chars()
        .filter(|character| matches!(character, '*' | '?' | '[' | ']' | '{' | '}' | ','))
        .count() as u32;
    let base = if !contains_glob_meta(&normalized) {
        1_000
    } else if normalized.contains("**") {
        400
    } else {
        800
    };

    (base + literal_chars.saturating_mul(10) + components.saturating_mul(5))
        .saturating_sub(wildcard_count.saturating_mul(5))
}

fn contains_glob_meta(pattern: &str) -> bool {
    pattern
        .chars()
        .any(|character| matches!(character, '*' | '?' | '[' | ']' | '{' | '}' | ','))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyValidationSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyValidationIssue {
    pub severity: PolicyValidationSeverity,
    pub code: &'static str,
    pub message: String,
    pub path: Option<String>,
    pub field: Option<String>,
}

pub fn collect_policy_files<I, P>(
    repo_root: impl AsRef<Path>,
    policy_dirs: I,
) -> (Vec<PathBuf>, Vec<PolicyValidationIssue>)
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let repo_root = repo_root.as_ref();
    let mut policy_files = Vec::new();
    let mut issues = Vec::new();

    for policy_dir in policy_dirs {
        let policy_dir = resolve_policy_dir(repo_root, policy_dir.as_ref());

        if !policy_dir.exists() {
            continue;
        }

        if !policy_dir.is_dir() {
            push_policy_issue(
                &mut issues,
                PolicyValidationSeverity::Error,
                "policy_dir_invalid",
                "Configured local policy path is not a directory.".to_string(),
                Some(&policy_dir),
                None,
            );
            continue;
        }

        for entry in WalkDir::new(&policy_dir) {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    push_policy_issue(
                        &mut issues,
                        PolicyValidationSeverity::Error,
                        "policy_dir_read_error",
                        format!(
                            "Failed to walk policy directory {}: {error}",
                            policy_dir.display()
                        ),
                        Some(&policy_dir),
                        None,
                    );
                    continue;
                }
            };

            if entry.file_type().is_file() && is_policy_file(entry.path()) {
                policy_files.push(entry.into_path());
            }
        }
    }

    policy_files.sort();
    (policy_files, issues)
}

pub fn validate_policy_files(policy_files: &[PathBuf]) -> Vec<PolicyValidationIssue> {
    let mut issues = Vec::new();
    let mut seen_ids: BTreeMap<String, PathBuf> = BTreeMap::new();

    for policy_file in policy_files {
        let raw = match fs::read_to_string(policy_file) {
            Ok(raw) => raw,
            Err(error) => {
                push_policy_issue(
                    &mut issues,
                    PolicyValidationSeverity::Error,
                    "policy_read_error",
                    format!("Failed to read policy file: {error}"),
                    Some(policy_file),
                    None,
                );
                continue;
            }
        };
        let value = match serde_yaml::from_str::<YamlValue>(&raw) {
            Ok(value) => value,
            Err(error) => {
                push_policy_issue(
                    &mut issues,
                    PolicyValidationSeverity::Error,
                    "policy_parse_error",
                    format!("Failed to parse policy YAML: {error}"),
                    Some(policy_file),
                    None,
                );
                continue;
            }
        };

        let Some(root) = value.as_mapping() else {
            push_policy_issue(
                &mut issues,
                PolicyValidationSeverity::Error,
                "policy_invalid_root",
                "Policy must be a YAML mapping.".to_string(),
                Some(policy_file),
                None,
            );
            continue;
        };

        validate_policy_value(root, policy_file, &mut seen_ids, &mut issues);

        if let Err(error) = serde_yaml::from_str::<Policy>(&raw) {
            push_policy_issue(
                &mut issues,
                PolicyValidationSeverity::Error,
                "policy_schema_error",
                format!("Policy does not match the documented schema: {error}"),
                Some(policy_file),
                None,
            );
        }
    }

    issues
}

fn validate_policy_value(
    root: &YamlMapping,
    policy_file: &Path,
    seen_ids: &mut BTreeMap<String, PathBuf>,
    issues: &mut Vec<PolicyValidationIssue>,
) {
    let id = match yaml_mapping_get(root, "id").and_then(YamlValue::as_str) {
        Some(id) if !id.trim().is_empty() => Some(id.trim().to_string()),
        _ => {
            push_policy_issue(
                issues,
                PolicyValidationSeverity::Error,
                "policy_missing_id",
                "Policy id is required and must be a non-empty string.".to_string(),
                Some(policy_file),
                Some("id"),
            );
            None
        }
    };

    if let Some(id) = &id {
        if let Some(first_path) = seen_ids.insert(id.clone(), policy_file.to_path_buf()) {
            push_policy_issue(
                issues,
                PolicyValidationSeverity::Error,
                "policy_duplicate_id",
                format!(
                    "Policy id `{id}` duplicates an id already defined in {}.",
                    first_path.display()
                ),
                Some(policy_file),
                Some("id"),
            );
        }
    }

    if !root.contains_key(YamlValue::String("version".to_string())) {
        push_policy_issue(
            issues,
            PolicyValidationSeverity::Error,
            "policy_missing_version",
            "Policy version is required.".to_string(),
            Some(policy_file),
            Some("version"),
        );
    }

    let active = match yaml_mapping_get(root, "status").and_then(YamlValue::as_str) {
        Some("active") => true,
        Some("draft" | "deprecated" | "disabled") => false,
        Some(status) => {
            push_policy_issue(
                issues,
                PolicyValidationSeverity::Error,
                "policy_invalid_status",
                format!(
                    "Policy status `{status}` is invalid; expected active, draft, deprecated, or disabled."
                ),
                Some(policy_file),
                Some("status"),
            );
            false
        }
        None => {
            push_policy_issue(
                issues,
                PolicyValidationSeverity::Error,
                "policy_invalid_status",
                "Policy status is required and must be active, draft, deprecated, or disabled."
                    .to_string(),
                Some(policy_file),
                Some("status"),
            );
            false
        }
    };

    let instruction_values =
        yaml_mapping_get(root, "instructions").and_then(YamlValue::as_sequence);
    if active {
        let has_non_empty_instruction = instruction_values.is_some_and(|instructions| {
            instructions
                .iter()
                .filter_map(YamlValue::as_str)
                .any(|instruction| !instruction.trim().is_empty())
        });
        if !has_non_empty_instruction {
            push_policy_issue(
                issues,
                PolicyValidationSeverity::Error,
                "policy_active_empty_instructions",
                "Active policies must define at least one non-empty instruction.".to_string(),
                Some(policy_file),
                Some("instructions"),
            );
        }
    }

    if active && is_broad_policy_value(yaml_mapping_get(root, "applies_when")) {
        push_policy_issue(
            issues,
            PolicyValidationSeverity::Warning,
            "policy_broad_active",
            "Active policy has no applies_when fields and will apply globally.".to_string(),
            Some(policy_file),
            Some("applies_when"),
        );
    }

    if let Some(instructions) = instruction_values {
        for (index, instruction) in instructions.iter().enumerate() {
            if let Some(instruction) = instruction.as_str() {
                if is_vague_instruction(instruction) {
                    push_policy_issue(
                        issues,
                        PolicyValidationSeverity::Warning,
                        "policy_vague_instruction",
                        format!(
                            "Instruction `{}` is too vague to be actionable.",
                            instruction.trim()
                        ),
                        Some(policy_file),
                        Some(&format!("instructions[{index}]")),
                    );
                }
            }
        }
    }
}

fn is_broad_policy_value(applies_when: Option<&YamlValue>) -> bool {
    let Some(applies_when) = applies_when.and_then(YamlValue::as_mapping) else {
        return true;
    };

    for field in [
        "repos",
        "paths",
        "languages",
        "frameworks",
        "package_managers",
        "task_types",
        "risk_flags",
    ] {
        if yaml_mapping_get(applies_when, field)
            .and_then(YamlValue::as_sequence)
            .is_some_and(|values| !values.is_empty())
        {
            return false;
        }
    }

    true
}

fn is_vague_instruction(instruction: &str) -> bool {
    let normalized = instruction
        .trim()
        .trim_end_matches('.')
        .to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "be careful" | "write clean code" | "make it good" | "use best practices"
    )
}

fn yaml_mapping_get<'a>(mapping: &'a YamlMapping, key: &str) -> Option<&'a YamlValue> {
    mapping.get(YamlValue::String(key.to_string()))
}

fn push_policy_issue(
    issues: &mut Vec<PolicyValidationIssue>,
    severity: PolicyValidationSeverity,
    code: &'static str,
    message: String,
    path: Option<&Path>,
    field: Option<&str>,
) {
    issues.push(PolicyValidationIssue {
        severity,
        code,
        message,
        path: path.map(|path| path.display().to_string()),
        field: field.map(str::to_string),
    });
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
  "warnings": [
    "Context budget omitted 9 candidate policies."
  ],
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

    const SAMPLE_BUNDLE_MARKDOWN: &str = r#"# Agent Policy Instructions

- Bundle ID: `apb_2026-05-31_001`
- Policy version: `2026-05-31.1`
- Status: `ok`

## Task Summary

Instructions for a TypeScript payment bug fix.

## Instructions

- Preserve refund idempotency semantics. (priority: critical, source: `domain.payments.refunds@7`)
- Add tests for provider retry and repeated refund request handling. (priority: high, source: `domain.payments.testing@2`)

## Required Checks

- `typescript.lint` (source: `lang.typescript.base@1`, resolved: no)
- `payments.unit_tests` (source: `domain.payments.testing@2`, resolved: no)

## Blocked Actions

- Do not edit production payment credentials.

## Sources

- `domain.payments.refunds@7`
- `domain.payments.testing@2`
- `lang.typescript.base@1`

## Context Budget

- tokens: 420/900 (approx_words); policies considered: 14; policies omitted: 9
- Lower priority or duplicate non-mandatory guidance excluded by context budget.

## Warnings

- Context budget omitted 9 candidate policies.
"#;

    const BUDGETED_MARKDOWN_SNAPSHOT: &str = r#"# Agent Policy Instructions

- Bundle ID: `apb_92c14b895f9c1aa4`
- Policy version: `policy.high@1`
- Status: `ok`

## Task Summary

No task summary provided.

## Instructions

- Keep the highest priority guidance. (source: `policy.high@1`)

## Required Checks

- None.

## Blocked Actions

- None.

## Sources

- `policy.high@1`

## Context Budget

- tokens: 6/900 (approx_words); policies considered: 2; policies omitted: 1
- Lower priority or duplicate non-mandatory guidance excluded by context budget.

## Warnings

- Context budget omitted 1 candidate policy.
"#;

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
            warnings: vec!["Context budget omitted 9 candidate policies.".into()],
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
    fn path_globs_match_any_task_file_and_explain_pattern() {
        let policies = vec![test_policy(
            "domain.payments.tests",
            AppliesWhen {
                paths: vec!["src/payments/**".into(), "tests/payments/**".into()],
                ..AppliesWhen::default()
            },
            "Use payment-specific test guidance.",
        )];
        let intent = test_intent(vec!["README.md", "tests/payments/refunds.test.ts"]);

        let bundle = build_instruction_bundle(&intent, &policies, default_build_options())
            .expect("path glob should match");

        assert_eq!(bundle.instructions.len(), 1);
        assert_eq!(
            bundle.explanations[0].reason,
            "Matched path `tests/payments/refunds.test.ts` matched `tests/payments/**`."
        );
    }

    #[test]
    fn nested_globs_match_migration_files() {
        let policies = vec![test_policy(
            "backend.migrations",
            AppliesWhen {
                paths: vec!["backend/**/migrations/*.sql".into()],
                ..AppliesWhen::default()
            },
            "Review migration rollback behavior.",
        )];
        let intent = test_intent(vec!["backend/billing/db/migrations/20260601_refunds.sql"]);

        let bundle = build_instruction_bundle(&intent, &policies, default_build_options())
            .expect("nested migration glob should match");

        assert_eq!(bundle.instructions.len(), 1);
        assert!(bundle.explanations[0]
            .reason
            .contains("backend/**/migrations/*.sql"));
    }

    #[test]
    fn unmatched_path_scoped_policy_is_excluded_when_files_are_provided() {
        let policies = vec![test_policy(
            "domain.payments.refunds",
            AppliesWhen {
                paths: vec!["src/payments/**".into()],
                ..AppliesWhen::default()
            },
            "Preserve refund idempotency.",
        )];
        let intent = test_intent(vec!["src/orders/order.ts"]);

        let bundle = build_instruction_bundle(&intent, &policies, default_build_options())
            .expect("bundle should build without path match");

        assert!(bundle.instructions.is_empty());
        assert!(bundle.explanations.is_empty());
    }

    #[test]
    fn more_specific_path_matches_rank_above_broad_globs() {
        let policies = vec![
            test_policy(
                "broad.src",
                AppliesWhen {
                    paths: vec!["src/**".into()],
                    ..AppliesWhen::default()
                },
                "Broad source guidance.",
            ),
            test_policy(
                "payments.src",
                AppliesWhen {
                    paths: vec!["src/payments/**".into()],
                    ..AppliesWhen::default()
                },
                "Payment source guidance.",
            ),
            test_policy(
                "exact.refunds",
                AppliesWhen {
                    paths: vec!["src/payments/refunds.ts".into()],
                    ..AppliesWhen::default()
                },
                "Exact refund file guidance.",
            ),
        ];
        let intent = test_intent(vec!["./src/payments/refunds.ts"]);

        let bundle = build_instruction_bundle(&intent, &policies, default_build_options())
            .expect("bundle should build");

        assert_eq!(
            bundle
                .instructions
                .iter()
                .map(|instruction| instruction.text.as_str())
                .collect::<Vec<_>>(),
            vec![
                "Exact refund file guidance.",
                "Payment source guidance.",
                "Broad source guidance.",
            ]
        );
    }

    #[test]
    fn broad_path_policy_does_not_outrank_domain_policy_without_priority() {
        let policies = vec![
            test_policy(
                "broad.src",
                AppliesWhen {
                    paths: vec!["src/**".into()],
                    ..AppliesWhen::default()
                },
                "Broad source guidance.",
            ),
            test_policy(
                "domain.payments",
                AppliesWhen {
                    risk_flags: vec!["payments".into()],
                    ..AppliesWhen::default()
                },
                "Payment domain guidance.",
            ),
        ];
        let mut intent = test_intent(vec!["src/payments/refunds.ts"]);
        intent.risk_flags = vec!["payments".into()];

        let bundle = build_instruction_bundle(&intent, &policies, default_build_options())
            .expect("bundle should build");

        assert_eq!(bundle.instructions[0].text, "Payment domain guidance.");
        assert_eq!(bundle.instructions[1].text, "Broad source guidance.");
    }

    #[test]
    fn budget_trims_instructions_and_reports_omitted_policies() {
        let policies = vec![
            test_policy(
                "policy.high",
                AppliesWhen::default(),
                "Keep the highest priority guidance.",
            ),
            test_policy(
                "policy.low",
                AppliesWhen::default(),
                "Omit lower priority guidance.",
            ),
        ];

        let bundle = build_instruction_bundle(
            &test_intent(Vec::new()),
            &policies,
            BundleBuildOptions {
                max_tokens: Some(900),
                max_instructions: Some(1),
                max_required_checks: Some(4),
                max_blocked_actions: Some(4),
            },
        )
        .expect("bundle should build");

        assert_eq!(bundle.instructions.len(), 1);
        assert_eq!(
            bundle.instructions[0].text,
            "Keep the highest priority guidance."
        );
        assert_eq!(bundle.context_budget.candidate_policies_considered, Some(2));
        assert_eq!(bundle.context_budget.candidate_policies_omitted, Some(1));
        assert_eq!(
            bundle.context_budget.reason.as_deref(),
            Some("Lower priority or duplicate non-mandatory guidance excluded by context budget.")
        );
        assert_eq!(
            bundle.warnings,
            vec!["Context budget omitted 1 candidate policy."]
        );
        assert_eq!(render_bundle_markdown(&bundle), BUDGETED_MARKDOWN_SNAPSHOT);
    }

    #[test]
    fn duplicate_instructions_checks_and_actions_are_removed_before_limits() {
        let mut first = test_policy(
            "policy.first",
            AppliesWhen::default(),
            "Use exact duplicate guidance.",
        );
        first.policy.required_checks = vec!["cargo test".into(), "cargo test".into()];
        first.policy.blocked_actions = vec![
            BlockedAction("Do not edit generated code.".into()),
            BlockedAction("Do not edit generated code.".into()),
        ];

        let mut second = test_policy(
            "policy.second",
            AppliesWhen::default(),
            "Use exact duplicate guidance.",
        );
        second.policy.instructions = vec![
            "Use exact duplicate guidance.".into(),
            "Keep unique guidance after duplicate.".into(),
        ];
        second.policy.required_checks = vec!["cargo test".into(), "cargo fmt --check".into()];
        second.policy.blocked_actions = vec![
            BlockedAction("Do not edit generated code.".into()),
            BlockedAction("Do not commit secrets.".into()),
        ];

        let bundle = build_instruction_bundle(
            &test_intent(Vec::new()),
            &[first, second],
            BundleBuildOptions {
                max_tokens: Some(900),
                max_instructions: Some(2),
                max_required_checks: Some(2),
                max_blocked_actions: Some(2),
            },
        )
        .expect("bundle should build");

        assert_eq!(
            bundle
                .instructions
                .iter()
                .map(|instruction| instruction.text.as_str())
                .collect::<Vec<_>>(),
            vec![
                "Use exact duplicate guidance.",
                "Keep unique guidance after duplicate.",
            ]
        );
        assert_eq!(
            bundle
                .required_checks
                .iter()
                .map(|check| check.id.as_str())
                .collect::<Vec<_>>(),
            vec!["cargo test", "cargo fmt --check"]
        );
        assert_eq!(
            bundle
                .blocked_actions
                .iter()
                .map(|action| action.0.as_str())
                .collect::<Vec<_>>(),
            vec!["Do not edit generated code.", "Do not commit secrets."]
        );
        assert_eq!(bundle.context_budget.candidate_policies_omitted, Some(0));
    }

    #[test]
    fn renders_bundle_markdown_snapshot() {
        let bundle: InstructionBundle = serde_json::from_str(SAMPLE_BUNDLE_JSON).unwrap();
        let markdown = render_bundle_markdown(&bundle);

        assert_eq!(markdown, SAMPLE_BUNDLE_MARKDOWN);
        assert!(!markdown.contains("applies_when:"));
        assert!(!markdown.contains("instructions:"));
    }

    fn fixture_simple_repo() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/simple-repo")
    }

    fn test_policy(id: &str, applies_when: AppliesWhen, instruction: &str) -> LoadedPolicy {
        LoadedPolicy {
            policy: Policy {
                id: id.into(),
                version: PolicyVersion::Integer(1),
                status: PolicyStatus::Active,
                owner: None,
                priority: None,
                applies_when,
                instructions: vec![instruction.into()],
                required_checks: Vec::new(),
                blocked_actions: Vec::new(),
                retrieval: None,
                metadata: None,
            },
            source_path: PathBuf::from(format!("{id}.yaml")),
        }
    }

    fn test_intent(files: Vec<&str>) -> TaskIntent {
        TaskIntent {
            repo: None,
            branch: None,
            task: None,
            files: files.into_iter().map(str::to_string).collect(),
            detected: None,
            risk_flags: Vec::new(),
            expected_commands: Vec::new(),
            expected_check_ids: Vec::new(),
            output_budget: None,
        }
    }

    fn default_build_options() -> BundleBuildOptions {
        BundleBuildOptions {
            max_tokens: None,
            max_instructions: None,
            max_required_checks: None,
            max_blocked_actions: None,
        }
    }
}
