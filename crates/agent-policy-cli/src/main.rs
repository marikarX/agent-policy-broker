use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use std::process::ExitCode;

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
    run(cli)
}

fn run(cli: Cli) -> ExitCode {
    let command_name = match cli.command {
        Commands::Get => "get",
        Commands::Discover => "discover",
        Commands::Validate => "validate",
        Commands::Inspect => "inspect",
        Commands::Migrate => "migrate",
        Commands::Index => "index",
        Commands::Serve => "serve",
        Commands::Registry(registry) => match registry.command {
            RegistryCommands::Sync => "registry sync",
        },
    };

    eprintln!(
        "command `{command_name}` is not implemented yet. Use `agent-policy {command_name} --help` for usage details."
    );
    ExitCode::from(1)
}

#[cfg(test)]
mod tests {
    use super::{Cli, GlobalArgs, OutputFormat};
    use clap::CommandFactory;

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
}
