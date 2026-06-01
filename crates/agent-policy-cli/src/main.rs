use clap::{Args, Parser, Subcommand, ValueEnum};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use agent_policy_config::{
    load_config, load_config_from_path, validate_config_file, RegistryConfig,
};
use agent_policy_core::{
    build_instruction_bundle, collect_policy_files, load_policies_from_dirs,
    load_policies_from_registry, render_bundle_json, render_bundle_markdown, validate_policy_files,
    AppliesWhen, BundleBuildOptions, DetectedContext, LoadedPolicy, OutputBudget, Policy,
    PolicyStatus, PolicyValidationSeverity, PolicyVersion, RegistryLoadOptions, SourceRef,
    TaskDetails, TaskIntent, TaskType,
};
use agent_policy_discover::{
    discover, discover_json, DiscoveryResult, InstructionSource, InstructionSourceType,
    MarkdownInstructionCandidate, MarkdownInstructionCandidateType,
};

#[derive(Debug, Parser)]
#[command(name = "agent-policy", version, about = "Agent Policy Broker CLI")]
struct Cli {
    #[command(flatten)]
    global: GlobalArgs,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, Debug, ValueEnum)]
enum OutputFormat {
    Json,
    Markdown,
}

#[derive(Debug, Args)]
struct GlobalArgs {
    #[arg(long, global = true, value_name = "path")]
    repo: Option<PathBuf>,
    #[arg(long, global = true, value_name = "path")]
    config: Option<PathBuf>,
    #[arg(long, global = true, value_enum)]
    format: Option<OutputFormat>,
    #[arg(long, global = true)]
    verbose: bool,
    #[arg(long, global = true)]
    quiet: bool,
    #[arg(long, global = true)]
    no_network: bool,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Compile a task-specific instruction bundle.
    Get(GetArgs),
    /// Discover existing instruction sources in a repository.
    Discover,
    /// Validate policies, config, and discovered instruction sources.
    Validate,
    /// Inspect repository guidance and produce an audit report.
    Inspect,
    /// Propose policy drafts from existing instruction sources.
    Migrate(MigrateArgs),
    /// Build or rebuild local retrieval indexes.
    Index,
    /// Manage policy registries.
    Registry(RegistryArgs),
    /// Run a local service for repeated lookups and integrations.
    Serve,
}

#[derive(Debug, Args)]
struct RegistryArgs {
    #[command(subcommand)]
    command: RegistryCommands,
}

#[derive(Debug, Args)]
struct GetArgs {
    #[arg(long, value_name = "text")]
    task: Option<String>,
    #[arg(long = "type", value_name = "task_type")]
    task_type: Option<String>,
    #[arg(long, value_name = "path", num_args = 1..)]
    files: Vec<String>,
    #[arg(long, value_name = "flag", num_args = 1..)]
    risk: Vec<String>,
    #[arg(long, value_name = "number")]
    max_instructions: Option<u32>,
    #[arg(long, value_name = "number")]
    max_tokens: Option<u32>,
}

#[derive(Debug, Args)]
struct MigrateArgs {
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    write: bool,
}

#[derive(Debug, Subcommand)]
enum RegistryCommands {
    /// Fetch or update a Git-backed policy registry.
    Sync,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::from(1)
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Commands::Discover => run_discover(&cli.global),
        Commands::Get(args) => run_get(&cli.global, args),
        Commands::Validate => run_validate(&cli.global),
        Commands::Inspect => run_inspect(&cli.global),
        Commands::Migrate(args) => run_migrate(&cli.global, args),
        Commands::Index => not_implemented("index"),
        Commands::Serve => not_implemented("serve"),
        Commands::Registry(registry) => match registry.command {
            RegistryCommands::Sync => not_implemented("registry sync"),
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InspectionReport {
    repo: String,
    summary: InspectionSummary,
    instruction_sources: Vec<InspectionSource>,
    candidate_instructions: Vec<InspectionCandidate>,
    duplicates: Vec<InspectionDuplicate>,
    conflicts: Vec<InspectionConflict>,
    migration_candidates: Vec<MigrationCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InspectionSummary {
    source_count: usize,
    candidate_instruction_count: usize,
    duplicate_count: usize,
    conflict_count: usize,
    migration_candidate_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InspectionSource {
    path: String,
    scope: String,
    source_type: InstructionSourceType,
    instruction_count: usize,
    labels: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InspectionCandidate {
    text: String,
    source: String,
    line: usize,
    scope: String,
    candidate_type: String,
    topic: String,
    migration_class: MigrationClass,
    target_policy: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InspectionDuplicate {
    instruction: String,
    sources: Vec<String>,
    suggestion: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InspectionConflict {
    topic: String,
    sources: Vec<String>,
    summary: String,
    suggestion: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MigrationCandidate {
    target_policy: String,
    source: String,
    migration_class: MigrationClass,
    instructions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MigrationClass {
    KeepLocal,
    RepoPolicy,
    SharedRegistryPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MigrationDryRunReport {
    repo: String,
    mode: &'static str,
    summary: MigrationDryRunSummary,
    drafts: Vec<PolicyDraft>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MigrationDryRunSummary {
    source_count: usize,
    candidate_instruction_count: usize,
    draft_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PolicyDraft {
    id: String,
    target_path: String,
    migration_class: MigrationClass,
    applies_when_paths: Vec<String>,
    instructions: Vec<String>,
    required_checks: Vec<String>,
    generated_from: Vec<PolicyDraftProvenance>,
    policy_yaml: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PolicyDraftProvenance {
    path: String,
    source_type: InstructionSourceType,
    scope: String,
    lines: Vec<usize>,
}

fn run_inspect(global: &GlobalArgs) -> anyhow::Result<()> {
    let repo = global.repo.as_deref().unwrap_or_else(|| Path::new("."));
    let discovered = discover(repo)?;
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

fn run_migrate(global: &GlobalArgs, args: MigrateArgs) -> anyhow::Result<()> {
    if args.dry_run && args.write {
        anyhow::bail!("migrate accepts either `--dry-run` or `--write`, not both");
    }
    if !args.dry_run && !args.write {
        anyhow::bail!("migrate requires either `--dry-run` or `--write`");
    }

    let repo = global.repo.as_deref().unwrap_or_else(|| Path::new("."));
    let discovered = discover(repo)?;
    let inspection = inspect_repo(repo, discovered);
    let mut report = migration_dry_run_report(&inspection);

    if args.write {
        write_migration_drafts(repo, &report.drafts)?;
        report.mode = "write";
    }

    match global.format.clone().unwrap_or(OutputFormat::Markdown) {
        OutputFormat::Json => {
            println!("{}", render_migration_dry_run_json(&report));
        }
        OutputFormat::Markdown => {
            print!("{}", render_migration_dry_run_markdown(&report));
        }
    }

    Ok(())
}

fn write_migration_drafts(repo: &Path, drafts: &[PolicyDraft]) -> anyhow::Result<()> {
    let migration_dir = repo.join(".agent-policy").join("migration");
    fs::create_dir_all(&migration_dir)?;

    for draft in drafts {
        let relative_target = Path::new(&draft.target_path);
        if relative_target.is_absolute()
            || relative_target.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::ParentDir | std::path::Component::RootDir
                )
            })
            || !relative_target.starts_with(".agent-policy/migration")
        {
            anyhow::bail!("refusing to write migration draft outside .agent-policy/migration");
        }

        let target = repo.join(relative_target);
        let parent = target
            .parent()
            .ok_or_else(|| anyhow::anyhow!("migration draft path has no parent"))?;
        if parent != migration_dir {
            anyhow::bail!("refusing to write nested migration draft path");
        }
        fs::write(target, &draft.policy_yaml)?;
    }

    Ok(())
}

fn inspect_repo(repo: &Path, discovered: DiscoveryResult) -> InspectionReport {
    let repo_name = repo
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(".")
        .to_string();
    let candidate_instructions = inspection_candidates(&discovered);
    let instruction_sources = discovered
        .instruction_sources
        .iter()
        .map(|source| inspection_source(source))
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

fn detect_inspection_duplicates(candidates: &[InspectionCandidate]) -> Vec<InspectionDuplicate> {
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

fn detect_inspection_conflicts(candidates: &[InspectionCandidate]) -> Vec<InspectionConflict> {
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
        let scope = normalize_scope_prefix(&candidate.provenance.scope).replace('/', ".");
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

fn migration_class_name(class: &MigrationClass) -> &'static str {
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

fn migration_dry_run_report(inspection: &InspectionReport) -> MigrationDryRunReport {
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
    out.push_str(&format!("id: {}\n", draft.id));
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

fn render_inspection_json(report: &InspectionReport) -> String {
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

fn render_migration_dry_run_json(report: &MigrationDryRunReport) -> String {
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

fn render_inspection_markdown(report: &InspectionReport) -> String {
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

fn render_migration_dry_run_markdown(report: &MigrationDryRunReport) -> String {
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
        out.push_str("\n\n```yaml\n");
        out.push_str(&draft.policy_yaml);
        out.push_str("```\n\n");
    }

    out
}

fn markdown_inline(text: &str) -> String {
    text.replace('`', "\\`")
        .replace('\n', " ")
        .replace('\r', " ")
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ValidationReport {
    status: ValidationStatus,
    summary: ValidationSummary,
    errors: Vec<ValidationIssue>,
    warnings: Vec<ValidationIssue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValidationStatus {
    Ok,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ValidationSummary {
    config_checked: bool,
    policy_files_checked: usize,
    error_count: usize,
    warning_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ValidationIssue {
    code: &'static str,
    message: String,
    path: Option<String>,
    field: Option<String>,
}

fn run_validate(global: &GlobalArgs) -> anyhow::Result<()> {
    let repo = global.repo.as_deref().unwrap_or_else(|| Path::new("."));
    let report = validate_repo(repo, global.config.as_deref());

    match global.format.clone().unwrap_or(OutputFormat::Markdown) {
        OutputFormat::Json => {
            println!("{}", render_validation_json(&report));
        }
        OutputFormat::Markdown => {
            print!("{}", render_validation_markdown(&report));
        }
    }

    if report.status == ValidationStatus::Failed {
        anyhow::bail!("validation failed")
    }

    Ok(())
}

fn validate_repo(repo: &Path, explicit_config: Option<&Path>) -> ValidationReport {
    let config_path = explicit_config
        .map(Path::to_path_buf)
        .unwrap_or_else(|| repo.join(agent_policy_config::REPO_CONFIG_FILE_NAME));
    let config_result = validate_config_file(&config_path);
    let (policy_files, policy_dir_issues) =
        collect_policy_files(repo, &config_result.local_policies);
    let policy_issues = validate_policy_files(&policy_files);

    let mut errors = config_result
        .errors
        .into_iter()
        .map(|issue| ValidationIssue {
            code: issue.code,
            message: issue.message,
            path: issue.path,
            field: issue.field,
        })
        .collect::<Vec<_>>();
    let mut warnings = Vec::new();

    for issue in policy_dir_issues.into_iter().chain(policy_issues) {
        let target = match issue.severity {
            PolicyValidationSeverity::Error => &mut errors,
            PolicyValidationSeverity::Warning => &mut warnings,
        };
        target.push(ValidationIssue {
            code: issue.code,
            message: issue.message,
            path: issue.path,
            field: issue.field,
        });
    }

    let status = if errors.is_empty() {
        ValidationStatus::Ok
    } else {
        ValidationStatus::Failed
    };
    let summary = ValidationSummary {
        config_checked: config_result.config_checked,
        policy_files_checked: policy_files.len(),
        error_count: errors.len(),
        warning_count: warnings.len(),
    };

    ValidationReport {
        status,
        summary,
        errors,
        warnings,
    }
}

fn render_validation_json(report: &ValidationReport) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"status\": \"");
    out.push_str(match report.status {
        ValidationStatus::Ok => "ok",
        ValidationStatus::Failed => "failed",
    });
    out.push_str("\",\n");
    out.push_str("  \"summary\": {\n");
    out.push_str(&format!(
        "    \"config_checked\": {},\n",
        report.summary.config_checked
    ));
    out.push_str(&format!(
        "    \"policy_files_checked\": {},\n",
        report.summary.policy_files_checked
    ));
    out.push_str(&format!(
        "    \"error_count\": {},\n",
        report.summary.error_count
    ));
    out.push_str(&format!(
        "    \"warning_count\": {}\n",
        report.summary.warning_count
    ));
    out.push_str("  }");

    if !report.errors.is_empty() {
        out.push_str(",\n  \"errors\": ");
        render_validation_issues_json(&mut out, &report.errors, 2);
    }
    if !report.warnings.is_empty() {
        out.push_str(",\n  \"warnings\": ");
        render_validation_issues_json(&mut out, &report.warnings, 2);
    }
    out.push_str("\n}");
    out
}

fn render_validation_issues_json(out: &mut String, issues: &[ValidationIssue], indent: usize) {
    let pad = " ".repeat(indent);
    let item_pad = " ".repeat(indent + 2);
    out.push_str("[\n");
    for (index, issue) in issues.iter().enumerate() {
        out.push_str(&item_pad);
        out.push_str("{\n");
        out.push_str(&format!(
            "{}  \"code\": \"{}\",\n",
            item_pad,
            json_escape(issue.code)
        ));
        out.push_str(&format!(
            "{}  \"message\": \"{}\"",
            item_pad,
            json_escape(&issue.message)
        ));
        if let Some(path) = &issue.path {
            out.push_str(&format!(
                ",\n{}  \"path\": \"{}\"",
                item_pad,
                json_escape(path)
            ));
        }
        if let Some(field) = &issue.field {
            out.push_str(&format!(
                ",\n{}  \"field\": \"{}\"",
                item_pad,
                json_escape(field)
            ));
        }
        out.push('\n');
        out.push_str(&item_pad);
        out.push('}');
        if index + 1 != issues.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&pad);
    out.push(']');
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character => escaped.push(character),
        }
    }
    escaped
}

fn render_validation_markdown(report: &ValidationReport) -> String {
    let mut out = String::new();
    out.push_str("# Agent Policy Validation\n\n");
    out.push_str("- Status: `");
    out.push_str(match report.status {
        ValidationStatus::Ok => "ok",
        ValidationStatus::Failed => "failed",
    });
    out.push_str("`\n");
    out.push_str(&format!(
        "- Checked {} policy file{}.\n",
        report.summary.policy_files_checked,
        if report.summary.policy_files_checked == 1 {
            ""
        } else {
            "s"
        }
    ));
    out.push_str(&format!(
        "- Errors: {}; warnings: {}.\n\n",
        report.summary.error_count, report.summary.warning_count
    ));

    render_validation_issue_section(&mut out, "Errors", &report.errors);
    render_validation_issue_section(&mut out, "Warnings", &report.warnings);

    out
}

fn render_validation_issue_section(out: &mut String, title: &str, issues: &[ValidationIssue]) {
    out.push_str("## ");
    out.push_str(title);
    out.push_str("\n\n");

    if issues.is_empty() {
        out.push_str("- None.\n\n");
        return;
    }

    for issue in issues {
        out.push_str("- `");
        out.push_str(issue.code);
        out.push_str("`: ");
        out.push_str(&issue.message);
        if let Some(path) = &issue.path {
            out.push_str(" (");
            out.push_str(path);
            if let Some(field) = &issue.field {
                out.push_str(", ");
                out.push_str(field);
            }
            out.push(')');
        }
        out.push('\n');
    }
    out.push('\n');
}

fn run_get(global: &GlobalArgs, args: GetArgs) -> anyhow::Result<()> {
    let repo = global
        .repo
        .as_deref()
        .unwrap_or_else(|| std::path::Path::new("."));
    let config = match &global.config {
        Some(path) => load_config_from_path(path)?,
        None => load_config(repo)?,
    };

    let intent = build_task_intent(repo, &config, &args);
    let mut policies = match &config.registry {
        Some(registry) => load_registry_policies(repo, registry)?,
        None => Vec::new(),
    };
    policies.extend(load_policies_from_dirs(repo, &config.local_policies)?);
    let discovered_sources = discover(repo)?;
    policies.extend(markdown_candidate_policies(
        repo,
        &discovered_sources,
        &intent.files,
    ));
    let bundle = build_instruction_bundle(
        &intent,
        &policies,
        BundleBuildOptions {
            max_tokens: args.max_tokens.or(Some(config.output_budget.max_tokens)),
            max_instructions: args
                .max_instructions
                .or(Some(config.output_budget.max_instructions)),
            max_required_checks: Some(config.output_budget.max_required_checks),
            max_blocked_actions: Some(config.output_budget.max_blocked_actions),
        },
    )?;

    match global.format.clone().unwrap_or(OutputFormat::Json) {
        OutputFormat::Json => {
            println!("{}", render_bundle_json(&bundle)?);
        }
        OutputFormat::Markdown => {
            print!("{}", render_bundle_markdown(&bundle));
        }
    }

    Ok(())
}

fn load_registry_policies(
    repo: &Path,
    registry: &RegistryConfig,
) -> anyhow::Result<Vec<LoadedPolicy>> {
    if registry.registry_type != "git" {
        anyhow::bail!(
            "unsupported registry type `{}`; only git is supported",
            registry.registry_type
        );
    }
    ensure_local_registry_url(&registry.url)?;

    let cache_dir = resolve_configured_path(repo, &registry.cache_dir)?;
    let source_name = cache_dir
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("registry")
        .to_string();

    load_policies_from_registry(
        &cache_dir,
        RegistryLoadOptions {
            source_name,
            ..RegistryLoadOptions::default()
        },
    )
}

fn ensure_local_registry_url(url: &str) -> anyhow::Result<()> {
    if url.starts_with("file://") {
        return Ok(());
    }
    let path = Path::new(url);
    if path.is_absolute() || url.starts_with('.') || !looks_like_remote_git_url(url) {
        return Ok(());
    }

    anyhow::bail!(
        "registry.url `{url}` is not a local filesystem path; network registry fetch is not implemented"
    )
}

fn looks_like_remote_git_url(url: &str) -> bool {
    url.contains("://") || url.starts_with("git@") || url.starts_with("ssh@")
}

fn resolve_configured_path(repo: &Path, raw: &str) -> anyhow::Result<PathBuf> {
    let expanded = expand_home(raw)?;
    let path = PathBuf::from(expanded);
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(repo.join(path))
    }
}

fn expand_home(raw: &str) -> anyhow::Result<String> {
    if raw == "~" {
        return std::env::var("HOME").map_err(|_| anyhow::anyhow!("HOME is not set"));
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        let home = std::env::var("HOME").map_err(|_| anyhow::anyhow!("HOME is not set"))?;
        return Ok(Path::new(&home).join(rest).display().to_string());
    }
    Ok(raw.to_string())
}

fn markdown_candidate_policies(
    repo: &Path,
    discovered: &DiscoveryResult,
    task_files: &[String],
) -> Vec<LoadedPolicy> {
    let mut policies = Vec::new();

    for source in &discovered.instruction_sources {
        if !scope_matches_task_files(&source.scope, task_files) {
            continue;
        }

        for candidate in &source.candidates {
            let (instructions, required_checks) = match candidate.candidate_type {
                MarkdownInstructionCandidateType::Instruction => {
                    (vec![candidate.text.clone()], Vec::new())
                }
                MarkdownInstructionCandidateType::RequiredCheck => {
                    (Vec::new(), vec![candidate.text.clone()])
                }
            };

            policies.push(LoadedPolicy {
                policy: Policy {
                    id: markdown_policy_id(&candidate.provenance.path, candidate.line),
                    version: PolicyVersion::Integer(1),
                    status: PolicyStatus::Active,
                    owner: None,
                    priority: None,
                    applies_when: AppliesWhen {
                        paths: scope_policy_paths(&candidate.provenance.scope),
                        ..AppliesWhen::default()
                    },
                    instructions,
                    required_checks,
                    blocked_actions: Vec::new(),
                    retrieval: None,
                    metadata: None,
                },
                source_path: repo.join(&candidate.provenance.path),
                source_ref: Some(SourceRef(markdown_source_ref(
                    &candidate.provenance.path,
                    candidate.line,
                    &candidate.provenance.scope,
                    &candidate.provenance.source_type,
                ))),
            });
        }
    }

    policies
}

fn scope_matches_task_files(scope: &str, task_files: &[String]) -> bool {
    if scope == "." {
        return true;
    }
    if task_files.is_empty() {
        return false;
    }

    let normalized_scope = normalize_scope_prefix(scope);
    task_files.iter().any(|file| {
        let normalized_file = normalize_task_file(file);
        normalized_file == normalized_scope
            || normalized_file.starts_with(&format!("{normalized_scope}/"))
    })
}

fn scope_policy_paths(scope: &str) -> Vec<String> {
    if scope == "." {
        Vec::new()
    } else {
        vec![scope.to_string()]
    }
}

fn markdown_source_ref(
    path: &str,
    line: usize,
    scope: &str,
    source_type: &InstructionSourceType,
) -> String {
    format!(
        "markdown:{}:{} scope={} type={}",
        path,
        line,
        scope,
        instruction_source_type_name(source_type)
    )
}

fn markdown_policy_id(path: &str, line: usize) -> String {
    let normalized = path
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '.'
            }
        })
        .collect::<String>();
    let slug = normalized
        .split('.')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(".");
    format!("markdown.{slug}.{line}")
}

fn instruction_source_type_name(source_type: &InstructionSourceType) -> &'static str {
    match source_type {
        InstructionSourceType::AgentsMd => "agents_md",
        InstructionSourceType::ClaudeMd => "claude_md",
        InstructionSourceType::CopilotInstructions => "copilot_instructions",
        InstructionSourceType::CursorRule => "cursor_rule",
    }
}

fn normalize_scope_prefix(scope: &str) -> String {
    scope
        .trim_end_matches("/**")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string()
}

fn normalize_task_file(file: &str) -> String {
    file.replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string()
}

fn build_task_intent(
    repo: &std::path::Path,
    config: &agent_policy_config::AgentPolicyConfig,
    args: &GetArgs,
) -> TaskIntent {
    TaskIntent {
        repo: repo
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string),
        branch: None,
        task: Some(TaskDetails {
            summary: args.task.clone(),
            task_type: args.task_type.clone().map(TaskType),
        }),
        files: args.files.clone(),
        detected: Some(detect_context(&args.files)),
        risk_flags: args.risk.clone(),
        expected_commands: Vec::new(),
        expected_check_ids: Vec::new(),
        output_budget: Some(OutputBudget {
            max_tokens: args.max_tokens.or(Some(config.output_budget.max_tokens)),
            max_instructions: args
                .max_instructions
                .or(Some(config.output_budget.max_instructions)),
            max_required_checks: Some(config.output_budget.max_required_checks),
            max_blocked_actions: Some(config.output_budget.max_blocked_actions),
            include_examples: Some(config.output_budget.include_examples),
            include_explanations: Some(config.output_budget.include_explanations.clone()),
        }),
    }
}

fn detect_context(files: &[String]) -> DetectedContext {
    let mut languages = Vec::new();

    for file in files {
        if matches_extension(file, &["ts", "tsx"]) {
            push_unique(&mut languages, "typescript".to_string());
        } else if matches_extension(file, &["js", "jsx", "mjs", "cjs"]) {
            push_unique(&mut languages, "javascript".to_string());
        } else if matches_extension(file, &["rs"]) {
            push_unique(&mut languages, "rust".to_string());
        } else if matches_extension(file, &["py"]) {
            push_unique(&mut languages, "python".to_string());
        }
    }

    DetectedContext {
        languages,
        frameworks: Vec::new(),
        package_manager: None,
    }
}

fn matches_extension(file: &str, extensions: &[&str]) -> bool {
    std::path::Path::new(file)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extensions.iter().any(|candidate| candidate == &extension))
}

fn push_unique(items: &mut Vec<String>, item: String) {
    if !items.contains(&item) {
        items.push(item);
    }
}

fn run_discover(global: &GlobalArgs) -> anyhow::Result<()> {
    let repo = global
        .repo
        .as_deref()
        .unwrap_or_else(|| std::path::Path::new("."));
    match global.format.clone().unwrap_or(OutputFormat::Json) {
        OutputFormat::Json => {
            let json = discover_json(repo)?;
            println!("{}", json);
            Ok(())
        }
        OutputFormat::Markdown => {
            anyhow::bail!("markdown output is not implemented for `discover`; use `--format json`")
        }
    }
}

fn not_implemented(command_name: &str) -> anyhow::Result<()> {
    anyhow::bail!(
        "command `{command_name}` is not implemented yet. Use `agent-policy {command_name} --help` for usage details."
    )
}

#[cfg(test)]
mod tests {
    use super::{
        detect_inspection_conflicts, detect_inspection_duplicates, inspect_repo,
        load_registry_policies, markdown_candidate_policies, migration_dry_run_report,
        render_inspection_json, render_inspection_markdown, render_migration_dry_run_json,
        render_migration_dry_run_markdown, render_validation_markdown, run,
        scope_matches_task_files, validate_repo, Cli, Commands, GlobalArgs, InspectionCandidate,
        MigrationClass, OutputFormat, RegistryCommands, ValidationStatus,
    };
    use agent_policy_config::load_config;
    use agent_policy_core::{
        build_instruction_bundle, BundleBuildOptions, DetectedContext, OutputBudget, TaskDetails,
        TaskIntent,
    };
    use agent_policy_discover::discover;
    use clap::{error::ErrorKind, CommandFactory, Parser};
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn clap_command_builds() {
        Cli::command().debug_assert();
    }

    #[test]
    fn global_args_support_format_values() {
        let _ = [
            OutputFormat::Json,
            OutputFormat::Markdown,
            // keep a concrete use of global args in tests so future refactors
            // don't accidentally remove CLI-level flags.
        ];
        let _ = std::mem::size_of::<GlobalArgs>();
    }

    #[test]
    fn parse_get_with_repo() {
        let cli = Cli::try_parse_from(["agent-policy", "get", "--repo", "."]).expect("parse get");
        assert_eq!(cli.global.repo, Some(PathBuf::from(".")));
        assert!(matches!(cli.command, Commands::Get(_)));
    }

    #[test]
    fn parse_get_inputs() {
        let cli = Cli::try_parse_from([
            "agent-policy",
            "get",
            "--repo",
            "fixtures/simple-repo",
            "--task",
            "fix refund retry handling",
            "--type",
            "fix_bug",
            "--files",
            "src/payments/refunds.ts",
            "--risk",
            "payments",
            "--max-instructions",
            "4",
            "--max-tokens",
            "600",
            "--format",
            "json",
        ])
        .expect("parse get inputs");

        match cli.command {
            Commands::Get(args) => {
                assert_eq!(args.task.as_deref(), Some("fix refund retry handling"));
                assert_eq!(args.task_type.as_deref(), Some("fix_bug"));
                assert_eq!(args.files, vec!["src/payments/refunds.ts"]);
                assert_eq!(args.risk, vec!["payments"]);
                assert_eq!(args.max_instructions, Some(4));
                assert_eq!(args.max_tokens, Some(600));
            }
            _ => panic!("expected get command"),
        }
    }

    #[test]
    fn parse_discover_with_json_format() {
        let cli = Cli::try_parse_from(["agent-policy", "discover", "--format", "json"])
            .expect("parse discover");
        assert!(matches!(cli.global.format, Some(OutputFormat::Json)));
        assert!(matches!(cli.command, Commands::Discover));
    }

    #[test]
    fn parse_validate_with_markdown_format() {
        let cli = Cli::try_parse_from(["agent-policy", "validate", "--format", "markdown"])
            .expect("parse validate");
        assert!(matches!(cli.global.format, Some(OutputFormat::Markdown)));
        assert!(matches!(cli.command, Commands::Validate));
    }

    #[test]
    fn parse_inspect_with_json_format() {
        let cli = Cli::try_parse_from(["agent-policy", "inspect", "--format", "json"])
            .expect("parse inspect");
        assert!(matches!(cli.global.format, Some(OutputFormat::Json)));
        assert!(matches!(cli.command, Commands::Inspect));
    }

    #[test]
    fn parse_migrate_dry_run_with_markdown_format() {
        let cli = Cli::try_parse_from([
            "agent-policy",
            "migrate",
            "--dry-run",
            "--format",
            "markdown",
        ])
        .expect("parse migrate");
        assert!(matches!(cli.global.format, Some(OutputFormat::Markdown)));
        match cli.command {
            Commands::Migrate(args) => {
                assert!(args.dry_run);
                assert!(!args.write);
            }
            _ => panic!("expected migrate command"),
        }
    }

    #[test]
    fn parse_registry_sync_with_no_network() {
        let cli = Cli::try_parse_from(["agent-policy", "registry", "sync", "--no-network"])
            .expect("parse registry sync");
        assert!(cli.global.no_network);
        match cli.command {
            Commands::Registry(registry) => {
                assert!(matches!(registry.command, RegistryCommands::Sync));
            }
            _ => panic!("expected registry command"),
        }
    }

    #[test]
    fn invalid_format_value_fails() {
        let err = Cli::try_parse_from(["agent-policy", "discover", "--format", "xml"])
            .expect_err("expected invalid format to fail");
        assert_eq!(err.kind(), ErrorKind::InvalidValue);
    }

    #[test]
    fn unknown_subcommand_fails() {
        let err = Cli::try_parse_from(["agent-policy", "unknown"])
            .expect_err("expected unknown subcommand to fail");
        assert_eq!(err.kind(), ErrorKind::InvalidSubcommand);
    }

    #[test]
    fn validate_valid_fixture_repo_passes() {
        let repo = fixture_repo("payments-repo");

        let report = validate_repo(&repo, None);

        assert_eq!(report.status, ValidationStatus::Ok);
        assert!(report.errors.is_empty());
        assert_eq!(report.summary.policy_files_checked, 2);
    }

    #[test]
    fn validate_invalid_fixture_repo_reports_useful_errors() {
        let repo = fixture_repo("invalid-policy-repo");

        let report = validate_repo(&repo, None);
        let error_codes = report
            .errors
            .iter()
            .map(|issue| issue.code)
            .collect::<Vec<_>>();
        let warning_codes = report
            .warnings
            .iter()
            .map(|issue| issue.code)
            .collect::<Vec<_>>();

        assert_eq!(report.status, ValidationStatus::Failed);
        assert!(error_codes.contains(&"config_invalid_sync_mode"));
        assert!(error_codes.contains(&"config_registry_missing_field"));
        assert!(error_codes.contains(&"config_invalid_output_budget"));
        assert!(error_codes.contains(&"policy_missing_id"));
        assert!(error_codes.contains(&"policy_missing_version"));
        assert!(error_codes.contains(&"policy_active_empty_instructions"));
        assert!(error_codes.contains(&"policy_duplicate_id"));
        assert!(error_codes.contains(&"policy_invalid_status"));
        assert!(warning_codes.contains(&"policy_broad_active"));
        assert!(warning_codes.contains(&"policy_vague_instruction"));

        let markdown = render_validation_markdown(&report);
        assert!(markdown.contains("config_invalid_sync_mode"));
        assert!(markdown.contains("policy_duplicate_id"));
    }

    #[test]
    fn validate_monorepo_uses_configured_policy_directories() {
        let repo = fixture_repo("monorepo");

        let report = validate_repo(&repo, None);

        assert_eq!(report.status, ValidationStatus::Ok);
        assert_eq!(report.summary.policy_files_checked, 2);
    }

    #[test]
    fn loads_registry_policies_from_configured_cache_dir() {
        let repo = fixture_repo("registry-app");
        let config = load_config(&repo).expect("registry config should load");
        let registry = config.registry.expect("registry should be configured");

        let policies =
            load_registry_policies(&repo, &registry).expect("local registry cache should load");

        assert_eq!(policies.len(), 1);
        assert_eq!(policies[0].policy.id, "org.security.secrets");
        assert_eq!(
            policies[0]
                .source_ref
                .as_ref()
                .map(|source| source.0.as_str()),
            Some("local-registry:org.security.secrets@3#0123456789ab")
        );
    }

    #[test]
    fn inspect_nested_fixture_reports_sources_candidates_and_migration_groups() {
        let repo = fixture_repo("nested-instructions");
        let discovered = discover(&repo).expect("discover fixture repo");
        let report = inspect_repo(&repo, discovered);

        assert_eq!(report.repo, "nested-instructions");
        assert!(report.summary.source_count >= 5);
        assert!(report.summary.candidate_instruction_count >= 9);
        assert!(report
            .instruction_sources
            .iter()
            .any(|source| source.path == "backend/payments/AGENTS.md"
                && source.scope == "backend/payments/**"));
        assert!(report
            .candidate_instructions
            .iter()
            .any(|candidate| candidate.text == "Never log payment secrets."
                && candidate.topic == "secrets"));
        assert!(report
            .migration_candidates
            .iter()
            .any(|candidate| candidate.source == "backend/payments/AGENTS.md"));

        let json = render_inspection_json(&report);
        assert!(json.contains("\"instruction_sources\""));
        assert!(json.contains("\"duplicates\""));
        assert!(json.contains("\"conflicts\""));

        let markdown = render_inspection_markdown(&report);
        assert!(markdown.contains("# Agent Policy Inspection"));
        assert!(markdown.contains("## Duplicates"));
        assert!(markdown.contains("## Conflicts"));
        assert!(markdown.contains("## Migration Candidates"));
    }

    #[test]
    fn migrate_dry_run_proposes_draft_policy_yaml_for_fixture_repo() {
        let repo = fixture_repo("nested-instructions");
        let discovered = discover(&repo).expect("discover fixture repo");
        let inspection = inspect_repo(&repo, discovered);
        let report = migration_dry_run_report(&inspection);

        assert_eq!(report.mode, "dry_run");
        assert!(report
            .drafts
            .iter()
            .any(|draft| draft.id == "local.backend.payments.payments"
                && draft.target_path
                    == ".agent-policy/migration/local.backend.payments.payments.yaml"
                && draft.policy_yaml.contains("status: draft")
                && draft.policy_yaml.contains("generated_from:")
                && draft.policy_yaml.contains("Preserve payment invariants.")));
        let payment_draft = report
            .drafts
            .iter()
            .find(|draft| draft.id == "local.backend.payments.payments")
            .expect("payment draft");
        assert_eq!(
            payment_draft.policy_yaml,
            PAYMENT_POLICY_DRY_RUN_YAML_SNAPSHOT
        );
        let checks_draft = report
            .drafts
            .iter()
            .find(|draft| draft.id == "repo.checks")
            .expect("checks draft");
        assert_eq!(
            checks_draft.policy_yaml,
            CHECKS_POLICY_DRY_RUN_YAML_SNAPSHOT
        );

        let json = render_migration_dry_run_json(&report);
        assert!(json.contains("\"mode\": \"dry_run\""));
        assert!(json.contains("\"policy_yaml\""));
        assert!(json.contains("generated_from"));

        let markdown = render_migration_dry_run_markdown(&report);
        assert!(markdown.contains("# Agent Policy Migration Dry Run"));
        assert!(markdown.contains("```yaml"));
        assert!(markdown.contains(".agent-policy/migration/local.backend.payments.payments.yaml"));
    }

    #[test]
    fn migrate_dry_run_does_not_modify_instruction_files() {
        let repo = fixture_repo("nested-instructions");
        let agents_path = repo.join("AGENTS.md");
        let before = fs::read_to_string(&agents_path).expect("read fixture before");

        let cli = Cli::try_parse_from([
            "agent-policy",
            "--repo",
            repo.to_str().expect("utf8 repo"),
            "migrate",
            "--dry-run",
            "--format",
            "json",
        ])
        .expect("parse migrate");
        run(cli).expect("run dry-run migration");

        let after = fs::read_to_string(&agents_path).expect("read fixture after");
        assert_eq!(after, before);
    }

    #[test]
    fn migrate_write_creates_draft_policy_files_without_touching_instruction_files() {
        let temp = TempRepo::copy_fixture("nested-instructions");
        let repo = temp.path();
        fs::write(
            repo.join("CLAUDE.md"),
            "# Claude Instructions\n\n- Keep Claude-specific guidance intact.\n",
        )
        .expect("write temp claude file");
        let repo_files_before = repo_file_contents(repo);
        let instruction_files_before = instruction_file_contents(repo);

        let cli = Cli::try_parse_from([
            "agent-policy",
            "--repo",
            repo.to_str().expect("utf8 repo"),
            "migrate",
            "--write",
            "--format",
            "json",
        ])
        .expect("parse migrate write");
        run(cli).expect("run write migration");

        assert_eq!(instruction_file_contents(repo), instruction_files_before);
        assert_only_migration_files_were_added(&repo_files_before, &repo_file_contents(repo));

        let migration_dir = repo.join(".agent-policy").join("migration");
        assert!(migration_dir.is_dir());

        let payment_policy_path = migration_dir.join("local.backend.payments.payments.yaml");
        let payment_policy =
            fs::read_to_string(&payment_policy_path).expect("read written payment policy");
        assert_eq!(payment_policy, PAYMENT_POLICY_DRY_RUN_YAML_SNAPSHOT);
        assert!(payment_policy.contains("status: draft"));
        assert!(payment_policy.contains("generated_from:"));
        assert!(payment_policy.contains("path: \"backend/payments/AGENTS.md\""));

        let first_written = written_migration_file_contents(&migration_dir);
        let cli = Cli::try_parse_from([
            "agent-policy",
            "--repo",
            repo.to_str().expect("utf8 repo"),
            "migrate",
            "--write",
            "--format",
            "json",
        ])
        .expect("parse second migrate write");
        run(cli).expect("run second write migration");
        assert_eq!(
            written_migration_file_contents(&migration_dir),
            first_written
        );
        assert_eq!(instruction_file_contents(repo), instruction_files_before);
        assert_only_migration_files_were_added(&repo_files_before, &repo_file_contents(repo));
    }

    #[test]
    fn inspect_reports_exact_duplicates_and_basic_conflicts() {
        let candidates = vec![
            test_inspection_candidate(
                "Use pnpm for package commands.",
                "AGENTS.md",
                2,
                ".",
                "package_manager",
                MigrationClass::RepoPolicy,
            ),
            test_inspection_candidate(
                "Use npm for package commands.",
                "frontend/AGENTS.md",
                3,
                "frontend/**",
                "package_manager",
                MigrationClass::KeepLocal,
            ),
            test_inspection_candidate(
                "Do not edit generated files directly.",
                "AGENTS.md",
                4,
                ".",
                "generated_files",
                MigrationClass::SharedRegistryPolicy,
            ),
            test_inspection_candidate(
                "Do not edit generated files directly.",
                "backend/AGENTS.md",
                4,
                "backend/**",
                "generated_files",
                MigrationClass::SharedRegistryPolicy,
            ),
            test_inspection_candidate(
                "Edit generated files directly for emergency fixes.",
                "backend/AGENTS.md",
                5,
                "backend/**",
                "generated_files",
                MigrationClass::KeepLocal,
            ),
        ];

        let duplicates = detect_inspection_duplicates(&candidates);
        assert_eq!(duplicates.len(), 1);
        assert_eq!(
            duplicates[0].instruction,
            "Do not edit generated files directly."
        );
        assert_eq!(
            duplicates[0].sources,
            vec!["AGENTS.md:4", "backend/AGENTS.md:4"]
        );

        let conflicts = detect_inspection_conflicts(&candidates);
        assert!(conflicts
            .iter()
            .any(|conflict| conflict.topic == "package_manager"));
        assert!(conflicts
            .iter()
            .any(|conflict| conflict.topic == "generated_files"));
    }

    #[test]
    fn markdown_candidates_are_added_to_get_bundle_with_provenance() {
        let repo = fixture_repo("nested-instructions");
        let discovered = discover(&repo).expect("discover fixture repo");
        let files = vec!["backend/payments/src/refunds.ts".to_string()];
        let policies = markdown_candidate_policies(&repo, &discovered, &files);
        let bundle = build_instruction_bundle(
            &TaskIntent {
                repo: Some("nested-instructions".into()),
                branch: None,
                task: Some(TaskDetails {
                    summary: Some("update refunds".into()),
                    task_type: None,
                }),
                files,
                detected: Some(DetectedContext::default()),
                risk_flags: Vec::new(),
                expected_commands: Vec::new(),
                expected_check_ids: Vec::new(),
                output_budget: Some(OutputBudget::default()),
            },
            &policies,
            BundleBuildOptions {
                max_tokens: Some(900),
                max_instructions: Some(20),
                max_required_checks: Some(20),
                max_blocked_actions: Some(20),
            },
        )
        .expect("build bundle");

        let instruction_texts = bundle
            .instructions
            .iter()
            .map(|instruction| instruction.text.as_str())
            .collect::<Vec<_>>();
        assert!(instruction_texts.contains(&"Use the repository policy broker configuration."));
        assert!(instruction_texts.contains(&"Backend changes require service-level tests."));
        assert!(instruction_texts.contains(&"Preserve payment invariants."));
        assert!(instruction_texts.contains(&"Never log payment secrets."));
        assert!(!instruction_texts
            .iter()
            .any(|text| text.contains("several examples")));

        assert!(bundle.required_checks.iter().any(|check| {
            check.id == "cargo test -p payments"
                && check.source.as_ref().is_some_and(|source| {
                    source.0.contains("markdown:backend/payments/AGENTS.md:8")
                        && source.0.contains("scope=backend/payments/**")
                        && source.0.contains("type=agents_md")
                })
        }));
    }

    #[test]
    fn nested_markdown_candidates_require_matching_task_files() {
        let repo = fixture_repo("nested-instructions");
        let discovered = discover(&repo).expect("discover fixture repo");
        let files = vec!["frontend/src/App.tsx".to_string()];
        let policies = markdown_candidate_policies(&repo, &discovered, &files);
        let bundle = build_instruction_bundle(
            &TaskIntent {
                repo: Some("nested-instructions".into()),
                branch: None,
                task: None,
                files,
                detected: Some(DetectedContext::default()),
                risk_flags: Vec::new(),
                expected_commands: Vec::new(),
                expected_check_ids: Vec::new(),
                output_budget: None,
            },
            &policies,
            BundleBuildOptions {
                max_tokens: Some(900),
                max_instructions: Some(20),
                max_required_checks: Some(20),
                max_blocked_actions: Some(20),
            },
        )
        .expect("build bundle");

        let instruction_texts = bundle
            .instructions
            .iter()
            .map(|instruction| instruction.text.as_str())
            .collect::<Vec<_>>();
        assert!(instruction_texts.contains(&"Prefer accessible controls."));
        assert!(!instruction_texts.contains(&"Backend changes require service-level tests."));
        assert!(!instruction_texts.contains(&"Preserve payment invariants."));
    }

    #[test]
    fn nested_scope_matching_is_file_based() {
        assert!(scope_matches_task_files(
            "backend/**",
            &["backend/payments/src/refunds.ts".into()]
        ));
        assert!(!scope_matches_task_files(
            "backend/**",
            &["frontend/src/App.tsx".into()]
        ));
        assert!(!scope_matches_task_files("backend/**", &[]));
        assert!(scope_matches_task_files(".", &[]));
    }

    fn test_inspection_candidate(
        text: &str,
        source: &str,
        line: usize,
        scope: &str,
        topic: &str,
        migration_class: MigrationClass,
    ) -> InspectionCandidate {
        InspectionCandidate {
            text: text.into(),
            source: source.into(),
            line,
            scope: scope.into(),
            candidate_type: "instruction".into(),
            topic: topic.into(),
            migration_class,
            target_policy: Some("test.policy".into()),
        }
    }

    fn fixture_repo(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
    }

    struct TempRepo {
        path: PathBuf,
    }

    impl TempRepo {
        fn copy_fixture(name: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "agent-policy-cli-{name}-{}-{nonce}",
                std::process::id()
            ));
            copy_dir_all(&fixture_repo(name), &path).expect("copy fixture to temp repo");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn copy_dir_all(source: &Path, destination: &Path) -> std::io::Result<()> {
        fs::create_dir_all(destination)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let target = destination.join(entry.file_name());
            if file_type.is_dir() {
                copy_dir_all(&entry.path(), &target)?;
            } else if file_type.is_file() {
                fs::copy(entry.path(), target)?;
            }
        }
        Ok(())
    }

    fn instruction_file_contents(repo: &Path) -> BTreeMap<String, String> {
        let mut contents = BTreeMap::new();
        collect_instruction_file_contents(repo, repo, &mut contents);
        contents
    }

    fn repo_file_contents(repo: &Path) -> BTreeMap<String, String> {
        let mut contents = BTreeMap::new();
        collect_repo_file_contents(repo, repo, &mut contents);
        contents
    }

    fn collect_repo_file_contents(
        repo: &Path,
        directory: &Path,
        contents: &mut BTreeMap<String, String>,
    ) {
        for entry in fs::read_dir(directory).expect("read directory") {
            let entry = entry.expect("read directory entry");
            let path = entry.path();
            let file_type = entry.file_type().expect("entry file type");
            if file_type.is_dir() {
                collect_repo_file_contents(repo, &path, contents);
            } else if file_type.is_file() {
                let relative = path
                    .strip_prefix(repo)
                    .expect("repo-relative file path")
                    .to_string_lossy()
                    .replace('\\', "/");
                contents.insert(relative, fs::read_to_string(&path).expect("read repo file"));
            }
        }
    }

    fn assert_only_migration_files_were_added(
        before: &BTreeMap<String, String>,
        after: &BTreeMap<String, String>,
    ) {
        for (path, contents) in before {
            assert_eq!(
                after.get(path),
                Some(contents),
                "pre-existing file changed: {path}"
            );
        }
        for path in after.keys() {
            assert!(
                before.contains_key(path) || path.starts_with(".agent-policy/migration/"),
                "unexpected generated path: {path}"
            );
        }
    }

    fn collect_instruction_file_contents(
        repo: &Path,
        directory: &Path,
        contents: &mut BTreeMap<String, String>,
    ) {
        for entry in fs::read_dir(directory).expect("read directory") {
            let entry = entry.expect("read directory entry");
            let path = entry.path();
            let file_type = entry.file_type().expect("entry file type");
            if file_type.is_dir() {
                collect_instruction_file_contents(repo, &path, contents);
            } else if file_type.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| matches!(name, "AGENTS.md" | "CLAUDE.md"))
            {
                let relative = path
                    .strip_prefix(repo)
                    .expect("repo-relative instruction path")
                    .to_string_lossy()
                    .replace('\\', "/");
                contents.insert(
                    relative,
                    fs::read_to_string(&path).expect("read instruction file"),
                );
            }
        }
    }

    fn written_migration_file_contents(migration_dir: &Path) -> BTreeMap<String, String> {
        let mut contents = BTreeMap::new();
        for entry in fs::read_dir(migration_dir).expect("read migration dir") {
            let entry = entry.expect("read migration entry");
            let path = entry.path();
            if entry.file_type().expect("migration file type").is_file() {
                contents.insert(
                    entry.file_name().to_string_lossy().into_owned(),
                    fs::read_to_string(path).expect("read migration file"),
                );
            }
        }
        contents
    }

    const PAYMENT_POLICY_DRY_RUN_YAML_SNAPSHOT: &str = r#"id: local.backend.payments.payments
version: 1
status: draft

applies_when:
  paths:
    - "backend/payments/**"

instructions:
  - "Preserve payment invariants."

metadata:
  generated_from:
    - path: "backend/payments/AGENTS.md"
      source_type: agents_md
      scope: "backend/payments/**"
      lines:
        - 3
  migration_status: proposed
  migration_class: keep_local
"#;

    const CHECKS_POLICY_DRY_RUN_YAML_SNAPSHOT: &str = r#"id: repo.checks
version: 1
status: draft

applies_when: {}

instructions: []

required_checks:
  - "cargo test"

metadata:
  generated_from:
    - path: "AGENTS.md"
      source_type: agents_md
      scope: "."
      lines:
        - 13
  migration_status: proposed
  migration_class: repo_policy
"#;
}
