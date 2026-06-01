use std::fs;
use std::path::Path;

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
    let migration_dir = repo.join(".agent-policy").join("migration");
    fs::create_dir_all(&migration_dir)?;

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
        fs::write(target, &draft.policy_yaml)?;
    }

    Ok(())
}
