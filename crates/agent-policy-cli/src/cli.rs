use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use crate::commands::{discover, get, index, inspect, migrate, registry, serve, validate};

#[derive(Debug, Parser)]
#[command(name = "agent-policy", version, about = "Agent Policy Broker CLI")]
pub(crate) struct Cli {
    #[command(flatten)]
    pub(crate) global: GlobalArgs,
    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Clone, Debug, ValueEnum)]
pub(crate) enum OutputFormat {
    Json,
    Markdown,
}

#[derive(Clone, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum InstructionDiscoveryMode {
    Generic,
    Codex,
}

#[derive(Clone, Debug, Args)]
pub(crate) struct GlobalArgs {
    #[arg(
        long,
        global = true,
        value_name = "path",
        help = "Repository path to inspect; defaults to the current directory"
    )]
    pub(crate) repo: Option<PathBuf>,
    #[arg(
        long,
        global = true,
        value_name = "path",
        help = "Explicit .agent-policy.yaml config path"
    )]
    pub(crate) config: Option<PathBuf>,
    #[arg(
        long,
        global = true,
        value_enum,
        help = "Output format for commands that support rendering"
    )]
    pub(crate) format: Option<OutputFormat>,
    #[arg(long, global = true, help = "Print diagnostic details when supported")]
    pub(crate) verbose: bool,
    #[arg(long, global = true, help = "Suppress nonessential output")]
    pub(crate) quiet: bool,
    #[arg(
        long,
        global = true,
        help = "Use only local files and cached registries; do not attempt network operations"
    )]
    pub(crate) no_network: bool,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Commands {
    /// Compile a task-specific instruction bundle.
    Get(GetArgs),
    /// Discover existing instruction sources in a repository.
    Discover(DiscoverArgs),
    /// Validate policies, config, and discovered instruction sources.
    Validate,
    /// Inspect repository guidance and produce an audit report.
    Inspect(InspectArgs),
    /// Propose policy drafts from existing instruction sources.
    Migrate(MigrateArgs),
    /// Build or rebuild local metadata and full-text indexes.
    Index,
    /// Manage policy registries.
    Registry(RegistryArgs),
    /// Run a local service for repeated lookups and integrations.
    Serve(ServeArgs),
}

#[derive(Debug, Args)]
pub(crate) struct RegistryArgs {
    #[command(subcommand)]
    pub(crate) command: RegistryCommands,
}

#[derive(Clone, Debug, Args)]
pub(crate) struct GetArgs {
    #[arg(long, value_name = "text", help = "Task summary")]
    pub(crate) task: Option<String>,
    #[arg(
        long = "type",
        value_name = "task_type",
        help = "Task type such as fix_bug, add_feature, refactor, test, or docs"
    )]
    pub(crate) task_type: Option<String>,
    #[arg(long, value_name = "path", num_args = 1.., help = "Relevant file paths")]
    pub(crate) files: Vec<String>,
    #[arg(
        long,
        value_name = "flag",
        num_args = 1..,
        help = "Risk flags such as auth, payments, migrations, public_api, or secrets"
    )]
    pub(crate) risk: Vec<String>,
    #[arg(
        long,
        value_name = "number",
        help = "Override instruction count budget"
    )]
    pub(crate) max_instructions: Option<u32>,
    #[arg(
        long,
        value_name = "number",
        help = "Override approximate output token budget"
    )]
    pub(crate) max_tokens: Option<u32>,
    #[arg(
        long = "instruction-mode",
        value_enum,
        default_value_t = InstructionDiscoveryMode::Generic,
        help = "Instruction discovery mode for Markdown guidance"
    )]
    pub(crate) instruction_mode: InstructionDiscoveryMode,
}

#[derive(Clone, Debug, Args)]
pub(crate) struct DiscoverArgs {
    #[arg(
        long,
        value_enum,
        default_value_t = InstructionDiscoveryMode::Generic,
        help = "Instruction discovery mode"
    )]
    pub(crate) mode: InstructionDiscoveryMode,
}

#[derive(Clone, Debug, Args)]
pub(crate) struct InspectArgs {
    #[arg(
        long,
        value_enum,
        default_value_t = InstructionDiscoveryMode::Generic,
        help = "Instruction discovery mode"
    )]
    pub(crate) mode: InstructionDiscoveryMode,
}

#[derive(Debug, Args)]
pub(crate) struct MigrateArgs {
    #[arg(long, help = "Print proposed draft policies without writing files")]
    pub(crate) dry_run: bool,
    #[arg(
        long,
        help = "Write proposed draft policies under .agent-policy/migration"
    )]
    pub(crate) write: bool,
}

#[derive(Clone, Debug, Args)]
pub(crate) struct ServeArgs {
    #[arg(
        long,
        default_value = "127.0.0.1",
        value_name = "host",
        help = "Host to bind; defaults to localhost"
    )]
    pub(crate) host: String,
    #[arg(
        long,
        default_value_t = 8765,
        value_name = "port",
        help = "Port to bind"
    )]
    pub(crate) port: u16,
}

#[derive(Debug, Subcommand)]
pub(crate) enum RegistryCommands {
    /// Validate and use a local cached Git policy registry.
    Sync,
}

pub(crate) fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Commands::Discover(args) => discover::run(&cli.global, args),
        Commands::Get(args) => get::run(&cli.global, args),
        Commands::Validate => validate::run(&cli.global),
        Commands::Inspect(args) => inspect::run(&cli.global, args),
        Commands::Migrate(args) => migrate::run(&cli.global, args),
        Commands::Index => index::run(&cli.global),
        Commands::Serve(args) => serve::run(&cli.global, args),
        Commands::Registry(registry) => match registry.command {
            RegistryCommands::Sync => registry::run_sync(&cli.global),
        },
    }
}

#[cfg(test)]
pub(crate) use crate::commands::get::{
    bm25_candidate_policy_ids_with_cache_dir, load_get_policies_with_cache_dir,
    load_registry_policies, markdown_candidate_policies, scope_matches_task_files,
};
#[cfg(test)]
pub(crate) use crate::commands::inspect::{
    detect_inspection_conflicts, detect_inspection_duplicates, inspect_repo,
    migration_dry_run_report, render_inspection_json, render_inspection_markdown,
    render_migration_dry_run_json, render_migration_dry_run_markdown, InspectionCandidate,
    MigrationClass,
};
#[cfg(test)]
pub(crate) use crate::commands::registry::{
    render_registry_sync_json, render_registry_sync_markdown, sync_registry, RegistrySyncStatus,
};
#[cfg(test)]
pub(crate) use crate::commands::validate::{
    render_validation_markdown, validate_repo, ValidationStatus,
};
#[cfg(test)]
pub(crate) use crate::indexing::{
    build_metadata_index_with_cache_dir, index_repo_source, search_fulltext_candidates,
    IndexManifest,
};
#[cfg(test)]
mod tests {
    use super::{
        bm25_candidate_policy_ids_with_cache_dir, build_metadata_index_with_cache_dir,
        detect_inspection_conflicts, detect_inspection_duplicates, index_repo_source, inspect_repo,
        load_get_policies_with_cache_dir, load_registry_policies, markdown_candidate_policies,
        migration_dry_run_report, render_inspection_json, render_inspection_markdown,
        render_migration_dry_run_json, render_migration_dry_run_markdown,
        render_registry_sync_json, render_registry_sync_markdown, render_validation_markdown, run,
        scope_matches_task_files, search_fulltext_candidates, sync_registry, validate_repo, Cli,
        Commands, GlobalArgs, IndexManifest, InspectionCandidate, InstructionDiscoveryMode,
        MigrationClass, OutputFormat, RegistryCommands, RegistrySyncStatus, ValidationStatus,
    };
    use agent_policy_config::{
        load_config, AgentPolicyConfig, RegistryConfig, RegistrySyncConfig, SyncMode,
    };
    use agent_policy_core::{
        build_instruction_bundle, build_instruction_bundle_with_bm25_candidates,
        load_policies_from_dirs, render_bundle_json, BundleBuildOptions, DetectedContext,
        LoadedPolicy, OutputBudget, TaskDetails, TaskIntent,
    };
    use agent_policy_discover::discover;
    use clap::{error::ErrorKind, CommandFactory, Parser};
    use rusqlite::Connection;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    const INDEX_POLICY_YAML: &str = r#"id: org.index.metadata
version: "2026.1"
status: active
owner: platform
priority: 42
applies_when:
  repos:
    - agent-policy-broker
  paths:
    - crates/**
  languages:
    - rust
  frameworks:
    - axum
  package_managers:
    - cargo
  task_types:
    - implementation
  risk_flags:
    - storage
instructions:
  - Keep index metadata deterministic.
"#;

    #[test]
    fn clap_command_builds() {
        Cli::command().debug_assert();
    }

    #[test]
    fn global_args_support_format_values() {
        let _ = [
            OutputFormat::Json,
            OutputFormat::Markdown,
            // keep a concrete use of global args in tests so future refactors
            // don't accidentally remove CLI-level flags.
        ];
        let _ = std::mem::size_of::<GlobalArgs>();
    }

    #[test]
    fn parse_get_with_repo() {
        let cli = Cli::try_parse_from(["agent-policy", "get", "--repo", "."]).expect("parse get");
        assert_eq!(cli.global.repo, Some(PathBuf::from(".")));
        assert!(matches!(cli.command, Commands::Get(_)));
    }

    #[test]
    fn parse_get_inputs() {
        let cli = Cli::try_parse_from([
            "agent-policy",
            "get",
            "--repo",
            "fixtures/simple-repo",
            "--task",
            "fix refund retry handling",
            "--type",
            "fix_bug",
            "--files",
            "src/payments/refunds.ts",
            "--risk",
            "payments",
            "--max-instructions",
            "4",
            "--max-tokens",
            "600",
            "--format",
            "json",
        ])
        .expect("parse get inputs");

        match cli.command {
            Commands::Get(args) => {
                assert_eq!(args.task.as_deref(), Some("fix refund retry handling"));
                assert_eq!(args.task_type.as_deref(), Some("fix_bug"));
                assert_eq!(args.files, vec!["src/payments/refunds.ts"]);
                assert_eq!(args.risk, vec!["payments"]);
                assert_eq!(args.max_instructions, Some(4));
                assert_eq!(args.max_tokens, Some(600));
            }
            _ => panic!("expected get command"),
        }
    }

    #[test]
    fn parse_discover_with_json_format() {
        let cli = Cli::try_parse_from(["agent-policy", "discover", "--format", "json"])
            .expect("parse discover");
        assert!(matches!(cli.global.format, Some(OutputFormat::Json)));
        assert!(matches!(cli.command, Commands::Discover(_)));
    }

    #[test]
    fn parse_codex_instruction_modes() {
        let discover = Cli::try_parse_from(["agent-policy", "discover", "--mode", "codex"])
            .expect("parse codex discover");
        match discover.command {
            Commands::Discover(args) => assert_eq!(args.mode, InstructionDiscoveryMode::Codex),
            _ => panic!("expected discover command"),
        }

        let inspect = Cli::try_parse_from(["agent-policy", "inspect", "--mode", "codex"])
            .expect("parse codex inspect");
        match inspect.command {
            Commands::Inspect(args) => assert_eq!(args.mode, InstructionDiscoveryMode::Codex),
            _ => panic!("expected inspect command"),
        }

        let get = Cli::try_parse_from(["agent-policy", "get", "--instruction-mode", "codex"])
            .expect("parse codex get");
        match get.command {
            Commands::Get(args) => {
                assert_eq!(args.instruction_mode, InstructionDiscoveryMode::Codex)
            }
            _ => panic!("expected get command"),
        }
    }

    #[test]
    fn parse_serve_defaults_to_localhost() {
        let cli = Cli::try_parse_from(["agent-policy", "serve"]).expect("parse serve");

        match cli.command {
            Commands::Serve(args) => {
                assert_eq!(args.host, "127.0.0.1");
                assert_eq!(args.port, 8765);
            }
            _ => panic!("expected serve command"),
        }
    }

    #[test]
    fn parse_validate_with_markdown_format() {
        let cli = Cli::try_parse_from(["agent-policy", "validate", "--format", "markdown"])
            .expect("parse validate");
        assert!(matches!(cli.global.format, Some(OutputFormat::Markdown)));
        assert!(matches!(cli.command, Commands::Validate));
    }

    #[test]
    fn parse_inspect_with_json_format() {
        let cli = Cli::try_parse_from(["agent-policy", "inspect", "--format", "json"])
            .expect("parse inspect");
        assert!(matches!(cli.global.format, Some(OutputFormat::Json)));
        assert!(matches!(cli.command, Commands::Inspect(_)));
    }

    #[test]
    fn parse_migrate_dry_run_with_markdown_format() {
        let cli = Cli::try_parse_from([
            "agent-policy",
            "migrate",
            "--dry-run",
            "--format",
            "markdown",
        ])
        .expect("parse migrate");
        assert!(matches!(cli.global.format, Some(OutputFormat::Markdown)));
        match cli.command {
            Commands::Migrate(args) => {
                assert!(args.dry_run);
                assert!(!args.write);
            }
            _ => panic!("expected migrate command"),
        }
    }

    #[test]
    fn parse_registry_sync_with_no_network() {
        let cli = Cli::try_parse_from(["agent-policy", "registry", "sync", "--no-network"])
            .expect("parse registry sync");
        assert!(cli.global.no_network);
        match cli.command {
            Commands::Registry(registry) => {
                assert!(matches!(registry.command, RegistryCommands::Sync));
            }
            _ => panic!("expected registry command"),
        }
    }

    #[test]
    fn invalid_format_value_fails() {
        let err = Cli::try_parse_from(["agent-policy", "discover", "--format", "xml"])
            .expect_err("expected invalid format to fail");
        assert_eq!(err.kind(), ErrorKind::InvalidValue);
    }

    #[test]
    fn unknown_subcommand_fails() {
        let err = Cli::try_parse_from(["agent-policy", "unknown"])
            .expect_err("expected unknown subcommand to fail");
        assert_eq!(err.kind(), ErrorKind::InvalidSubcommand);
    }

    #[test]
    fn validate_valid_fixture_repo_passes() {
        let repo = fixture_repo("payments-repo");

        let report = validate_repo(&repo, None);

        assert_eq!(report.status, ValidationStatus::Ok);
        assert!(report.errors.is_empty());
        assert_eq!(report.summary.policy_files_checked, 2);
    }

    #[test]
    fn validate_invalid_fixture_repo_reports_useful_errors() {
        let repo = fixture_repo("invalid-policy-repo");

        let report = validate_repo(&repo, None);
        let error_codes = report
            .errors
            .iter()
            .map(|issue| issue.code)
            .collect::<Vec<_>>();
        let warning_codes = report
            .warnings
            .iter()
            .map(|issue| issue.code)
            .collect::<Vec<_>>();

        assert_eq!(report.status, ValidationStatus::Failed);
        assert!(error_codes.contains(&"config_invalid_sync_mode"));
        assert!(error_codes.contains(&"config_registry_missing_field"));
        assert!(error_codes.contains(&"config_invalid_output_budget"));
        assert!(error_codes.contains(&"policy_missing_id"));
        assert!(error_codes.contains(&"policy_missing_version"));
        assert!(error_codes.contains(&"policy_active_empty_instructions"));
        assert!(error_codes.contains(&"policy_duplicate_id"));
        assert!(error_codes.contains(&"policy_invalid_status"));
        assert!(warning_codes.contains(&"policy_broad_active"));
        assert!(warning_codes.contains(&"policy_vague_instruction"));

        let markdown = render_validation_markdown(&report);
        assert!(markdown.contains("config_invalid_sync_mode"));
        assert!(markdown.contains("policy_duplicate_id"));
    }

    #[test]
    fn validate_monorepo_uses_configured_policy_directories() {
        let repo = fixture_repo("monorepo");

        let report = validate_repo(&repo, None);

        assert_eq!(report.status, ValidationStatus::Ok);
        assert_eq!(report.summary.policy_files_checked, 2);
    }

    #[test]
    fn loads_registry_policies_from_configured_cache_dir() {
        let temp = TempDir::new("registry-app-local-path");
        let repo = temp.path().join("registry-app");
        let registry_cache = temp.path().join("local-registry");
        copy_dir_all_without_git(&fixture_repo("registry-app"), &repo)
            .expect("copy registry app fixture");
        copy_dir_all_without_git(&fixture_repo("local-registry"), &registry_cache)
            .expect("copy local registry fixture");

        let config = load_config(&repo).expect("registry config should load");
        let registry = config.registry.expect("registry should be configured");

        let policies =
            load_registry_policies(&repo, &registry).expect("local registry cache should load");

        assert_eq!(policies.len(), 1);
        assert_eq!(policies[0].policy.id, "org.security.secrets");
        assert_eq!(
            policies[0]
                .source_ref
                .as_ref()
                .map(|source| source.0.as_str()),
            Some("local-registry:org.security.secrets@3")
        );
    }

    #[test]
    fn registry_sync_local_path_registry_is_noop_success() {
        let repo = fixture_repo("registry-app");
        let config = load_config(&repo).expect("registry config should load");
        let registry = config.registry.expect("registry should be configured");

        let report = sync_registry(&repo, &registry, true).expect("sync local path registry");

        assert_eq!(report.status, RegistrySyncStatus::LocalPath);
        assert_eq!(report.mode, SyncMode::Manual);
        assert!(report.commit.is_none());
        assert!(report.message.contains("nothing to sync"));
    }

    #[test]
    fn registry_sync_offline_uses_cached_git_without_fetching() {
        let temp = TempDir::new("registry-sync-offline");
        let repo = temp.path();
        let cache_dir = repo.join("registry-cache");
        let head = init_git_registry(&cache_dir);
        let registry = test_registry(&cache_dir, "main", SyncMode::Offline);

        let report = sync_registry(repo, &registry, false).expect("offline sync");

        assert_eq!(report.status, RegistrySyncStatus::Offline);
        assert_eq!(report.commit.as_deref(), Some(head.as_str()));
        assert_eq!(report.requested_ref, "main");
        assert!(render_registry_sync_markdown(&report).contains("without network access"));
    }

    #[test]
    fn registry_sync_no_network_uses_cached_git_without_fetching() {
        let temp = TempDir::new("registry-sync-no-network");
        let repo = temp.path();
        let cache_dir = repo.join("registry-cache");
        let head = init_git_registry(&cache_dir);
        let mut registry = test_registry(&cache_dir, "main", SyncMode::Manual);
        registry.url = "https://example.invalid/company/registry.git".to_string();

        let report = sync_registry(repo, &registry, true).expect("no-network sync");

        assert_eq!(report.status, RegistrySyncStatus::Offline);
        assert_eq!(report.commit.as_deref(), Some(head.as_str()));
        assert!(render_registry_sync_json(&report).contains("\"status\": \"offline\""));
    }

    #[test]
    fn registry_sync_pinned_validates_current_commit() {
        let temp = TempDir::new("registry-sync-pinned");
        let repo = temp.path();
        let cache_dir = repo.join("registry-cache");
        let head = init_git_registry(&cache_dir);
        let registry = test_registry(&cache_dir, &head, SyncMode::Pinned);

        let report = sync_registry(repo, &registry, false).expect("pinned sync");

        assert_eq!(report.status, RegistrySyncStatus::Pinned);
        assert_eq!(report.commit.as_deref(), Some(head.as_str()));
    }

    #[test]
    fn registry_sync_pinned_rejects_mismatched_commit() {
        let temp = TempDir::new("registry-sync-pinned-mismatch");
        let repo = temp.path();
        let cache_dir = repo.join("registry-cache");
        init_git_registry(&cache_dir);
        let wrong_commit = "0123456789abcdef0123456789abcdef01234567";
        let registry = test_registry(&cache_dir, wrong_commit, SyncMode::Pinned);

        let error = sync_registry(repo, &registry, false).expect_err("pinned mismatch");

        assert!(format!("{error:#}").contains("registry_pinned_mismatch"));
        assert!(format!("{error:#}").contains(wrong_commit));
    }

    #[test]
    fn registry_sync_missing_registry_reports_useful_error() {
        let temp = TempDir::new("registry-sync-missing");
        let repo = temp.path();
        let cache_dir = repo.join("missing-cache");
        let registry = test_registry(&cache_dir, "main", SyncMode::Offline);

        let error = sync_registry(repo, &registry, false).expect_err("missing cache");

        let message = format!("{error:#}");
        assert!(message.contains("registry_not_found"));
        assert!(message.contains("offline mode cannot clone or fetch"));
        assert!(message.contains("missing-cache"));
    }

    #[test]
    fn registry_sync_requires_configured_registry() {
        let repo = fixture_repo("payments-repo");
        let cli = Cli::try_parse_from([
            "agent-policy",
            "--repo",
            repo.to_str().expect("utf8 repo"),
            "registry",
            "sync",
        ])
        .expect("parse registry sync");

        let error = run(cli).expect_err("missing configured registry");

        assert!(format!("{error:#}").contains("registry_not_found"));
    }

    #[test]
    fn index_builds_metadata_sqlite_and_manifest_in_temp_cache() {
        let temp = TempDir::new("index-metadata");
        let repo = temp.path().join("repo");
        let registry_dir = temp.path().join("registry-cache");
        fs::create_dir_all(&repo).expect("create temp repo");
        let head = init_git_registry_with_policy(&registry_dir, INDEX_POLICY_YAML);
        let mut registry = test_registry(&registry_dir, "main", SyncMode::Manual);
        registry.url = registry_dir.display().to_string();
        let config = AgentPolicyConfig {
            registry: Some(registry),
            ..AgentPolicyConfig::default()
        };
        let cache_dir = temp.path().join("cache");

        let report = build_metadata_index_with_cache_dir(&repo, &config, &cache_dir)
            .expect("build metadata index");

        assert_eq!(report.policy_count, 1);
        assert!(!report.stale_before_build);
        assert!(report.metadata_path.exists());
        assert_eq!(
            report.metadata_path,
            cache_dir
                .join("indexes")
                .join("registry-cache")
                .join("metadata.sqlite")
        );
        assert!(report.manifest_path.exists());

        let manifest: IndexManifest = serde_json::from_str(
            &fs::read_to_string(&report.manifest_path).expect("read manifest"),
        )
        .expect("parse manifest");
        assert_eq!(manifest.source.name, "registry-cache");
        assert_eq!(manifest.source.commit.as_deref(), Some(head.as_str()));
        assert_eq!(manifest.indexes.metadata, "metadata.sqlite");
        assert_eq!(manifest.indexes.fulltext, "fulltext");
        assert!(report.fulltext_path.exists());
        assert!(report.fulltext_document_count >= 1);

        let connection = Connection::open(&report.metadata_path).expect("open metadata sqlite");
        let row = connection
            .query_row(
                "SELECT version, status, owner, priority, source_path, registry_commit
                 FROM policies WHERE id = 'org.index.metadata'",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                },
            )
            .expect("read indexed policy");
        assert_eq!(row.0, "2026.1");
        assert_eq!(row.1, "active");
        assert_eq!(row.2, "platform");
        assert_eq!(row.3, 42);
        assert!(row.4.ends_with("policies/policy.yaml"));
        assert_eq!(row.5, head);

        let mut statement = connection
            .prepare(
                "SELECT field, value FROM applies_when
                 WHERE policy_id = 'org.index.metadata'
                 ORDER BY field, value",
            )
            .expect("prepare applies_when query");
        let values = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .expect("query applies_when")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect applies_when");
        assert!(values.contains(&("frameworks".to_string(), "axum".to_string())));
        assert!(values.contains(&("languages".to_string(), "rust".to_string())));
        assert!(values.contains(&("package_managers".to_string(), "cargo".to_string())));
        assert!(values.contains(&("paths".to_string(), "crates/**".to_string())));
        assert!(values.contains(&("repos".to_string(), "agent-policy-broker".to_string())));
        assert!(values.contains(&("risk_flags".to_string(), "storage".to_string())));
        assert!(values.contains(&("task_types".to_string(), "implementation".to_string())));
    }

    #[test]
    fn index_builds_fulltext_candidates_with_nested_provenance_and_selected_docs() {
        let temp = TempRepo::copy_fixture("nested-instructions");
        let repo = temp.path();
        let policies_dir = repo.join(".agent-policy").join("policies");
        fs::create_dir_all(&policies_dir).expect("create policy dir");
        fs::write(
            policies_dir.join("payments.yaml"),
            r#"id: org.payments.refunds
version: 1
status: active
priority: 30
applies_when:
  paths:
    - backend/payments/**
retrieval:
  semantic_terms:
    - refund idempotency settlement retry
instructions:
  - Preserve payment invariants during refund changes.
"#,
        )
        .expect("write payments policy");
        fs::create_dir_all(repo.join("docs")).expect("create docs dir");
        fs::write(
            repo.join("docs").join("refund-playbook.md"),
            "# Refund playbook\n\nUse settlement reconciliation for refund retries.\n",
        )
        .expect("write selected doc");
        let config = AgentPolicyConfig {
            index: agent_policy_config::IndexConfig {
                include: vec!["docs/**/*.md".to_string()],
                exclude: Vec::new(),
                vector: agent_policy_config::VectorIndexConfig::default(),
            },
            ..AgentPolicyConfig::default()
        };
        let cache_dir = repo.join(".cache-test");

        let report = build_metadata_index_with_cache_dir(repo, &config, &cache_dir)
            .expect("build fulltext index");
        assert!(report.fulltext_path.exists());
        assert!(report.fulltext_document_count >= 10);

        let source = index_repo_source(repo).expect("repo source");
        let mut warnings = Vec::new();
        let candidates = search_fulltext_candidates(
            &cache_dir,
            &source,
            "refund settlement retry",
            8,
            &mut warnings,
        )
        .expect("search fulltext candidates");
        let ids = candidates
            .iter()
            .map(|candidate| candidate.id.as_str())
            .collect::<Vec<_>>();

        assert!(warnings.is_empty());
        assert!(ids.contains(&"org.payments.refunds"));
        assert!(ids.contains(&"doc:docs/refund-playbook.md"));

        let markdown_candidates =
            search_fulltext_candidates(&cache_dir, &source, "payment secrets", 8, &mut warnings)
                .expect("search markdown candidates");
        assert!(markdown_candidates.iter().any(|candidate| candidate
            .id
            .starts_with("markdown.backend.payments.agents.md.")));
    }

    #[test]
    fn registry_fulltext_index_does_not_walk_source_root_for_docs() {
        let temp = TempDir::new("registry-fulltext-boundary");
        let repo = temp.path().join("repo");
        let registry_dir = temp.path().join("registry-cache");
        fs::create_dir_all(&repo).expect("create temp repo");
        init_git_registry_with_policy(
            &registry_dir,
            "id: org.registry.safe\nversion: 1\nstatus: active\napplies_when: {}\ninstructions:\n  - Registry policy guidance stays searchable.\n",
        );
        fs::create_dir_all(registry_dir.join(".ssh")).expect("create sensitive dir");
        fs::write(
            registry_dir.join(".ssh").join("id_rsa"),
            "APB_REGISTRY_FULLTEXT_SECRET_DO_NOT_INDEX",
        )
        .expect("write sensitive file");
        fs::write(
            registry_dir.join("AGENTS.md"),
            "Registry-root markdown instructions should not be discovered during indexing.",
        )
        .expect("write registry-root instruction file");
        let mut registry = test_registry(&registry_dir, "main", SyncMode::Manual);
        registry.url = registry_dir.display().to_string();
        let config = AgentPolicyConfig {
            registry: Some(registry),
            index: agent_policy_config::IndexConfig {
                include: vec![".ssh/id_rsa".to_string(), "AGENTS.md".to_string()],
                exclude: Vec::new(),
                vector: agent_policy_config::VectorIndexConfig::default(),
            },
            ..AgentPolicyConfig::default()
        };
        let cache_dir = temp.path().join("cache");

        let report = build_metadata_index_with_cache_dir(&repo, &config, &cache_dir)
            .expect("build registry index");

        assert_eq!(report.fulltext_document_count, 1);
        let mut warnings = Vec::new();
        let leaked_candidates = search_fulltext_candidates(
            &cache_dir,
            &report.source,
            "APB_REGISTRY_FULLTEXT_SECRET_DO_NOT_INDEX discovered",
            8,
            &mut warnings,
        )
        .expect("search registry fulltext index");
        assert!(warnings.is_empty());
        assert!(leaked_candidates.is_empty());
    }

    #[test]
    fn bm25_candidates_do_not_override_exact_metadata_or_policy_priority() {
        let temp = TempDir::new("bm25-priority");
        let repo = temp.path().join("repo");
        let policies_dir = repo.join(".agent-policy").join("policies");
        fs::create_dir_all(&policies_dir).expect("create policy dir");
        fs::write(
            policies_dir.join("high.yaml"),
            r#"id: org.priority.high
version: 1
status: active
priority: 100
applies_when:
  paths:
    - src/payments/**
instructions:
  - Follow the high priority payment change process.
"#,
        )
        .expect("write high priority policy");
        fs::write(
            policies_dir.join("low.yaml"),
            r#"id: org.priority.low
version: 1
status: active
priority: 1
applies_when:
  paths:
    - src/payments/**
instructions:
  - Refund settlement retry keywords are useful candidate guidance only.
"#,
        )
        .expect("write low priority policy");
        let config = AgentPolicyConfig::default();
        let cache_dir = temp.path().join("cache");
        build_metadata_index_with_cache_dir(&repo, &config, &cache_dir).expect("build index");

        let source = index_repo_source(&repo).expect("repo source");
        let mut warnings = Vec::new();
        let candidates = search_fulltext_candidates(
            &cache_dir,
            &source,
            "refund settlement retry",
            4,
            &mut warnings,
        )
        .expect("search fulltext candidates");
        assert_eq!(
            candidates.first().map(|candidate| candidate.id.as_str()),
            Some("org.priority.low")
        );

        let policies = load_policies_from_dirs(&repo, &config.local_policies)
            .expect("load policies for bundle");
        let bundle = build_instruction_bundle(
            &TaskIntent {
                repo: Some("repo".to_string()),
                branch: None,
                task: Some(TaskDetails {
                    summary: Some("refund settlement retry".to_string()),
                    task_type: None,
                }),
                files: vec!["src/payments/refunds.ts".to_string()],
                detected: Some(DetectedContext::default()),
                risk_flags: Vec::new(),
                expected_commands: Vec::new(),
                expected_check_ids: Vec::new(),
                output_budget: None,
            },
            &policies,
            BundleBuildOptions {
                max_tokens: Some(2000),
                max_instructions: Some(10),
                max_required_checks: Some(10),
                max_blocked_actions: Some(10),
            },
        )
        .expect("build bundle");

        assert_eq!(
            bundle
                .instructions
                .first()
                .map(|instruction| instruction.text.as_str()),
            Some("Follow the high priority payment change process.")
        );
    }

    #[test]
    fn get_rejects_bm25_policy_candidate_when_paths_conflict() {
        let temp = TempDir::new("bm25-keyword-only");
        let repo = temp.path().join("repo");
        let policies_dir = repo.join(".agent-policy").join("policies");
        fs::create_dir_all(&policies_dir).expect("create policy dir");
        fs::write(
            policies_dir.join("keyword.yaml"),
            r#"id: org.keyword.refunds
version: 1
status: active
applies_when:
  paths:
    - docs/legacy-refunds/**
retrieval:
  semantic_terms:
    - refund settlement reconciliation retry idempotency
instructions:
  - Preserve refund settlement reconciliation during retry changes.
"#,
        )
        .expect("write keyword policy");
        let config = AgentPolicyConfig::default();
        let cache_dir = temp.path().join("cache");
        build_metadata_index_with_cache_dir(&repo, &config, &cache_dir).expect("build index");
        let intent = TaskIntent {
            repo: Some("repo".to_string()),
            branch: None,
            task: Some(TaskDetails {
                summary: Some("refund settlement retry".to_string()),
                task_type: None,
            }),
            files: vec!["src/payments/refunds.rs".to_string()],
            detected: Some(DetectedContext {
                languages: vec!["rust".to_string()],
                frameworks: Vec::new(),
                package_manager: None,
            }),
            risk_flags: Vec::new(),
            expected_commands: Vec::new(),
            expected_check_ids: Vec::new(),
            output_budget: None,
        };
        let mut warnings = Vec::new();
        let bm25_ids = bm25_candidate_policy_ids_with_cache_dir(
            &repo,
            &config,
            &intent,
            &cache_dir,
            &mut warnings,
        )
        .expect("bm25 candidates");
        let policies =
            load_policies_from_dirs(&repo, &config.local_policies).expect("load policies");

        let bundle = build_instruction_bundle_with_bm25_candidates(
            &intent,
            &policies,
            BundleBuildOptions {
                max_tokens: Some(900),
                max_instructions: Some(4),
                max_required_checks: Some(4),
                max_blocked_actions: Some(4),
            },
            &bm25_ids,
        )
        .expect("build bundle");

        assert!(warnings.is_empty());
        assert!(bm25_ids.contains("org.keyword.refunds"));
        assert!(bundle.instructions.is_empty());
        assert!(bundle.explanations.is_empty());
        assert_eq!(bundle.context_budget.exact_candidate_policies, Some(0));
        assert_eq!(bundle.context_budget.bm25_candidate_policies, Some(0));
    }

    #[test]
    fn exact_path_and_risk_policy_outranks_generic_bm25_match() {
        let temp = TempDir::new("bm25-exact-outranks");
        let repo = temp.path().join("repo");
        let policies_dir = repo.join(".agent-policy").join("policies");
        fs::create_dir_all(&policies_dir).expect("create policy dir");
        fs::write(
            policies_dir.join("exact.yaml"),
            r#"id: org.exact.payments
version: 1
status: active
applies_when:
  paths:
    - src/payments/**
  risk_flags:
    - payments
instructions:
  - Follow the payment-specific change process.
"#,
        )
        .expect("write exact policy");
        fs::write(
            policies_dir.join("generic.yaml"),
            r#"id: org.generic.refunds
version: 1
status: active
applies_when:
  paths:
    - docs/legacy-refunds/**
instructions:
  - Generic refund settlement retry guidance is candidate-only context.
"#,
        )
        .expect("write generic policy");
        let config = AgentPolicyConfig::default();
        let cache_dir = temp.path().join("cache");
        build_metadata_index_with_cache_dir(&repo, &config, &cache_dir).expect("build index");
        let intent = TaskIntent {
            repo: Some("repo".to_string()),
            branch: None,
            task: Some(TaskDetails {
                summary: Some("refund settlement retry".to_string()),
                task_type: None,
            }),
            files: vec!["src/payments/refunds.rs".to_string()],
            detected: Some(DetectedContext {
                languages: vec!["rust".to_string()],
                frameworks: Vec::new(),
                package_manager: None,
            }),
            risk_flags: vec!["payments".to_string()],
            expected_commands: Vec::new(),
            expected_check_ids: Vec::new(),
            output_budget: None,
        };
        let mut warnings = Vec::new();
        let bm25_ids = bm25_candidate_policy_ids_with_cache_dir(
            &repo,
            &config,
            &intent,
            &cache_dir,
            &mut warnings,
        )
        .expect("bm25 candidates");
        let policies =
            load_policies_from_dirs(&repo, &config.local_policies).expect("load policies");

        let bundle = build_instruction_bundle_with_bm25_candidates(
            &intent,
            &policies,
            BundleBuildOptions {
                max_tokens: Some(900),
                max_instructions: Some(2),
                max_required_checks: Some(4),
                max_blocked_actions: Some(4),
            },
            &bm25_ids,
        )
        .expect("build bundle");

        assert!(warnings.is_empty());
        assert!(bm25_ids.contains("org.generic.refunds"));
        assert_eq!(
            bundle.instructions[0].text,
            "Follow the payment-specific change process."
        );
        assert_eq!(bundle.instructions.len(), 1);
        assert_eq!(bundle.context_budget.exact_candidate_policies, Some(1));
        assert_eq!(bundle.context_budget.bm25_candidate_policies, Some(0));
    }

    #[test]
    fn index_reports_stale_manifest_when_registry_commit_changes() {
        let temp = TempDir::new("index-stale");
        let repo = temp.path().join("repo");
        let registry_dir = temp.path().join("registry-cache");
        fs::create_dir_all(&repo).expect("create temp repo");
        let first_head = init_git_registry_with_policy(&registry_dir, INDEX_POLICY_YAML);
        let mut registry = test_registry(&registry_dir, "main", SyncMode::Manual);
        registry.url = registry_dir.display().to_string();
        let config = AgentPolicyConfig {
            registry: Some(registry),
            ..AgentPolicyConfig::default()
        };
        let cache_dir = temp.path().join("cache");

        let first_report = build_metadata_index_with_cache_dir(&repo, &config, &cache_dir)
            .expect("first index build");
        assert!(!first_report.stale_before_build);

        fs::write(
            registry_dir.join("policies").join("second.yaml"),
            "id: org.index.second\nversion: 1\nstatus: active\napplies_when: {}\ninstructions:\n  - Second policy.\n",
        )
        .expect("write second policy");
        git(&registry_dir, &["add", "."]);
        git(
            &registry_dir,
            &[
                "-c",
                "user.name=Agent Policy Tests",
                "-c",
                "user.email=agent-policy-tests@example.invalid",
                "commit",
                "-m",
                "second registry commit",
            ],
        );
        let second_head = git_stdout(&registry_dir, &["rev-parse", "HEAD"]);
        assert_ne!(first_head, second_head);

        let second_report = build_metadata_index_with_cache_dir(&repo, &config, &cache_dir)
            .expect("second index build");
        assert!(second_report.stale_before_build);

        let manifest: IndexManifest = serde_json::from_str(
            &fs::read_to_string(&second_report.manifest_path).expect("read updated manifest"),
        )
        .expect("parse updated manifest");
        assert_eq!(
            manifest.source.commit.as_deref(),
            Some(second_head.as_str())
        );
    }

    #[test]
    fn get_uses_valid_metadata_index_for_candidate_lookup() {
        let temp = TempDir::new("get-indexed");
        let repo = temp.path().join("repo");
        write_get_policy_fixture(&repo);
        let config = AgentPolicyConfig::default();
        let cache_dir = temp.path().join("cache");
        build_metadata_index_with_cache_dir(&repo, &config, &cache_dir).expect("build index");

        let indexed = load_get_policies_with_cache_dir(&repo, &config, &cache_dir)
            .expect("load indexed policies");
        let direct =
            load_policies_from_dirs(&repo, &config.local_policies).expect("load direct policies");

        assert!(indexed.warnings.is_empty());
        assert_eq!(
            policy_ids(&indexed.policies),
            vec!["org.get.active".to_string()]
        );
        assert_eq!(
            get_bundle_json(&indexed.policies),
            get_bundle_json(&direct),
            "indexed lookup should produce the same bundle content as direct loading"
        );
    }

    #[test]
    fn get_falls_back_when_metadata_index_is_missing() {
        let temp = TempDir::new("get-missing-index");
        let repo = temp.path().join("repo");
        write_get_policy_fixture(&repo);
        let config = AgentPolicyConfig::default();
        let cache_dir = temp.path().join("cache");

        let loaded = load_get_policies_with_cache_dir(&repo, &config, &cache_dir)
            .expect("load direct policies without index");

        assert!(loaded
            .warnings
            .iter()
            .any(|warning| warning.contains("Metadata index missing")));
        assert_eq!(
            get_bundle_json(&loaded.policies),
            get_bundle_json(
                &load_policies_from_dirs(&repo, &config.local_policies)
                    .expect("load direct policies")
            )
        );
    }

    #[test]
    fn get_falls_back_when_metadata_index_is_stale() {
        let temp = TempDir::new("get-stale-index");
        let repo = temp.path().join("repo");
        write_get_policy_fixture(&repo);
        let config = AgentPolicyConfig::default();
        let cache_dir = temp.path().join("cache");
        let report =
            build_metadata_index_with_cache_dir(&repo, &config, &cache_dir).expect("build index");
        let mut manifest: IndexManifest = serde_json::from_str(
            &fs::read_to_string(&report.manifest_path).expect("read manifest"),
        )
        .expect("parse manifest");
        manifest.source.path = temp.path().join("other-repo").display().to_string();
        fs::write(
            &report.manifest_path,
            format!(
                "{}\n",
                serde_json::to_string_pretty(&manifest).expect("serialize manifest")
            ),
        )
        .expect("write stale manifest");

        let loaded = load_get_policies_with_cache_dir(&repo, &config, &cache_dir)
            .expect("load policies with stale index");

        assert!(loaded
            .warnings
            .iter()
            .any(|warning| warning.contains("is stale")));
        assert_eq!(
            get_bundle_json(&loaded.policies),
            get_bundle_json(
                &load_policies_from_dirs(&repo, &config.local_policies)
                    .expect("load direct policies")
            )
        );
    }

    #[test]
    fn inspect_nested_fixture_reports_sources_candidates_and_migration_groups() {
        let repo = fixture_repo("nested-instructions");
        let discovered = discover(&repo).expect("discover fixture repo");
        let report = inspect_repo(&repo, discovered);

        assert_eq!(report.repo, "nested-instructions");
        assert!(report.summary.source_count >= 5);
        assert!(report.summary.candidate_instruction_count >= 9);
        assert!(report
            .instruction_sources
            .iter()
            .any(|source| source.path == "backend/payments/AGENTS.md"
                && source.scope == "backend/payments/**"));
        assert!(report
            .candidate_instructions
            .iter()
            .any(|candidate| candidate.text == "Never log payment secrets."
                && candidate.topic == "secrets"));
        assert!(report
            .migration_candidates
            .iter()
            .any(|candidate| candidate.source == "backend/payments/AGENTS.md"));

        let json = render_inspection_json(&report);
        assert!(json.contains("\"instruction_sources\""));
        assert!(json.contains("\"duplicates\""));
        assert!(json.contains("\"conflicts\""));

        let markdown = render_inspection_markdown(&report);
        assert!(markdown.contains("# Agent Policy Inspection"));
        assert!(markdown.contains("## Duplicates"));
        assert!(markdown.contains("## Conflicts"));
        assert!(markdown.contains("## Migration Candidates"));
    }

    #[test]
    fn migrate_dry_run_proposes_draft_policy_yaml_for_fixture_repo() {
        let repo = fixture_repo("nested-instructions");
        let discovered = discover(&repo).expect("discover fixture repo");
        let inspection = inspect_repo(&repo, discovered);
        let report = migration_dry_run_report(&inspection);

        assert_eq!(report.mode, "dry_run");
        assert!(report
            .drafts
            .iter()
            .any(|draft| draft.id == "local.backend.payments.payments"
                && draft.target_path
                    == ".agent-policy/migration/local.backend.payments.payments.yaml"
                && draft.policy_yaml.contains("status: draft")
                && draft.policy_yaml.contains("generated_from:")
                && draft.policy_yaml.contains("Preserve payment invariants.")));
        let payment_draft = report
            .drafts
            .iter()
            .find(|draft| draft.id == "local.backend.payments.payments")
            .expect("payment draft");
        assert_eq!(
            payment_draft.policy_yaml,
            PAYMENT_POLICY_DRY_RUN_YAML_SNAPSHOT
        );
        let checks_draft = report
            .drafts
            .iter()
            .find(|draft| draft.id == "repo.checks")
            .expect("checks draft");
        assert_eq!(
            checks_draft.policy_yaml,
            CHECKS_POLICY_DRY_RUN_YAML_SNAPSHOT
        );

        let json = render_migration_dry_run_json(&report);
        assert!(json.contains("\"mode\": \"dry_run\""));
        assert!(json.contains("\"policy_yaml\""));
        assert!(json.contains("generated_from"));

        let markdown = render_migration_dry_run_markdown(&report);
        assert!(markdown.contains("# Agent Policy Migration Dry Run"));
        assert!(markdown.contains("```yaml"));
        assert!(markdown.contains(".agent-policy/migration/local.backend.payments.payments.yaml"));
    }

    #[test]
    fn migrate_dry_run_does_not_modify_instruction_files() {
        let repo = fixture_repo("nested-instructions");
        let agents_path = repo.join("AGENTS.md");
        let before = fs::read_to_string(&agents_path).expect("read fixture before");

        let cli = Cli::try_parse_from([
            "agent-policy",
            "--repo",
            repo.to_str().expect("utf8 repo"),
            "migrate",
            "--dry-run",
            "--format",
            "json",
        ])
        .expect("parse migrate");
        run(cli).expect("run dry-run migration");

        let after = fs::read_to_string(&agents_path).expect("read fixture after");
        assert_eq!(after, before);
    }

    #[test]
    fn migrate_write_creates_draft_policy_files_without_touching_instruction_files() {
        let temp = TempRepo::copy_fixture("nested-instructions");
        let repo = temp.path();
        fs::write(
            repo.join("CLAUDE.md"),
            "# Claude Instructions\n\n- Keep Claude-specific guidance intact.\n",
        )
        .expect("write temp claude file");
        let repo_files_before = repo_file_contents(repo);
        let instruction_files_before = instruction_file_contents(repo);

        let cli = Cli::try_parse_from([
            "agent-policy",
            "--repo",
            repo.to_str().expect("utf8 repo"),
            "migrate",
            "--write",
            "--format",
            "json",
        ])
        .expect("parse migrate write");
        run(cli).expect("run write migration");

        assert_eq!(instruction_file_contents(repo), instruction_files_before);
        assert_only_migration_files_were_added(&repo_files_before, &repo_file_contents(repo));

        let migration_dir = repo.join(".agent-policy").join("migration");
        assert!(migration_dir.is_dir());

        let payment_policy_path = migration_dir.join("local.backend.payments.payments.yaml");
        let payment_policy =
            fs::read_to_string(&payment_policy_path).expect("read written payment policy");
        assert_eq!(payment_policy, PAYMENT_POLICY_DRY_RUN_YAML_SNAPSHOT);
        assert!(payment_policy.contains("status: draft"));
        assert!(payment_policy.contains("generated_from:"));
        assert!(payment_policy.contains("path: \"backend/payments/AGENTS.md\""));

        let first_written = written_migration_file_contents(&migration_dir);
        let cli = Cli::try_parse_from([
            "agent-policy",
            "--repo",
            repo.to_str().expect("utf8 repo"),
            "migrate",
            "--write",
            "--format",
            "json",
        ])
        .expect("parse second migrate write");
        run(cli).expect("run second write migration");
        assert_eq!(
            written_migration_file_contents(&migration_dir),
            first_written
        );
        assert_eq!(instruction_file_contents(repo), instruction_files_before);
        assert_only_migration_files_were_added(&repo_files_before, &repo_file_contents(repo));
    }

    #[test]
    fn inspect_reports_exact_duplicates_and_basic_conflicts() {
        let candidates = vec![
            test_inspection_candidate(
                "Use pnpm for package commands.",
                "AGENTS.md",
                2,
                ".",
                "package_manager",
                MigrationClass::RepoPolicy,
            ),
            test_inspection_candidate(
                "Use npm for package commands.",
                "frontend/AGENTS.md",
                3,
                "frontend/**",
                "package_manager",
                MigrationClass::KeepLocal,
            ),
            test_inspection_candidate(
                "Do not edit generated files directly.",
                "AGENTS.md",
                4,
                ".",
                "generated_files",
                MigrationClass::SharedRegistryPolicy,
            ),
            test_inspection_candidate(
                "Do not edit generated files directly.",
                "backend/AGENTS.md",
                4,
                "backend/**",
                "generated_files",
                MigrationClass::SharedRegistryPolicy,
            ),
            test_inspection_candidate(
                "Edit generated files directly for emergency fixes.",
                "backend/AGENTS.md",
                5,
                "backend/**",
                "generated_files",
                MigrationClass::KeepLocal,
            ),
        ];

        let duplicates = detect_inspection_duplicates(&candidates);
        assert_eq!(duplicates.len(), 1);
        assert_eq!(
            duplicates[0].instruction,
            "Do not edit generated files directly."
        );
        assert_eq!(
            duplicates[0].sources,
            vec!["AGENTS.md:4", "backend/AGENTS.md:4"]
        );

        let conflicts = detect_inspection_conflicts(&candidates);
        assert!(conflicts
            .iter()
            .any(|conflict| conflict.topic == "package_manager"));
        assert!(conflicts
            .iter()
            .any(|conflict| conflict.topic == "generated_files"));
    }

    #[test]
    fn markdown_candidates_are_added_to_get_bundle_with_provenance() {
        let repo = fixture_repo("nested-instructions");
        let discovered = discover(&repo).expect("discover fixture repo");
        let files = vec!["backend/payments/src/refunds.ts".to_string()];
        let policies = markdown_candidate_policies(&repo, &discovered, &files, &[".".to_string()]);
        let bundle = build_instruction_bundle(
            &TaskIntent {
                repo: Some("nested-instructions".into()),
                branch: None,
                task: Some(TaskDetails {
                    summary: Some("update refunds".into()),
                    task_type: None,
                }),
                files,
                detected: Some(DetectedContext::default()),
                risk_flags: Vec::new(),
                expected_commands: Vec::new(),
                expected_check_ids: Vec::new(),
                output_budget: Some(OutputBudget::default()),
            },
            &policies,
            BundleBuildOptions {
                max_tokens: Some(900),
                max_instructions: Some(20),
                max_required_checks: Some(20),
                max_blocked_actions: Some(20),
            },
        )
        .expect("build bundle");

        let instruction_texts = bundle
            .instructions
            .iter()
            .map(|instruction| instruction.text.as_str())
            .collect::<Vec<_>>();
        assert!(instruction_texts.contains(&"Use the repository policy broker configuration."));
        assert!(instruction_texts.contains(&"Backend changes require service-level tests."));
        assert!(instruction_texts.contains(&"Preserve payment invariants."));
        assert!(instruction_texts.contains(&"Never log payment secrets."));
        assert!(!instruction_texts
            .iter()
            .any(|text| text.contains("several examples")));

        assert!(bundle.required_checks.iter().any(|check| {
            check.id == "cargo test -p payments"
                && check.source.as_ref().is_some_and(|source| {
                    source.0.contains("markdown:backend/payments/AGENTS.md:8")
                        && source.0.contains("scope=backend/payments/**")
                        && source.0.contains("type=agents_md")
                })
        }));
    }

    #[test]
    fn nested_markdown_candidates_require_matching_task_files() {
        let repo = fixture_repo("nested-instructions");
        let discovered = discover(&repo).expect("discover fixture repo");
        let files = vec!["frontend/src/App.tsx".to_string()];
        let policies = markdown_candidate_policies(&repo, &discovered, &files, &[".".to_string()]);
        let bundle = build_instruction_bundle(
            &TaskIntent {
                repo: Some("nested-instructions".into()),
                branch: None,
                task: None,
                files,
                detected: Some(DetectedContext::default()),
                risk_flags: Vec::new(),
                expected_commands: Vec::new(),
                expected_check_ids: Vec::new(),
                output_budget: None,
            },
            &policies,
            BundleBuildOptions {
                max_tokens: Some(900),
                max_instructions: Some(20),
                max_required_checks: Some(20),
                max_blocked_actions: Some(20),
            },
        )
        .expect("build bundle");

        let instruction_texts = bundle
            .instructions
            .iter()
            .map(|instruction| instruction.text.as_str())
            .collect::<Vec<_>>();
        assert!(instruction_texts.contains(&"Prefer accessible controls."));
        assert!(!instruction_texts.contains(&"Backend changes require service-level tests."));
        assert!(!instruction_texts.contains(&"Preserve payment invariants."));
    }

    #[test]
    fn nested_scope_matching_is_file_based() {
        assert!(scope_matches_task_files(
            "backend/**",
            &["backend/payments/src/refunds.ts".into()]
        ));
        assert!(!scope_matches_task_files(
            "backend/**",
            &["frontend/src/App.tsx".into()]
        ));
        assert!(!scope_matches_task_files("backend/**", &[]));
        assert!(scope_matches_task_files(".", &[]));
    }

    fn test_inspection_candidate(
        text: &str,
        source: &str,
        line: usize,
        scope: &str,
        topic: &str,
        migration_class: MigrationClass,
    ) -> InspectionCandidate {
        InspectionCandidate {
            text: text.into(),
            source: source.into(),
            line,
            scope: scope.into(),
            candidate_type: "instruction".into(),
            topic: topic.into(),
            migration_class,
            target_policy: Some("test.policy".into()),
        }
    }

    fn write_get_policy_fixture(repo: &Path) {
        let policies_dir = repo.join(".agent-policy").join("policies");
        fs::create_dir_all(&policies_dir).expect("create policies dir");
        fs::write(
            policies_dir.join("active.yaml"),
            r#"id: org.get.active
version: 1
status: active
priority: 10
applies_when:
  paths:
    - crates/**
  languages:
    - rust
instructions:
  - Use the get metadata index when it is valid.
"#,
        )
        .expect("write active policy");
        fs::write(
            policies_dir.join("draft.yaml"),
            r#"id: org.get.draft
version: 1
status: draft
applies_when: {}
instructions:
  - Draft guidance should not appear in get bundles.
"#,
        )
        .expect("write draft policy");
    }

    fn policy_ids(policies: &[LoadedPolicy]) -> Vec<String> {
        policies
            .iter()
            .map(|loaded| loaded.policy.id.clone())
            .collect()
    }

    fn get_bundle_json(policies: &[LoadedPolicy]) -> String {
        let intent = TaskIntent {
            repo: Some("repo".to_string()),
            branch: None,
            task: Some(TaskDetails {
                summary: Some("implement indexed get".to_string()),
                task_type: None,
            }),
            files: vec!["crates/agent-policy-cli/src/main.rs".to_string()],
            detected: Some(DetectedContext {
                languages: vec!["rust".to_string()],
                frameworks: Vec::new(),
                package_manager: None,
            }),
            risk_flags: Vec::new(),
            expected_commands: Vec::new(),
            expected_check_ids: Vec::new(),
            output_budget: Some(OutputBudget {
                max_tokens: Some(2000),
                max_instructions: Some(10),
                max_required_checks: Some(10),
                max_blocked_actions: Some(10),
                include_examples: Some(false),
                include_explanations: Some("brief".to_string()),
            }),
        };
        let bundle = build_instruction_bundle(
            &intent,
            policies,
            BundleBuildOptions {
                max_tokens: Some(2000),
                max_instructions: Some(10),
                max_required_checks: Some(10),
                max_blocked_actions: Some(10),
            },
        )
        .expect("build bundle");
        render_bundle_json(&bundle).expect("render bundle json")
    }

    fn fixture_repo(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
    }

    struct TempRepo {
        path: PathBuf,
    }

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(name: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "agent-policy-cli-{name}-{}-{nonce}",
                std::process::id()
            ));
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

    impl TempRepo {
        fn copy_fixture(name: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "agent-policy-cli-{name}-{}-{nonce}",
                std::process::id()
            ));
            copy_dir_all(&fixture_repo(name), &path).expect("copy fixture to temp repo");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn copy_dir_all(source: &Path, destination: &Path) -> std::io::Result<()> {
        fs::create_dir_all(destination)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let target = destination.join(entry.file_name());
            if file_type.is_dir() {
                copy_dir_all(&entry.path(), &target)?;
            } else if file_type.is_file() {
                fs::copy(entry.path(), target)?;
            }
        }
        Ok(())
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

    fn instruction_file_contents(repo: &Path) -> BTreeMap<String, String> {
        let mut contents = BTreeMap::new();
        collect_instruction_file_contents(repo, repo, &mut contents);
        contents
    }

    fn repo_file_contents(repo: &Path) -> BTreeMap<String, String> {
        let mut contents = BTreeMap::new();
        collect_repo_file_contents(repo, repo, &mut contents);
        contents
    }

    fn collect_repo_file_contents(
        repo: &Path,
        directory: &Path,
        contents: &mut BTreeMap<String, String>,
    ) {
        for entry in fs::read_dir(directory).expect("read directory") {
            let entry = entry.expect("read directory entry");
            let path = entry.path();
            let file_type = entry.file_type().expect("entry file type");
            if file_type.is_dir() {
                collect_repo_file_contents(repo, &path, contents);
            } else if file_type.is_file() {
                let relative = path
                    .strip_prefix(repo)
                    .expect("repo-relative file path")
                    .to_string_lossy()
                    .replace('\\', "/");
                contents.insert(relative, fs::read_to_string(&path).expect("read repo file"));
            }
        }
    }

    fn test_registry(cache_dir: &Path, requested_ref: &str, mode: SyncMode) -> RegistryConfig {
        RegistryConfig {
            registry_type: "git".to_string(),
            url: "https://example.invalid/company/registry.git".to_string(),
            r#ref: requested_ref.to_string(),
            cache_dir: cache_dir.display().to_string(),
            sync: RegistrySyncConfig {
                mode,
                max_age_minutes: None,
            },
        }
    }

    fn init_git_registry(path: &Path) -> String {
        init_git_registry_with_policy(
            path,
            "id: org.test\nversion: 1\nstatus: active\ninstructions:\n  - Test policy.\n",
        )
    }

    fn init_git_registry_with_policy(path: &Path, policy_yaml: &str) -> String {
        fs::create_dir_all(path.join("policies")).expect("create registry policy dir");
        fs::write(path.join("policies").join("policy.yaml"), policy_yaml).expect("write policy");
        git(path, &["init"]);
        git(path, &["checkout", "-b", "main"]);
        git(path, &["add", "."]);
        git(
            path,
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
        git_stdout(path, &["rev-parse", "HEAD"])
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

    fn assert_only_migration_files_were_added(
        before: &BTreeMap<String, String>,
        after: &BTreeMap<String, String>,
    ) {
        for (path, contents) in before {
            assert_eq!(
                after.get(path),
                Some(contents),
                "pre-existing file changed: {path}"
            );
        }
        for path in after.keys() {
            assert!(
                before.contains_key(path) || path.starts_with(".agent-policy/migration/"),
                "unexpected generated path: {path}"
            );
        }
    }

    fn collect_instruction_file_contents(
        repo: &Path,
        directory: &Path,
        contents: &mut BTreeMap<String, String>,
    ) {
        for entry in fs::read_dir(directory).expect("read directory") {
            let entry = entry.expect("read directory entry");
            let path = entry.path();
            let file_type = entry.file_type().expect("entry file type");
            if file_type.is_dir() {
                collect_instruction_file_contents(repo, &path, contents);
            } else if file_type.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| matches!(name, "AGENTS.md" | "CLAUDE.md"))
            {
                let relative = path
                    .strip_prefix(repo)
                    .expect("repo-relative instruction path")
                    .to_string_lossy()
                    .replace('\\', "/");
                contents.insert(
                    relative,
                    fs::read_to_string(&path).expect("read instruction file"),
                );
            }
        }
    }

    fn written_migration_file_contents(migration_dir: &Path) -> BTreeMap<String, String> {
        let mut contents = BTreeMap::new();
        for entry in fs::read_dir(migration_dir).expect("read migration dir") {
            let entry = entry.expect("read migration entry");
            let path = entry.path();
            if entry.file_type().expect("migration file type").is_file() {
                contents.insert(
                    entry.file_name().to_string_lossy().into_owned(),
                    fs::read_to_string(path).expect("read migration file"),
                );
            }
        }
        contents
    }

    const PAYMENT_POLICY_DRY_RUN_YAML_SNAPSHOT: &str = r#"id: local.backend.payments.payments
version: 1
status: draft

applies_when:
  paths:
    - "backend/payments/**"

instructions:
  - "Preserve payment invariants."

metadata:
  generated_from:
    - path: "backend/payments/AGENTS.md"
      source_type: agents_md
      scope: "backend/payments/**"
      lines:
        - 3
  migration_status: proposed
  migration_class: keep_local
"#;

    const CHECKS_POLICY_DRY_RUN_YAML_SNAPSHOT: &str = r#"id: repo.checks
version: 1
status: draft

applies_when: {}

instructions: []

required_checks:
  - "cargo test"

metadata:
  generated_from:
    - path: "AGENTS.md"
      source_type: agents_md
      scope: "."
      lines:
        - 13
  migration_status: proposed
  migration_class: repo_policy
"#;
}
