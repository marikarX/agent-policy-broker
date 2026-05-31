use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use agent_policy_core::load_policies_from_dirs;

#[test]
fn loads_multiple_policies_from_default_policy_tree() {
    let repo_dir = create_temp_dir("default-policy-tree");
    let policy_dir = repo_dir.join(".agent-policy").join("policies");
    fs::create_dir_all(policy_dir.join("nested")).expect("policy tree should be created");
    fs::copy(
        fixture_path("valid/typescript.yaml"),
        policy_dir.join("typescript.yaml"),
    )
    .expect("fixture should be copied");
    fs::copy(
        fixture_path("valid/payments.yml"),
        policy_dir.join("nested").join("payments.yml"),
    )
    .expect("fixture should be copied");

    let loaded = load_policies_from_dirs(&repo_dir, [".agent-policy/policies"])
        .expect("default policy tree should load");

    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].policy.id, "domain.payments.refunds");
    assert_eq!(
        loaded[0].source_path,
        policy_dir.join("nested").join("payments.yml")
    );
    assert_eq!(loaded[1].policy.id, "lang.typescript.base");
    assert_eq!(loaded[1].source_path, policy_dir.join("typescript.yaml"));
}

#[test]
fn loads_policies_from_multiple_configured_directories() {
    let repo_dir = create_temp_dir("configured-policy-dirs");
    let default_dir = repo_dir.join(".agent-policy").join("policies");
    let extra_dir = repo_dir.join("custom-policies");
    fs::create_dir_all(&default_dir).expect("default policy dir should be created");
    fs::create_dir_all(extra_dir.join("services")).expect("extra policy dir should be created");
    fs::copy(
        fixture_path("valid/typescript.yaml"),
        default_dir.join("typescript.yaml"),
    )
    .expect("fixture should be copied");
    fs::copy(
        fixture_path("valid/payments.yml"),
        extra_dir.join("services").join("payments.yml"),
    )
    .expect("fixture should be copied");

    let loaded = load_policies_from_dirs(&repo_dir, [".agent-policy/policies", "custom-policies"])
        .expect("configured policy dirs should load");

    let mut ids = loaded
        .iter()
        .map(|item| item.policy.id.as_str())
        .collect::<Vec<_>>();
    ids.sort_unstable();

    assert_eq!(loaded.len(), 2);
    assert_eq!(ids, vec!["domain.payments.refunds", "lang.typescript.base"]);
}

#[test]
fn missing_policy_directories_are_ignored() {
    let repo_dir = create_temp_dir("missing-policy-dirs");

    let loaded = load_policies_from_dirs(&repo_dir, [".agent-policy/policies", "custom-policies"])
        .expect("missing policy dirs should be ignored");

    assert!(loaded.is_empty());
}

#[test]
fn invalid_yaml_reports_path_and_parse_error() {
    let repo_dir = create_temp_dir("invalid-policy-yaml");
    let policy_dir = repo_dir.join(".agent-policy").join("policies");
    fs::create_dir_all(&policy_dir).expect("policy dir should be created");
    let invalid_path = policy_dir.join("broken.yaml");
    fs::copy(fixture_path("invalid/broken.yaml"), &invalid_path).expect("fixture should be copied");

    let error = load_policies_from_dirs(&repo_dir, [".agent-policy/policies"])
        .expect_err("invalid yaml should fail");
    let message = format!("{error:#}");

    assert!(message.contains("failed to parse policy file"));
    assert!(message.contains("broken.yaml"));
    assert!(message.contains("did not find expected"));
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
        "agent-policy-core-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("temp dir should be created");
    path
}
