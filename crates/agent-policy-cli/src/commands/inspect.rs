use std::collections::BTreeMap;
use std::path::Path;

use agent_policy_discover::{
    discover, discover_codex, DiscoveryResult, InstructionSource, InstructionSourceType,
    MarkdownInstructionCandidate, MarkdownInstructionCandidateType,
};

use crate::cli::{GlobalArgs, InspectArgs, InstructionDiscoveryMode, OutputFormat};
use crate::commands::discover::codex_options;
use crate::commands::get::normalize_scope_prefix;
use crate::render::{instruction_source_type_name, json_escape, push_unique};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InspectionReport {
    pub(crate) repo: String,
    pub(crate) summary: InspectionSummary,
    pub(crate) instruction_sources: Vec<InspectionSource>,
    pub(crate) candidate_instructions: Vec<InspectionCandidate>,
    pub(crate) duplicates: Vec<InspectionDuplicate>,
    pub(crate) conflicts: Vec<InspectionConflict>,
    pub(crate) migration_candidates: Vec<MigrationCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InspectionSummary {
    pub(crate) source_count: usize,
    pub(crate) candidate_instruction_count: usize,
    pub(crate) duplicate_count: usize,
    pub(crate) conflict_count: usize,
    pub(crate) migration_candidate_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InspectionSource {
    pub(crate) path: String,
    pub(crate) scope: String,
    pub(crate) source_type: InstructionSourceType,
    pub(crate) instruction_count: usize,
    pub(crate) labels: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InspectionCandidate {
    pub(crate) text: String,
    pub(crate) source: String,
    pub(crate) line: usize,
    pub(crate) scope: String,
    pub(crate) candidate_type: String,
    pub(crate) topic: String,
    pub(crate) migration_class: MigrationClass,
    pub(crate) target_policy: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InspectionDuplicate {
    pub(crate) instruction: String,
    pub(crate) sources: Vec<String>,
    pub(crate) suggestion: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InspectionConflict {
    pub(crate) topic: String,
    pub(crate) sources: Vec<String>,
    pub(crate) summary: String,
    pub(crate) suggestion: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MigrationCandidate {
    pub(crate) target_policy: String,
    pub(crate) source: String,
    pub(crate) migration_class: MigrationClass,
    pub(crate) instructions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MigrationClass {
    KeepLocal,
    RepoPolicy,
    SharedRegistryPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MigrationDryRunReport {
    pub(crate) repo: String,
    pub(crate) mode: &'static str,
    pub(crate) summary: MigrationDryRunSummary,
    pub(crate) drafts: Vec<PolicyDraft>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MigrationDryRunSummary {
    pub(crate) source_count: usize,
    pub(crate) candidate_instruction_count: usize,
    pub(crate) draft_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PolicyDraft {
    pub(crate) id: String,
    pub(crate) target_path: String,
    pub(crate) migration_class: MigrationClass,
    pub(crate) applies_when_paths: Vec<String>,
    pub(crate) instructions: Vec<String>,
    pub(crate) required_checks: Vec<String>,
    pub(crate) generated_from: Vec<PolicyDraftProvenance>,
    pub(crate) policy_yaml: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PolicyDraftProvenance {
    path: String,
    source_type: InstructionSourceType,
    scope: String,
    lines: Vec<usize>,
}

pub(crate) fn run(global: &GlobalArgs, args: InspectArgs) -> anyhow::Result<()> {
    let repo = global.repo.as_deref().unwrap_or_else(|| Path::new("."));
    let discovered = match args.mode {
        InstructionDiscoveryMode::Generic => discover(repo)?,
        InstructionDiscoveryMode::Codex => discover_codex(repo, codex_options(global, repo)?)?,
    };
    let report = inspect_repo(repo, discovered);

    match global.format.clone().unwrap_or(OutputFormat::Markdown) {
        OutputFormat::Json => {
            println!("{}", render_inspection_json(&report));
        }
        OutputFormat::Markdown => {
            print!("{}", render_inspection_markdown(&report));
        }
    }

    Ok(())
}

pub(crate) fn inspect_repo(repo: &Path, discovered: DiscoveryResult) -> InspectionReport {
    let repo_name = repo
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(".")
        .to_string();
    let candidate_instructions = inspection_candidates(&discovered);
    let instruction_sources = discovered
        .instruction_sources
        .iter()
        .map(inspection_source)
        .collect::<Vec<_>>();
    let duplicates = detect_inspection_duplicates(&candidate_instructions);
    let conflicts = detect_inspection_conflicts(&candidate_instructions);
    let migration_candidates = classify_migration_candidates(&candidate_instructions);

    InspectionReport {
        repo: repo_name,
        summary: InspectionSummary {
            source_count: instruction_sources.len(),
            candidate_instruction_count: candidate_instructions.len(),
            duplicate_count: duplicates.len(),
            conflict_count: conflicts.len(),
            migration_candidate_count: migration_candidates.len(),
        },
        instruction_sources,
        candidate_instructions,
        duplicates,
        conflicts,
        migration_candidates,
    }
}

fn inspection_source(source: &InstructionSource) -> InspectionSource {
    let mut labels = Vec::new();
    push_labels_from_path(&mut labels, &source.path);
    for candidate in &source.candidates {
        push_unique(&mut labels, candidate_topic(candidate).to_string());
    }

    InspectionSource {
        path: source.path.clone(),
        scope: source.scope.clone(),
        source_type: source.source_type.clone(),
        instruction_count: source.candidates.len(),
        labels,
    }
}

fn inspection_candidates(discovered: &DiscoveryResult) -> Vec<InspectionCandidate> {
    discovered
        .instruction_sources
        .iter()
        .flat_map(|source| {
            source.candidates.iter().map(|candidate| {
                let topic = candidate_topic(candidate).to_string();
                let (migration_class, target_policy) =
                    classify_candidate_migration(candidate, &topic);
                InspectionCandidate {
                    text: candidate.text.clone(),
                    source: candidate.provenance.path.clone(),
                    line: candidate.line,
                    scope: candidate.provenance.scope.clone(),
                    candidate_type: match candidate.candidate_type {
                        MarkdownInstructionCandidateType::Instruction => "instruction",
                        MarkdownInstructionCandidateType::RequiredCheck => "required_check",
                    }
                    .to_string(),
                    topic,
                    migration_class,
                    target_policy,
                }
            })
        })
        .collect()
}

pub(crate) fn detect_inspection_duplicates(
    candidates: &[InspectionCandidate],
) -> Vec<InspectionDuplicate> {
    let mut by_instruction = BTreeMap::<String, Vec<&InspectionCandidate>>::new();
    for candidate in candidates
        .iter()
        .filter(|candidate| candidate.candidate_type == "instruction")
    {
        by_instruction
            .entry(candidate.text.clone())
            .or_default()
            .push(candidate);
    }

    by_instruction
        .into_iter()
        .filter_map(|(instruction, matches)| {
            if matches.len() < 2 {
                return None;
            }
            let sources = matches
                .iter()
                .map(|candidate| source_line_ref(candidate))
                .collect::<Vec<_>>();
            let suggestion = if matches
                .iter()
                .any(|candidate| candidate.migration_class == MigrationClass::SharedRegistryPolicy)
            {
                "Move repeated guidance to a shared registry policy.".to_string()
            } else {
                "Move repeated guidance to a repo policy or keep the narrowest scoped copy."
                    .to_string()
            };
            Some(InspectionDuplicate {
                instruction,
                sources,
                suggestion,
            })
        })
        .collect()
}

pub(crate) fn detect_inspection_conflicts(
    candidates: &[InspectionCandidate],
) -> Vec<InspectionConflict> {
    let mut conflicts = Vec::new();
    detect_package_manager_conflicts(candidates, &mut conflicts);
    detect_generated_file_conflicts(candidates, &mut conflicts);
    detect_secret_conflicts(candidates, &mut conflicts);
    conflicts
}

fn detect_package_manager_conflicts(
    candidates: &[InspectionCandidate],
    conflicts: &mut Vec<InspectionConflict>,
) {
    let package_manager_candidates = candidates
        .iter()
        .filter_map(|candidate| {
            package_manager_preference(&candidate.text).map(|pm| (pm, candidate))
        })
        .collect::<Vec<_>>();

    for (index, (left_pm, left)) in package_manager_candidates.iter().enumerate() {
        for (right_pm, right) in package_manager_candidates.iter().skip(index + 1) {
            if left_pm == right_pm {
                continue;
            }
            if conflicts.iter().any(|conflict| {
                conflict.topic == "package_manager"
                    && conflict.sources.contains(&source_line_ref(left))
                    && conflict.sources.contains(&source_line_ref(right))
            }) {
                continue;
            }
            let winner = more_specific_candidate(left, right);
            conflicts.push(InspectionConflict {
                topic: "package_manager".to_string(),
                sources: vec![source_line_ref(left), source_line_ref(right)],
                summary: format!(
                    "{} says {}; {} says {}.",
                    left.source, left_pm, right.source, right_pm
                ),
                suggestion: format!(
                    "Keep the `{}` guidance scoped to `{}` if this is an intentional override.",
                    package_manager_preference(&winner.text).unwrap_or("package manager"),
                    winner.scope
                ),
            });
        }
    }
}

fn detect_generated_file_conflicts(
    candidates: &[InspectionCandidate],
    conflicts: &mut Vec<InspectionConflict>,
) {
    let prohibits = candidates
        .iter()
        .filter(|candidate| generated_file_mode(&candidate.text) == Some("avoid_direct_edit"))
        .collect::<Vec<_>>();
    let allows = candidates
        .iter()
        .filter(|candidate| generated_file_mode(&candidate.text) == Some("direct_edit"))
        .collect::<Vec<_>>();

    for prohibit in &prohibits {
        for allow in &allows {
            conflicts.push(InspectionConflict {
                topic: "generated_files".to_string(),
                sources: vec![source_line_ref(prohibit), source_line_ref(allow)],
                summary: "Generated-file guidance both prohibits and asks for direct edits."
                    .to_string(),
                suggestion:
                    "Prefer updating the generator or source schema; scope any exception narrowly."
                        .to_string(),
            });
        }
    }
}

fn detect_secret_conflicts(
    candidates: &[InspectionCandidate],
    conflicts: &mut Vec<InspectionConflict>,
) {
    let prohibits = candidates
        .iter()
        .filter(|candidate| secret_mode(&candidate.text) == Some("protect"))
        .collect::<Vec<_>>();
    let allows = candidates
        .iter()
        .filter(|candidate| secret_mode(&candidate.text) == Some("allow"))
        .collect::<Vec<_>>();

    for prohibit in &prohibits {
        for allow in &allows {
            conflicts.push(InspectionConflict {
                topic: "secrets".to_string(),
                sources: vec![source_line_ref(prohibit), source_line_ref(allow)],
                summary:
                    "Secret-handling guidance both protects secrets and permits exposing them."
                        .to_string(),
                suggestion:
                    "Keep the stricter safety rule and remove or rewrite the weaker guidance."
                        .to_string(),
            });
        }
    }
}

fn classify_migration_candidates(candidates: &[InspectionCandidate]) -> Vec<MigrationCandidate> {
    let mut grouped = BTreeMap::<(String, String, String), Vec<String>>::new();
    for candidate in candidates
        .iter()
        .filter(|candidate| candidate.candidate_type == "instruction")
    {
        if let Some(target_policy) = &candidate.target_policy {
            grouped
                .entry((
                    target_policy.clone(),
                    candidate.source.clone(),
                    migration_class_name(&candidate.migration_class).to_string(),
                ))
                .or_default()
                .push(candidate.text.clone());
        }
    }

    grouped
        .into_iter()
        .map(
            |((target_policy, source, migration_class), instructions)| MigrationCandidate {
                target_policy,
                source,
                migration_class: migration_class_from_name(&migration_class),
                instructions,
            },
        )
        .collect()
}

fn classify_candidate_migration(
    candidate: &MarkdownInstructionCandidate,
    topic: &str,
) -> (MigrationClass, Option<String>) {
    if candidate.candidate_type == MarkdownInstructionCandidateType::RequiredCheck {
        return (
            if candidate.provenance.scope == "." {
                MigrationClass::RepoPolicy
            } else {
                MigrationClass::KeepLocal
            },
            Some(target_policy_for_candidate(candidate, topic)),
        );
    }

    let class = if matches!(topic, "generated_files" | "secrets" | "security") {
        MigrationClass::SharedRegistryPolicy
    } else if candidate.provenance.scope == "." {
        MigrationClass::RepoPolicy
    } else {
        MigrationClass::KeepLocal
    };
    let target_policy = Some(target_policy_for_candidate(candidate, topic));
    (class, target_policy)
}

fn target_policy_for_candidate(candidate: &MarkdownInstructionCandidate, topic: &str) -> String {
    match topic {
        "generated_files" => return "org.generated-files".to_string(),
        "secrets" | "security" => return "org.security".to_string(),
        _ => {}
    }

    if candidate.provenance.scope != "." {
        let scope = policy_id_scope_component(&candidate.provenance.scope);
        if !scope.is_empty() {
            return format!("local.{scope}.{topic}");
        }
    }

    match topic {
        "payments" => "domain.payments".to_string(),
        "api_contracts" => "repo.api-contracts".to_string(),
        "tests" | "required_check" => "repo.checks".to_string(),
        "package_manager" => "repo.package-manager".to_string(),
        _ => "repo.instructions".to_string(),
    }
}

fn policy_id_scope_component(scope: &str) -> String {
    normalize_scope_prefix(scope)
        .split('/')
        .filter_map(sanitize_policy_id_segment)
        .collect::<Vec<_>>()
        .join(".")
}

fn sanitize_policy_id_segment(segment: &str) -> Option<String> {
    let mut sanitized = String::new();
    let mut last_was_separator = false;

    for character in segment.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
            sanitized.push(character.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator {
            sanitized.push('-');
            last_was_separator = true;
        }
    }

    let sanitized = sanitized.trim_matches('-').to_string();
    (!sanitized.is_empty()).then_some(sanitized)
}

fn candidate_topic(candidate: &MarkdownInstructionCandidate) -> &'static str {
    if candidate.candidate_type == MarkdownInstructionCandidateType::RequiredCheck {
        return "required_check";
    }

    let text = normalized_conflict_text(&candidate.text);
    if package_manager_preference(&candidate.text).is_some() {
        "package_manager"
    } else if text.contains("generated") {
        "generated_files"
    } else if text.contains("secret") || text.contains("credential") || text.contains("token") {
        "secrets"
    } else if text.contains("payment") || text.contains("refund") || text.contains("billing") {
        "payments"
    } else if text.contains("api") || text.contains("contract") {
        "api_contracts"
    } else if text.contains("test") || text.contains("check") || text.contains("validate") {
        "tests"
    } else if text.contains("accessible") || text.contains("react") {
        "frontend"
    } else if text.contains("policy broker") || text.contains("policy guidance") {
        "policy_broker"
    } else {
        "general"
    }
}

fn push_labels_from_path(labels: &mut Vec<String>, path: &str) {
    let normalized = path.to_ascii_lowercase();
    for (needle, label) in [
        ("frontend", "frontend"),
        ("backend", "backend"),
        ("payments", "payments"),
        ("react", "react"),
        ("copilot", "copilot"),
        ("cursor", "cursor"),
    ] {
        if normalized.contains(needle) {
            push_unique(labels, label.to_string());
        }
    }
}

fn package_manager_preference(text: &str) -> Option<&'static str> {
    let normalized = normalized_conflict_text(text);
    let mut found = Vec::new();
    for package_manager in ["npm", "pnpm", "yarn"] {
        if normalized
            .split_whitespace()
            .any(|word| word == package_manager)
        {
            found.push(package_manager);
        }
    }
    if found.len() == 1 {
        Some(found[0])
    } else {
        None
    }
}

fn generated_file_mode(text: &str) -> Option<&'static str> {
    let normalized = normalized_conflict_text(text);
    if !normalized.contains("generated") {
        return None;
    }
    if normalized.contains("do not")
        || normalized.contains("never")
        || normalized.contains("avoid")
        || normalized.contains("instead")
    {
        Some("avoid_direct_edit")
    } else if normalized.contains("edit")
        || normalized.contains("modify")
        || normalized.contains("change")
    {
        Some("direct_edit")
    } else {
        None
    }
}

fn secret_mode(text: &str) -> Option<&'static str> {
    let normalized = normalized_conflict_text(text);
    if !(normalized.contains("secret")
        || normalized.contains("credential")
        || normalized.contains("token"))
    {
        return None;
    }
    if normalized.contains("do not")
        || normalized.contains("never")
        || normalized.contains("avoid")
        || normalized.contains("protect")
    {
        Some("protect")
    } else if normalized.contains("may")
        || normalized.contains("allow")
        || normalized.contains("commit")
        || normalized.contains("log")
    {
        Some("allow")
    } else {
        None
    }
}

fn normalized_conflict_text(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn more_specific_candidate<'a>(
    left: &'a InspectionCandidate,
    right: &'a InspectionCandidate,
) -> &'a InspectionCandidate {
    if scope_depth(&left.scope) >= scope_depth(&right.scope) {
        left
    } else {
        right
    }
}

fn scope_depth(scope: &str) -> usize {
    normalize_scope_prefix(scope)
        .split('/')
        .filter(|part| !part.is_empty())
        .count()
}

fn source_line_ref(candidate: &InspectionCandidate) -> String {
    format!("{}:{}", candidate.source, candidate.line)
}

pub(crate) fn migration_class_name(class: &MigrationClass) -> &'static str {
    match class {
        MigrationClass::KeepLocal => "keep_local",
        MigrationClass::RepoPolicy => "repo_policy",
        MigrationClass::SharedRegistryPolicy => "shared_registry_policy",
    }
}

fn migration_class_from_name(name: &str) -> MigrationClass {
    match name {
        "shared_registry_policy" => MigrationClass::SharedRegistryPolicy,
        "repo_policy" => MigrationClass::RepoPolicy,
        _ => MigrationClass::KeepLocal,
    }
}

pub(crate) fn migration_dry_run_report(inspection: &InspectionReport) -> MigrationDryRunReport {
    let source_types = inspection
        .instruction_sources
        .iter()
        .map(|source| (source.path.clone(), source.source_type.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut grouped = BTreeMap::<String, DraftGroup>::new();

    for candidate in &inspection.candidate_instructions {
        let Some(target_policy) = &candidate.target_policy else {
            continue;
        };
        let group = grouped
            .entry(target_policy.clone())
            .or_insert_with(|| DraftGroup::new(target_policy, &candidate.migration_class));
        group.migration_class =
            strongest_migration_class(&group.migration_class, &candidate.migration_class);
        if candidate.candidate_type == "required_check" {
            push_unique(&mut group.required_checks, candidate.text.clone());
        } else {
            push_unique(&mut group.instructions, candidate.text.clone());
        }
        if candidate.scope != "." {
            push_unique(&mut group.applies_when_paths, candidate.scope.clone());
        }
        let source_type = source_types
            .get(&candidate.source)
            .cloned()
            .unwrap_or(InstructionSourceType::AgentsMd);
        group.add_provenance(
            &candidate.source,
            source_type,
            &candidate.scope,
            candidate.line,
        );
    }

    let drafts = grouped
        .into_values()
        .map(|group| group.into_policy_draft())
        .collect::<Vec<_>>();

    MigrationDryRunReport {
        repo: inspection.repo.clone(),
        mode: "dry_run",
        summary: MigrationDryRunSummary {
            source_count: inspection.summary.source_count,
            candidate_instruction_count: inspection.summary.candidate_instruction_count,
            draft_count: drafts.len(),
        },
        drafts,
    }
}

#[derive(Debug)]
struct DraftGroup {
    id: String,
    migration_class: MigrationClass,
    applies_when_paths: Vec<String>,
    instructions: Vec<String>,
    required_checks: Vec<String>,
    generated_from: Vec<PolicyDraftProvenance>,
}

impl DraftGroup {
    fn new(id: &str, migration_class: &MigrationClass) -> Self {
        Self {
            id: id.to_string(),
            migration_class: migration_class.clone(),
            applies_when_paths: Vec::new(),
            instructions: Vec::new(),
            required_checks: Vec::new(),
            generated_from: Vec::new(),
        }
    }

    fn add_provenance(
        &mut self,
        path: &str,
        source_type: InstructionSourceType,
        scope: &str,
        line: usize,
    ) {
        if let Some(existing) = self.generated_from.iter_mut().find(|item| {
            item.path == path && item.scope == scope && item.source_type == source_type
        }) {
            if !existing.lines.contains(&line) {
                existing.lines.push(line);
                existing.lines.sort_unstable();
            }
            return;
        }

        self.generated_from.push(PolicyDraftProvenance {
            path: path.to_string(),
            source_type,
            scope: scope.to_string(),
            lines: vec![line],
        });
    }

    fn into_policy_draft(mut self) -> PolicyDraft {
        self.generated_from.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.lines.cmp(&right.lines))
        });
        let target_path = suggested_policy_path(&self.id);
        let mut draft = PolicyDraft {
            id: self.id,
            target_path,
            migration_class: self.migration_class,
            applies_when_paths: self.applies_when_paths,
            instructions: self.instructions,
            required_checks: self.required_checks,
            generated_from: self.generated_from,
            policy_yaml: String::new(),
        };
        draft.policy_yaml = render_policy_draft_yaml(&draft);
        draft
    }
}

fn strongest_migration_class(left: &MigrationClass, right: &MigrationClass) -> MigrationClass {
    if migration_class_rank(right) > migration_class_rank(left) {
        right.clone()
    } else {
        left.clone()
    }
}

fn migration_class_rank(class: &MigrationClass) -> u8 {
    match class {
        MigrationClass::KeepLocal => 0,
        MigrationClass::RepoPolicy => 1,
        MigrationClass::SharedRegistryPolicy => 2,
    }
}

fn suggested_policy_path(policy_id: &str) -> String {
    let file_name = policy_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    format!(".agent-policy/migration/{file_name}.yaml")
}

fn render_policy_draft_yaml(draft: &PolicyDraft) -> String {
    let mut out = String::new();
    out.push_str("id: ");
    out.push_str(&yaml_string(&draft.id));
    out.push('\n');
    out.push_str("version: 1\n");
    out.push_str("status: draft\n\n");
    out.push_str("applies_when:");
    if draft.applies_when_paths.is_empty() {
        out.push_str(" {}\n\n");
    } else {
        out.push('\n');
        out.push_str("  paths:\n");
        for path in &draft.applies_when_paths {
            out.push_str("    - ");
            out.push_str(&yaml_string(path));
            out.push('\n');
        }
        out.push('\n');
    }

    render_yaml_string_list(&mut out, "instructions", &draft.instructions);
    if !draft.required_checks.is_empty() {
        out.push('\n');
        render_yaml_string_list(&mut out, "required_checks", &draft.required_checks);
    }

    out.push('\n');
    out.push_str("metadata:\n");
    out.push_str("  generated_from:\n");
    for provenance in &draft.generated_from {
        out.push_str("    - path: ");
        out.push_str(&yaml_string(&provenance.path));
        out.push('\n');
        out.push_str("      source_type: ");
        out.push_str(instruction_source_type_name(&provenance.source_type));
        out.push('\n');
        out.push_str("      scope: ");
        out.push_str(&yaml_string(&provenance.scope));
        out.push('\n');
        out.push_str("      lines:");
        if provenance.lines.is_empty() {
            out.push_str(" []\n");
        } else {
            out.push('\n');
            for line in &provenance.lines {
                out.push_str(&format!("        - {line}\n"));
            }
        }
    }
    out.push_str("  migration_status: proposed\n");
    out.push_str("  migration_class: ");
    out.push_str(migration_class_name(&draft.migration_class));
    out.push('\n');
    out
}

fn render_yaml_string_list(out: &mut String, field: &str, values: &[String]) {
    out.push_str(field);
    out.push(':');
    if values.is_empty() {
        out.push_str(" []\n");
        return;
    }
    out.push('\n');
    for value in values {
        out.push_str("  - ");
        out.push_str(&yaml_string(value));
        out.push('\n');
    }
}

fn yaml_string(value: &str) -> String {
    format!("\"{}\"", json_escape(value))
}

pub(crate) fn render_inspection_json(report: &InspectionReport) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"repo\": \"{}\",\n", json_escape(&report.repo)));
    out.push_str("  \"summary\": {\n");
    out.push_str(&format!(
        "    \"source_count\": {},\n",
        report.summary.source_count
    ));
    out.push_str(&format!(
        "    \"candidate_instruction_count\": {},\n",
        report.summary.candidate_instruction_count
    ));
    out.push_str(&format!(
        "    \"duplicate_count\": {},\n",
        report.summary.duplicate_count
    ));
    out.push_str(&format!(
        "    \"conflict_count\": {},\n",
        report.summary.conflict_count
    ));
    out.push_str(&format!(
        "    \"migration_candidate_count\": {}\n",
        report.summary.migration_candidate_count
    ));
    out.push_str("  },\n");

    out.push_str("  \"instruction_sources\": ");
    render_inspection_sources_json(&mut out, &report.instruction_sources, 2);
    out.push_str(",\n  \"candidate_instructions\": ");
    render_inspection_candidates_json(&mut out, &report.candidate_instructions, 2);
    out.push_str(",\n  \"duplicates\": ");
    render_inspection_duplicates_json(&mut out, &report.duplicates, 2);
    out.push_str(",\n  \"conflicts\": ");
    render_inspection_conflicts_json(&mut out, &report.conflicts, 2);
    out.push_str(",\n  \"migration_candidates\": ");
    render_migration_candidates_json(&mut out, &report.migration_candidates, 2);
    out.push_str("\n}");
    out
}

fn render_inspection_sources_json(out: &mut String, sources: &[InspectionSource], indent: usize) {
    let pad = " ".repeat(indent);
    let item_pad = " ".repeat(indent + 2);
    out.push_str("[\n");
    for (index, source) in sources.iter().enumerate() {
        out.push_str(&item_pad);
        out.push_str("{\n");
        out.push_str(&format!(
            "{}  \"path\": \"{}\",\n",
            item_pad,
            json_escape(&source.path)
        ));
        out.push_str(&format!(
            "{}  \"scope\": \"{}\",\n",
            item_pad,
            json_escape(&source.scope)
        ));
        out.push_str(&format!(
            "{}  \"type\": \"{}\",\n",
            item_pad,
            instruction_source_type_name(&source.source_type)
        ));
        out.push_str(&format!(
            "{}  \"instruction_count\": {},\n",
            item_pad, source.instruction_count
        ));
        out.push_str(&format!("{}  \"labels\": ", item_pad));
        render_string_array_json(out, &source.labels, indent + 4);
        out.push('\n');
        out.push_str(&item_pad);
        out.push('}');
        if index + 1 != sources.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&pad);
    out.push(']');
}

fn render_inspection_candidates_json(
    out: &mut String,
    candidates: &[InspectionCandidate],
    indent: usize,
) {
    let pad = " ".repeat(indent);
    let item_pad = " ".repeat(indent + 2);
    out.push_str("[\n");
    for (index, candidate) in candidates.iter().enumerate() {
        out.push_str(&item_pad);
        out.push_str("{\n");
        out.push_str(&format!(
            "{}  \"text\": \"{}\",\n",
            item_pad,
            json_escape(&candidate.text)
        ));
        out.push_str(&format!(
            "{}  \"source\": \"{}\",\n",
            item_pad,
            json_escape(&candidate.source)
        ));
        out.push_str(&format!("{}  \"line\": {},\n", item_pad, candidate.line));
        out.push_str(&format!(
            "{}  \"scope\": \"{}\",\n",
            item_pad,
            json_escape(&candidate.scope)
        ));
        out.push_str(&format!(
            "{}  \"candidate_type\": \"{}\",\n",
            item_pad,
            json_escape(&candidate.candidate_type)
        ));
        out.push_str(&format!(
            "{}  \"topic\": \"{}\",\n",
            item_pad,
            json_escape(&candidate.topic)
        ));
        out.push_str(&format!(
            "{}  \"migration_class\": \"{}\"",
            item_pad,
            migration_class_name(&candidate.migration_class)
        ));
        if let Some(target_policy) = &candidate.target_policy {
            out.push_str(&format!(
                ",\n{}  \"target_policy\": \"{}\"",
                item_pad,
                json_escape(target_policy)
            ));
        }
        out.push('\n');
        out.push_str(&item_pad);
        out.push('}');
        if index + 1 != candidates.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&pad);
    out.push(']');
}

fn render_inspection_duplicates_json(
    out: &mut String,
    duplicates: &[InspectionDuplicate],
    indent: usize,
) {
    let pad = " ".repeat(indent);
    let item_pad = " ".repeat(indent + 2);
    out.push_str("[\n");
    for (index, duplicate) in duplicates.iter().enumerate() {
        out.push_str(&item_pad);
        out.push_str("{\n");
        out.push_str(&format!(
            "{}  \"instruction\": \"{}\",\n",
            item_pad,
            json_escape(&duplicate.instruction)
        ));
        out.push_str(&format!("{}  \"sources\": ", item_pad));
        render_string_array_json(out, &duplicate.sources, indent + 4);
        out.push_str(",\n");
        out.push_str(&format!(
            "{}  \"suggestion\": \"{}\"\n",
            item_pad,
            json_escape(&duplicate.suggestion)
        ));
        out.push_str(&item_pad);
        out.push('}');
        if index + 1 != duplicates.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&pad);
    out.push(']');
}

fn render_inspection_conflicts_json(
    out: &mut String,
    conflicts: &[InspectionConflict],
    indent: usize,
) {
    let pad = " ".repeat(indent);
    let item_pad = " ".repeat(indent + 2);
    out.push_str("[\n");
    for (index, conflict) in conflicts.iter().enumerate() {
        out.push_str(&item_pad);
        out.push_str("{\n");
        out.push_str(&format!(
            "{}  \"topic\": \"{}\",\n",
            item_pad,
            json_escape(&conflict.topic)
        ));
        out.push_str(&format!("{}  \"sources\": ", item_pad));
        render_string_array_json(out, &conflict.sources, indent + 4);
        out.push_str(",\n");
        out.push_str(&format!(
            "{}  \"summary\": \"{}\",\n",
            item_pad,
            json_escape(&conflict.summary)
        ));
        out.push_str(&format!(
            "{}  \"suggestion\": \"{}\"\n",
            item_pad,
            json_escape(&conflict.suggestion)
        ));
        out.push_str(&item_pad);
        out.push('}');
        if index + 1 != conflicts.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&pad);
    out.push(']');
}

fn render_migration_candidates_json(
    out: &mut String,
    candidates: &[MigrationCandidate],
    indent: usize,
) {
    let pad = " ".repeat(indent);
    let item_pad = " ".repeat(indent + 2);
    out.push_str("[\n");
    for (index, candidate) in candidates.iter().enumerate() {
        out.push_str(&item_pad);
        out.push_str("{\n");
        out.push_str(&format!(
            "{}  \"target_policy\": \"{}\",\n",
            item_pad,
            json_escape(&candidate.target_policy)
        ));
        out.push_str(&format!(
            "{}  \"source\": \"{}\",\n",
            item_pad,
            json_escape(&candidate.source)
        ));
        out.push_str(&format!(
            "{}  \"migration_class\": \"{}\",\n",
            item_pad,
            migration_class_name(&candidate.migration_class)
        ));
        out.push_str(&format!("{}  \"instructions\": ", item_pad));
        render_string_array_json(out, &candidate.instructions, indent + 4);
        out.push('\n');
        out.push_str(&item_pad);
        out.push('}');
        if index + 1 != candidates.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&pad);
    out.push(']');
}

pub(crate) fn render_migration_dry_run_json(report: &MigrationDryRunReport) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"repo\": \"{}\",\n", json_escape(&report.repo)));
    out.push_str(&format!("  \"mode\": \"{}\",\n", report.mode));
    out.push_str("  \"summary\": {\n");
    out.push_str(&format!(
        "    \"source_count\": {},\n",
        report.summary.source_count
    ));
    out.push_str(&format!(
        "    \"candidate_instruction_count\": {},\n",
        report.summary.candidate_instruction_count
    ));
    out.push_str(&format!(
        "    \"draft_count\": {}\n",
        report.summary.draft_count
    ));
    out.push_str("  },\n");
    out.push_str("  \"drafts\": ");
    render_policy_drafts_json(&mut out, &report.drafts, 2);
    out.push_str("\n}");
    out
}

fn render_policy_drafts_json(out: &mut String, drafts: &[PolicyDraft], indent: usize) {
    let pad = " ".repeat(indent);
    let item_pad = " ".repeat(indent + 2);
    out.push_str("[\n");
    for (index, draft) in drafts.iter().enumerate() {
        out.push_str(&item_pad);
        out.push_str("{\n");
        out.push_str(&format!(
            "{}  \"id\": \"{}\",\n",
            item_pad,
            json_escape(&draft.id)
        ));
        out.push_str(&format!(
            "{}  \"target_path\": \"{}\",\n",
            item_pad,
            json_escape(&draft.target_path)
        ));
        out.push_str(&format!(
            "{}  \"migration_class\": \"{}\",\n",
            item_pad,
            migration_class_name(&draft.migration_class)
        ));
        out.push_str(&format!("{}  \"generated_from\": ", item_pad));
        render_policy_draft_provenance_json(out, &draft.generated_from, indent + 4);
        out.push_str(",\n");
        out.push_str(&format!(
            "{}  \"policy_yaml\": \"{}\"\n",
            item_pad,
            json_escape(&draft.policy_yaml)
        ));
        out.push_str(&item_pad);
        out.push('}');
        if index + 1 != drafts.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&pad);
    out.push(']');
}

fn render_policy_draft_provenance_json(
    out: &mut String,
    generated_from: &[PolicyDraftProvenance],
    indent: usize,
) {
    let pad = " ".repeat(indent);
    let item_pad = " ".repeat(indent + 2);
    out.push_str("[\n");
    for (index, provenance) in generated_from.iter().enumerate() {
        out.push_str(&item_pad);
        out.push_str("{\n");
        out.push_str(&format!(
            "{}  \"path\": \"{}\",\n",
            item_pad,
            json_escape(&provenance.path)
        ));
        out.push_str(&format!(
            "{}  \"source_type\": \"{}\",\n",
            item_pad,
            instruction_source_type_name(&provenance.source_type)
        ));
        out.push_str(&format!(
            "{}  \"scope\": \"{}\",\n",
            item_pad,
            json_escape(&provenance.scope)
        ));
        out.push_str(&format!("{}  \"lines\": ", item_pad));
        render_usize_array_json(out, &provenance.lines, indent + 4);
        out.push('\n');
        out.push_str(&item_pad);
        out.push('}');
        if index + 1 != generated_from.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&pad);
    out.push(']');
}

fn render_usize_array_json(out: &mut String, values: &[usize], indent: usize) {
    if values.is_empty() {
        out.push_str("[]");
        return;
    }

    let pad = " ".repeat(indent);
    let item_pad = " ".repeat(indent + 2);
    out.push_str("[\n");
    for (index, value) in values.iter().enumerate() {
        out.push_str(&format!("{item_pad}{value}"));
        if index + 1 != values.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&pad);
    out.push(']');
}

fn render_string_array_json(out: &mut String, values: &[String], indent: usize) {
    if values.is_empty() {
        out.push_str("[]");
        return;
    }

    let pad = " ".repeat(indent);
    let item_pad = " ".repeat(indent + 2);
    out.push_str("[\n");
    for (index, value) in values.iter().enumerate() {
        out.push_str(&item_pad);
        out.push('"');
        out.push_str(&json_escape(value));
        out.push('"');
        if index + 1 != values.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&pad);
    out.push(']');
}

pub(crate) fn render_inspection_markdown(report: &InspectionReport) -> String {
    let mut out = String::new();
    out.push_str("# Agent Policy Inspection\n\n");
    out.push_str(&format!("- Repository: `{}`\n", report.repo));
    out.push_str(&format!(
        "- Sources: {}; candidate instructions: {}.\n",
        report.summary.source_count, report.summary.candidate_instruction_count
    ));
    out.push_str(&format!(
        "- Duplicates: {}; conflicts: {}; migration groups: {}.\n\n",
        report.summary.duplicate_count,
        report.summary.conflict_count,
        report.summary.migration_candidate_count
    ));

    out.push_str("## Instruction Sources\n\n");
    if report.instruction_sources.is_empty() {
        out.push_str("- None found.\n\n");
    } else {
        out.push_str("| Path | Scope | Type | Instructions | Labels |\n");
        out.push_str("| --- | --- | --- | ---: | --- |\n");
        for source in &report.instruction_sources {
            out.push_str(&format!(
                "| `{}` | `{}` | `{}` | {} | {} |\n",
                source.path,
                source.scope,
                instruction_source_type_name(&source.source_type),
                source.instruction_count,
                markdown_list_value(&source.labels)
            ));
        }
        out.push('\n');
    }

    out.push_str("## Candidate Instructions\n\n");
    if report.candidate_instructions.is_empty() {
        out.push_str("- None extracted.\n\n");
    } else {
        for candidate in &report.candidate_instructions {
            out.push_str(&format!(
                "- `{}` {} (`{}`, {}, {})\n",
                source_line_ref(candidate),
                candidate.text,
                candidate.topic,
                migration_class_name(&candidate.migration_class),
                candidate
                    .target_policy
                    .as_deref()
                    .unwrap_or("no target policy")
            ));
        }
        out.push('\n');
    }

    render_duplicate_section(&mut out, &report.duplicates);
    render_conflict_section(&mut out, &report.conflicts);
    render_migration_section(&mut out, &report.migration_candidates);
    out
}

pub(crate) fn render_migration_dry_run_markdown(report: &MigrationDryRunReport) -> String {
    let mut out = String::new();
    if report.mode == "write" {
        out.push_str("# Agent Policy Migration Write\n\n");
    } else {
        out.push_str("# Agent Policy Migration Dry Run\n\n");
    }
    out.push_str(&format!("- Repository: `{}`\n", report.repo));
    out.push_str(&format!("- Mode: `{}`\n", report.mode));
    out.push_str(&format!(
        "- Proposed drafts: {}; sources: {}; candidate instructions: {}.\n\n",
        report.summary.draft_count,
        report.summary.source_count,
        report.summary.candidate_instruction_count
    ));

    out.push_str("## Proposed Files\n\n");
    if report.drafts.is_empty() {
        out.push_str("- None proposed.\n");
        return out;
    }

    for draft in &report.drafts {
        out.push_str(&format!(
            "### `{}`\n\n",
            markdown_inline(&draft.target_path)
        ));
        out.push_str(&format!("- Policy: `{}`\n", markdown_inline(&draft.id)));
        out.push_str(&format!(
            "- Migration class: `{}`\n",
            migration_class_name(&draft.migration_class)
        ));
        out.push_str("- Generated from: ");
        out.push_str(
            &draft
                .generated_from
                .iter()
                .map(|provenance| {
                    format!(
                        "`{}` lines {}",
                        markdown_inline(&provenance.path),
                        provenance
                            .lines
                            .iter()
                            .map(|line| line.to_string())
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                })
                .collect::<Vec<_>>()
                .join(", "),
        );
        out.push_str("\n");
        out.push_str(&markdown_code_block("yaml", &draft.policy_yaml));
        out.push_str("\n");
    }

    out
}

fn markdown_inline(text: &str) -> String {
    text.replace('`', "\\`").replace(['\n', '\r'], " ")
}

fn markdown_code_block(language: &str, content: &str) -> String {
    let fence_len = max_backtick_run(content).saturating_add(1).max(3);
    let fence = "`".repeat(fence_len);
    let mut out = String::new();
    out.push_str(&fence);
    out.push_str(language);
    out.push('\n');
    out.push_str(content);
    if !content.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&fence);
    out.push('\n');
    out
}

fn max_backtick_run(content: &str) -> usize {
    let mut max_run = 0;
    let mut current_run = 0;
    for character in content.chars() {
        if character == '`' {
            current_run += 1;
            max_run = max_run.max(current_run);
        } else {
            current_run = 0;
        }
    }
    max_run
}

fn render_duplicate_section(out: &mut String, duplicates: &[InspectionDuplicate]) {
    out.push_str("## Duplicates\n\n");
    if duplicates.is_empty() {
        out.push_str("- None detected.\n\n");
        return;
    }
    for duplicate in duplicates {
        out.push_str(&format!(
            "- {} ({})\n  Suggestion: {}\n",
            duplicate.instruction,
            duplicate
                .sources
                .iter()
                .map(|source| format!("`{source}`"))
                .collect::<Vec<_>>()
                .join(", "),
            duplicate.suggestion
        ));
    }
    out.push('\n');
}

fn render_conflict_section(out: &mut String, conflicts: &[InspectionConflict]) {
    out.push_str("## Conflicts\n\n");
    if conflicts.is_empty() {
        out.push_str("- None detected.\n\n");
        return;
    }
    for conflict in conflicts {
        out.push_str(&format!(
            "- `{}`: {} ({})\n  Suggestion: {}\n",
            conflict.topic,
            conflict.summary,
            conflict
                .sources
                .iter()
                .map(|source| format!("`{source}`"))
                .collect::<Vec<_>>()
                .join(", "),
            conflict.suggestion
        ));
    }
    out.push('\n');
}

fn render_migration_section(out: &mut String, candidates: &[MigrationCandidate]) {
    out.push_str("## Migration Candidates\n\n");
    if candidates.is_empty() {
        out.push_str("- None proposed.\n");
        return;
    }
    for candidate in candidates {
        out.push_str(&format!(
            "- `{}` from `{}` ({})\n",
            candidate.target_policy,
            candidate.source,
            migration_class_name(&candidate.migration_class)
        ));
        for instruction in &candidate.instructions {
            out.push_str(&format!("  - {instruction}\n"));
        }
    }
}

fn markdown_list_value(values: &[String]) -> String {
    if values.is_empty() {
        "None".to_string()
    } else {
        values
            .iter()
            .map(|value| format!("`{value}`"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}
