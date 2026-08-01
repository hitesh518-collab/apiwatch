use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::diff::{Change, Severity};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub ignore: Vec<IgnoreRule>,
    #[serde(default)]
    pub severity: Vec<SeverityOverride>,
    #[serde(default)]
    pub fail_on: Option<FailOnThresholds>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IgnoreRule {
    pub rule: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub method: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeverityOverride {
    pub change: String,
    pub severity: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FailOnThresholds {
    #[serde(default)]
    pub breaking: usize,
    #[serde(default)]
    pub warning: usize,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let config: Self = serde_yml::from_str(&contents)
            .with_context(|| format!("failed to parse config {}", path.display()))?;
        Ok(config)
    }

    pub fn discover(start: &Path) -> Result<Self> {
        let dir = if start.is_dir() {
            start.to_path_buf()
        } else {
            start
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."))
        };
        for ancestor in dir.ancestors() {
            let candidate = ancestor.join(".apiwatch.yaml");
            if candidate.exists() {
                return Self::load(&candidate);
            }
        }
        anyhow::bail!(".apiwatch.yaml not found starting from {}", start.display())
    }
}

pub fn apply_config(changes: &mut Vec<Change>, config: &Config) {
    changes.retain(|change| !change_is_ignored(change, &config.ignore));

    for change in changes.iter_mut() {
        for rule_override in &config.severity {
            if change_matches_severity_override(change, rule_override) {
                change.severity = parse_severity(&rule_override.severity);
            }
        }
    }
}

pub fn compute_exit_code(changes: &[Change], fail_on: Option<&FailOnThresholds>) -> i32 {
    if let Some(thresholds) = fail_on {
        let breaking_count = changes
            .iter()
            .filter(|c| c.severity == Severity::Breaking)
            .count();
        let warning_count = changes
            .iter()
            .filter(|c| c.severity == Severity::Warning)
            .count();
        if breaking_count > thresholds.breaking || warning_count > thresholds.warning {
            return 1;
        }
        return 0;
    }
    if changes.iter().any(|c| c.severity == Severity::Breaking) {
        1
    } else {
        0
    }
}

fn change_is_ignored(change: &Change, ignore_rules: &[IgnoreRule]) -> bool {
    for rule in ignore_rules {
        if ignore_rule_matches(change, rule) {
            return true;
        }
    }
    false
}

fn ignore_rule_matches(change: &Change, rule: &IgnoreRule) -> bool {
    if !rule_keywords_match(&rule.rule, &change.message) {
        return false;
    }
    if let Some(ref path_pattern) = rule.path {
        if !glob_path_match(path_pattern, &change.operation.path) {
            return false;
        }
    }
    if let Some(ref method_pattern) = rule.method {
        if !method_pattern.eq_ignore_ascii_case(change.operation.method.as_str()) {
            return false;
        }
    }
    true
}

fn rule_keywords_match(rule: &str, message: &str) -> bool {
    let message_lower = message.to_lowercase();
    rule.split('-').all(|keyword| {
        let keyword_lower = keyword.to_lowercase();
        message_lower.contains(&keyword_lower)
    })
}

fn change_matches_severity_override(change: &Change, rule_override: &SeverityOverride) -> bool {
    let keywords: Vec<&str> = rule_override.change.split('-').collect();
    let message_lower = change.message.to_lowercase();
    keywords
        .iter()
        .all(|kw| message_lower.contains(&kw.to_lowercase()))
}

fn parse_severity(raw: &str) -> Severity {
    match raw.to_lowercase().as_str() {
        "breaking" => Severity::Breaking,
        "warning" => Severity::Warning,
        "non_breaking" | "nonbreaking" => Severity::NonBreaking,
        _ => Severity::Warning,
    }
}

fn glob_path_match(pattern: &str, path: &str) -> bool {
    let pattern_segments: Vec<&str> = pattern
        .trim_start_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    let path_segments: Vec<&str> = path
        .trim_start_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    if pattern_segments.len() != path_segments.len() {
        return false;
    }
    for (pat, seg) in pattern_segments.iter().zip(path_segments.iter()) {
        if !segment_match(pat, seg) {
            return false;
        }
    }
    true
}

fn segment_match(pattern: &str, segment: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if pattern.starts_with('{') && pattern.ends_with('}') {
        return segment.starts_with('{') && segment.ends_with('}');
    }
    pattern == segment
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{HttpMethod, OperationKey};

    fn change(severity: Severity, method: HttpMethod, path: &str, message: &str) -> Change {
        Change {
            severity,
            operation: OperationKey {
                method,
                path: path.to_string(),
            },
            message: message.to_string(),
        }
    }

    #[test]
    fn glob_match_wildcard_matches_single_segment() {
        assert!(glob_path_match("/deprecated/*", "/deprecated/old"));
        assert!(glob_path_match("/api/*", "/api/v1"));
        assert!(!glob_path_match("/deprecated/*", "/deprecated/old/extra"));
        assert!(!glob_path_match("/deprecated/*", "/other/old"));
    }

    #[test]
    fn glob_match_param_matches_template_segments() {
        assert!(glob_path_match("/users/{param}", "/users/{id}"));
        assert!(glob_path_match("/users/{param}", "/users/{userId}"));
        assert!(!glob_path_match("/users/{param}", "/users/123"));
    }

    #[test]
    fn glob_match_exact_segments() {
        assert!(glob_path_match("/users", "/users"));
        assert!(!glob_path_match("/users", "/teams"));
    }

    #[test]
    fn rule_keywords_split_by_hyphen() {
        assert!(rule_keywords_match(
            "parameter-removed",
            "query parameter removed_param removed"
        ));
        assert!(rule_keywords_match("endpoint-added", "endpoint added"));
        assert!(rule_keywords_match(
            "response-status-removed",
            "response status 200 removed"
        ));
        assert!(!rule_keywords_match("parameter-removed", "endpoint added"));
    }

    #[test]
    fn ignore_rule_filters_by_keyword_and_path() {
        let rules = vec![IgnoreRule {
            rule: "parameter-removed".to_string(),
            path: Some("/deprecated/*".to_string()),
            method: None,
        }];
        let c = change(
            Severity::Breaking,
            HttpMethod::Get,
            "/deprecated/old",
            "query parameter removed_param removed",
        );
        assert!(change_is_ignored(&c, &rules));
    }

    #[test]
    fn ignore_rule_does_not_match_different_path() {
        let rules = vec![IgnoreRule {
            rule: "parameter-removed".to_string(),
            path: Some("/deprecated/*".to_string()),
            method: None,
        }];
        let c = change(
            Severity::Breaking,
            HttpMethod::Get,
            "/stable",
            "query parameter id removed",
        );
        assert!(!change_is_ignored(&c, &rules));
    }

    #[test]
    fn ignore_rule_matches_specific_method() {
        let rules = vec![IgnoreRule {
            rule: "parameter-removed".to_string(),
            path: None,
            method: Some("GET".to_string()),
        }];
        let c = change(
            Severity::Breaking,
            HttpMethod::Get,
            "/users",
            "query parameter id removed",
        );
        assert!(change_is_ignored(&c, &rules));

        let c_post = change(
            Severity::Breaking,
            HttpMethod::Post,
            "/users",
            "query parameter id removed",
        );
        assert!(!change_is_ignored(&c_post, &rules));
    }

    #[test]
    fn severity_override_changes_breaking_to_warning() {
        let override_rules = [SeverityOverride {
            change: "endpoint-added".to_string(),
            severity: "warning".to_string(),
        }];
        let mut c = change(
            Severity::NonBreaking,
            HttpMethod::Get,
            "/new",
            "endpoint added",
        );
        assert!(change_matches_severity_override(&c, &override_rules[0]));
        c.severity = parse_severity(&override_rules[0].severity);
        assert_eq!(c.severity, Severity::Warning);
    }

    #[test]
    fn apply_config_filters_and_overrides() {
        let mut changes = vec![
            change(
                Severity::Breaking,
                HttpMethod::Get,
                "/deprecated/old",
                "query parameter removed_param removed",
            ),
            change(
                Severity::NonBreaking,
                HttpMethod::Get,
                "/new",
                "endpoint added",
            ),
        ];
        let config = Config {
            ignore: vec![IgnoreRule {
                rule: "parameter-removed".to_string(),
                path: Some("/deprecated/*".to_string()),
                method: None,
            }],
            severity: vec![SeverityOverride {
                change: "endpoint-added".to_string(),
                severity: "warning".to_string(),
            }],
            fail_on: None,
        };

        apply_config(&mut changes, &config);

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].severity, Severity::Warning);
        assert_eq!(changes[0].message, "endpoint added");
    }

    #[test]
    fn fail_on_breaking_zero_exits_one() {
        let changes = vec![change(
            Severity::Breaking,
            HttpMethod::Get,
            "/users",
            "endpoint removed",
        )];
        let thresholds = FailOnThresholds {
            breaking: 0,
            warning: 0,
        };
        assert_eq!(compute_exit_code(&changes, Some(&thresholds)), 1);
    }

    #[test]
    fn fail_on_breaking_one_allows_single_breaking() {
        let changes = vec![change(
            Severity::Breaking,
            HttpMethod::Get,
            "/users",
            "endpoint removed",
        )];
        let thresholds = FailOnThresholds {
            breaking: 1,
            warning: 0,
        };
        assert_eq!(compute_exit_code(&changes, Some(&thresholds)), 0);
    }

    #[test]
    fn fail_on_default_behavior_exits_one_for_breaking() {
        let changes = vec![change(
            Severity::Breaking,
            HttpMethod::Get,
            "/users",
            "endpoint removed",
        )];
        assert_eq!(compute_exit_code(&changes, None), 1);
    }

    #[test]
    fn load_config_parses_basic_yaml() {
        let config = Config::load(std::path::Path::new("testdata/config/basic.yaml"))
            .expect("config should parse");
        assert_eq!(config.ignore.len(), 1);
        assert_eq!(config.ignore[0].rule, "parameter-removed");
        assert_eq!(config.ignore[0].path.as_deref(), Some("/deprecated/*"));
        assert_eq!(config.severity.len(), 1);
        assert_eq!(config.severity[0].change, "endpoint-added");
        assert_eq!(config.severity[0].severity, "warning");
        assert!(config.fail_on.is_some());
        let fo = config.fail_on.as_ref().unwrap();
        assert_eq!(fo.breaking, 0);
        assert_eq!(fo.warning, 10);
    }
}
