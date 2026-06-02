use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use agent_policy_config::{
    load_config, load_config_from_path, AgentPolicyConfig, OutputBudgetConfig, SyncMode,
};

#[test]
fn missing_config_returns_defaults() {
    let repo_dir = create_temp_dir("missing-config");

    let config = load_config(&repo_dir).expect("missing repo config should fall back to defaults");

    assert_eq!(config, AgentPolicyConfig::default());
    assert!(!config.index.vector.enabled);
    assert!(config.index.include.is_empty());
}

#[test]
fn valid_agent_policy_yaml_loads_correctly() {
    let repo_dir = fixture_repo("payments-repo");

    let config = load_config(&repo_dir).expect("repo config should parse");

    assert!(config.registry.is_none());
    assert_eq!(config.local_policies, vec![".agent-policy/policies"]);
    assert_eq!(
        config.instruction_sources.include,
        vec!["AGENTS.md", "**/AGENTS.md"]
    );
    assert_eq!(config.instruction_sources.exclude, vec!["node_modules/**"]);
    assert_eq!(
        config.output_budget,
        OutputBudgetConfig {
            max_tokens: 900,
            max_instructions: 8,
            max_required_checks: 4,
            max_blocked_actions: 4,
            include_examples: false,
            include_explanations: "compact".to_string(),
        }
    );
}

#[test]
fn explicit_config_path_loads_correctly() {
    let config = load_config_from_path(fixture_path("valid.agent-policy.yaml"))
        .expect("valid fixture should parse");

    assert_eq!(config.output_budget.max_tokens, 1024);
    assert_eq!(config.output_budget.include_explanations, "full");
}

#[test]
fn codex_config_fields_load_correctly() {
    let repo_dir = create_temp_dir("codex-config");
    fs::write(
        repo_dir.join(".agent-policy.yaml"),
        r#"
codex:
  enabled: true
  home: /tmp/codex-home
  current_dir: backend/payments
  project_doc_fallback_filenames:
    - INSTRUCTIONS.md
    - .rules.md
  project_doc_max_bytes: 64
  include_global: true
"#,
    )
    .expect("config should be written");

    let config = load_config(&repo_dir).expect("codex config should parse");

    assert!(config.codex.enabled);
    assert_eq!(config.codex.home.as_deref(), Some("/tmp/codex-home"));
    assert_eq!(
        config.codex.current_dir.as_deref(),
        Some("backend/payments")
    );
    assert_eq!(
        config.codex.project_doc_fallback_filenames,
        vec!["INSTRUCTIONS.md", ".rules.md"]
    );
    assert_eq!(config.codex.project_doc_max_bytes, 64);
    assert!(config.codex.include_global);
}

#[test]
fn explicit_registry_config_loads_documented_git_shape() {
    let config = load_config_from_path(fixture_path("valid.agent-policy.yaml"))
        .expect("trusted explicit registry config should parse");
    let registry = config.registry.expect("registry should be configured");

    assert_eq!(registry.registry_type, "git");
    assert_eq!(
        registry.url,
        "git@github.com:company/agent-policy-registry.git"
    );
    assert_eq!(registry.r#ref, "main");
    assert_eq!(
        registry.cache_dir,
        "~/.cache/agent-policy/registries/company"
    );
    assert_eq!(registry.sync.mode, SyncMode::Auto);
}

#[test]
fn repository_registry_config_is_rejected() {
    let repo_dir = fixture_repo("registry-app");

    let error = load_config(&repo_dir).expect_err("repo registry config should fail closed");

    assert!(format!("{error:#}").contains("must not configure registry"));
}

#[test]
fn repository_trusted_instruction_sources_are_rejected() {
    let repo_dir = create_temp_dir("trusted-instructions");
    fs::write(
        repo_dir.join(".agent-policy.yaml"),
        "instruction_sources:
  trusted:
    - ATTACKER.md
",
    )
    .expect("config should be written");

    let error = load_config(&repo_dir).expect_err("trusted repo instructions should fail closed");

    assert!(format!("{error:#}").contains("must not configure instruction_sources.trusted"));
}

#[test]
fn repository_output_budget_is_clamped_to_safe_minimums() {
    let repo_dir = create_temp_dir("low-output-budget");
    fs::write(
        repo_dir.join(".agent-policy.yaml"),
        r#"
output_budget:
  max_tokens: 1
  max_instructions: 1
  max_required_checks: 0
  max_blocked_actions: 0
  include_examples: true
  include_explanations: terse
"#,
    )
    .expect("config should be written");

    let config = load_config(&repo_dir).expect("repo budget should be clamped");

    assert_eq!(
        config.output_budget,
        OutputBudgetConfig {
            max_tokens: 900,
            max_instructions: 8,
            max_required_checks: 4,
            max_blocked_actions: 4,
            include_examples: true,
            include_explanations: "terse".to_string(),
        }
    );
}

#[test]
fn unsupported_registry_type_is_rejected() {
    let repo_dir = create_temp_dir("bad-registry-type");
    fs::write(
        repo_dir.join(".agent-policy.yaml"),
        "registry:\n  type: s3\n  url: ./registry\n  ref: main\n  cache_dir: ./registry\n",
    )
    .expect("config should be written");

    let error = load_config_from_path(repo_dir.join(".agent-policy.yaml"))
        .expect_err("unsupported registry type should fail");

    assert!(format!("{error:#}").contains("registry.type must be git"));
}

#[test]
fn invalid_fixture_config_returns_useful_error() {
    let error =
        load_config_from_path(fixture_repo("invalid-policy-repo").join(".agent-policy.yaml"))
            .expect_err("invalid fixture should fail");
    let message = format!("{error:#}");

    assert!(message.contains("failed to parse config file"));
    assert!(message.contains("invalid-policy-repo"));
}

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn fixture_repo(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

fn create_temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "agent-policy-config-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("temp dir should be created");
    path
}
