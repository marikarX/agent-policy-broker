use std::fs;
use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use agent_policy_discover::discover;

use crate::cli::{GlobalArgs, MigrateArgs, OutputFormat};
use crate::commands::inspect::{
    inspect_repo, migration_dry_run_report, render_migration_dry_run_json,
    render_migration_dry_run_markdown, PolicyDraft,
};

pub(crate) fn run(global: &GlobalArgs, args: MigrateArgs) -> anyhow::Result<()> {
    if args.dry_run && args.write {
        anyhow::bail!("migrate accepts either `--dry-run` or `--write`, not both");
    }
    if !args.dry_run && !args.write {
        anyhow::bail!("migrate requires either `--dry-run` or `--write`");
    }

    let repo = global.repo.as_deref().unwrap_or_else(|| Path::new("."));
    let discovered = discover(repo)?;
    let inspection = inspect_repo(repo, discovered);
    let mut report = migration_dry_run_report(&inspection);

    if args.write {
        write_migration_drafts(repo, &report.drafts)?;
        report.mode = "write";
    }

    match global.format.clone().unwrap_or(OutputFormat::Markdown) {
        OutputFormat::Json => {
            println!("{}", render_migration_dry_run_json(&report));
        }
        OutputFormat::Markdown => {
            print!("{}", render_migration_dry_run_markdown(&report));
        }
    }

    Ok(())
}

fn write_migration_drafts(repo: &Path, drafts: &[PolicyDraft]) -> anyhow::Result<()> {
    let repo = repo.canonicalize()?;
    let migration_dir = ensure_safe_migration_dir(&repo)?;

    for draft in drafts {
        let relative_target = Path::new(&draft.target_path);
        if relative_target.is_absolute()
            || relative_target.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::ParentDir | std::path::Component::RootDir
                )
            })
            || !relative_target.starts_with(".agent-policy/migration")
        {
            anyhow::bail!("refusing to write migration draft outside .agent-policy/migration");
        }

        let target = repo.join(relative_target);
        let parent = target
            .parent()
            .ok_or_else(|| anyhow::anyhow!("migration draft path has no parent"))?;
        if parent != migration_dir {
            anyhow::bail!("refusing to write nested migration draft path");
        }

        write_migration_draft_file(&target, &draft.policy_yaml)?;
    }

    Ok(())
}

fn ensure_safe_migration_dir(repo: &Path) -> anyhow::Result<PathBuf> {
    let agent_policy_dir = repo.join(".agent-policy");
    ensure_safe_directory(&agent_policy_dir, ".agent-policy")?;

    let migration_dir = agent_policy_dir.join("migration");
    ensure_safe_directory(&migration_dir, ".agent-policy/migration")?;

    let migration_dir = migration_dir.canonicalize()?;
    if !migration_dir.starts_with(repo) {
        anyhow::bail!("refusing to write migration drafts outside the repository");
    }

    Ok(migration_dir)
}

fn ensure_safe_directory(path: &Path, label: &str) -> anyhow::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                anyhow::bail!("refusing to use symlinked migration directory component {label}");
            }
            if !metadata.is_dir() {
                anyhow::bail!("migration directory component {label} is not a directory");
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            fs::create_dir(path)?;
            let metadata = fs::symlink_metadata(path)?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                anyhow::bail!("refusing to use unsafe migration directory component {label}");
            }
        }
        Err(error) => return Err(error.into()),
    }

    Ok(())
}

fn write_migration_draft_file(target: &Path, policy_yaml: &str) -> anyhow::Result<()> {
    match fs::symlink_metadata(target) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                anyhow::bail!("refusing to overwrite symlinked migration draft");
            }
            if !metadata.is_file() {
                anyhow::bail!("refusing to overwrite non-file migration draft path");
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    set_no_follow(&mut options);

    let mut file = options.open(target)?;
    file.write_all(policy_yaml.as_bytes())?;
    file.sync_all()?;

    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn set_no_follow(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;

    const O_NOFOLLOW: i32 = 0o400000;
    options.custom_flags(O_NOFOLLOW);
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
fn set_no_follow(_options: &mut OpenOptions) {}

#[cfg(not(unix))]
fn set_no_follow(_options: &mut OpenOptions) {}
