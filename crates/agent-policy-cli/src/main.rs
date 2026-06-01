use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use agent_policy_config::{load_config, load_config_from_path, validate_config_file};
use agent_policy_core::{
    build_instruction_bundle, collect_policy_files, load_policies_from_dirs, render_bundle_json,
    render_bundle_markdown, validate_policy_files, AppliesWhen, BundleBuildOptions,
    DetectedContext, LoadedPolicy, OutputBudget, Policy, PolicyStatus, PolicyValidationSeverity,
    PolicyVersion, SourceRef, TaskDetails, TaskIntent, TaskType,
};
use agent_policy_discover::{
    discover, discover_json, DiscoveryResult, InstructionSourceType,
    MarkdownInstructionCandidateType,
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
    Migrate,
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
        Commands::Inspect => not_implemented("inspect"),
        Commands::Migrate => not_implemented("migrate"),
        Commands::Index => not_implemented("index"),
        Commands::Serve => not_implemented("serve"),
        Commands::Registry(registry) => match registry.command {
            RegistryCommands::Sync => not_implemented("registry sync"),
        },
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
    let mut policies = load_policies_from_dirs(repo, &config.local_policies)?;
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
        markdown_candidate_policies, render_validation_markdown, scope_matches_task_files,
        validate_repo, Cli, Commands, GlobalArgs, OutputFormat, RegistryCommands, ValidationStatus,
    };
    use agent_policy_core::{
        build_instruction_bundle, BundleBuildOptions, DetectedContext, OutputBudget, TaskDetails,
        TaskIntent,
    };
    use agent_policy_discover::discover;
    use clap::{error::ErrorKind, CommandFactory, Parser};
    use std::path::PathBuf;

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

    fn fixture_repo(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
    }
}
