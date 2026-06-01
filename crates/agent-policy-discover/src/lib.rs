//! Instruction source discovery.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<MarkdownInstructionCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarkdownInstructionCandidate {
    pub text: String,
    pub line: usize,
    pub candidate_type: MarkdownInstructionCandidateType,
    pub provenance: MarkdownInstructionProvenance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MarkdownInstructionCandidateType {
    Instruction,
    RequiredCheck,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarkdownInstructionProvenance {
    pub path: String,
    pub scope: String,
    #[serde(rename = "type")]
    pub source_type: InstructionSourceType,
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

        if let Some(mut source) = classify_source(relative_path) {
            let content = fs::read_to_string(absolute_path)
                .with_context(|| format!("failed to read {}", absolute_path.display()))?;
            source.candidates = extract_markdown_instruction_candidates(&source, &content);
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
        candidates: Vec::new(),
    })
}

pub fn extract_markdown_instruction_candidates(
    source: &InstructionSource,
    content: &str,
) -> Vec<MarkdownInstructionCandidate> {
    let mut candidates = Vec::new();
    let mut paragraph = Vec::<(usize, String)>::new();
    let mut in_fence = false;
    let mut command_fence = false;

    for (index, line) in content.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();

        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            flush_paragraph(source, &mut paragraph, &mut candidates);
            if in_fence {
                in_fence = false;
                command_fence = false;
            } else {
                in_fence = true;
                command_fence = is_command_fence(trimmed);
            }
            continue;
        }

        if in_fence {
            if command_fence {
                if let Some(command) = command_from_fence_line(trimmed) {
                    candidates.push(candidate(
                        source,
                        command,
                        line_number,
                        MarkdownInstructionCandidateType::RequiredCheck,
                    ));
                }
            }
            continue;
        }

        if trimmed.is_empty() || trimmed.starts_with('#') {
            flush_paragraph(source, &mut paragraph, &mut candidates);
            continue;
        }

        if let Some(text) = list_item_text(trimmed) {
            flush_paragraph(source, &mut paragraph, &mut candidates);
            candidates.push(candidate(
                source,
                text,
                line_number,
                MarkdownInstructionCandidateType::Instruction,
            ));
            continue;
        }

        paragraph.push((line_number, trimmed.to_string()));
    }

    flush_paragraph(source, &mut paragraph, &mut candidates);
    candidates
}

fn flush_paragraph(
    source: &InstructionSource,
    paragraph: &mut Vec<(usize, String)>,
    candidates: &mut Vec<MarkdownInstructionCandidate>,
) {
    if paragraph.is_empty() {
        return;
    }

    let line = paragraph[0].0;
    let text = paragraph
        .iter()
        .map(|(_, line)| line.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    paragraph.clear();

    if is_short_instruction_paragraph(&text) {
        candidates.push(candidate(
            source,
            text,
            line,
            MarkdownInstructionCandidateType::Instruction,
        ));
    }
}

fn candidate(
    source: &InstructionSource,
    text: String,
    line: usize,
    candidate_type: MarkdownInstructionCandidateType,
) -> MarkdownInstructionCandidate {
    MarkdownInstructionCandidate {
        text: normalize_instruction_text(&text),
        line,
        candidate_type,
        provenance: MarkdownInstructionProvenance {
            path: source.path.clone(),
            scope: source.scope.clone(),
            source_type: source.source_type.clone(),
        },
    }
}

fn list_item_text(trimmed: &str) -> Option<String> {
    let marker = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "));
    if let Some(text) = marker {
        return nonempty_candidate_text(text);
    }

    let marker_end = trimmed
        .char_indices()
        .take_while(|(_, character)| character.is_ascii_digit())
        .last()
        .map(|(index, character)| index + character.len_utf8())?;
    if marker_end == 0 {
        return None;
    }
    let rest = &trimmed[marker_end..];
    let rest = rest
        .strip_prefix(". ")
        .or_else(|| rest.strip_prefix(") "))?;
    nonempty_candidate_text(rest)
}

fn nonempty_candidate_text(text: &str) -> Option<String> {
    let normalized = normalize_instruction_text(text);
    (!normalized.is_empty()).then_some(normalized)
}

fn is_short_instruction_paragraph(text: &str) -> bool {
    let normalized = normalize_instruction_text(text);
    let word_count = normalized.split_whitespace().count();
    if word_count == 0 || word_count > 18 {
        return false;
    }

    let lower = normalized.to_ascii_lowercase();
    let first = lower
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_matches(|character: char| !character.is_ascii_alphabetic());

    matches!(
        first,
        "add"
            | "always"
            | "avoid"
            | "check"
            | "do"
            | "ensure"
            | "follow"
            | "keep"
            | "must"
            | "never"
            | "prefer"
            | "preserve"
            | "run"
            | "use"
            | "validate"
            | "verify"
    ) || lower.contains(" must ")
        || lower.contains(" should ")
        || lower.contains(" require ")
        || lower.contains(" requires ")
        || lower.starts_with("do not ")
        || lower.starts_with("never ")
        || lower.starts_with("always ")
}

fn is_command_fence(trimmed: &str) -> bool {
    let language = trimmed
        .trim_start_matches("```")
        .trim_start_matches("~~~")
        .trim()
        .to_ascii_lowercase();
    language.is_empty()
        || matches!(
            language.as_str(),
            "bash" | "sh" | "shell" | "zsh" | "console" | "terminal"
        )
}

fn command_from_fence_line(trimmed: &str) -> Option<String> {
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let command = trimmed.strip_prefix("$ ").unwrap_or(trimmed);
    nonempty_candidate_text(command)
}

fn normalize_instruction_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
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
    use super::{
        discover, DiscoveryResult, InstructionSourceKind, InstructionSourceType,
        MarkdownInstructionCandidateType,
    };
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
                "backend/AGENTS.md",
                "backend/payments/AGENTS.md",
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
            .find(|source| source.path == "backend/payments/AGENTS.md")
            .expect("payments source");
        assert_eq!(payments.scope, "backend/payments/**");
        assert_eq!(payments.source_type, InstructionSourceType::AgentsMd);
        assert_eq!(payments.source_kind, InstructionSourceKind::Agents);

        let root_agents = sources
            .iter()
            .find(|source| source.path == "AGENTS.md")
            .expect("root agents source");
        assert_eq!(root_agents.scope, ".");
        assert!(root_agents.is_root);
        assert!(!root_agents.is_nested);
        assert!(root_agents.candidates.iter().any(|candidate| candidate.text
            == "Use the repository policy broker configuration."
            && candidate.line == 3));

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
    fn extracts_markdown_candidates_without_raw_explanatory_paragraphs() {
        let result = discover(fixture_repo()).expect("discover fixture repo");
        let root = result
            .instruction_sources
            .iter()
            .find(|source| source.path == "AGENTS.md")
            .expect("root source");

        assert_eq!(
            root.candidates
                .iter()
                .map(|candidate| (
                    candidate.text.as_str(),
                    candidate.line,
                    &candidate.candidate_type
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    "Use the repository policy broker configuration.",
                    3,
                    &MarkdownInstructionCandidateType::Instruction,
                ),
                (
                    "Prefer focused policy changes.",
                    5,
                    &MarkdownInstructionCandidateType::Instruction,
                ),
                (
                    "Keep generated bundles concise.",
                    6,
                    &MarkdownInstructionCandidateType::Instruction,
                ),
                (
                    "cargo test",
                    13,
                    &MarkdownInstructionCandidateType::RequiredCheck,
                ),
            ]
        );
        assert!(!root
            .candidates
            .iter()
            .any(|candidate| candidate.text.contains("several examples")));
        assert!(root.candidates.iter().all(|candidate| {
            candidate.provenance.path == "AGENTS.md"
                && candidate.provenance.scope == "."
                && candidate.provenance.source_type == InstructionSourceType::AgentsMd
        }));
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
    fn json_output_includes_extracted_candidates() {
        let result = discover(fixture_repo()).expect("discover fixture repo");
        let json = serde_json::to_string_pretty(&result).expect("serialize result");

        assert!(json.contains("\"candidates\""));
        assert!(json.contains("\"text\": \"Preserve payment invariants.\""));
        assert!(json.contains("\"line\": 3"));
        assert!(json.contains("\"scope\": \"backend/payments/**\""));
        assert!(json.contains("\"type\": \"agents_md\""));
    }

    fn fixture_repo() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/nested-instructions")
    }
}
