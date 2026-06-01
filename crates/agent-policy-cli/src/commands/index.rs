use std::path::Path;

use agent_policy_config::{load_config, load_config_from_path};

use crate::cli::{GlobalArgs, OutputFormat};
use crate::indexing::{
    build_metadata_index, render_index_report_json, render_index_report_markdown,
};

pub(crate) fn run(global: &GlobalArgs) -> anyhow::Result<()> {
    let repo = global.repo.as_deref().unwrap_or_else(|| Path::new("."));
    let config = match &global.config {
        Some(path) => load_config_from_path(path)?,
        None => load_config(repo)?,
    };
    let report = build_metadata_index(repo, &config)?;

    if !global.quiet {
        match global.format.clone().unwrap_or(OutputFormat::Markdown) {
            OutputFormat::Json => println!("{}", render_index_report_json(&report)),
            OutputFormat::Markdown => print!("{}", render_index_report_markdown(&report)),
        }
    }

    Ok(())
}
