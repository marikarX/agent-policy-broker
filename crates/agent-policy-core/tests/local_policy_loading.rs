use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

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
fn loads_policies_from_local_registry_fixture_without_git_commit_metadata() {
    let temp = TempDir::new("agent-policy-core-local-registry");
    let registry_dir = temp.path().join("local-registry");
    copy_dir_all_without_git(&fixture_repo("local-registry"), &registry_dir)
        .expect("copy local registry fixture");

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
        Some("local-registry:org.security.secrets@3")
    );
}

#[test]
fn loads_policies_from_temp_git_registry_with_commit_metadata() {
    let temp = TempDir::new("agent-policy-core-registry");
    let registry_dir = temp.path();
    fs::create_dir_all(registry_dir.join("policies")).expect("create registry policy dir");
    fs::write(
        registry_dir.join("policies").join("policy.yaml"),
        "id: org.security.secrets\nversion: 3\nstatus: active\napplies_when: {}\ninstructions:\n  - Never expose secrets.\n",
    )
    .expect("write registry policy");

    git(registry_dir, &["init"]);
    git(registry_dir, &["checkout", "-b", "main"]);
    git(registry_dir, &["add", "."]);
    git(
        registry_dir,
        &[
            "-c",
            "user.name=Agent Policy Tests",
            "-c",
            "user.email=agent-policy-tests@example.invalid",
            "commit",
            "-m",
            "initial registry",
        ],
    );
    let head = git_stdout(registry_dir, &["rev-parse", "HEAD"]);
    let short_head = head.get(..12).unwrap_or(&head);

    let loaded = load_policies_from_registry(
        registry_dir,
        RegistryLoadOptions {
            source_name: "local-registry".to_string(),
            ..RegistryLoadOptions::default()
        },
    )
    .expect("temp git registry should load");

    let expected_source_ref = format!("local-registry:org.security.secrets@3#{short_head}");
    assert_eq!(
        loaded[0]
            .source_ref
            .as_ref()
            .map(|source| source.0.as_str()),
        Some(expected_source_ref.as_str())
    );
}

#[test]
fn registry_and_local_policies_merge_into_bundle_sources() {
    let repo_dir = fixture_repo("registry-app");
    let temp = TempDir::new("agent-policy-core-local-registry");
    let registry_dir = temp.path().join("local-registry");
    copy_dir_all_without_git(&fixture_repo("local-registry"), &registry_dir)
        .expect("copy local registry fixture");
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

    assert!(sources.contains(&"local-registry:org.security.secrets@3"));
    assert!(sources.contains(&"repo.registry-app.tests@1"));
}

fn fixture_repo(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn git(path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {:?} failed:\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_stdout(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {:?} failed:\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn copy_dir_all_without_git(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        if entry.file_name() == ".git" {
            continue;
        }
        let file_type = entry.file_type()?;
        let target = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all_without_git(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
