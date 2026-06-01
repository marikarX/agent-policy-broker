use std::path::{Path, PathBuf};

use agent_policy_core::{
    build_instruction_bundle, load_policies_from_dirs, load_policies_from_registry,
    BundleBuildOptions, RegistryLoadOptions, TaskDetails, TaskIntent, TaskType,
};

#[test]
fn loads_multiple_policies_from_default_policy_tree() {
    let repo_dir = fixture_repo("payments-repo");
    let policy_dir = repo_dir.join(".agent-policy").join("policies");

    let loaded = load_policies_from_dirs(&repo_dir, [".agent-policy/policies"])
        .expect("default policy tree should load");

    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].policy.id, "domain.payments.refunds");
    assert_eq!(loaded[0].source_path, policy_dir.join("payments.yaml"));
    assert_eq!(loaded[1].policy.id, "lang.typescript.base");
    assert_eq!(loaded[1].source_path, policy_dir.join("typescript.yaml"));
}

#[test]
fn loads_policies_from_multiple_configured_directories() {
    let repo_dir = fixture_repo("monorepo");

    let loaded = load_policies_from_dirs(
        &repo_dir,
        [
            ".agent-policy/policies",
            "packages/web/.agent-policy/policies",
        ],
    )
    .expect("configured policy dirs should load");

    let mut ids = loaded
        .iter()
        .map(|item| item.policy.id.as_str())
        .collect::<Vec<_>>();
    ids.sort_unstable();

    assert_eq!(loaded.len(), 2);
    assert_eq!(ids, vec!["lang.rust.base", "pkg.web.react"]);
}

#[test]
fn missing_policy_directories_are_ignored() {
    let repo_dir = fixture_repo("nested-instructions");

    let loaded = load_policies_from_dirs(&repo_dir, ["custom-policies"])
        .expect("missing policy dirs should be ignored");

    assert!(loaded.is_empty());
}

#[test]
fn invalid_policy_reports_path_and_parse_error() {
    let repo_dir = fixture_repo("invalid-policy-repo");

    let error = load_policies_from_dirs(&repo_dir, [".agent-policy/policies"])
        .expect_err("invalid policy should fail");
    let message = format!("{error:#}");

    assert!(message.contains("failed to parse policy file"));
    assert!(message.contains("duplicate-b.yaml"));
    assert!(message.contains("retired"));
}

#[test]
fn loads_policies_from_local_registry_with_git_commit_metadata() {
    let registry_dir = fixture_repo("local-registry");

    let loaded = load_policies_from_registry(
        &registry_dir,
        RegistryLoadOptions {
            source_name: "local-registry".to_string(),
            ..RegistryLoadOptions::default()
        },
    )
    .expect("local registry fixture should load");

    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].policy.id, "org.security.secrets");
    assert_eq!(
        loaded[0]
            .source_ref
            .as_ref()
            .map(|source| source.0.as_str()),
        Some("local-registry:org.security.secrets@3#0123456789ab")
    );
}

#[test]
fn registry_and_local_policies_merge_into_bundle_sources() {
    let repo_dir = fixture_repo("registry-app");
    let registry_dir = fixture_repo("local-registry");
    let mut policies = load_policies_from_registry(
        &registry_dir,
        RegistryLoadOptions {
            source_name: "local-registry".to_string(),
            ..RegistryLoadOptions::default()
        },
    )
    .expect("registry fixture should load");
    policies.extend(
        load_policies_from_dirs(&repo_dir, [".agent-policy/policies"])
            .expect("local policies should load"),
    );

    let bundle = build_instruction_bundle(
        &TaskIntent {
            repo: Some("registry-app".to_string()),
            branch: None,
            task: Some(TaskDetails {
                summary: Some("Update source".to_string()),
                task_type: Some(TaskType("fix_bug".to_string())),
            }),
            files: vec!["src/lib.rs".to_string()],
            detected: None,
            risk_flags: Vec::new(),
            expected_commands: Vec::new(),
            expected_check_ids: Vec::new(),
            output_budget: None,
        },
        &policies,
        BundleBuildOptions {
            max_tokens: Some(900),
            max_instructions: Some(8),
            max_required_checks: Some(4),
            max_blocked_actions: Some(4),
        },
    )
    .expect("bundle should build");

    let sources = bundle
        .sources
        .iter()
        .map(|source| source.0.as_str())
        .collect::<Vec<_>>();

    assert!(sources.contains(&"local-registry:org.security.secrets@3#0123456789ab"));
    assert!(sources.contains(&"repo.registry-app.tests@1"));
}

fn fixture_repo(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}
