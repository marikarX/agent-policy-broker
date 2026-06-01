use agent_policy_config::{load_config, load_config_from_path};
use agent_policy_discover::{discover_codex_json, discover_json, CodexDiscoveryOptions};

use crate::cli::{DiscoverArgs, GlobalArgs, InstructionDiscoveryMode, OutputFormat};

pub(crate) fn run(global: &GlobalArgs, args: DiscoverArgs) -> anyhow::Result<()> {
    let repo = global
        .repo
        .as_deref()
        .unwrap_or_else(|| std::path::Path::new("."));
    match global.format.clone().unwrap_or(OutputFormat::Json) {
        OutputFormat::Json => {
            let json = match args.mode {
                InstructionDiscoveryMode::Generic => discover_json(repo)?,
                InstructionDiscoveryMode::Codex => {
                    discover_codex_json(repo, codex_options(global, repo)?)?
                }
            };
            println!("{}", json);
            Ok(())
        }
        OutputFormat::Markdown => {
            anyhow::bail!("markdown output is not implemented for `discover`; use `--format json`")
        }
    }
}

pub(crate) fn codex_options(
    global: &GlobalArgs,
    repo: &std::path::Path,
) -> anyhow::Result<CodexDiscoveryOptions> {
    let config = match &global.config {
        Some(path) => load_config_from_path(path)?,
        None => load_config(repo)?,
    };
    Ok(CodexDiscoveryOptions {
        codex_home: config.codex.home.map(std::path::PathBuf::from),
        current_dir: config.codex.current_dir.map(std::path::PathBuf::from),
        project_doc_fallback_filenames: config.codex.project_doc_fallback_filenames,
        project_doc_max_bytes: config.codex.project_doc_max_bytes,
        include_global: config.codex.include_global,
    })
}
