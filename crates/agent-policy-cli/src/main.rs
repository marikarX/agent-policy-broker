use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use std::process::ExitCode;

use agent_policy_discover::discover_json;

#[derive(Debug, Parser)]
#[command(name = "agent-policy", version, about = "Agent Policy Broker CLI")]
struct Cli {
    #[command(flatten)]
    global: GlobalArgs,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, Debug, ValueEnum)]
enum OutputFormat {
    Json,
    Markdown,
}

#[derive(Debug, Args)]
struct GlobalArgs {
    #[arg(long, global = true, value_name = "path")]
    repo: Option<PathBuf>,
    #[arg(long, global = true, value_name = "path")]
    config: Option<PathBuf>,
    #[arg(long, global = true, value_enum)]
    format: Option<OutputFormat>,
    #[arg(long, global = true)]
    verbose: bool,
    #[arg(long, global = true)]
    quiet: bool,
    #[arg(long, global = true)]
    no_network: bool,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Compile a task-specific instruction bundle.
    Get,
    /// Discover existing instruction sources in a repository.
    Discover,
    /// Validate policies, config, and discovered instruction sources.
    Validate,
    /// Inspect repository guidance and produce an audit report.
    Inspect,
    /// Propose policy drafts from existing instruction sources.
    Migrate,
    /// Build or rebuild local retrieval indexes.
    Index,
    /// Manage policy registries.
    Registry(RegistryArgs),
    /// Run a local service for repeated lookups and integrations.
    Serve,
}

#[derive(Debug, Args)]
struct RegistryArgs {
    #[command(subcommand)]
    command: RegistryCommands,
}

#[derive(Debug, Subcommand)]
enum RegistryCommands {
    /// Fetch or update a Git-backed policy registry.
    Sync,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::from(1)
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Commands::Discover => run_discover(&cli.global),
        Commands::Get => not_implemented("get"),
        Commands::Validate => not_implemented("validate"),
        Commands::Inspect => not_implemented("inspect"),
        Commands::Migrate => not_implemented("migrate"),
        Commands::Index => not_implemented("index"),
        Commands::Serve => not_implemented("serve"),
        Commands::Registry(registry) => match registry.command {
            RegistryCommands::Sync => not_implemented("registry sync"),
        },
    }
}

fn run_discover(global: &GlobalArgs) -> anyhow::Result<()> {
    let repo = global
        .repo
        .as_deref()
        .unwrap_or_else(|| std::path::Path::new("."));
    match global.format.clone().unwrap_or(OutputFormat::Json) {
        OutputFormat::Json => {
            let json = discover_json(repo)?;
            println!("{}", json);
            Ok(())
        }
        OutputFormat::Markdown => {
            anyhow::bail!("markdown output is not implemented for `discover`; use `--format json`")
        }
    }
}

fn not_implemented(command_name: &str) -> anyhow::Result<()> {
    anyhow::bail!(
        "command `{command_name}` is not implemented yet. Use `agent-policy {command_name} --help` for usage details."
    )
}

#[cfg(test)]
mod tests {
    use super::{Cli, Commands, GlobalArgs, OutputFormat, RegistryCommands};
    use clap::{error::ErrorKind, CommandFactory, Parser};
    use std::path::PathBuf;

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
        assert!(matches!(cli.command, Commands::Get));
    }

    #[test]
    fn parse_discover_with_json_format() {
        let cli = Cli::try_parse_from(["agent-policy", "discover", "--format", "json"])
            .expect("parse discover");
        assert!(matches!(cli.global.format, Some(OutputFormat::Json)));
        assert!(matches!(cli.command, Commands::Discover));
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
}
