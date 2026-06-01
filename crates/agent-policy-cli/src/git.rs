use std::path::Path;
use std::process::Command;

pub(crate) fn is_git_worktree(path: &Path) -> bool {
    path.join(".git").exists()
}

pub(crate) fn git_rev_parse(repo: &Path, rev: &str) -> anyhow::Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("rev-parse")
        .arg("--verify")
        .arg(rev)
        .output()?;
    if !output.status.success() {
        anyhow::bail!(
            "git rev-parse failed for `{}` in {}: {}",
            rev,
            repo.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(crate) fn is_full_sha(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|character| character.is_ascii_hexdigit())
}
