use std::collections::BTreeSet;
use std::path::Path;

use agent_policy_config::{load_config, load_config_from_path, RegistryConfig};
use agent_policy_core::{
    build_instruction_bundle_with_bm25_candidates, load_policies_from_dirs,
    load_policies_from_registry, render_bundle_json, render_bundle_markdown, AppliesWhen,
    BundleBuildOptions, DetectedContext, InstructionBundle, LoadedPolicy, OutputBudget, Policy,
    PolicyStatus, PolicyVersion, RegistryLoadOptions, SourceRef, TaskDetails, TaskIntent, TaskType,
};
use agent_policy_discover::{
    discover, DiscoveryResult, InstructionSourceType, MarkdownInstructionCandidateType,
};

use crate::cli::{GetArgs, GlobalArgs, OutputFormat};
use crate::indexing::{
    agent_policy_cache_dir, get_indexed_policy_ids, index_registry_source, index_repo_source,
    search_fulltext_candidates,
};
use crate::paths::resolve_configured_path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GetPolicyLoad {
    pub(crate) policies: Vec<LoadedPolicy>,
    pub(crate) warnings: Vec<String>,
}

pub(crate) fn run(global: &GlobalArgs, args: GetArgs) -> anyhow::Result<()> {
    let bundle = build_instruction_bundle_for_get(global, &args)?;

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

pub(crate) fn build_instruction_bundle_for_get(
    global: &GlobalArgs,
    args: &GetArgs,
) -> anyhow::Result<InstructionBundle> {
    let repo = global
        .repo
        .as_deref()
        .unwrap_or_else(|| std::path::Path::new("."));
    let config = match &global.config {
        Some(path) => load_config_from_path(path)?,
        None => load_config(repo)?,
    };

    let intent = build_task_intent(repo, &config, &args);
    let loaded = load_get_policies(repo, &config)?;
    let mut policies = loaded.policies;
    let mut warnings = loaded.warnings;
    let discovered_sources = discover(repo)?;
    policies.extend(markdown_candidate_policies(
        repo,
        &discovered_sources,
        &intent.files,
    ));
    let bm25_candidate_ids = bm25_candidate_policy_ids(repo, &config, &intent, &mut warnings)?;
    let mut bundle = build_instruction_bundle_with_bm25_candidates(
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
        &bm25_candidate_ids,
    )?;
    bundle.warnings.extend(warnings);
    Ok(bundle)
}

fn bm25_candidate_policy_ids(
    repo: &Path,
    config: &agent_policy_config::AgentPolicyConfig,
    intent: &TaskIntent,
    warnings: &mut Vec<String>,
) -> anyhow::Result<BTreeSet<String>> {
    bm25_candidate_policy_ids_with_cache_dir(
        repo,
        config,
        intent,
        &agent_policy_cache_dir()?,
        warnings,
    )
}

pub(crate) fn bm25_candidate_policy_ids_with_cache_dir(
    repo: &Path,
    config: &agent_policy_config::AgentPolicyConfig,
    intent: &TaskIntent,
    cache_dir: &Path,
    warnings: &mut Vec<String>,
) -> anyhow::Result<BTreeSet<String>> {
    let query = bm25_query(intent);
    if query.is_empty() {
        return Ok(BTreeSet::new());
    }

    let source = match &config.registry {
        Some(registry) => index_registry_source(repo, registry)?,
        None => index_repo_source(repo)?,
    };
    let candidates = search_fulltext_candidates(
        cache_dir,
        &source,
        &query,
        bm25_candidate_limit(config),
        warnings,
    )?;

    Ok(candidates
        .into_iter()
        .map(|candidate| candidate.id)
        .collect())
}

fn bm25_query(intent: &TaskIntent) -> String {
    let mut parts = Vec::new();
    if let Some(task) = &intent.task {
        if let Some(summary) = &task.summary {
            parts.push(summary.as_str());
        }
        if let Some(task_type) = &task.task_type {
            parts.push(task_type.0.as_str());
        }
    }
    parts.extend(intent.risk_flags.iter().map(String::as_str));
    parts.extend(intent.files.iter().map(String::as_str));
    parts.join(" ")
}

fn bm25_candidate_limit(config: &agent_policy_config::AgentPolicyConfig) -> usize {
    let max_instructions = config.output_budget.max_instructions as usize;
    max_instructions.saturating_mul(3).max(8)
}

fn load_get_policies(
    repo: &Path,
    config: &agent_policy_config::AgentPolicyConfig,
) -> anyhow::Result<GetPolicyLoad> {
    load_get_policies_with_cache_dir(repo, config, &agent_policy_cache_dir()?)
}

pub(crate) fn load_get_policies_with_cache_dir(
    repo: &Path,
    config: &agent_policy_config::AgentPolicyConfig,
    cache_dir: &Path,
) -> anyhow::Result<GetPolicyLoad> {
    let mut warnings = Vec::new();
    let mut policies = Vec::new();

    if let Some(registry) = &config.registry {
        let source = index_registry_source(repo, registry)?;
        let indexed_ids = get_indexed_policy_ids(cache_dir, &source, &mut warnings)?;
        let registry_policies = load_registry_policies(repo, registry)?;
        policies.extend(filter_loaded_policies(
            registry_policies,
            indexed_ids.as_ref(),
        ));
        policies.extend(load_policies_from_dirs(repo, &config.local_policies)?);
    } else {
        let source = index_repo_source(repo)?;
        let indexed_ids = get_indexed_policy_ids(cache_dir, &source, &mut warnings)?;
        let local_policies = load_policies_from_dirs(repo, &config.local_policies)?;
        policies.extend(filter_loaded_policies(local_policies, indexed_ids.as_ref()));
    }

    Ok(GetPolicyLoad { policies, warnings })
}

fn filter_loaded_policies(
    policies: Vec<LoadedPolicy>,
    indexed_ids: Option<&BTreeSet<String>>,
) -> Vec<LoadedPolicy> {
    match indexed_ids {
        Some(ids) => policies
            .into_iter()
            .filter(|loaded| ids.contains(&loaded.policy.id))
            .collect(),
        None => policies,
    }
}

pub(crate) fn load_registry_policies(
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

pub(crate) fn looks_like_remote_git_url(url: &str) -> bool {
    url.contains("://") || url.starts_with("git@") || url.starts_with("ssh@")
}

pub(crate) fn markdown_candidate_policies(
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

pub(crate) fn scope_matches_task_files(scope: &str, task_files: &[String]) -> bool {
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

pub(crate) fn normalize_scope_prefix(scope: &str) -> String {
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
