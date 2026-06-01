use clap::{Args, Parser, Subcommand, ValueEnum};
use globset::Glob;
use std::collections::BTreeMap;
use std::fs;
use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
mod cli;
mod commands;
mod git;
mod indexing;
mod paths;
mod render;

use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();
    match cli::run(cli) {
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
        Commands::Index => run_index(&cli.global),
        Commands::Serve => not_implemented("serve"),
        Commands::Registry(registry) => match registry.command {
            RegistryCommands::Sync => run_registry_sync(&cli.global),
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RegistrySyncReport {
    cache_dir: PathBuf,
    mode: SyncMode,
    status: RegistrySyncStatus,
    commit: Option<String>,
    requested_ref: String,
    message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegistrySyncStatus {
    LocalPath,
    Cached,
    Offline,
    Pinned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IndexBuildReport {
    source: IndexSource,
    index_dir: PathBuf,
    metadata_path: PathBuf,
    manifest_path: PathBuf,
    policy_count: usize,
    stale_before_build: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IndexSource {
    kind: IndexSourceKind,
    name: String,
    root: PathBuf,
    url: Option<String>,
    requested_ref: Option<String>,
    commit: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum IndexSourceKind {
    Registry,
    Repo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct IndexManifest {
    schema_version: u32,
    source: IndexManifestSource,
    indexes: IndexManifestIndexes,
    created_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct IndexManifestSource {
    kind: IndexSourceKind,
    name: String,
    path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
    requested_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    commit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct IndexManifestIndexes {
    metadata: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GetPolicyLoad {
    policies: Vec<LoadedPolicy>,
    warnings: Vec<String>,
}

fn run_registry_sync(global: &GlobalArgs) -> anyhow::Result<()> {
    let repo = global.repo.as_deref().unwrap_or_else(|| Path::new("."));
    let config = match &global.config {
        Some(path) => load_config_from_path(path)?,
        None => load_config(repo)?,
    };
    let registry = config
        .registry
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("registry_not_found: no policy registry is configured"))?;
    let report = sync_registry(repo, registry, global.no_network)?;

    if !global.quiet {
        match global.format.clone().unwrap_or(OutputFormat::Markdown) {
            OutputFormat::Json => println!("{}", render_registry_sync_json(&report)),
            OutputFormat::Markdown => print!("{}", render_registry_sync_markdown(&report)),
        }
    }

    Ok(())
}

fn run_index(global: &GlobalArgs) -> anyhow::Result<()> {
    let repo = global.repo.as_deref().unwrap_or_else(|| Path::new("."));
    let config = match &global.config {
        Some(path) => load_config_from_path(path)?,
        None => load_config(repo)?,
    };
    let report = build_metadata_index(repo, &config)?;

    if !global.quiet {
        match global.format.clone().unwrap_or(OutputFormat::Markdown) {
            OutputFormat::Json => println!("{}", render_index_report_json(&report)),
            OutputFormat::Markdown => print!("{}", render_index_report_markdown(&report)),
        }
    }

    Ok(())
}

fn build_metadata_index(
    repo: &Path,
    config: &agent_policy_config::AgentPolicyConfig,
) -> anyhow::Result<IndexBuildReport> {
    build_metadata_index_with_cache_dir(repo, config, &agent_policy_cache_dir()?)
}

fn build_metadata_index_with_cache_dir(
    repo: &Path,
    config: &agent_policy_config::AgentPolicyConfig,
    cache_dir: &Path,
) -> anyhow::Result<IndexBuildReport> {
    let (source, policies) = if let Some(registry) = &config.registry {
        let source = index_registry_source(repo, registry)?;
        let policies = load_policies_from_registry(
            &source.root,
            RegistryLoadOptions {
                source_name: source.name.clone(),
                ..RegistryLoadOptions::default()
            },
        )?;
        (source, policies)
    } else {
        let source = index_repo_source(repo)?;
        let policies = load_policies_from_dirs(repo, &config.local_policies)?;
        (source, policies)
    };

    let index_dir = index_dir_for_source(cache_dir, &source.name);
    let metadata_path = index_dir.join("metadata.sqlite");
    let manifest_path = index_dir.join("manifest.json");
    let stale_before_build = read_index_manifest(&manifest_path)?
        .as_ref()
        .is_some_and(|manifest| index_manifest_is_stale(manifest, &source));

    fs::create_dir_all(&index_dir)?;
    write_metadata_sqlite(&metadata_path, &policies, source.commit.as_deref())?;
    let manifest = index_manifest(&source)?;
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    fs::write(&manifest_path, format!("{manifest_json}\n"))?;

    Ok(IndexBuildReport {
        source,
        index_dir,
        metadata_path,
        manifest_path,
        policy_count: policies.len(),
        stale_before_build,
    })
}

fn index_registry_source(repo: &Path, registry: &RegistryConfig) -> anyhow::Result<IndexSource> {
    if registry.registry_type != "git" {
        anyhow::bail!(
            "unsupported registry type `{}`; only git is supported",
            registry.registry_type
        );
    }
    let root = resolve_configured_path(repo, &registry.cache_dir)?;
    let name = source_name_from_path(&root);
    let commit = git_commit_if_available(&root)?;
    Ok(IndexSource {
        kind: IndexSourceKind::Registry,
        name,
        root,
        url: Some(registry.url.clone()),
        requested_ref: Some(registry.r#ref.clone()),
        commit,
    })
}

fn index_repo_source(repo: &Path) -> anyhow::Result<IndexSource> {
    let root = repo.to_path_buf();
    let name = source_name_from_path(&root);
    let commit = git_commit_if_available(&root)?;
    Ok(IndexSource {
        kind: IndexSourceKind::Repo,
        name,
        root,
        url: None,
        requested_ref: None,
        commit,
    })
}

fn source_name_from_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("repo")
        .to_string()
}

fn index_dir_for_source(cache_dir: &Path, source_name: &str) -> PathBuf {
    cache_dir.join("indexes").join(source_name)
}

fn agent_policy_cache_dir() -> anyhow::Result<PathBuf> {
    if let Some(cache_home) = std::env::var_os("XDG_CACHE_HOME") {
        return Ok(PathBuf::from(cache_home).join("agent-policy"));
    }
    let home = std::env::var("HOME").map_err(|_| anyhow::anyhow!("HOME is not set"))?;
    Ok(Path::new(&home).join(".cache").join("agent-policy"))
}

fn git_commit_if_available(path: &Path) -> anyhow::Result<Option<String>> {
    if !is_git_worktree(path) {
        return Ok(None);
    }
    git_rev_parse(path, "HEAD").map(Some)
}

fn read_index_manifest(path: &Path) -> anyhow::Result<Option<IndexManifest>> {
    match fs::read_to_string(path) {
        Ok(raw) => Ok(Some(serde_json::from_str(&raw).map_err(|error| {
            anyhow::anyhow!("failed to parse index manifest {}: {error}", path.display())
        })?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error)
            .map_err(anyhow::Error::from)
            .with_context(|| format!("failed to read index manifest {}", path.display())),
    }
}

fn index_manifest_is_stale(manifest: &IndexManifest, source: &IndexSource) -> bool {
    manifest.source.kind != source.kind
        || manifest.source.name != source.name
        || manifest.source.path != source.root.display().to_string()
        || manifest.source.commit != source.commit
}

fn index_manifest(source: &IndexSource) -> anyhow::Result<IndexManifest> {
    Ok(IndexManifest {
        schema_version: 1,
        source: IndexManifestSource {
            kind: source.kind,
            name: source.name.clone(),
            path: source.root.display().to_string(),
            url: source.url.clone(),
            requested_ref: source.requested_ref.clone(),
            commit: source.commit.clone(),
        },
        indexes: IndexManifestIndexes {
            metadata: "metadata.sqlite".to_string(),
        },
        created_at_unix: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
    })
}

fn write_metadata_sqlite(
    path: &Path,
    policies: &[LoadedPolicy],
    registry_commit: Option<&str>,
) -> anyhow::Result<()> {
    if path.exists() {
        fs::remove_file(path)?;
    }
    let mut connection = Connection::open(path)?;
    connection.execute_batch(
        "
        PRAGMA foreign_keys = ON;
        CREATE TABLE policies (
            id TEXT PRIMARY KEY NOT NULL,
            version TEXT NOT NULL,
            status TEXT NOT NULL,
            owner TEXT,
            priority INTEGER,
            source_path TEXT NOT NULL,
            registry_commit TEXT
        );
        CREATE TABLE applies_when (
            policy_id TEXT NOT NULL,
            field TEXT NOT NULL,
            value TEXT NOT NULL,
            FOREIGN KEY(policy_id) REFERENCES policies(id) ON DELETE CASCADE
        );
        CREATE INDEX idx_policies_status ON policies(status);
        CREATE INDEX idx_policies_priority ON policies(priority);
        CREATE INDEX idx_applies_when_field_value ON applies_when(field, value);
        ",
    )?;

    let transaction = connection.transaction()?;
    for loaded in policies {
        let policy = &loaded.policy;
        transaction.execute(
            "INSERT INTO policies
                (id, version, status, owner, priority, source_path, registry_commit)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                policy.id.as_str(),
                policy_version_string(&policy.version),
                policy_status_name(policy.status),
                policy.owner.as_deref(),
                policy.priority.map(i64::from),
                loaded.source_path.display().to_string(),
                registry_commit,
            ],
        )?;
        insert_applies_when_values(&transaction, &policy.id, &policy.applies_when)?;
    }
    transaction.commit()?;

    Ok(())
}

fn insert_applies_when_values(
    connection: &Connection,
    policy_id: &str,
    applies_when: &AppliesWhen,
) -> anyhow::Result<()> {
    insert_applies_when_strings(connection, policy_id, "repos", &applies_when.repos)?;
    insert_applies_when_strings(connection, policy_id, "paths", &applies_when.paths)?;
    insert_applies_when_strings(connection, policy_id, "languages", &applies_when.languages)?;
    insert_applies_when_strings(
        connection,
        policy_id,
        "frameworks",
        &applies_when.frameworks,
    )?;
    insert_applies_when_strings(
        connection,
        policy_id,
        "package_managers",
        &applies_when.package_managers,
    )?;
    let task_types = applies_when
        .task_types
        .iter()
        .map(|task_type| task_type.0.clone())
        .collect::<Vec<_>>();
    insert_applies_when_strings(connection, policy_id, "task_types", &task_types)?;
    insert_applies_when_strings(
        connection,
        policy_id,
        "risk_flags",
        &applies_when.risk_flags,
    )?;
    Ok(())
}

fn insert_applies_when_strings(
    connection: &Connection,
    policy_id: &str,
    field: &str,
    values: &[String],
) -> anyhow::Result<()> {
    for value in values {
        connection.execute(
            "INSERT INTO applies_when (policy_id, field, value) VALUES (?1, ?2, ?3)",
            params![policy_id, field, value],
        )?;
    }
    Ok(())
}

fn policy_version_string(version: &PolicyVersion) -> String {
    match version {
        PolicyVersion::Integer(value) => value.to_string(),
        PolicyVersion::Text(value) => value.clone(),
    }
}

fn policy_status_name(status: PolicyStatus) -> &'static str {
    match status {
        PolicyStatus::Draft => "draft",
        PolicyStatus::Active => "active",
        PolicyStatus::Deprecated => "deprecated",
        PolicyStatus::Disabled => "disabled",
    }
}

fn render_index_report_json(report: &IndexBuildReport) -> String {
    format!(
        "{{\n  \"status\": \"ok\",\n  \"source\": {{\n    \"kind\": \"{}\",\n    \"name\": \"{}\",\n    \"commit\": {}\n  }},\n  \"index_dir\": \"{}\",\n  \"metadata\": \"{}\",\n  \"manifest\": \"{}\",\n  \"policy_count\": {},\n  \"stale_before_build\": {}\n}}\n",
        index_source_kind_name(report.source.kind),
        json_escape(&report.source.name),
        report
            .source
            .commit
            .as_ref()
            .map(|commit| format!("\"{}\"", json_escape(commit)))
            .unwrap_or_else(|| "null".to_string()),
        json_escape(&report.index_dir.display().to_string()),
        json_escape(&report.metadata_path.display().to_string()),
        json_escape(&report.manifest_path.display().to_string()),
        report.policy_count,
        report.stale_before_build
    )
}

fn render_index_report_markdown(report: &IndexBuildReport) -> String {
    let mut out = String::new();
    out.push_str("# Agent Policy Index\n\n");
    out.push_str("- Status: `ok`\n");
    out.push_str(&format!(
        "- Source: `{}` `{}`\n",
        index_source_kind_name(report.source.kind),
        markdown_inline(&report.source.name)
    ));
    if let Some(commit) = &report.source.commit {
        out.push_str(&format!("- Commit: `{}`\n", commit));
    }
    out.push_str(&format!("- Policies: `{}`\n", report.policy_count));
    out.push_str(&format!(
        "- Metadata: `{}`\n",
        report.metadata_path.display()
    ));
    out.push_str(&format!(
        "- Manifest: `{}`\n",
        report.manifest_path.display()
    ));
    out.push_str(&format!(
        "- Stale before build: `{}`\n",
        if report.stale_before_build {
            "yes"
        } else {
            "no"
        }
    ));
    out
}

fn index_source_kind_name(kind: IndexSourceKind) -> &'static str {
    match kind {
        IndexSourceKind::Registry => "registry",
        IndexSourceKind::Repo => "repo",
    }
}

fn sync_registry(
    repo: &Path,
    registry: &RegistryConfig,
    no_network: bool,
) -> anyhow::Result<RegistrySyncReport> {
    if registry.registry_type != "git" {
        anyhow::bail!(
            "unsupported registry type `{}`; only git is supported",
            registry.registry_type
        );
    }

    let cache_dir = resolve_configured_path(repo, &registry.cache_dir)?;
    let url_path = local_registry_url_path(repo, &registry.url)?;
    if is_local_path_registry(&cache_dir, url_path.as_deref()) {
        return Ok(RegistrySyncReport {
            cache_dir,
            mode: registry.sync.mode,
            status: RegistrySyncStatus::LocalPath,
            commit: None,
            requested_ref: registry.r#ref.clone(),
            message: "local path registry; nothing to sync".to_string(),
        });
    }

    if !cache_dir.exists() {
        let mode_hint = if registry.sync.mode == SyncMode::Offline {
            "offline mode cannot clone or fetch"
        } else if no_network {
            "--no-network is set"
        } else {
            "network clone is not implemented"
        };
        anyhow::bail!(
            "registry_not_found: registry cache directory {} does not exist ({mode_hint})",
            cache_dir.display()
        );
    }
    if !cache_dir.is_dir() {
        anyhow::bail!(
            "registry_not_found: registry cache path {} is not a directory",
            cache_dir.display()
        );
    }
    if !is_git_worktree(&cache_dir) {
        anyhow::bail!(
            "registry_not_found: registry cache {} is not a Git worktree",
            cache_dir.display()
        );
    }

    let head = git_rev_parse(&cache_dir, "HEAD")?;
    let status = match registry.sync.mode {
        SyncMode::Pinned => {
            validate_pinned_ref(&cache_dir, &registry.r#ref, &head)?;
            RegistrySyncStatus::Pinned
        }
        SyncMode::Offline => {
            validate_requested_ref_if_available(&cache_dir, &registry.r#ref, &head)?;
            RegistrySyncStatus::Offline
        }
        SyncMode::Manual | SyncMode::Auto => {
            validate_requested_ref_if_available(&cache_dir, &registry.r#ref, &head)?;
            if no_network {
                RegistrySyncStatus::Offline
            } else {
                RegistrySyncStatus::Cached
            }
        }
    };

    let message = match status {
        RegistrySyncStatus::Pinned => "pinned registry cache matches requested ref",
        RegistrySyncStatus::Offline => "using cached registry without network access",
        RegistrySyncStatus::Cached => "using cached registry; network fetch is not implemented",
        RegistrySyncStatus::LocalPath => "local path registry; nothing to sync",
    }
    .to_string();

    Ok(RegistrySyncReport {
        cache_dir,
        mode: registry.sync.mode,
        status,
        commit: Some(head),
        requested_ref: registry.r#ref.clone(),
        message,
    })
}

fn local_registry_url_path(repo: &Path, url: &str) -> anyhow::Result<Option<PathBuf>> {
    if let Some(path) = url.strip_prefix("file://") {
        return Ok(Some(resolve_configured_path(repo, path)?));
    }
    if looks_like_remote_git_url(url) {
        return Ok(None);
    }
    Ok(Some(resolve_configured_path(repo, url)?))
}

fn is_local_path_registry(cache_dir: &Path, url_path: Option<&Path>) -> bool {
    match url_path {
        Some(path) => {
            path == cache_dir
                && cache_dir.exists()
                && cache_dir.is_dir()
                && !looks_like_remote_git_url(&path.display().to_string())
        }
        None => false,
    }
}

fn is_git_worktree(path: &Path) -> bool {
    path.join(".git").exists()
}

fn validate_pinned_ref(cache_dir: &Path, requested_ref: &str, head: &str) -> anyhow::Result<()> {
    if is_full_sha(requested_ref) {
        if head == requested_ref {
            return Ok(());
        }
        anyhow::bail!(
            "registry_pinned_mismatch: registry cache {} is at commit {}, expected {}",
            cache_dir.display(),
            head,
            requested_ref
        );
    }
    validate_requested_ref_if_available(cache_dir, requested_ref, head)
}

fn validate_requested_ref_if_available(
    cache_dir: &Path,
    requested_ref: &str,
    head: &str,
) -> anyhow::Result<()> {
    if is_full_sha(requested_ref) {
        if head == requested_ref {
            return Ok(());
        }
        anyhow::bail!(
            "registry_ref_mismatch: registry cache {} is at commit {}, expected {}",
            cache_dir.display(),
            head,
            requested_ref
        );
    }

    match git_rev_parse(cache_dir, &format!("{requested_ref}^{{commit}}")) {
        Ok(ref_commit) if ref_commit == head => Ok(()),
        Ok(ref_commit) => anyhow::bail!(
            "registry_ref_mismatch: registry cache {} is at commit {}, but ref {} points to {}",
            cache_dir.display(),
            head,
            requested_ref,
            ref_commit
        ),
        Err(_) => Ok(()),
    }
}

fn git_rev_parse(repo: &Path, rev: &str) -> anyhow::Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("rev-parse")
        .arg("--verify")
        .arg(rev)
        .output()?;
    if !output.status.success() {
        anyhow::bail!(
            "git rev-parse failed for `{}` in {}: {}",
            rev,
            repo.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn is_full_sha(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|character| character.is_ascii_hexdigit())
}

fn render_registry_sync_json(report: &RegistrySyncReport) -> String {
    let commit = report
        .commit
        .as_ref()
        .map(|commit| format!("\"{}\"", json_escape(commit)))
        .unwrap_or_else(|| "null".to_string());
    format!(
        "{{\n  \"status\": \"{}\",\n  \"mode\": \"{}\",\n  \"cache_dir\": \"{}\",\n  \"ref\": \"{}\",\n  \"commit\": {},\n  \"message\": \"{}\"\n}}\n",
        registry_sync_status_name(report.status),
        sync_mode_name(report.mode),
        json_escape(&report.cache_dir.display().to_string()),
        json_escape(&report.requested_ref),
        commit,
        json_escape(&report.message)
    )
}

fn render_registry_sync_markdown(report: &RegistrySyncReport) -> String {
    let mut out = String::new();
    out.push_str("# Registry Sync\n\n");
    out.push_str(&format!(
        "- Status: `{}`\n",
        registry_sync_status_name(report.status)
    ));
    out.push_str(&format!("- Mode: `{}`\n", sync_mode_name(report.mode)));
    out.push_str(&format!("- Cache: `{}`\n", report.cache_dir.display()));
    out.push_str(&format!(
        "- Ref: `{}`\n",
        markdown_inline(&report.requested_ref)
    ));
    if let Some(commit) = &report.commit {
        out.push_str(&format!("- Commit: `{}`\n", commit));
    }
    out.push_str(&format!("- Message: {}\n", report.message));
    out
}

fn registry_sync_status_name(status: RegistrySyncStatus) -> &'static str {
    match status {
        RegistrySyncStatus::LocalPath => "local_path",
        RegistrySyncStatus::Cached => "cached",
        RegistrySyncStatus::Offline => "offline",
        RegistrySyncStatus::Pinned => "pinned",
    }
}

fn sync_mode_name(mode: SyncMode) -> &'static str {
    match mode {
        SyncMode::Manual => "manual",
        SyncMode::Auto => "auto",
        SyncMode::Pinned => "pinned",
        SyncMode::Offline => "offline",
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
    let repo = repo.canonicalize()?;
    let migration_dir = ensure_safe_migration_dir(&repo)?;

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
        write_migration_draft_file(&target, &draft.policy_yaml)?;
    }

    Ok(())
}

fn ensure_safe_migration_dir(repo: &Path) -> anyhow::Result<PathBuf> {
    let agent_policy_dir = repo.join(".agent-policy");
    ensure_safe_directory(&agent_policy_dir, ".agent-policy")?;

    let migration_dir = agent_policy_dir.join("migration");
    ensure_safe_directory(&migration_dir, ".agent-policy/migration")?;

    let migration_dir = migration_dir.canonicalize()?;
    if !migration_dir.starts_with(repo) {
        anyhow::bail!("refusing to write migration drafts outside the repository");
    }

    Ok(migration_dir)
}

fn ensure_safe_directory(path: &Path, label: &str) -> anyhow::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                anyhow::bail!("refusing to use symlinked migration directory component {label}");
            }
            if !metadata.is_dir() {
                anyhow::bail!("migration directory component {label} is not a directory");
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            fs::create_dir(path)?;
            let metadata = fs::symlink_metadata(path)?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                anyhow::bail!("refusing to use unsafe migration directory component {label}");
            }
        }
        Err(error) => return Err(error.into()),
    }

    Ok(())
}

fn write_migration_draft_file(target: &Path, policy_yaml: &str) -> anyhow::Result<()> {
    match fs::symlink_metadata(target) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                anyhow::bail!("refusing to overwrite symlinked migration draft");
            }
            if !metadata.is_file() {
                anyhow::bail!("refusing to overwrite non-file migration draft path");
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    set_no_follow(&mut options);

    let mut file = options.open(target)?;
    file.write_all(policy_yaml.as_bytes())?;
    file.sync_all()?;

    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn set_no_follow(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;

    const O_NOFOLLOW: i32 = 0o400000;
    options.custom_flags(O_NOFOLLOW);
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
fn set_no_follow(_options: &mut OpenOptions) {}

#[cfg(not(unix))]
fn set_no_follow(_options: &mut OpenOptions) {}

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
    let (config, trusted_registry) = match &global.config {
        Some(path) => {
            let config = load_config_from_path(path)?;
            let trusted_registry = config.registry.clone();
            (config, trusted_registry)
        }
        // Repository config is branch-controlled, so `get` only treats registry
        // settings as trusted when they come from an explicit operator config.
        None => (load_config(repo)?, None),
    };

    let intent = build_task_intent(repo, &config, &args);
    let mut policies = match &trusted_registry {
        Some(registry) => load_registry_policies(repo, registry)?,
        None => Vec::new(),
    };
    policies.extend(load_policies_from_dirs(repo, &config.local_policies)?);
    let discovered_sources = discover(repo)?;
    policies.extend(markdown_candidate_policies(
        repo,
        &discovered_sources,
        &intent.files,
        &config.instruction_sources.trusted,
    ));
    let mut bundle = build_instruction_bundle(
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
    bundle.warnings.extend(loaded.warnings);

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

fn load_get_policies(
    repo: &Path,
    config: &agent_policy_config::AgentPolicyConfig,
) -> anyhow::Result<GetPolicyLoad> {
    load_get_policies_with_cache_dir(repo, config, &agent_policy_cache_dir()?)
}

fn load_get_policies_with_cache_dir(
    repo: &Path,
    config: &agent_policy_config::AgentPolicyConfig,
    cache_dir: &Path,
) -> anyhow::Result<GetPolicyLoad> {
    let mut warnings = Vec::new();
    let mut policies = Vec::new();

    if let Some(registry) = &config.registry {
        let source = index_registry_source(repo, registry)?;
        get_indexed_policy_ids(cache_dir, &source, &mut warnings)?;
        let registry_policies = load_registry_policies(repo, registry)?;
        policies.extend(filter_active_loaded_policies(registry_policies));
        policies.extend(load_policies_from_dirs(repo, &config.local_policies)?);
    } else {
        let source = index_repo_source(repo)?;
        get_indexed_policy_ids(cache_dir, &source, &mut warnings)?;
        let local_policies = load_policies_from_dirs(repo, &config.local_policies)?;
        policies.extend(filter_active_loaded_policies(local_policies));
    }

    Ok(GetPolicyLoad { policies, warnings })
}

fn get_indexed_policy_ids(
    cache_dir: &Path,
    source: &IndexSource,
    warnings: &mut Vec<String>,
) -> anyhow::Result<Option<BTreeSet<String>>> {
    let index_dir = index_dir_for_source(cache_dir, &source.name);
    let manifest_path = index_dir.join("manifest.json");
    let manifest = match read_index_manifest(&manifest_path) {
        Ok(Some(manifest)) => manifest,
        Ok(None) => {
            warnings.push(format!(
                "Metadata index missing at {}; loaded policies directly.",
                manifest_path.display()
            ));
            return Ok(None);
        }
        Err(error) => {
            warnings.push(format!(
                "Metadata index manifest at {} is invalid or unreadable ({error:#}); loaded policies directly.",
                manifest_path.display()
            ));
            return Ok(None);
        }
    };

    if index_manifest_is_stale(&manifest, source) {
        warnings.push(format!(
            "Metadata index at {} is stale; loaded policies directly.",
            index_dir.display()
        ));
        return Ok(None);
    }

    let metadata_path = index_dir.join(&manifest.indexes.metadata);
    match read_indexed_policy_ids(&metadata_path) {
        Ok(ids) => Ok(Some(ids)),
        Err(error) => {
            warnings.push(format!(
                "Metadata index at {} is invalid or unreadable ({error:#}); loaded policies directly.",
                metadata_path.display()
            ));
            Ok(None)
        }
    }
}

fn read_indexed_policy_ids(path: &Path) -> anyhow::Result<BTreeSet<String>> {
    let connection = Connection::open(path)
        .with_context(|| format!("failed to open metadata index {}", path.display()))?;
    let mut statement = connection
        .prepare("SELECT id FROM policies WHERE status = 'active' ORDER BY id")
        .with_context(|| format!("failed to query metadata index {}", path.display()))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    rows.collect::<Result<BTreeSet<_>, _>>()
        .map_err(anyhow::Error::from)
}

fn filter_active_loaded_policies(policies: Vec<LoadedPolicy>) -> Vec<LoadedPolicy> {
    policies
        .into_iter()
        .filter(|loaded| loaded.policy.status == PolicyStatus::Active)
        .collect()
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
    trusted_sources: &[String],
) -> Vec<LoadedPolicy> {
    let mut policies = Vec::new();

    for source in &discovered.instruction_sources {
        if !instruction_source_is_trusted(repo, source, trusted_sources) {
            continue;
        }
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

fn instruction_source_is_trusted(
    repo: &Path,
    source: &InstructionSource,
    trusted_sources: &[String],
) -> bool {
    if trusted_sources.is_empty() {
        return false;
    }

    let relative_path = normalize_match_path(&source.path);
    let repo_path = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    let absolute_path = normalize_match_path(&repo_path.join(&source.path));

    trusted_sources.iter().any(|trusted_source| {
        trusted_source_matches(trusted_source, &relative_path, &absolute_path)
    })
}

fn trusted_source_matches(trusted_source: &str, relative_path: &str, absolute_path: &str) -> bool {
    let trusted_source = trusted_source.trim();
    if trusted_source.is_empty() {
        return false;
    }

    let trusted_path = Path::new(trusted_source);
    if trusted_path.is_absolute() {
        let trusted_path = if contains_glob_pattern(trusted_source) {
            trusted_path.to_path_buf()
        } else {
            trusted_path
                .canonicalize()
                .unwrap_or_else(|_| trusted_path.to_path_buf())
        };
        let trusted_path = normalize_match_path(&trusted_path);
        trusted_path_matches(&trusted_path, absolute_path)
    } else {
        let trusted_path = normalize_task_file(trusted_source);
        trusted_path_matches(&trusted_path, relative_path)
    }
}

fn trusted_path_matches(trusted_path: &str, candidate_path: &str) -> bool {
    if trusted_path == "." {
        return true;
    }

    if contains_glob_pattern(trusted_path) {
        return Glob::new(trusted_path)
            .map(|glob| glob.compile_matcher().is_match(candidate_path))
            .unwrap_or(false);
    }

    let trusted_path = trusted_path.trim_end_matches('/');
    candidate_path == trusted_path || candidate_path.starts_with(&format!("{trusted_path}/"))
}

fn contains_glob_pattern(path: &str) -> bool {
    path.contains('*') || path.contains('?') || path.contains('[') || path.contains('{')
}

fn normalize_match_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
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
        build_metadata_index_with_cache_dir, detect_inspection_conflicts,
        detect_inspection_duplicates, inspect_repo, load_get_policies_with_cache_dir,
        load_registry_policies, markdown_candidate_policies, migration_dry_run_report,
        render_inspection_json, render_inspection_markdown, render_migration_dry_run_json,
        render_migration_dry_run_markdown, render_registry_sync_json,
        render_registry_sync_markdown, render_validation_markdown, run, scope_matches_task_files,
        sync_registry, validate_repo, Cli, Commands, GlobalArgs, IndexManifest,
        InspectionCandidate, MigrationClass, OutputFormat, RegistryCommands, RegistrySyncStatus,
        ValidationStatus,
    };
    use agent_policy_config::{
        load_config, AgentPolicyConfig, RegistryConfig, RegistrySyncConfig, SyncMode,
    };
    use agent_policy_core::{
        build_instruction_bundle, load_policies_from_dirs, render_bundle_json, BundleBuildOptions,
        DetectedContext, LoadedPolicy, OutputBudget, TaskDetails, TaskIntent,
    };
    use agent_policy_discover::discover;
    use clap::{error::ErrorKind, CommandFactory, Parser};
    use rusqlite::Connection;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    const INDEX_POLICY_YAML: &str = r#"id: org.index.metadata
version: "2026.1"
status: active
owner: platform
priority: 42
applies_when:
  repos:
    - agent-policy-broker
  paths:
    - crates/**
  languages:
    - rust
  frameworks:
    - axum
  package_managers:
    - cargo
  task_types:
    - implementation
  risk_flags:
    - storage
instructions:
  - Keep index metadata deterministic.
"#;

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
    fn get_ignores_repository_controlled_registry_without_explicit_config() {
        let temp = TempDir::new("get-ignores-repo-registry");
        let repo = temp.path();
        fs::write(
            repo.join(".agent-policy.yaml"),
            r#"registry:
  type: git
  url: ./benign-looking-registry
  ref: main
  cache_dir: ./attacker-controlled-registry
"#,
        )
        .expect("write repo config");

        let cli = Cli::try_parse_from([
            "agent-policy",
            "--repo",
            repo.to_str().expect("utf8 repo"),
            "get",
            "--task",
            "review PR",
            "--format",
            "json",
        ])
        .expect("parse get");

        run(cli).expect("repo-controlled registry should not be loaded by get");
    }

    #[test]
    fn get_loads_registry_from_explicit_config() {
        let temp = TempDir::new("get-explicit-registry");
        let repo = temp.path();
        let config_path = repo.join("trusted-config.yaml");
        fs::write(
            &config_path,
            r#"registry:
  type: git
  url: ./trusted-registry
  ref: main
  cache_dir: ./missing-trusted-cache
"#,
        )
        .expect("write explicit config");

        let cli = Cli::try_parse_from([
            "agent-policy",
            "--repo",
            repo.to_str().expect("utf8 repo"),
            "--config",
            config_path.to_str().expect("utf8 config"),
            "get",
            "--task",
            "review PR",
            "--format",
            "json",
        ])
        .expect("parse get");

        let error = run(cli).expect_err("explicit registry config should still be loaded");
        assert!(format!("{error:#}").contains("registry cache directory"));
        assert!(format!("{error:#}").contains("missing-trusted-cache"));
    }

    #[test]
    fn registry_sync_local_path_registry_is_noop_success() {
        let repo = fixture_repo("registry-app");
        let config = load_config(&repo).expect("registry config should load");
        let registry = config.registry.expect("registry should be configured");

        let report = sync_registry(&repo, &registry, true).expect("sync local path registry");

        assert_eq!(report.status, RegistrySyncStatus::LocalPath);
        assert_eq!(report.mode, SyncMode::Manual);
        assert!(report.commit.is_none());
        assert!(report.message.contains("nothing to sync"));
    }

    #[test]
    fn registry_sync_offline_uses_cached_git_without_fetching() {
        let temp = TempDir::new("registry-sync-offline");
        let repo = temp.path();
        let cache_dir = repo.join("registry-cache");
        let head = init_git_registry(&cache_dir);
        let registry = test_registry(&cache_dir, "main", SyncMode::Offline);

        let report = sync_registry(repo, &registry, false).expect("offline sync");

        assert_eq!(report.status, RegistrySyncStatus::Offline);
        assert_eq!(report.commit.as_deref(), Some(head.as_str()));
        assert_eq!(report.requested_ref, "main");
        assert!(render_registry_sync_markdown(&report).contains("without network access"));
    }

    #[test]
    fn registry_sync_no_network_uses_cached_git_without_fetching() {
        let temp = TempDir::new("registry-sync-no-network");
        let repo = temp.path();
        let cache_dir = repo.join("registry-cache");
        let head = init_git_registry(&cache_dir);
        let mut registry = test_registry(&cache_dir, "main", SyncMode::Manual);
        registry.url = "https://example.invalid/company/registry.git".to_string();

        let report = sync_registry(repo, &registry, true).expect("no-network sync");

        assert_eq!(report.status, RegistrySyncStatus::Offline);
        assert_eq!(report.commit.as_deref(), Some(head.as_str()));
        assert!(render_registry_sync_json(&report).contains("\"status\": \"offline\""));
    }

    #[test]
    fn registry_sync_pinned_validates_current_commit() {
        let temp = TempDir::new("registry-sync-pinned");
        let repo = temp.path();
        let cache_dir = repo.join("registry-cache");
        let head = init_git_registry(&cache_dir);
        let registry = test_registry(&cache_dir, &head, SyncMode::Pinned);

        let report = sync_registry(repo, &registry, false).expect("pinned sync");

        assert_eq!(report.status, RegistrySyncStatus::Pinned);
        assert_eq!(report.commit.as_deref(), Some(head.as_str()));
    }

    #[test]
    fn registry_sync_pinned_rejects_mismatched_commit() {
        let temp = TempDir::new("registry-sync-pinned-mismatch");
        let repo = temp.path();
        let cache_dir = repo.join("registry-cache");
        init_git_registry(&cache_dir);
        let wrong_commit = "0123456789abcdef0123456789abcdef01234567";
        let registry = test_registry(&cache_dir, wrong_commit, SyncMode::Pinned);

        let error = sync_registry(repo, &registry, false).expect_err("pinned mismatch");

        assert!(format!("{error:#}").contains("registry_pinned_mismatch"));
        assert!(format!("{error:#}").contains(wrong_commit));
    }

    #[test]
    fn registry_sync_missing_registry_reports_useful_error() {
        let temp = TempDir::new("registry-sync-missing");
        let repo = temp.path();
        let cache_dir = repo.join("missing-cache");
        let registry = test_registry(&cache_dir, "main", SyncMode::Offline);

        let error = sync_registry(repo, &registry, false).expect_err("missing cache");

        let message = format!("{error:#}");
        assert!(message.contains("registry_not_found"));
        assert!(message.contains("offline mode cannot clone or fetch"));
        assert!(message.contains("missing-cache"));
    }

    #[test]
    fn registry_sync_requires_configured_registry() {
        let repo = fixture_repo("payments-repo");
        let cli = Cli::try_parse_from([
            "agent-policy",
            "--repo",
            repo.to_str().expect("utf8 repo"),
            "registry",
            "sync",
        ])
        .expect("parse registry sync");

        let error = run(cli).expect_err("missing configured registry");

        assert!(format!("{error:#}").contains("registry_not_found"));
    }

    #[test]
    fn index_builds_metadata_sqlite_and_manifest_in_temp_cache() {
        let temp = TempDir::new("index-metadata");
        let repo = temp.path().join("repo");
        let registry_dir = temp.path().join("registry-cache");
        fs::create_dir_all(&repo).expect("create temp repo");
        let head = init_git_registry_with_policy(&registry_dir, INDEX_POLICY_YAML);
        let mut registry = test_registry(&registry_dir, "main", SyncMode::Manual);
        registry.url = registry_dir.display().to_string();
        let config = AgentPolicyConfig {
            registry: Some(registry),
            ..AgentPolicyConfig::default()
        };
        let cache_dir = temp.path().join("cache");

        let report = build_metadata_index_with_cache_dir(&repo, &config, &cache_dir)
            .expect("build metadata index");

        assert_eq!(report.policy_count, 1);
        assert!(!report.stale_before_build);
        assert!(report.metadata_path.exists());
        assert_eq!(
            report.metadata_path,
            cache_dir
                .join("indexes")
                .join("registry-cache")
                .join("metadata.sqlite")
        );
        assert!(report.manifest_path.exists());

        let manifest: IndexManifest = serde_json::from_str(
            &fs::read_to_string(&report.manifest_path).expect("read manifest"),
        )
        .expect("parse manifest");
        assert_eq!(manifest.source.name, "registry-cache");
        assert_eq!(manifest.source.commit.as_deref(), Some(head.as_str()));
        assert_eq!(manifest.indexes.metadata, "metadata.sqlite");

        let connection = Connection::open(&report.metadata_path).expect("open metadata sqlite");
        let row = connection
            .query_row(
                "SELECT version, status, owner, priority, source_path, registry_commit
                 FROM policies WHERE id = 'org.index.metadata'",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                },
            )
            .expect("read indexed policy");
        assert_eq!(row.0, "2026.1");
        assert_eq!(row.1, "active");
        assert_eq!(row.2, "platform");
        assert_eq!(row.3, 42);
        assert!(row.4.ends_with("policies/policy.yaml"));
        assert_eq!(row.5, head);

        let mut statement = connection
            .prepare(
                "SELECT field, value FROM applies_when
                 WHERE policy_id = 'org.index.metadata'
                 ORDER BY field, value",
            )
            .expect("prepare applies_when query");
        let values = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .expect("query applies_when")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect applies_when");
        assert!(values.contains(&("frameworks".to_string(), "axum".to_string())));
        assert!(values.contains(&("languages".to_string(), "rust".to_string())));
        assert!(values.contains(&("package_managers".to_string(), "cargo".to_string())));
        assert!(values.contains(&("paths".to_string(), "crates/**".to_string())));
        assert!(values.contains(&("repos".to_string(), "agent-policy-broker".to_string())));
        assert!(values.contains(&("risk_flags".to_string(), "storage".to_string())));
        assert!(values.contains(&("task_types".to_string(), "implementation".to_string())));
    }

    #[test]
    fn index_reports_stale_manifest_when_registry_commit_changes() {
        let temp = TempDir::new("index-stale");
        let repo = temp.path().join("repo");
        let registry_dir = temp.path().join("registry-cache");
        fs::create_dir_all(&repo).expect("create temp repo");
        let first_head = init_git_registry_with_policy(&registry_dir, INDEX_POLICY_YAML);
        let mut registry = test_registry(&registry_dir, "main", SyncMode::Manual);
        registry.url = registry_dir.display().to_string();
        let config = AgentPolicyConfig {
            registry: Some(registry),
            ..AgentPolicyConfig::default()
        };
        let cache_dir = temp.path().join("cache");

        let first_report = build_metadata_index_with_cache_dir(&repo, &config, &cache_dir)
            .expect("first index build");
        assert!(!first_report.stale_before_build);

        fs::write(
            registry_dir.join("policies").join("second.yaml"),
            "id: org.index.second\nversion: 1\nstatus: active\napplies_when: {}\ninstructions:\n  - Second policy.\n",
        )
        .expect("write second policy");
        git(&registry_dir, &["add", "."]);
        git(
            &registry_dir,
            &[
                "-c",
                "user.name=Agent Policy Tests",
                "-c",
                "user.email=agent-policy-tests@example.invalid",
                "commit",
                "-m",
                "second registry commit",
            ],
        );
        let second_head = git_stdout(&registry_dir, &["rev-parse", "HEAD"]);
        assert_ne!(first_head, second_head);

        let second_report = build_metadata_index_with_cache_dir(&repo, &config, &cache_dir)
            .expect("second index build");
        assert!(second_report.stale_before_build);

        let manifest: IndexManifest = serde_json::from_str(
            &fs::read_to_string(&second_report.manifest_path).expect("read updated manifest"),
        )
        .expect("parse updated manifest");
        assert_eq!(
            manifest.source.commit.as_deref(),
            Some(second_head.as_str())
        );
    }

    #[test]
    fn get_uses_valid_metadata_index_for_candidate_lookup() {
        let temp = TempDir::new("get-indexed");
        let repo = temp.path().join("repo");
        write_get_policy_fixture(&repo);
        let config = AgentPolicyConfig::default();
        let cache_dir = temp.path().join("cache");
        build_metadata_index_with_cache_dir(&repo, &config, &cache_dir).expect("build index");

        let indexed = load_get_policies_with_cache_dir(&repo, &config, &cache_dir)
            .expect("load indexed policies");
        let direct =
            load_policies_from_dirs(&repo, &config.local_policies).expect("load direct policies");

        assert!(indexed.warnings.is_empty());
        assert_eq!(
            policy_ids(&indexed.policies),
            vec!["org.get.active".to_string()]
        );
        assert_eq!(
            get_bundle_json(&indexed.policies),
            get_bundle_json(&direct),
            "indexed lookup should produce the same bundle content as direct loading"
        );
    }

    #[test]
    fn get_does_not_allow_metadata_index_to_suppress_authoritative_policies() {
        let temp = TempDir::new("get-tampered-index");
        let repo = temp.path().join("repo");
        write_get_policy_fixture(&repo);
        let config = AgentPolicyConfig::default();
        let cache_dir = temp.path().join("cache");
        let report =
            build_metadata_index_with_cache_dir(&repo, &config, &cache_dir).expect("build index");
        let connection = Connection::open(&report.metadata_path).expect("open metadata sqlite");
        connection
            .execute("DELETE FROM policies WHERE id = 'org.get.active'", [])
            .expect("tamper metadata index");
        drop(connection);

        let loaded = load_get_policies_with_cache_dir(&repo, &config, &cache_dir)
            .expect("load policies with tampered index");

        assert!(loaded.warnings.is_empty());
        assert_eq!(
            policy_ids(&loaded.policies),
            vec!["org.get.active".to_string()],
            "authoritative policy files must remain authoritative even when the derived index omits them"
        );
        assert_eq!(
            get_bundle_json(&loaded.policies),
            get_bundle_json(
                &load_policies_from_dirs(&repo, &config.local_policies)
                    .expect("load direct policies")
            )
        );
    }

    #[test]
    fn get_falls_back_when_metadata_index_is_missing() {
        let temp = TempDir::new("get-missing-index");
        let repo = temp.path().join("repo");
        write_get_policy_fixture(&repo);
        let config = AgentPolicyConfig::default();
        let cache_dir = temp.path().join("cache");

        let loaded = load_get_policies_with_cache_dir(&repo, &config, &cache_dir)
            .expect("load direct policies without index");

        assert!(loaded
            .warnings
            .iter()
            .any(|warning| warning.contains("Metadata index missing")));
        assert_eq!(
            get_bundle_json(&loaded.policies),
            get_bundle_json(
                &load_policies_from_dirs(&repo, &config.local_policies)
                    .expect("load direct policies")
            )
        );
    }

    #[test]
    fn get_falls_back_when_metadata_index_is_stale() {
        let temp = TempDir::new("get-stale-index");
        let repo = temp.path().join("repo");
        write_get_policy_fixture(&repo);
        let config = AgentPolicyConfig::default();
        let cache_dir = temp.path().join("cache");
        let report =
            build_metadata_index_with_cache_dir(&repo, &config, &cache_dir).expect("build index");
        let mut manifest: IndexManifest = serde_json::from_str(
            &fs::read_to_string(&report.manifest_path).expect("read manifest"),
        )
        .expect("parse manifest");
        manifest.source.path = temp.path().join("other-repo").display().to_string();
        fs::write(
            &report.manifest_path,
            format!(
                "{}\n",
                serde_json::to_string_pretty(&manifest).expect("serialize manifest")
            ),
        )
        .expect("write stale manifest");

        let loaded = load_get_policies_with_cache_dir(&repo, &config, &cache_dir)
            .expect("load policies with stale index");

        assert!(loaded
            .warnings
            .iter()
            .any(|warning| warning.contains("is stale")));
        assert_eq!(
            get_bundle_json(&loaded.policies),
            get_bundle_json(
                &load_policies_from_dirs(&repo, &config.local_policies)
                    .expect("load direct policies")
            )
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

    #[cfg(unix)]
    #[test]
    fn migrate_write_rejects_symlinked_draft_file() {
        use std::os::unix::fs::symlink;

        let temp = TempRepo::copy_fixture("nested-instructions");
        let repo = temp.path();
        let migration_dir = repo.join(".agent-policy").join("migration");
        fs::create_dir_all(&migration_dir).expect("create migration dir");
        let outside_target = repo.join("outside-target.txt");
        fs::write(&outside_target, "ORIGINAL_SENTINEL").expect("write outside target");
        symlink(
            &outside_target,
            migration_dir.join("local.backend.payments.payments.yaml"),
        )
        .expect("create symlinked draft");

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

        let error = run(cli).expect_err("symlinked draft must be rejected");
        assert!(
            error
                .to_string()
                .contains("refusing to overwrite symlinked migration draft"),
            "unexpected error: {error:#}"
        );
        assert_eq!(
            fs::read_to_string(&outside_target).expect("read outside target"),
            "ORIGINAL_SENTINEL"
        );
    }

    #[cfg(unix)]
    #[test]
    fn migrate_write_rejects_symlinked_migration_directory_component() {
        use std::os::unix::fs::symlink;

        let temp = TempRepo::copy_fixture("nested-instructions");
        let repo = temp.path();
        let outside_dir = repo.join("outside-agent-policy");
        fs::create_dir(&outside_dir).expect("create outside dir");
        symlink(&outside_dir, repo.join(".agent-policy")).expect("create .agent-policy symlink");

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

        let error = run(cli).expect_err("symlinked .agent-policy must be rejected");
        assert!(
            error
                .to_string()
                .contains("refusing to use symlinked migration directory component .agent-policy"),
            "unexpected error: {error:#}"
        );
        assert!(!outside_dir.join("migration").exists());
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
        let policies = markdown_candidate_policies(&repo, &discovered, &files, &[".".into()]);
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
        let policies = markdown_candidate_policies(&repo, &discovered, &files, &[".".into()]);
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
    fn untrusted_markdown_candidates_are_not_added_to_get_bundle() {
        let repo = fixture_repo("nested-instructions");
        let discovered = discover(&repo).expect("discover fixture repo");
        let files = vec!["backend/payments/src/refunds.ts".to_string()];
        let policies = markdown_candidate_policies(&repo, &discovered, &files, &[]);

        assert!(policies.is_empty());
    }

    #[test]
    fn markdown_candidate_policies_require_trusted_sources() {
        let repo = fixture_repo("nested-instructions");
        let discovered = discover(&repo).expect("discover fixture repo");
        let files = vec!["backend/payments/src/refunds.ts".to_string()];

        let policies = markdown_candidate_policies(
            &repo,
            &discovered,
            &files,
            &["backend/payments/AGENTS.md".into()],
        );

        assert!(policies.iter().all(|policy| policy
            .source_ref
            .as_ref()
            .is_some_and(|source| source.0.contains("markdown:backend/payments/AGENTS.md"))));
        assert!(policies.iter().any(|policy| policy
            .policy
            .instructions
            .contains(&"Preserve payment invariants.".to_string())));
    }

    #[test]
    fn instruction_source_trust_matches_relative_and_absolute_paths() {
        let repo = fixture_repo("nested-instructions");
        let discovered = discover(&repo).expect("discover fixture repo");
        let payment_source = discovered
            .instruction_sources
            .iter()
            .find(|source| source.path == "backend/payments/AGENTS.md")
            .expect("payment source");

        assert!(instruction_source_is_trusted(
            &repo,
            payment_source,
            &["backend/payments".into()]
        ));
        assert!(instruction_source_is_trusted(
            &repo,
            payment_source,
            &[repo.join("backend/payments").to_string_lossy().into_owned()]
        ));
        assert!(!instruction_source_is_trusted(
            &repo,
            payment_source,
            &["backend/AGENTS.md".into()]
        ));
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

    fn write_get_policy_fixture(repo: &Path) {
        let policies_dir = repo.join(".agent-policy").join("policies");
        fs::create_dir_all(&policies_dir).expect("create policies dir");
        fs::write(
            policies_dir.join("active.yaml"),
            r#"id: org.get.active
version: 1
status: active
priority: 10
applies_when:
  paths:
    - crates/**
  languages:
    - rust
instructions:
  - Use the get metadata index when it is valid.
"#,
        )
        .expect("write active policy");
        fs::write(
            policies_dir.join("draft.yaml"),
            r#"id: org.get.draft
version: 1
status: draft
applies_when: {}
instructions:
  - Draft guidance should not appear in get bundles.
"#,
        )
        .expect("write draft policy");
    }

    fn policy_ids(policies: &[LoadedPolicy]) -> Vec<String> {
        policies
            .iter()
            .map(|loaded| loaded.policy.id.clone())
            .collect()
    }

    fn get_bundle_json(policies: &[LoadedPolicy]) -> String {
        let intent = TaskIntent {
            repo: Some("repo".to_string()),
            branch: None,
            task: Some(TaskDetails {
                summary: Some("implement indexed get".to_string()),
                task_type: None,
            }),
            files: vec!["crates/agent-policy-cli/src/main.rs".to_string()],
            detected: Some(DetectedContext {
                languages: vec!["rust".to_string()],
                frameworks: Vec::new(),
                package_manager: None,
            }),
            risk_flags: Vec::new(),
            expected_commands: Vec::new(),
            expected_check_ids: Vec::new(),
            output_budget: Some(OutputBudget {
                max_tokens: Some(2000),
                max_instructions: Some(10),
                max_required_checks: Some(10),
                max_blocked_actions: Some(10),
                include_examples: Some(false),
                include_explanations: Some("brief".to_string()),
            }),
        };
        let bundle = build_instruction_bundle(
            &intent,
            policies,
            BundleBuildOptions {
                max_tokens: Some(2000),
                max_instructions: Some(10),
                max_required_checks: Some(10),
                max_blocked_actions: Some(10),
            },
        )
        .expect("build bundle");
        render_bundle_json(&bundle).expect("render bundle json")
    }

    fn fixture_repo(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
    }

    struct TempRepo {
        path: PathBuf,
    }

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(name: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "agent-policy-cli-{name}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
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

    fn test_registry(cache_dir: &Path, requested_ref: &str, mode: SyncMode) -> RegistryConfig {
        RegistryConfig {
            registry_type: "git".to_string(),
            url: "https://example.invalid/company/registry.git".to_string(),
            r#ref: requested_ref.to_string(),
            cache_dir: cache_dir.display().to_string(),
            sync: RegistrySyncConfig {
                mode,
                max_age_minutes: None,
            },
        }
    }

    fn init_git_registry(path: &Path) -> String {
        init_git_registry_with_policy(
            path,
            "id: org.test\nversion: 1\nstatus: active\ninstructions:\n  - Test policy.\n",
        )
    }

    fn init_git_registry_with_policy(path: &Path, policy_yaml: &str) -> String {
        fs::create_dir_all(path.join("policies")).expect("create registry policy dir");
        fs::write(path.join("policies").join("policy.yaml"), policy_yaml).expect("write policy");
        git(path, &["init"]);
        git(path, &["checkout", "-b", "main"]);
        git(path, &["add", "."]);
        git(
            path,
            &[
                "-c",
                "user.name=Agent Policy Tests",
                "-c",
                "user.email=agent-policy-tests@example.invalid",
                "commit",
                "-m",
                "initial registry",
            ],
        );
        git_stdout(path, &["rev-parse", "HEAD"])
    }

    fn git(path: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {:?} failed:\nstdout:\n{}\nstderr:\n{}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn git_stdout(path: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {:?} failed:\nstdout:\n{}\nstderr:\n{}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
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
