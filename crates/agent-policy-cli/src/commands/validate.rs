use std::path::Path;

use agent_policy_config::validate_config_file;
use agent_policy_core::{collect_policy_files, validate_policy_files, PolicyValidationSeverity};

use crate::cli::{GlobalArgs, OutputFormat};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidationReport {
    pub(crate) status: ValidationStatus,
    pub(crate) summary: ValidationSummary,
    pub(crate) errors: Vec<ValidationIssue>,
    pub(crate) warnings: Vec<ValidationIssue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValidationStatus {
    Ok,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidationSummary {
    pub(crate) config_checked: bool,
    pub(crate) policy_files_checked: usize,
    pub(crate) error_count: usize,
    pub(crate) warning_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidationIssue {
    pub(crate) code: &'static str,
    pub(crate) message: String,
    pub(crate) path: Option<String>,
    pub(crate) field: Option<String>,
}

pub(crate) fn run(global: &GlobalArgs) -> anyhow::Result<()> {
    let repo = global.repo.as_deref().unwrap_or_else(|| Path::new("."));
    let report = validate_repo(repo, global.config.as_deref());

    match global.format.clone().unwrap_or(OutputFormat::Markdown) {
        OutputFormat::Json => {
            println!("{}", render_validation_json(&report));
        }
        OutputFormat::Markdown => {
            print!("{}", render_validation_markdown(&report));
        }
    }

    if report.status == ValidationStatus::Failed {
        anyhow::bail!("validation failed")
    }

    Ok(())
}

pub(crate) fn validate_repo(repo: &Path, explicit_config: Option<&Path>) -> ValidationReport {
    let config_path = explicit_config
        .map(Path::to_path_buf)
        .unwrap_or_else(|| repo.join(agent_policy_config::REPO_CONFIG_FILE_NAME));
    let config_result = validate_config_file(&config_path);
    let (policy_files, policy_dir_issues) =
        collect_policy_files(repo, &config_result.local_policies);
    let policy_issues = validate_policy_files(&policy_files);

    let mut errors = config_result
        .errors
        .into_iter()
        .map(|issue| ValidationIssue {
            code: issue.code,
            message: issue.message,
            path: issue.path,
            field: issue.field,
        })
        .collect::<Vec<_>>();
    let mut warnings = Vec::new();

    for issue in policy_dir_issues.into_iter().chain(policy_issues) {
        let target = match issue.severity {
            PolicyValidationSeverity::Error => &mut errors,
            PolicyValidationSeverity::Warning => &mut warnings,
        };
        target.push(ValidationIssue {
            code: issue.code,
            message: issue.message,
            path: issue.path,
            field: issue.field,
        });
    }

    let status = if errors.is_empty() {
        ValidationStatus::Ok
    } else {
        ValidationStatus::Failed
    };
    let summary = ValidationSummary {
        config_checked: config_result.config_checked,
        policy_files_checked: policy_files.len(),
        error_count: errors.len(),
        warning_count: warnings.len(),
    };

    ValidationReport {
        status,
        summary,
        errors,
        warnings,
    }
}

fn render_validation_json(report: &ValidationReport) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"status\": \"");
    out.push_str(match report.status {
        ValidationStatus::Ok => "ok",
        ValidationStatus::Failed => "failed",
    });
    out.push_str("\",\n");
    out.push_str("  \"summary\": {\n");
    out.push_str(&format!(
        "    \"config_checked\": {},\n",
        report.summary.config_checked
    ));
    out.push_str(&format!(
        "    \"policy_files_checked\": {},\n",
        report.summary.policy_files_checked
    ));
    out.push_str(&format!(
        "    \"error_count\": {},\n",
        report.summary.error_count
    ));
    out.push_str(&format!(
        "    \"warning_count\": {}\n",
        report.summary.warning_count
    ));
    out.push_str("  }");

    if !report.errors.is_empty() {
        out.push_str(",\n  \"errors\": ");
        render_validation_issues_json(&mut out, &report.errors, 2);
    }
    if !report.warnings.is_empty() {
        out.push_str(",\n  \"warnings\": ");
        render_validation_issues_json(&mut out, &report.warnings, 2);
    }
    out.push_str("\n}");
    out
}

fn render_validation_issues_json(out: &mut String, issues: &[ValidationIssue], indent: usize) {
    let pad = " ".repeat(indent);
    let item_pad = " ".repeat(indent + 2);
    out.push_str("[\n");
    for (index, issue) in issues.iter().enumerate() {
        out.push_str(&item_pad);
        out.push_str("{\n");
        out.push_str(&format!(
            "{}  \"code\": \"{}\",\n",
            item_pad,
            json_escape(issue.code)
        ));
        out.push_str(&format!(
            "{}  \"message\": \"{}\"",
            item_pad,
            json_escape(&issue.message)
        ));
        if let Some(path) = &issue.path {
            out.push_str(&format!(
                ",\n{}  \"path\": \"{}\"",
                item_pad,
                json_escape(path)
            ));
        }
        if let Some(field) = &issue.field {
            out.push_str(&format!(
                ",\n{}  \"field\": \"{}\"",
                item_pad,
                json_escape(field)
            ));
        }
        out.push('\n');
        out.push_str(&item_pad);
        out.push('}');
        if index + 1 != issues.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&pad);
    out.push(']');
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character => escaped.push(character),
        }
    }
    escaped
}

pub(crate) fn render_validation_markdown(report: &ValidationReport) -> String {
    let mut out = String::new();
    out.push_str("# Agent Policy Validation\n\n");
    out.push_str("- Status: `");
    out.push_str(match report.status {
        ValidationStatus::Ok => "ok",
        ValidationStatus::Failed => "failed",
    });
    out.push_str("`\n");
    out.push_str(&format!(
        "- Checked {} policy file{}.\n",
        report.summary.policy_files_checked,
        if report.summary.policy_files_checked == 1 {
            ""
        } else {
            "s"
        }
    ));
    out.push_str(&format!(
        "- Errors: {}; warnings: {}.\n\n",
        report.summary.error_count, report.summary.warning_count
    ));

    render_validation_issue_section(&mut out, "Errors", &report.errors);
    render_validation_issue_section(&mut out, "Warnings", &report.warnings);

    out
}

fn render_validation_issue_section(out: &mut String, title: &str, issues: &[ValidationIssue]) {
    out.push_str("## ");
    out.push_str(title);
    out.push_str("\n\n");

    if issues.is_empty() {
        out.push_str("- None.\n\n");
        return;
    }

    for issue in issues {
        out.push_str("- `");
        out.push_str(issue.code);
        out.push_str("`: ");
        out.push_str(&issue.message);
        if let Some(path) = &issue.path {
            out.push_str(" (");
            out.push_str(path);
            if let Some(field) = &issue.field {
                out.push_str(", ");
                out.push_str(field);
            }
            out.push(')');
        }
        out.push('\n');
    }
    out.push('\n');
}
