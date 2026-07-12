use anyhow::{Context, Result};
use serde::Serialize;

use crate::diff::{Change, Severity};
use crate::lockfile::VerifyChange;

#[derive(Serialize)]
struct DiffJson<'a> {
    version: u8,
    command: &'static str,
    summary: DiffSummary,
    changes: Vec<DiffJsonChange<'a>>,
}

#[derive(Serialize)]
struct DiffSummary {
    breaking: usize,
    warning: usize,
    non_breaking: usize,
}

#[derive(Serialize)]
struct DiffJsonChange<'a> {
    severity: &'static str,
    method: &'static str,
    path: &'a str,
    message: &'a str,
}

pub fn render_changes_json(changes: &[Change]) -> Result<String> {
    let mut summary = DiffSummary {
        breaking: 0,
        warning: 0,
        non_breaking: 0,
    };
    let rendered_changes = changes
        .iter()
        .map(|change| {
            let severity = match change.severity {
                Severity::Breaking => {
                    summary.breaking += 1;
                    "breaking"
                }
                Severity::Warning => {
                    summary.warning += 1;
                    "warning"
                }
                Severity::NonBreaking => {
                    summary.non_breaking += 1;
                    "non_breaking"
                }
            };

            DiffJsonChange {
                severity,
                method: change.operation.method.as_str(),
                path: &change.operation.path,
                message: &change.message,
            }
        })
        .collect();

    let rendered = serde_json::to_string(&DiffJson {
        version: 1,
        command: "diff",
        summary,
        changes: rendered_changes,
    })
    .context("failed to serialize Diff JSON output")?;

    Ok(format!("{rendered}\n"))
}

pub fn render_changes(changes: &[Change]) -> String {
    if changes.is_empty() {
        return "No changes detected.\n".to_string();
    }

    let mut rendered = String::new();
    render_group(
        &mut rendered,
        "Breaking changes",
        changes,
        Severity::Breaking,
    );
    render_group(&mut rendered, "Warnings", changes, Severity::Warning);
    render_group(
        &mut rendered,
        "Non-breaking changes",
        changes,
        Severity::NonBreaking,
    );
    rendered
}

fn render_group(rendered: &mut String, title: &str, changes: &[Change], severity: Severity) {
    let group: Vec<_> = changes
        .iter()
        .filter(|change| change.severity == severity)
        .collect();

    if group.is_empty() {
        return;
    }

    if !rendered.is_empty() {
        rendered.push('\n');
    }

    rendered.push_str(title);
    rendered.push('\n');

    for change in group {
        rendered.push_str("- ");
        rendered.push_str(change.operation.method.as_str());
        rendered.push(' ');
        rendered.push_str(&change.operation.path);
        rendered.push_str(": ");
        rendered.push_str(&change.message);
        rendered.push('\n');
    }
}

pub fn render_verify_changes(changes: &[VerifyChange]) -> String {
    let mut rendered = String::new();

    for change in changes {
        rendered.push_str(change.kind.as_str());
        rendered.push(' ');
        rendered.push_str(&change.method);
        rendered.push(' ');
        rendered.push_str(&change.path);
        rendered.push('\n');
    }

    rendered
}
