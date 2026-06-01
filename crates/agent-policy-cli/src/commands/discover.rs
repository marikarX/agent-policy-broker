use agent_policy_discover::discover_json;

use crate::cli::{GlobalArgs, OutputFormat};

pub(crate) fn run(global: &GlobalArgs) -> anyhow::Result<()> {
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
