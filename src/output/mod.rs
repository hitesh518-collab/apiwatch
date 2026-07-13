use std::fmt::Write;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::diff::{Change, Severity};
use crate::lockfile::{VerifyChange, VerifyChangeKind};

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

#[derive(Serialize)]
struct VerifyJson<'a> {
    version: u8,
    command: &'static str,
    name: &'a str,
    summary: VerifySummary,
    changes: Vec<VerifyJsonChange<'a>>,
}

#[derive(Serialize)]
struct VerifySummary {
    removed: usize,
    added: usize,
}

#[derive(Serialize)]
struct VerifyJsonChange<'a> {
    kind: &'static str,
    method: &'a str,
    path: &'a str,
}

#[derive(Serialize)]
struct SarifLog {
    #[serde(rename = "$schema")]
    schema: String,
    version: String,
    runs: Vec<SarifRun>,
}

#[derive(Serialize)]
struct SarifRun {
    tool: SarifTool,
    results: Vec<SarifResult>,
}

#[derive(Serialize)]
struct SarifTool {
    driver: SarifDriver,
}

#[derive(Serialize)]
struct SarifDriver {
    name: String,
    #[serde(rename = "semanticVersion")]
    semantic_version: String,
    rules: Vec<SarifRule>,
}

#[derive(Serialize)]
struct SarifRule {
    id: String,
    name: String,
    #[serde(rename = "shortDescription")]
    short_description: SarifMessage,
    help: SarifMessage,
    #[serde(rename = "defaultConfiguration")]
    default_configuration: SarifDefaultConfiguration,
    properties: SarifRuleProperties,
}

#[derive(Serialize)]
struct SarifDefaultConfiguration {
    level: String,
}

#[derive(Serialize)]
struct SarifRuleProperties {
    precision: String,
    #[serde(rename = "problem.severity")]
    problem_severity: String,
}

#[derive(Serialize)]
struct SarifResult {
    #[serde(rename = "ruleId")]
    rule_id: String,
    level: String,
    message: SarifMessage,
    locations: Vec<SarifLocation>,
    #[serde(rename = "partialFingerprints")]
    partial_fingerprints: SarifPartialFingerprints,
}

#[derive(Serialize)]
struct SarifMessage {
    text: String,
}

#[derive(Serialize)]
struct SarifLocation {
    #[serde(rename = "physicalLocation")]
    physical_location: SarifPhysicalLocation,
}

#[derive(Serialize)]
struct SarifPhysicalLocation {
    #[serde(rename = "artifactLocation")]
    artifact_location: SarifArtifactLocation,
}

#[derive(Serialize)]
struct SarifArtifactLocation {
    uri: String,
}

#[derive(Serialize)]
struct SarifPartialFingerprints {
    #[serde(rename = "apiwatch/v1")]
    apiwatch_v1: String,
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

pub fn render_verify_changes_json(name: &str, changes: &[VerifyChange]) -> Result<String> {
    let mut summary = VerifySummary {
        removed: 0,
        added: 0,
    };
    let rendered_changes = changes
        .iter()
        .map(|change| {
            let kind = match change.kind {
                VerifyChangeKind::Removed => {
                    summary.removed += 1;
                    "removed"
                }
                VerifyChangeKind::Added => {
                    summary.added += 1;
                    "added"
                }
            };

            VerifyJsonChange {
                kind,
                method: &change.method,
                path: &change.path,
            }
        })
        .collect();

    let rendered = serde_json::to_string(&VerifyJson {
        version: 1,
        command: "verify",
        name,
        summary,
        changes: rendered_changes,
    })
    .context("failed to serialize Verify JSON output")?;

    Ok(format!("{rendered}\n"))
}

pub fn render_changes_sarif(artifact_path: &Path, changes: &[Change]) -> Result<String> {
    let artifact_uri = render_artifact_uri(artifact_path);
    let results = changes
        .iter()
        .map(|change| {
            let (rule_id, level) = match change.severity {
                Severity::Breaking => ("apiwatch/diff-breaking", "error"),
                Severity::Warning => ("apiwatch/diff-warning", "warning"),
                Severity::NonBreaking => ("apiwatch/diff-non-breaking", "note"),
            };
            let method = change.operation.method.as_str();
            let message = change.message.clone();

            sarif_result(
                rule_id,
                level,
                message.clone(),
                artifact_uri.clone(),
                format!(
                    "diff:{rule_id}:{method}:{}:{message}",
                    change.operation.path
                ),
            )
        })
        .collect();

    render_sarif(results)
}

pub fn render_verify_changes_sarif(
    artifact_path: &Path,
    name: &str,
    changes: &[VerifyChange],
) -> Result<String> {
    let artifact_uri = render_artifact_uri(artifact_path);
    let results = changes
        .iter()
        .map(|change| {
            let (rule_id, level, prefix) = match change.kind {
                VerifyChangeKind::Removed => (
                    "apiwatch/verify-removed",
                    "error",
                    "locked operation removed",
                ),
                VerifyChangeKind::Added => (
                    "apiwatch/verify-added",
                    "warning",
                    "unlocked operation added",
                ),
            };
            let message = format!("{prefix}: {} {}", change.method, change.path);

            sarif_result(
                rule_id,
                level,
                message,
                artifact_uri.clone(),
                format!("verify:{name}:{rule_id}:{}:{}", change.method, change.path),
            )
        })
        .collect();

    render_sarif(results)
}

fn render_artifact_uri(artifact_path: &Path) -> String {
    let path = artifact_path.to_string_lossy();
    let mut uri = String::with_capacity(path.len());

    for byte in path.bytes() {
        match byte {
            b'\\' => uri.push('/'),
            b'/' | b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                uri.push(byte.into())
            }
            _ => write!(&mut uri, "%{byte:02X}").expect("writing to a String cannot fail"),
        }
    }

    uri
}

fn render_sarif(results: Vec<SarifResult>) -> Result<String> {
    let rendered = serde_json::to_string(&SarifLog {
        schema: "https://json.schemastore.org/sarif-2.1.0.json".to_string(),
        version: "2.1.0".to_string(),
        runs: vec![SarifRun {
            tool: SarifTool {
                driver: SarifDriver {
                    name: "apiwatch".to_string(),
                    semantic_version: env!("CARGO_PKG_VERSION").to_string(),
                    rules: sarif_rules(),
                },
            },
            results,
        }],
    })
    .context("failed to serialize SARIF output")?;

    Ok(format!("{rendered}\n"))
}

fn sarif_result(
    rule_id: &str,
    level: &str,
    message: String,
    artifact_uri: String,
    fingerprint: String,
) -> SarifResult {
    SarifResult {
        rule_id: rule_id.to_string(),
        level: level.to_string(),
        message: SarifMessage { text: message },
        locations: vec![SarifLocation {
            physical_location: SarifPhysicalLocation {
                artifact_location: SarifArtifactLocation { uri: artifact_uri },
            },
        }],
        partial_fingerprints: SarifPartialFingerprints {
            apiwatch_v1: fingerprint,
        },
    }
}

fn sarif_rules() -> Vec<SarifRule> {
    vec![
        sarif_rule(
            "apiwatch/diff-breaking",
            "Breaking API change",
            "A contract change is classified as breaking.",
            "Review the breaking API contract change before deployment.",
            "error",
            "error",
        ),
        sarif_rule(
            "apiwatch/diff-warning",
            "API change warning",
            "A contract change needs review but is not classified as breaking.",
            "Review the API contract change before deployment.",
            "warning",
            "warning",
        ),
        sarif_rule(
            "apiwatch/diff-non-breaking",
            "Non-breaking API change",
            "A contract change is classified as non-breaking.",
            "Review the non-breaking API contract change before deployment.",
            "note",
            "recommendation",
        ),
        sarif_rule(
            "apiwatch/verify-removed",
            "Locked operation removed",
            "A locked operation is missing from the current contract.",
            "Restore the locked operation or update the lock entry after review.",
            "error",
            "error",
        ),
        sarif_rule(
            "apiwatch/verify-added",
            "Unlocked operation added",
            "The current contract exposes an operation absent from the lock entry.",
            "Review the added operation and update the lock entry if intended.",
            "warning",
            "warning",
        ),
    ]
}

fn sarif_rule(
    id: &str,
    name: &str,
    short_description: &str,
    help: &str,
    level: &str,
    problem_severity: &str,
) -> SarifRule {
    SarifRule {
        id: id.to_string(),
        name: name.to_string(),
        short_description: SarifMessage {
            text: short_description.to_string(),
        },
        help: SarifMessage {
            text: help.to_string(),
        },
        default_configuration: SarifDefaultConfiguration {
            level: level.to_string(),
        },
        properties: SarifRuleProperties {
            precision: "high".to_string(),
            problem_severity: problem_severity.to_string(),
        },
    }
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::render_artifact_uri;

    #[test]
    fn render_artifact_uri_normalizes_backslashes() {
        assert_eq!(
            render_artifact_uri(Path::new(r"testdata\openapi\endpoint_removed_new.yaml")),
            "testdata/openapi/endpoint_removed_new.yaml"
        );
    }
}
