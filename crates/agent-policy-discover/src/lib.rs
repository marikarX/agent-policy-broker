//! Instruction source discovery.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveryResult {
    pub instruction_sources: Vec<InstructionSource>,
}

impl DiscoveryResult {
    pub fn empty() -> Self {
        Self {
            instruction_sources: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstructionSource {
    pub path: String,
    pub scope: String,
    #[serde(rename = "type")]
    pub source_type: InstructionSourceType,
    pub source_kind: InstructionSourceKind,
    pub is_root: bool,
    pub is_nested: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstructionSourceType {
    AgentsMd,
    ClaudeMd,
    CopilotInstructions,
    CursorRule,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstructionSourceKind {
    Agents,
    Claude,
    Copilot,
    Cursor,
}

pub fn discover_json(repo: impl AsRef<Path>) -> Result<String> {
    let result = discover(repo)?;
    serde_json::to_string_pretty(&result).context("failed to serialize discovery result")
}

pub fn discover(repo: impl AsRef<Path>) -> Result<DiscoveryResult> {
    let repo = repo.as_ref();
    let mut instruction_sources = Vec::new();

    for entry in WalkDir::new(repo)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !is_excluded_entry(entry))
    {
        let entry = entry.with_context(|| format!("failed to walk {}", repo.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }

        let absolute_path = entry.path();
        let relative_path = absolute_path
            .strip_prefix(repo)
            .with_context(|| format!("failed to relativize {}", absolute_path.display()))?;

        if let Some(source) = classify_source(relative_path) {
            instruction_sources.push(source);
        }
    }

    instruction_sources.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(DiscoveryResult {
        instruction_sources,
    })
}

fn is_excluded_entry(entry: &DirEntry) -> bool {
    if entry.depth() == 0 {
        return false;
    }

    matches!(
        entry.file_name().to_str(),
        Some("node_modules" | "target" | ".git" | "vendor")
    )
}

fn classify_source(relative_path: &Path) -> Option<InstructionSource> {
    let file_name = relative_path.file_name()?.to_str()?;
    let (source_type, source_kind, scope_dir) = match file_name {
        "AGENTS.md" => (
            InstructionSourceType::AgentsMd,
            InstructionSourceKind::Agents,
            relative_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default(),
        ),
        "CLAUDE.md" => (
            InstructionSourceType::ClaudeMd,
            InstructionSourceKind::Claude,
            relative_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default(),
        ),
        "copilot-instructions.md"
            if relative_path == Path::new(".github/copilot-instructions.md") =>
        {
            (
                InstructionSourceType::CopilotInstructions,
                InstructionSourceKind::Copilot,
                PathBuf::new(),
            )
        }
        _ if is_cursor_rule(relative_path) => (
            InstructionSourceType::CursorRule,
            InstructionSourceKind::Cursor,
            cursor_scope_dir(relative_path),
        ),
        _ => return None,
    };

    let scope = scope_for_dir(&scope_dir);
    let is_root = scope == ".";
    Some(InstructionSource {
        path: normalize_path(relative_path),
        scope,
        source_type,
        source_kind,
        is_root,
        is_nested: !is_root,
    })
}

fn is_cursor_rule(relative_path: &Path) -> bool {
    let components = components(relative_path);
    components.len() > 2
        && components
            .windows(2)
            .any(|parts| parts == [".cursor", "rules"])
}

fn cursor_scope_dir(relative_path: &Path) -> PathBuf {
    let mut scope = PathBuf::new();
    for component in relative_path.components() {
        match component {
            Component::Normal(part) if part == ".cursor" => break,
            Component::Normal(part) => scope.push(part),
            _ => {}
        }
    }
    scope
}

fn scope_for_dir(scope_dir: &Path) -> String {
    if scope_dir.as_os_str().is_empty() {
        ".".to_string()
    } else {
        format!("{}/**", normalize_path(scope_dir))
    }
}

fn normalize_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{discover, DiscoveryResult, InstructionSourceKind, InstructionSourceType};
    use std::path::PathBuf;

    #[test]
    fn empty_result_has_no_sources() {
        assert!(DiscoveryResult::empty().instruction_sources.is_empty());
    }

    #[test]
    fn discovers_instruction_sources_with_scopes() {
        let result = discover(fixture_repo()).expect("discover fixture repo");
        let sources = result.instruction_sources;

        assert_eq!(
            sources
                .iter()
                .map(|source| source.path.as_str())
                .collect::<Vec<_>>(),
            vec![
                ".cursor/rules/root.md",
                ".github/copilot-instructions.md",
                "AGENTS.md",
                "CLAUDE.md",
                "backend/AGENTS.md",
                "backend/payments/CLAUDE.md",
                "frontend/.cursor/rules/react.md",
            ]
        );

        let backend = sources
            .iter()
            .find(|source| source.path == "backend/AGENTS.md")
            .expect("backend source");
        assert_eq!(backend.scope, "backend/**");
        assert_eq!(backend.source_type, InstructionSourceType::AgentsMd);
        assert_eq!(backend.source_kind, InstructionSourceKind::Agents);
        assert!(!backend.is_root);
        assert!(backend.is_nested);

        let payments = sources
            .iter()
            .find(|source| source.path == "backend/payments/CLAUDE.md")
            .expect("payments source");
        assert_eq!(payments.scope, "backend/payments/**");
        assert_eq!(payments.source_type, InstructionSourceType::ClaudeMd);
        assert_eq!(payments.source_kind, InstructionSourceKind::Claude);

        let root_agents = sources
            .iter()
            .find(|source| source.path == "AGENTS.md")
            .expect("root agents source");
        assert_eq!(root_agents.scope, ".");
        assert!(root_agents.is_root);
        assert!(!root_agents.is_nested);

        let cursor = sources
            .iter()
            .find(|source| source.path == "frontend/.cursor/rules/react.md")
            .expect("frontend cursor source");
        assert_eq!(cursor.scope, "frontend/**");
        assert_eq!(cursor.source_type, InstructionSourceType::CursorRule);
        assert_eq!(cursor.source_kind, InstructionSourceKind::Cursor);

        let copilot = sources
            .iter()
            .find(|source| source.path == ".github/copilot-instructions.md")
            .expect("copilot source");
        assert_eq!(copilot.scope, ".");
        assert_eq!(
            copilot.source_type,
            InstructionSourceType::CopilotInstructions
        );
        assert_eq!(copilot.source_kind, InstructionSourceKind::Copilot);
    }

    #[test]
    fn skips_ignored_directories() {
        let result = discover(fixture_repo()).expect("discover fixture repo");
        let paths = result
            .instruction_sources
            .iter()
            .map(|source| source.path.as_str())
            .collect::<Vec<_>>();

        assert!(!paths.iter().any(|path| path.starts_with("node_modules/")));
        assert!(!paths.iter().any(|path| path.starts_with("target/")));
        assert!(!paths.iter().any(|path| path.starts_with(".git/")));
        assert!(!paths.iter().any(|path| path.starts_with("vendor/")));
    }

    #[test]
    fn json_output_is_stable() {
        let result = discover(fixture_repo()).expect("discover fixture repo");
        let json = serde_json::to_string_pretty(&result).expect("serialize result");

        assert_eq!(
            json,
            include_str!("../tests/fixtures/discover.expected.json").trim_end()
        );
    }

    fn fixture_repo() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/repo")
    }
}
