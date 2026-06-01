//! Instruction source discovery.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

pub const CODEX_DEFAULT_PROJECT_DOC_MAX_BYTES: usize = 32_768;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveryResult {
    pub instruction_sources: Vec<InstructionSource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub omissions: Vec<DiscoveryOmission>,
}

impl DiscoveryResult {
    pub fn empty() -> Self {
        Self {
            instruction_sources: Vec::new(),
            omissions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveryOmission {
    pub path: String,
    pub reason: DiscoveryOmissionReason,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryOmissionReason {
    Empty,
    Shadowed,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes_read: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_bytes: Option<usize>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub truncated: bool,
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

pub fn discover_codex_json(
    repo: impl AsRef<Path>,
    options: CodexDiscoveryOptions,
) -> Result<String> {
    let result = discover_codex(repo, options)?;
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
        omissions: Vec::new(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexDiscoveryOptions {
    pub codex_home: Option<PathBuf>,
    pub current_dir: Option<PathBuf>,
    pub project_doc_fallback_filenames: Vec<String>,
    pub project_doc_max_bytes: usize,
    pub include_global: bool,
}

impl Default for CodexDiscoveryOptions {
    fn default() -> Self {
        Self {
            codex_home: None,
            current_dir: None,
            project_doc_fallback_filenames: Vec::new(),
            project_doc_max_bytes: CODEX_DEFAULT_PROJECT_DOC_MAX_BYTES,
            include_global: false,
        }
    }
}

pub fn discover_codex(
    repo: impl AsRef<Path>,
    options: CodexDiscoveryOptions,
) -> Result<DiscoveryResult> {
    let repo = repo.as_ref();
    let repo_abs =
        absolute_path(repo).with_context(|| format!("failed to resolve {}", repo.display()))?;
    let repo_real = fs::canonicalize(&repo_abs)
        .with_context(|| format!("failed to canonicalize {}", repo_abs.display()))?;
    let current_abs = match options.current_dir {
        Some(current_dir) if current_dir.is_absolute() => current_dir,
        Some(current_dir) => repo_abs.join(current_dir),
        None => repo_abs.clone(),
    };
    let current_real = fs::canonicalize(&current_abs)
        .with_context(|| format!("failed to canonicalize {}", current_abs.display()))?;
    let current_relative = current_real.strip_prefix(&repo_real).with_context(|| {
        format!(
            "codex current_dir {} must be inside project root {}",
            current_abs.display(),
            repo_abs.display()
        )
    })?;
    let max_bytes = options.project_doc_max_bytes;
    let mut result = DiscoveryResult::empty();

    if options.include_global {
        let codex_home = options.codex_home.unwrap_or_else(default_codex_home);
        let global_candidates = [
            codex_home.join("AGENTS.override.md"),
            codex_home.join("AGENTS.md"),
        ];
        choose_codex_source(
            &global_candidates,
            None,
            &codex_home,
            max_bytes,
            &mut result,
        )?;
    }

    for dir in codex_directory_chain(current_relative) {
        let absolute_dir = repo_real.join(&dir);
        let candidates =
            codex_project_candidates(&absolute_dir, &options.project_doc_fallback_filenames);
        choose_codex_source(
            &candidates,
            Some(&repo_real),
            &absolute_dir,
            max_bytes,
            &mut result,
        )?;
    }

    Ok(result)
}

fn choose_codex_source(
    candidates: &[PathBuf],
    repo_root: Option<&Path>,
    scope_dir: &Path,
    max_bytes: usize,
    result: &mut DiscoveryResult,
) -> Result<()> {
    let mut selected = false;
    for candidate in candidates {
        let Some(read_path) = safe_instruction_path(candidate, repo_root)? else {
            continue;
        };
        let file = read_instruction_file(&read_path, max_bytes)
            .with_context(|| format!("failed to read {}", candidate.display()))?;
        if selected {
            result.omissions.push(DiscoveryOmission {
                path: display_source_path(candidate, repo_root),
                reason: DiscoveryOmissionReason::Shadowed,
                bytes: Some(file.original_bytes),
            });
            continue;
        }
        if file.content.trim().is_empty() {
            result.omissions.push(DiscoveryOmission {
                path: display_source_path(candidate, repo_root),
                reason: DiscoveryOmissionReason::Empty,
                bytes: Some(file.original_bytes),
            });
            continue;
        }

        let relative_scope_dir = match repo_root {
            Some(root) => scope_dir.strip_prefix(root).unwrap_or(scope_dir),
            None => Path::new(""),
        };
        let mut source = agents_source(
            display_source_path(candidate, repo_root),
            scope_for_dir(relative_scope_dir),
        );
        source.bytes_read = Some(file.bytes_read);
        source.original_bytes = Some(file.original_bytes);
        source.truncated = file.truncated;
        source.candidates = extract_markdown_instruction_candidates(&source, &file.content);
        result.instruction_sources.push(source);
        selected = true;
    }
    Ok(())
}

struct InstructionFile {
    content: String,
    original_bytes: usize,
    bytes_read: usize,
    truncated: bool,
}

fn safe_instruction_path(path: &Path, repo_root: Option<&Path>) -> Result<Option<PathBuf>> {
    let metadata = match path.symlink_metadata() {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file() {
        return Ok(None);
    }

    let canonical = fs::canonicalize(path)
        .with_context(|| format!("failed to canonicalize {}", path.display()))?;
    if let Some(root) = repo_root {
        if !canonical.starts_with(root) {
            return Ok(None);
        }
    }

    Ok(Some(canonical))
}

fn read_instruction_file(path: &Path, max_bytes: usize) -> Result<InstructionFile> {
    let original_bytes = path.metadata()?.len().try_into().unwrap_or(usize::MAX);
    let truncated = original_bytes > max_bytes;
    let mut file = File::open(path)?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(max_bytes as u64)
        .read_to_end(&mut bytes)?;
    let bytes_read = bytes.len();
    let content = String::from_utf8_lossy(&bytes).into_owned();
    Ok(InstructionFile {
        content,
        original_bytes,
        bytes_read,
        truncated,
    })
}

fn codex_project_candidates(dir: &Path, fallback_names: &[String]) -> Vec<PathBuf> {
    let mut candidates = vec![dir.join("AGENTS.override.md"), dir.join("AGENTS.md")];
    candidates.extend(
        fallback_names
            .iter()
            .filter_map(|name| safe_fallback_filename(name).map(|filename| dir.join(filename))),
    );
    candidates
}

fn safe_fallback_filename(name: &str) -> Option<&Path> {
    if name.trim().is_empty() {
        return None;
    }

    let path = Path::new(name);
    let mut components = path.components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Some(path),
        _ => None,
    }
}

fn codex_directory_chain(current_relative: &Path) -> Vec<PathBuf> {
    let mut chain = vec![PathBuf::new()];
    let mut cursor = PathBuf::new();
    for component in current_relative.components() {
        if let Component::Normal(part) = component {
            cursor.push(part);
            chain.push(cursor.clone());
        }
    }
    chain
}

fn agents_source(path: String, scope: String) -> InstructionSource {
    let is_root = scope == ".";
    InstructionSource {
        path,
        scope,
        source_type: InstructionSourceType::AgentsMd,
        source_kind: InstructionSourceKind::Agents,
        is_root,
        is_nested: !is_root,
        bytes_read: None,
        original_bytes: None,
        truncated: false,
        candidates: Vec::new(),
    }
}

fn display_source_path(path: &Path, repo_root: Option<&Path>) -> String {
    match repo_root.and_then(|root| path.strip_prefix(root).ok()) {
        Some(relative) => normalize_path(relative),
        None => path.display().to_string(),
    }
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(env::current_dir()?.join(path))
    }
}

fn default_codex_home() -> PathBuf {
    env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")))
        .unwrap_or_else(|| PathBuf::from(".codex"))
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
        bytes_read: None,
        original_bytes: None,
        truncated: false,
        candidates: Vec::new(),
    })
}

fn is_false(value: &bool) -> bool {
    !*value
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
        discover, discover_codex, CodexDiscoveryOptions, DiscoveryOmissionReason, DiscoveryResult,
        InstructionSourceKind, InstructionSourceType, MarkdownInstructionCandidateType,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

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

    #[test]
    fn codex_project_walk_uses_override_fallback_and_current_dir_chain() {
        let repo = create_temp_dir("codex-chain");
        fs::create_dir_all(repo.join("backend/payments")).expect("create payments");
        fs::create_dir_all(repo.join("backend/search")).expect("create sibling");
        fs::write(repo.join("AGENTS.md"), "- Root generic.\n").expect("write root agents");
        fs::write(repo.join("AGENTS.override.md"), "- Root override.\n")
            .expect("write root override");
        fs::write(repo.join("backend/AGENTS.md"), "- Backend agents.\n")
            .expect("write backend agents");
        fs::write(repo.join("backend/CUSTOM.md"), "- Backend fallback.\n")
            .expect("write backend fallback");
        fs::write(
            repo.join("backend/payments/CUSTOM.md"),
            "- Payments fallback.\n",
        )
        .expect("write payments fallback");
        fs::write(repo.join("backend/search/AGENTS.md"), "- Sibling agents.\n")
            .expect("write sibling agents");

        let result = discover_codex(
            &repo,
            CodexDiscoveryOptions {
                current_dir: Some(PathBuf::from("backend/payments")),
                project_doc_fallback_filenames: vec!["CUSTOM.md".to_string()],
                ..CodexDiscoveryOptions::default()
            },
        )
        .expect("discover codex");

        assert_eq!(
            result
                .instruction_sources
                .iter()
                .map(|source| source.path.as_str())
                .collect::<Vec<_>>(),
            vec![
                "AGENTS.override.md",
                "backend/AGENTS.md",
                "backend/payments/CUSTOM.md"
            ]
        );
        assert_eq!(
            result
                .instruction_sources
                .iter()
                .map(|source| source.scope.as_str())
                .collect::<Vec<_>>(),
            vec![".", "backend/**", "backend/payments/**"]
        );
        assert!(result
            .omissions
            .iter()
            .any(|omission| omission.path == "AGENTS.md"
                && omission.reason == DiscoveryOmissionReason::Shadowed));
        assert!(result
            .omissions
            .iter()
            .any(|omission| omission.path == "backend/CUSTOM.md"
                && omission.reason == DiscoveryOmissionReason::Shadowed));
        assert!(!result
            .instruction_sources
            .iter()
            .any(|source| source.path.starts_with("backend/search/")));
    }

    #[test]
    fn codex_global_instructions_are_included_when_requested() {
        let repo = create_temp_dir("codex-global-repo");
        let codex_home = create_temp_dir("codex-home");
        fs::write(repo.join("AGENTS.md"), "- Project root.\n").expect("write project");
        fs::write(codex_home.join("AGENTS.md"), "- Global root.\n").expect("write global");

        let result = discover_codex(
            &repo,
            CodexDiscoveryOptions {
                codex_home: Some(codex_home.clone()),
                include_global: true,
                ..CodexDiscoveryOptions::default()
            },
        )
        .expect("discover codex");

        assert_eq!(result.instruction_sources.len(), 2);
        assert_eq!(
            result.instruction_sources[0].path,
            codex_home.join("AGENTS.md").display().to_string()
        );
        assert_eq!(result.instruction_sources[1].path, "AGENTS.md");
    }

    #[test]
    fn codex_skips_empty_files_and_reports_truncation() {
        let repo = create_temp_dir("codex-empty-truncated");
        fs::create_dir_all(repo.join("src")).expect("create src");
        fs::write(repo.join("AGENTS.override.md"), "\n \n").expect("write empty override");
        fs::write(repo.join("AGENTS.md"), "- Root agents.\n").expect("write root agents");
        fs::write(
            repo.join("src/AGENTS.md"),
            "- Always keep this guidance visible.\n",
        )
        .expect("write nested agents");

        let result = discover_codex(
            &repo,
            CodexDiscoveryOptions {
                current_dir: Some(repo.join("src")),
                project_doc_max_bytes: 12,
                ..CodexDiscoveryOptions::default()
            },
        )
        .expect("discover codex");

        assert_eq!(
            result
                .instruction_sources
                .iter()
                .map(|source| source.path.as_str())
                .collect::<Vec<_>>(),
            vec!["AGENTS.md", "src/AGENTS.md"]
        );
        assert!(result
            .omissions
            .iter()
            .any(|omission| omission.path == "AGENTS.override.md"
                && omission.reason == DiscoveryOmissionReason::Empty));
        let nested = result
            .instruction_sources
            .iter()
            .find(|source| source.path == "src/AGENTS.md")
            .expect("nested source");
        assert!(nested.truncated);
        assert_eq!(nested.bytes_read, Some(12));
        assert_eq!(nested.original_bytes, Some(37));
    }

    #[test]
    fn codex_rejects_fallback_paths_outside_repo() {
        let repo = create_temp_dir("codex-fallback-escape");
        let outside = create_temp_dir("codex-fallback-outside");
        let outside_file = outside.join("secret.md");
        fs::write(&outside_file, "- Outside secret.\n").expect("write outside secret");
        fs::write(repo.join("SAFE.md"), "- Safe fallback.\n").expect("write safe fallback");

        let result = discover_codex(
            &repo,
            CodexDiscoveryOptions {
                project_doc_fallback_filenames: vec![
                    outside_file.display().to_string(),
                    "../secret.md".to_string(),
                    "SAFE.md".to_string(),
                ],
                ..CodexDiscoveryOptions::default()
            },
        )
        .expect("discover codex");

        assert_eq!(result.instruction_sources.len(), 1);
        assert_eq!(result.instruction_sources[0].path, "SAFE.md");
        assert!(result.instruction_sources[0]
            .candidates
            .iter()
            .any(|candidate| candidate.text == "Safe fallback."));
        assert!(!serde_json::to_string(&result)
            .expect("serialize result")
            .contains("Outside secret"));
    }

    #[test]
    fn codex_rejects_symlinked_project_sources_outside_repo() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let repo = create_temp_dir("codex-symlink-escape");
            let outside = create_temp_dir("codex-symlink-outside");
            fs::write(outside.join("AGENTS.md"), "- Outside symlink secret.\n")
                .expect("write outside agents");
            symlink(outside.join("AGENTS.md"), repo.join("AGENTS.md")).expect("create symlink");

            let result =
                discover_codex(&repo, CodexDiscoveryOptions::default()).expect("discover codex");

            assert!(result.instruction_sources.is_empty());
            assert!(!serde_json::to_string(&result)
                .expect("serialize result")
                .contains("Outside symlink secret"));
        }
    }

    #[test]
    fn codex_current_dir_symlink_must_stay_inside_repo() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let repo = create_temp_dir("codex-current-symlink-repo");
            let outside = create_temp_dir("codex-current-symlink-outside");
            symlink(&outside, repo.join("linked-outside")).expect("create current dir symlink");

            let error = discover_codex(
                &repo,
                CodexDiscoveryOptions {
                    current_dir: Some(PathBuf::from("linked-outside")),
                    ..CodexDiscoveryOptions::default()
                },
            )
            .expect_err("current_dir symlink should be rejected");

            assert!(error.to_string().contains("must be inside project root"));
        }
    }

    fn fixture_repo() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/nested-instructions")
    }

    fn create_temp_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "agent-policy-discover-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("temp dir should be created");
        path
    }
}
