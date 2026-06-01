use std::path::{Path, PathBuf};

use agent_policy_core::load_policies_from_dirs;

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

fn fixture_repo(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}
