use std::path::{Path, PathBuf};

use agent_policy_config::{load_config, load_config_from_path, RegistryConfig, SyncMode};

use crate::cli::{GlobalArgs, OutputFormat};
use crate::git::{git_rev_parse, is_full_sha, is_git_worktree};
use crate::paths::resolve_configured_path;
use crate::render::{json_escape, markdown_inline};

use super::get::looks_like_remote_git_url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegistrySyncReport {
    pub(crate) cache_dir: PathBuf,
    pub(crate) mode: SyncMode,
    pub(crate) status: RegistrySyncStatus,
    pub(crate) commit: Option<String>,
    pub(crate) requested_ref: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RegistrySyncStatus {
    LocalPath,
    Cached,
    Offline,
    Pinned,
}

pub(crate) fn run_sync(global: &GlobalArgs) -> anyhow::Result<()> {
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

pub(crate) fn sync_registry(
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

pub(crate) fn render_registry_sync_json(report: &RegistrySyncReport) -> String {
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

pub(crate) fn render_registry_sync_markdown(report: &RegistrySyncReport) -> String {
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
