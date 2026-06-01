use std::path::{Path, PathBuf};

pub(crate) fn resolve_configured_path(repo: &Path, raw: &str) -> anyhow::Result<PathBuf> {
    let expanded = expand_home(raw)?;
    let path = PathBuf::from(expanded);
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(repo.join(path))
    }
}

fn expand_home(raw: &str) -> anyhow::Result<String> {
    if raw == "~" {
        return std::env::var("HOME").map_err(|_| anyhow::anyhow!("HOME is not set"));
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        let home = std::env::var("HOME").map_err(|_| anyhow::anyhow!("HOME is not set"))?;
        return Ok(Path::new(&home).join(rest).display().to_string());
    }
    Ok(raw.to_string())
}
