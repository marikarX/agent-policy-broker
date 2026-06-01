//! Configuration loading for Agent Policy Broker.

use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};

pub const REPO_CONFIG_FILE_NAME: &str = ".agent-policy.yaml";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AgentPolicyConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry: Option<RegistryConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub local_policies: Vec<String>,
    #[serde(default)]
    pub instruction_sources: InstructionSourcesConfig,
    #[serde(default)]
    pub index: IndexConfig,
    #[serde(default)]
    pub output_budget: OutputBudgetConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RegistryConfig {
    #[serde(rename = "type")]
    pub registry_type: String,
    pub url: String,
    pub r#ref: String,
    pub cache_dir: String,
    #[serde(default)]
    pub sync: RegistrySyncConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RegistrySyncConfig {
    pub mode: SyncMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_age_minutes: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncMode {
    Manual,
    Auto,
    Pinned,
    Offline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InstructionSourcesConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trusted: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct IndexConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub vector: VectorIndexConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct VectorIndexConfig {
    pub enabled: bool,
    pub backend: VectorIndexBackend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VectorIndexBackend {
    SqliteVec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OutputBudgetConfig {
    pub max_tokens: u32,
    pub max_instructions: u32,
    pub max_required_checks: u32,
    pub max_blocked_actions: u32,
    pub include_examples: bool,
    pub include_explanations: String,
}

impl Default for AgentPolicyConfig {
    fn default() -> Self {
        Self {
            registry: None,
            local_policies: vec![".agent-policy/policies".to_string()],
            instruction_sources: InstructionSourcesConfig::default(),
            index: IndexConfig::default(),
            output_budget: OutputBudgetConfig::default(),
        }
    }
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            registry_type: "git".to_string(),
            url: String::new(),
            r#ref: "main".to_string(),
            cache_dir: String::new(),
            sync: RegistrySyncConfig::default(),
        }
    }
}

impl Default for RegistrySyncConfig {
    fn default() -> Self {
        Self {
            mode: SyncMode::Manual,
            max_age_minutes: None,
        }
    }
}

impl Default for InstructionSourcesConfig {
    fn default() -> Self {
        Self {
            include: vec![
                "AGENTS.md".to_string(),
                "CLAUDE.md".to_string(),
                ".github/copilot-instructions.md".to_string(),
                ".cursor/rules/**".to_string(),
                "**/AGENTS.md".to_string(),
                "**/CLAUDE.md".to_string(),
            ],
            exclude: vec!["node_modules/**".to_string(), "vendor/**".to_string()],
            trusted: Vec::new(),
        }
    }
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            include: Vec::new(),
            exclude: vec!["node_modules/**".to_string()],
            vector: VectorIndexConfig::default(),
        }
    }
}

impl Default for VectorIndexConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            backend: VectorIndexBackend::SqliteVec,
        }
    }
}

impl Default for OutputBudgetConfig {
    fn default() -> Self {
        Self {
            max_tokens: 900,
            max_instructions: 8,
            max_required_checks: 4,
            max_blocked_actions: 4,
            include_examples: false,
            include_explanations: "compact".to_string(),
        }
    }
}

pub fn load_config(repo_path: impl AsRef<Path>) -> Result<AgentPolicyConfig> {
    let config_path = repo_path.as_ref().join(REPO_CONFIG_FILE_NAME);
    if !config_path.exists() {
        return resolve_config(ConfigPrecedenceLayers {
            repository: None,
            ..ConfigPrecedenceLayers::default()
        });
    }

    let repository = Some(read_config_patch(&config_path)?);
    resolve_config(ConfigPrecedenceLayers {
        repository,
        ..ConfigPrecedenceLayers::default()
    })
}

pub fn load_config_from_path(path: impl AsRef<Path>) -> Result<AgentPolicyConfig> {
    let path = path.as_ref();
    let explicit_file = Some(read_config_patch(path)?);
    resolve_config(ConfigPrecedenceLayers {
        explicit_file,
        ..ConfigPrecedenceLayers::default()
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigValidationResult {
    pub config_checked: bool,
    pub local_policies: Vec<String>,
    pub errors: Vec<ConfigValidationIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigValidationIssue {
    pub code: &'static str,
    pub message: String,
    pub path: Option<String>,
    pub field: Option<String>,
}

pub fn validate_config_file(path: impl AsRef<Path>) -> ConfigValidationResult {
    let path = path.as_ref();
    if !path.exists() {
        return ConfigValidationResult {
            config_checked: false,
            local_policies: AgentPolicyConfig::default().local_policies,
            errors: Vec::new(),
        };
    }

    let mut errors = Vec::new();
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) => {
            push_config_error(
                &mut errors,
                "config_read_error",
                format!("Failed to read config file: {error}"),
                Some(path),
                None,
            );
            return ConfigValidationResult {
                config_checked: true,
                local_policies: AgentPolicyConfig::default().local_policies,
                errors,
            };
        }
    };

    let value = match serde_yaml::from_str::<Value>(&raw) {
        Ok(value) => value,
        Err(error) => {
            push_config_error(
                &mut errors,
                "config_parse_error",
                format!("Failed to parse config YAML: {error}"),
                Some(path),
                None,
            );
            return ConfigValidationResult {
                config_checked: true,
                local_policies: AgentPolicyConfig::default().local_policies,
                errors,
            };
        }
    };

    let Some(root) = value.as_mapping() else {
        push_config_error(
            &mut errors,
            "config_invalid_root",
            "Config must be a YAML mapping.".to_string(),
            Some(path),
            None,
        );
        return ConfigValidationResult {
            config_checked: true,
            local_policies: AgentPolicyConfig::default().local_policies,
            errors,
        };
    };

    validate_config_registry(root, path, &mut errors);
    validate_config_output_budget(root, path, &mut errors);

    if let Err(error) = load_config_from_path(path) {
        push_config_error(
            &mut errors,
            "config_schema_error",
            format!("Config does not match the documented schema: {error:#}"),
            Some(path),
            None,
        );
    }

    ConfigValidationResult {
        config_checked: true,
        local_policies: local_policy_dirs_from_config(root)
            .unwrap_or_else(|| AgentPolicyConfig::default().local_policies),
        errors,
    }
}

fn validate_config_registry(root: &Mapping, path: &Path, errors: &mut Vec<ConfigValidationIssue>) {
    let Some(registry) = mapping_get(root, "registry") else {
        return;
    };
    let Some(registry) = registry.as_mapping() else {
        push_config_error(
            errors,
            "config_registry_invalid",
            "registry must be a mapping when configured.".to_string(),
            Some(path),
            Some("registry"),
        );
        return;
    };

    for field in ["type", "url", "ref", "cache_dir"] {
        match mapping_get(registry, field).and_then(Value::as_str) {
            Some(value) if !value.trim().is_empty() => {}
            _ => push_config_error(
                errors,
                "config_registry_missing_field",
                format!("registry.{field} is required when registry is configured."),
                Some(path),
                Some(&format!("registry.{field}")),
            ),
        }
    }

    if let Some(registry_type) = mapping_get(registry, "type").and_then(Value::as_str) {
        if registry_type != "git" {
            push_config_error(
                errors,
                "config_invalid_registry_type",
                format!("registry.type `{registry_type}` is invalid; expected git."),
                Some(path),
                Some("registry.type"),
            );
        }
    }

    if let Some(sync) = mapping_get(registry, "sync").and_then(Value::as_mapping) {
        if let Some(mode) = mapping_get(sync, "mode") {
            match mode.as_str() {
                Some("manual" | "auto" | "pinned" | "offline") => {}
                Some(value) => push_config_error(
                    errors,
                    "config_invalid_sync_mode",
                    format!(
                        "registry.sync.mode `{value}` is invalid; expected manual, auto, pinned, or offline."
                    ),
                    Some(path),
                    Some("registry.sync.mode"),
                ),
                None => push_config_error(
                    errors,
                    "config_invalid_sync_mode",
                    "registry.sync.mode must be a string.".to_string(),
                    Some(path),
                    Some("registry.sync.mode"),
                ),
            }
        }
    }
}

fn validate_config_output_budget(
    root: &Mapping,
    path: &Path,
    errors: &mut Vec<ConfigValidationIssue>,
) {
    let Some(output_budget) = mapping_get(root, "output_budget") else {
        return;
    };
    let Some(output_budget) = output_budget.as_mapping() else {
        push_config_error(
            errors,
            "config_output_budget_invalid",
            "output_budget must be a mapping.".to_string(),
            Some(path),
            Some("output_budget"),
        );
        return;
    };

    for field in [
        "max_tokens",
        "max_instructions",
        "max_required_checks",
        "max_blocked_actions",
    ] {
        if let Some(value) = mapping_get(output_budget, field) {
            match value.as_i64() {
                Some(number) if number > 0 => {}
                Some(_) => push_config_error(
                    errors,
                    "config_invalid_output_budget",
                    format!("output_budget.{field} must be greater than zero."),
                    Some(path),
                    Some(&format!("output_budget.{field}")),
                ),
                None => push_config_error(
                    errors,
                    "config_invalid_output_budget",
                    format!("output_budget.{field} must be an integer."),
                    Some(path),
                    Some(&format!("output_budget.{field}")),
                ),
            }
        }
    }
}

fn local_policy_dirs_from_config(root: &Mapping) -> Option<Vec<String>> {
    let local_policies = mapping_get(root, "local_policies")?.as_sequence()?;
    let dirs = local_policies
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();

    if dirs.is_empty() {
        None
    } else {
        Some(dirs)
    }
}

fn mapping_get<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a Value> {
    mapping.get(Value::String(key.to_string()))
}

fn push_config_error(
    errors: &mut Vec<ConfigValidationIssue>,
    code: &'static str,
    message: String,
    path: Option<&Path>,
    field: Option<&str>,
) {
    errors.push(ConfigValidationIssue {
        code,
        message,
        path: path.map(|path| path.display().to_string()),
        field: field.map(str::to_string),
    });
}

fn read_config_patch(path: &Path) -> Result<AgentPolicyConfigPatch> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read config file {}", path.display()))?;
    serde_yaml::from_str::<AgentPolicyConfigPatch>(&raw)
        .with_context(|| format!("failed to parse config file {}", path.display()))
}

fn resolve_config(layers: ConfigPrecedenceLayers) -> Result<AgentPolicyConfig> {
    let mut config = AgentPolicyConfig::default();

    for patch in [
        layers.repository,
        layers.registry,
        layers.trusted_operator,
        layers.explicit_file,
        layers.cli,
    ]
    .into_iter()
    .flatten()
    {
        config.apply_patch(patch)?;
    }

    config.validate()?;
    Ok(config)
}

#[derive(Debug, Default)]
struct ConfigPrecedenceLayers {
    cli: Option<AgentPolicyConfigPatch>,
    explicit_file: Option<AgentPolicyConfigPatch>,
    trusted_operator: Option<AgentPolicyConfigPatch>,
    registry: Option<AgentPolicyConfigPatch>,
    repository: Option<AgentPolicyConfigPatch>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct AgentPolicyConfigPatch {
    registry: Option<RegistryConfigPatch>,
    local_policies: Option<Vec<String>>,
    instruction_sources: Option<InstructionSourcesConfigPatch>,
    index: Option<IndexConfigPatch>,
    output_budget: Option<OutputBudgetConfigPatch>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RegistryConfigPatch {
    #[serde(rename = "type")]
    registry_type: Option<String>,
    url: Option<String>,
    r#ref: Option<String>,
    cache_dir: Option<String>,
    sync: Option<RegistrySyncConfigPatch>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RegistrySyncConfigPatch {
    mode: Option<SyncMode>,
    max_age_minutes: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct InstructionSourcesConfigPatch {
    include: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
    trusted: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct IndexConfigPatch {
    include: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
    vector: Option<VectorIndexConfigPatch>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct VectorIndexConfigPatch {
    enabled: Option<bool>,
    backend: Option<VectorIndexBackend>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct OutputBudgetConfigPatch {
    max_tokens: Option<u32>,
    max_instructions: Option<u32>,
    max_required_checks: Option<u32>,
    max_blocked_actions: Option<u32>,
    include_examples: Option<bool>,
    include_explanations: Option<String>,
}

impl AgentPolicyConfig {
    fn apply_patch(&mut self, patch: AgentPolicyConfigPatch) -> Result<()> {
        if let Some(registry_patch) = patch.registry {
            self.registry = Some(match self.registry.take() {
                Some(mut registry) => {
                    registry.apply_patch(registry_patch);
                    registry
                }
                None => RegistryConfig::from_patch(registry_patch)?,
            });
        }

        if let Some(local_policies) = patch.local_policies {
            self.local_policies = local_policies;
        }

        if let Some(instruction_sources_patch) = patch.instruction_sources {
            self.instruction_sources
                .apply_patch(instruction_sources_patch);
        }

        if let Some(index_patch) = patch.index {
            self.index.apply_patch(index_patch);
        }

        if let Some(output_budget_patch) = patch.output_budget {
            self.output_budget.apply_patch(output_budget_patch);
        }

        Ok(())
    }

    fn validate(&self) -> Result<()> {
        if let Some(registry) = &self.registry {
            registry.validate()?;
        }

        Ok(())
    }
}

impl RegistryConfig {
    fn from_patch(patch: RegistryConfigPatch) -> Result<Self> {
        let mut registry = Self::default();
        registry.apply_patch(patch);
        Ok(registry)
    }

    fn apply_patch(&mut self, patch: RegistryConfigPatch) {
        if let Some(registry_type) = patch.registry_type {
            self.registry_type = registry_type;
        }
        if let Some(url) = patch.url {
            self.url = url;
        }
        if let Some(r#ref) = patch.r#ref {
            self.r#ref = r#ref;
        }
        if let Some(cache_dir) = patch.cache_dir {
            self.cache_dir = cache_dir;
        }
        if let Some(sync_patch) = patch.sync {
            self.sync.apply_patch(sync_patch);
        }
    }

    fn validate(&self) -> Result<()> {
        if self.registry_type != "git" {
            bail!("registry.type must be git");
        }
        if self.url.trim().is_empty() {
            bail!("registry.url must not be empty");
        }
        if self.r#ref.trim().is_empty() {
            bail!("registry.ref must not be empty");
        }
        if self.cache_dir.trim().is_empty() {
            bail!("registry.cache_dir must not be empty");
        }

        Ok(())
    }
}

impl RegistrySyncConfig {
    fn apply_patch(&mut self, patch: RegistrySyncConfigPatch) {
        if let Some(mode) = patch.mode {
            self.mode = mode;
        }
        if let Some(max_age_minutes) = patch.max_age_minutes {
            self.max_age_minutes = Some(max_age_minutes);
        }
    }
}

impl InstructionSourcesConfig {
    fn apply_patch(&mut self, patch: InstructionSourcesConfigPatch) {
        if let Some(include) = patch.include {
            self.include = include;
        }
        if let Some(exclude) = patch.exclude {
            self.exclude = exclude;
        }
        if let Some(trusted) = patch.trusted {
            self.trusted = trusted;
        }
    }
}

impl IndexConfig {
    fn apply_patch(&mut self, patch: IndexConfigPatch) {
        if let Some(include) = patch.include {
            self.include = include;
        }
        if let Some(exclude) = patch.exclude {
            self.exclude = exclude;
        }
        if let Some(vector_patch) = patch.vector {
            self.vector.apply_patch(vector_patch);
        }
    }
}

impl VectorIndexConfig {
    fn apply_patch(&mut self, patch: VectorIndexConfigPatch) {
        if let Some(enabled) = patch.enabled {
            self.enabled = enabled;
        }
        if let Some(backend) = patch.backend {
            self.backend = backend;
        }
    }
}

impl OutputBudgetConfig {
    fn apply_patch(&mut self, patch: OutputBudgetConfigPatch) {
        if let Some(max_tokens) = patch.max_tokens {
            self.max_tokens = max_tokens;
        }
        if let Some(max_instructions) = patch.max_instructions {
            self.max_instructions = max_instructions;
        }
        if let Some(max_required_checks) = patch.max_required_checks {
            self.max_required_checks = max_required_checks;
        }
        if let Some(max_blocked_actions) = patch.max_blocked_actions {
            self.max_blocked_actions = max_blocked_actions;
        }
        if let Some(include_examples) = patch.include_examples {
            self.include_examples = include_examples;
        }
        if let Some(include_explanations) = patch.include_explanations {
            self.include_explanations = include_explanations;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_conservative() {
        let cfg = AgentPolicyConfig::default();

        assert_eq!(cfg.local_policies, vec![".agent-policy/policies"]);
        assert!(cfg.registry.is_none());
        assert!(cfg.index.include.is_empty());
        assert!(!cfg.index.vector.enabled);
        assert_eq!(cfg.index.vector.backend, VectorIndexBackend::SqliteVec);
        assert_eq!(cfg.output_budget.max_tokens, 900);
        assert!(!cfg.output_budget.include_examples);
    }

    #[test]
    fn vector_config_is_a_disabled_placeholder_by_default() {
        let patch: AgentPolicyConfigPatch = serde_yaml::from_str(
            r#"
index:
  vector:
    enabled: true
    backend: sqlite_vec
"#,
        )
        .expect("vector config patch should parse");
        let mut cfg = AgentPolicyConfig::default();

        cfg.apply_patch(patch).expect("apply vector patch");

        assert!(cfg.index.vector.enabled);
        assert_eq!(cfg.index.vector.backend, VectorIndexBackend::SqliteVec);
    }
}
