use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use agent_policy_config::{AgentPolicyConfig, IndexConfig, RegistryConfig};
use agent_policy_core::{
    load_policies_from_dirs, load_policies_from_registry, AppliesWhen, LoadedPolicy, PolicyStatus,
    PolicyVersion, RegistryLoadOptions,
};
use agent_policy_discover::{discover, MarkdownInstructionCandidateType};
use anyhow::Context;
use globset::{Glob, GlobSet, GlobSetBuilder};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Schema, Value, STORED, STRING, TEXT};
use tantivy::{doc, Index, TantivyDocument};
use walkdir::{DirEntry, WalkDir};

use crate::git::{git_rev_parse, is_git_worktree};
use crate::paths::resolve_configured_path;

const INDEX_SCHEMA_VERSION: u32 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IndexBuildReport {
    pub(crate) source: IndexSource,
    pub(crate) index_dir: PathBuf,
    pub(crate) metadata_path: PathBuf,
    pub(crate) fulltext_path: PathBuf,
    pub(crate) manifest_path: PathBuf,
    pub(crate) policy_count: usize,
    pub(crate) fulltext_document_count: usize,
    pub(crate) stale_before_build: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IndexSource {
    pub(crate) kind: IndexSourceKind,
    pub(crate) name: String,
    root: PathBuf,
    url: Option<String>,
    requested_ref: Option<String>,
    pub(crate) commit: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IndexSourceKind {
    Registry,
    Repo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct IndexManifest {
    pub(crate) schema_version: u32,
    pub(crate) source: IndexManifestSource,
    pub(crate) indexes: IndexManifestIndexes,
    pub(crate) created_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct IndexManifestSource {
    pub(crate) kind: IndexSourceKind,
    pub(crate) name: String,
    pub(crate) path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) url: Option<String>,
    #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
    pub(crate) requested_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) commit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct IndexManifestIndexes {
    pub(crate) metadata: String,
    pub(crate) fulltext: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FullTextCandidate {
    pub(crate) id: String,
}

#[derive(Clone, Copy)]
struct FullTextFields {
    id: Field,
    kind: Field,
    content: Field,
    source_path: Field,
    source_line: Field,
}

pub(crate) fn build_metadata_index(
    repo: &Path,
    config: &AgentPolicyConfig,
) -> anyhow::Result<IndexBuildReport> {
    build_metadata_index_with_cache_dir(repo, config, &agent_policy_cache_dir()?)
}

pub(crate) fn build_metadata_index_with_cache_dir(
    repo: &Path,
    config: &AgentPolicyConfig,
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
    let fulltext_path = index_dir.join("fulltext");
    let manifest_path = index_dir.join("manifest.json");
    let stale_before_build = read_index_manifest(&manifest_path)?
        .as_ref()
        .is_some_and(|manifest| index_manifest_is_stale(manifest, &source));

    fs::create_dir_all(&index_dir)?;
    write_metadata_sqlite(&metadata_path, &policies, source.commit.as_deref())?;
    let fulltext_document_count = write_fulltext_index(
        &fulltext_path,
        &source.root,
        config,
        &policies,
        source.kind == IndexSourceKind::Repo,
    )?;
    let manifest = index_manifest(&source)?;
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    fs::write(&manifest_path, format!("{manifest_json}\n"))?;

    Ok(IndexBuildReport {
        source,
        index_dir,
        metadata_path,
        fulltext_path,
        manifest_path,
        policy_count: policies.len(),
        fulltext_document_count,
        stale_before_build,
    })
}

pub(crate) fn index_registry_source(
    repo: &Path,
    registry: &RegistryConfig,
) -> anyhow::Result<IndexSource> {
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

pub(crate) fn index_repo_source(repo: &Path) -> anyhow::Result<IndexSource> {
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

pub(crate) fn agent_policy_cache_dir() -> anyhow::Result<PathBuf> {
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

pub(crate) fn read_index_manifest(path: &Path) -> anyhow::Result<Option<IndexManifest>> {
    match fs::read_to_string(path) {
        Ok(raw) => Ok(Some(serde_json::from_str(&raw).map_err(|error| {
            anyhow::anyhow!("failed to parse index manifest {}: {error}", path.display())
        })?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(anyhow::Error::from(error))
            .with_context(|| format!("failed to read index manifest {}", path.display())),
    }
}

pub(crate) fn index_manifest_is_stale(manifest: &IndexManifest, source: &IndexSource) -> bool {
    manifest.schema_version != INDEX_SCHEMA_VERSION
        || manifest.source.kind != source.kind
        || manifest.source.name != source.name
        || manifest.source.path != source.root.display().to_string()
        || manifest.source.commit != source.commit
}

pub(crate) fn get_indexed_policy_ids(
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

pub(crate) fn search_fulltext_candidates(
    cache_dir: &Path,
    source: &IndexSource,
    query: &str,
    limit: usize,
    warnings: &mut Vec<String>,
) -> anyhow::Result<Vec<FullTextCandidate>> {
    if query.split_whitespace().next().is_none() || limit == 0 {
        return Ok(Vec::new());
    }

    let index_dir = index_dir_for_source(cache_dir, &source.name);
    let manifest_path = index_dir.join("manifest.json");
    let manifest = match read_index_manifest(&manifest_path) {
        Ok(Some(manifest)) => manifest,
        Ok(None) => {
            warnings.push(format!(
                "Full-text index missing at {}; skipped BM25 candidate guidance.",
                manifest_path.display()
            ));
            return Ok(Vec::new());
        }
        Err(error) => {
            warnings.push(format!(
                "Full-text index manifest at {} is invalid or unreadable ({error:#}); skipped BM25 candidate guidance.",
                manifest_path.display()
            ));
            return Ok(Vec::new());
        }
    };

    if index_manifest_is_stale(&manifest, source) {
        warnings.push(format!(
            "Full-text index at {} is stale; skipped BM25 candidate guidance.",
            index_dir.display()
        ));
        return Ok(Vec::new());
    }

    let fulltext_path = index_dir.join(&manifest.indexes.fulltext);
    match read_fulltext_candidates(&fulltext_path, query, limit) {
        Ok(candidates) => Ok(candidates),
        Err(error) => {
            warnings.push(format!(
                "Full-text index at {} is invalid or unreadable ({error:#}); skipped BM25 candidate guidance.",
                fulltext_path.display()
            ));
            Ok(Vec::new())
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

fn index_manifest(source: &IndexSource) -> anyhow::Result<IndexManifest> {
    Ok(IndexManifest {
        schema_version: INDEX_SCHEMA_VERSION,
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
            fulltext: "fulltext".to_string(),
        },
        created_at_unix: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
    })
}

fn write_fulltext_index(
    path: &Path,
    source_root: &Path,
    config: &AgentPolicyConfig,
    policies: &[LoadedPolicy],
    include_source_documents: bool,
) -> anyhow::Result<usize> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path)?;

    let (schema, fields) = fulltext_schema();
    let index = Index::create_in_dir(path, schema)?;
    let mut writer = index.writer(50_000_000)?;
    let mut count = 0usize;

    for loaded in policies {
        let policy = &loaded.policy;
        for instruction in &policy.instructions {
            writer.add_document(fulltext_document(
                &fields,
                &policy.id,
                "policy_instruction",
                instruction,
                loaded.source_path.display().to_string(),
                None,
            ))?;
            count += 1;
        }
        if let Some(retrieval) = &policy.retrieval {
            for term in &retrieval.semantic_terms {
                writer.add_document(fulltext_document(
                    &fields,
                    &policy.id,
                    "policy_semantic_term",
                    term,
                    loaded.source_path.display().to_string(),
                    None,
                ))?;
                count += 1;
            }
        }
    }

    if include_source_documents {
        for source in discover(source_root)?.instruction_sources {
            for candidate in source.candidates {
                let kind = match candidate.candidate_type {
                    MarkdownInstructionCandidateType::Instruction => "markdown_instruction",
                    MarkdownInstructionCandidateType::RequiredCheck => "markdown_required_check",
                };
                writer.add_document(fulltext_document(
                    &fields,
                    &markdown_candidate_id(&candidate.provenance.path, candidate.line),
                    kind,
                    &candidate.text,
                    candidate.provenance.path,
                    Some(candidate.line),
                ))?;
                count += 1;
            }
        }

        for selected_doc in selected_docs(source_root, &config.index)? {
            writer.add_document(fulltext_document(
                &fields,
                &format!("doc:{}", selected_doc.relative_path),
                "selected_doc",
                &selected_doc.content,
                selected_doc.relative_path,
                None,
            ))?;
            count += 1;
        }
    }

    writer.commit()?;
    Ok(count)
}

fn read_fulltext_candidates(
    path: &Path,
    query: &str,
    limit: usize,
) -> anyhow::Result<Vec<FullTextCandidate>> {
    let index = Index::open_in_dir(path)?;
    let schema = index.schema();
    let id = schema
        .get_field("id")
        .map_err(|error| anyhow::anyhow!("full-text schema missing id field: {error}"))?;
    let content = schema
        .get_field("content")
        .map_err(|error| anyhow::anyhow!("full-text schema missing content field: {error}"))?;
    let reader = index.reader()?;
    let searcher = reader.searcher();
    let parser = QueryParser::for_index(&index, vec![content]);
    let parsed = parser.parse_query(query)?;
    let top_docs = searcher.search(&parsed, &TopDocs::with_limit(limit.saturating_mul(3)))?;
    let mut seen = BTreeSet::new();
    let mut candidates = Vec::new();

    for (_score, address) in top_docs {
        let document: TantivyDocument = searcher.doc(address)?;
        let Some(value) = document.get_first(id).and_then(|value| value.as_str()) else {
            continue;
        };
        if seen.insert(value.to_string()) {
            candidates.push(FullTextCandidate {
                id: value.to_string(),
            });
        }
        if candidates.len() >= limit {
            break;
        }
    }

    Ok(candidates)
}

fn fulltext_schema() -> (Schema, FullTextFields) {
    let mut builder = Schema::builder();
    let id = builder.add_text_field("id", STRING | STORED);
    let kind = builder.add_text_field("kind", STRING | STORED);
    let content = builder.add_text_field("content", TEXT | STORED);
    let source_path = builder.add_text_field("source_path", STRING | STORED);
    let source_line = builder.add_u64_field("source_line", STORED);
    let schema = builder.build();
    (
        schema,
        FullTextFields {
            id,
            kind,
            content,
            source_path,
            source_line,
        },
    )
}

fn fulltext_document(
    fields: &FullTextFields,
    id: &str,
    kind: &str,
    content: &str,
    source_path: String,
    source_line: Option<usize>,
) -> TantivyDocument {
    let mut document = doc!(
        fields.id => id.to_string(),
        fields.kind => kind.to_string(),
        fields.content => content.to_string(),
        fields.source_path => source_path,
    );
    if let Some(line) = source_line {
        document.add_u64(fields.source_line, line as u64);
    }
    document
}

#[derive(Debug)]
struct SelectedDoc {
    relative_path: String,
    content: String,
}

fn selected_docs(root: &Path, config: &IndexConfig) -> anyhow::Result<Vec<SelectedDoc>> {
    if config.include.is_empty() {
        return Ok(Vec::new());
    }

    let include = glob_set(&config.include)?;
    let exclude = glob_set(&config.exclude)?;
    let mut docs = Vec::new();

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !is_excluded_doc_entry(entry, root, &exclude))
    {
        let entry = entry.with_context(|| format!("failed to walk {}", root.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(root)
            .with_context(|| format!("failed to relativize {}", entry.path().display()))?;
        if !include.is_match(relative) || exclude.is_match(relative) {
            continue;
        }
        let content = fs::read_to_string(entry.path())
            .with_context(|| format!("failed to read selected doc {}", entry.path().display()))?;
        docs.push(SelectedDoc {
            relative_path: normalize_path(relative),
            content,
        });
    }

    docs.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(docs)
}

fn is_excluded_doc_entry(entry: &DirEntry, root: &Path, exclude: &GlobSet) -> bool {
    if entry.depth() == 0 {
        return false;
    }
    match entry.path().strip_prefix(root) {
        Ok(relative) => exclude.is_match(relative),
        Err(_) => false,
    }
}

fn glob_set(patterns: &[String]) -> anyhow::Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern)?);
    }
    Ok(builder.build()?)
}

fn markdown_candidate_id(path: &str, line: usize) -> String {
    let normalized = path
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '.'
            }
        })
        .collect::<String>()
        .split('.')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(".");
    format!("markdown.{normalized}.{line}")
}

fn normalize_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
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

pub(crate) fn render_index_report_json(report: &IndexBuildReport) -> String {
    format!(
        "{{\n  \"status\": \"ok\",\n  \"source\": {{\n    \"kind\": \"{}\",\n    \"name\": \"{}\",\n    \"commit\": {}\n  }},\n  \"index_dir\": \"{}\",\n  \"metadata\": \"{}\",\n  \"fulltext\": \"{}\",\n  \"manifest\": \"{}\",\n  \"policy_count\": {},\n  \"fulltext_document_count\": {},\n  \"stale_before_build\": {}\n}}\n",
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
        json_escape(&report.fulltext_path.display().to_string()),
        json_escape(&report.manifest_path.display().to_string()),
        report.policy_count,
        report.fulltext_document_count,
        report.stale_before_build
    )
}

pub(crate) fn render_index_report_markdown(report: &IndexBuildReport) -> String {
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
        "- Full-text documents: `{}`\n",
        report.fulltext_document_count
    ));
    out.push_str(&format!(
        "- Metadata: `{}`\n",
        report.metadata_path.display()
    ));
    out.push_str(&format!(
        "- Full-text: `{}`\n",
        report.fulltext_path.display()
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

fn markdown_inline(text: &str) -> String {
    text.replace('`', "\\`").replace(['\n', '\r'], " ")
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
