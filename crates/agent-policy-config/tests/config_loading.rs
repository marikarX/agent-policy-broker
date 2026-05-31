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
}

#[test]
fn valid_agent_policy_yaml_loads_correctly() {
    let repo_dir = create_temp_dir("repo-config");
    fs::copy(
        fixture_path("valid.agent-policy.yaml"),
        repo_dir.join(".agent-policy.yaml"),
    )
    .expect("fixture should be copied into the temp repo");

    let config = load_config(&repo_dir).expect("repo config should parse");

    let registry = config.registry.as_ref().expect("registry should be present");
    assert_eq!(registry.registry_type, "git");
    assert_eq!(registry.url, "git@github.com:company/agent-policy-registry.git");
    assert_eq!(registry.r#ref, "main");
    assert_eq!(
        registry.cache_dir,
        "~/.cache/agent-policy/registries/company"
    );
    assert_eq!(registry.sync.mode, SyncMode::Auto);
    assert_eq!(registry.sync.max_age_minutes, Some(15));

    assert_eq!(config.local_policies, vec![".agent-policy/policies"]);
    assert_eq!(
        config.instruction_sources.trusted,
        vec!["/etc/agent-policy/trusted-instructions"]
    );
    assert_eq!(config.index.include, vec![".agent-policy/policies", "docs"]);
    assert_eq!(
        config.output_budget,
        OutputBudgetConfig {
            max_tokens: 1024,
            max_instructions: 6,
            max_required_checks: 3,
            max_blocked_actions: 2,
            include_examples: true,
            include_explanations: "full".to_string(),
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
fn invalid_yaml_returns_useful_error() {
    let error = load_config_from_path(fixture_path("invalid.agent-policy.yaml"))
        .expect_err("invalid fixture should fail");
    let message = format!("{error:#}");

    assert!(message.contains("failed to parse config file"));
    assert!(message.contains("invalid.agent-policy.yaml"));
}

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
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
