use std::collections::BTreeMap;
use std::collections::HashSet;
use std::path::Path;

use url;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Har {
    log: HarLog,
}

#[derive(Debug, Deserialize)]
struct HarLog {
    entries: Vec<HarEntry>,
}

#[derive(Debug, Deserialize)]
struct HarEntry {
    request: HarRequest,
    response: HarResponse,
}

#[derive(Debug, Deserialize)]
struct HarRequest {
    method: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct HarResponse {
    status: u16,
    content: HarContent,
}

#[derive(Debug, Deserialize)]
struct HarContent {
    #[serde(default)]
    #[serde(rename = "mimeType")]
    mime_type: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    encoding: Option<String>,
}

#[derive(Debug)]
pub struct HarRecording {
    pub method: String,
    pub path: String,
    pub body: serde_json::Value,
}

#[derive(Debug)]
pub enum HarSkipReason {
    NonJsonContentType(String),
    NonMatchingStatus { status: u16, path: String },
    EmptyBody,
    JsonParseError(String),
    Base64Encoded,
}

pub type HarRecordings = BTreeMap<String, Vec<HarRecording>>;

fn is_json_content_type(mime_type: &str) -> bool {
    let mime_type = mime_type.trim();
    if mime_type.is_empty() {
        return false;
    }
    let lower = mime_type.to_lowercase();
    lower.starts_with("application/json")
        || lower.starts_with("application/vnd.")
}

fn entry_identity(method: &str, path: &str) -> String {
    format!("{} {}", method.to_uppercase(), path)
}

fn parse_path_identities(identities: &[String]) -> Result<Vec<(String, String)>> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for identity in identities {
        let (method, path) = identity
            .split_once(' ')
            .ok_or_else(|| anyhow::anyhow!(
                "invalid --path-identity '{}': expected 'METHOD /path'",
                identity
            ))?;
        let method = method.to_uppercase();
        let path = path.trim().to_string();
        if path.is_empty() {
            anyhow::bail!("invalid --path-identity '{}': path part is empty", identity);
        }
        if !seen.insert((method.clone(), path.clone())) {
            anyhow::bail!("duplicate --path-identity '{}'", identity);
        }
        result.push((method, path));
    }
    Ok(result)
}

pub fn load_har(
    path: &Path,
    path_identities: &[String],
    status_filter: &[u16],
) -> Result<(HarRecordings, Vec<(String, HarSkipReason)>)> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read HAR file {}", path.display()))?;
    let har: Har = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse HAR file {}", path.display()))?;
    if har.log.entries.is_empty() {
        anyhow::bail!("HAR file contains no entries");
    }

    let identities = if path_identities.is_empty() {
        None
    } else {
        Some(parse_path_identities(path_identities)?)
    };

    let mut recordings: HarRecordings = BTreeMap::new();
    let mut skips: Vec<(String, HarSkipReason)> = Vec::new();

    for entry in &har.log.entries {
        let method = entry.request.method.trim().to_uppercase();
        if method.is_empty() {
            continue;
        }

        let parsed_url = match url::Url::parse(&entry.request.url) {
            Ok(u) => u,
            Err(_) => {
                skips.push((
                    format!("{} (invalid URL)", entry.request.url),
                    HarSkipReason::EmptyBody,
                ));
                continue;
            }
        };
        let path = parsed_url.path().to_string();
        let skip_label = format!("{} {}", method, path);

        // Status filter
        if !status_filter.is_empty() {
            if !status_filter.contains(&entry.response.status) {
                skips.push((skip_label, HarSkipReason::NonMatchingStatus {
                    status: entry.response.status,
                    path: path.clone(),
                }));
                continue;
            }
        } else if entry.response.status < 200 || entry.response.status >= 300 {
            skips.push((skip_label, HarSkipReason::NonMatchingStatus {
                status: entry.response.status,
                path: path.clone(),
            }));
            continue;
        }

        // Encoding check
        if let Some(ref enc) = entry.response.content.encoding {
            if enc == "base64" {
                skips.push((skip_label, HarSkipReason::Base64Encoded));
                continue;
            }
        }

        // Content-type check
        if !is_json_content_type(&entry.response.content.mime_type) {
            skips.push((skip_label, HarSkipReason::NonJsonContentType(
                entry.response.content.mime_type.clone(),
            )));
            continue;
        }

        // Body check
        let text = entry.response.content.text.trim().to_string();
        if text.is_empty() {
            skips.push((skip_label, HarSkipReason::EmptyBody));
            continue;
        }

        // JSON parse
        let body: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                skips.push((skip_label, HarSkipReason::JsonParseError(e.to_string())));
                continue;
            }
        };

        // Determine entry key
        let key = if let Some(ref ids) = identities {
            let mut matched = None;
            for (ident_method, ident_path) in ids {
                if method == *ident_method && path.starts_with(ident_path.as_str()) {
                    matched = Some(entry_identity(ident_method, ident_path));
                    break;
                }
            }
            match matched {
                Some(k) => k,
                None => {
                    skips.push((skip_label, HarSkipReason::NonMatchingStatus {
                        status: entry.response.status,
                        path: path.clone(),
                    }));
                    continue;
                }
            }
        } else {
            entry_identity(&method, &path)
        };

        recordings
            .entry(key)
            .or_default()
            .push(HarRecording { method, path, body });
    }

    Ok((recordings, skips))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_json_content_types() {
        assert!(super::is_json_content_type("application/json"));
        assert!(super::is_json_content_type("application/json; charset=utf-8"));
        assert!(super::is_json_content_type("APPLICATION/JSON"));
        assert!(super::is_json_content_type("application/json+hal"));
        assert!(!super::is_json_content_type("text/plain"));
        assert!(!super::is_json_content_type("application/xml"));
        assert!(!super::is_json_content_type(""));
    }

    #[test]
    fn parses_valid_path_identities() {
        let ids = parse_path_identities(&[
            "GET /api/users".to_string(),
            "POST /api/orders".to_string(),
        ])
        .expect("should parse");
        assert_eq!(ids, vec![
            ("GET".to_string(), "/api/users".to_string()),
            ("POST".to_string(), "/api/orders".to_string()),
        ]);
    }

    #[test]
    fn parses_path_identity_without_space_as_error() {
        assert!(parse_path_identities(&["no-space".to_string()]).is_err());
    }

    #[test]
    fn normalizes_method_to_uppercase() {
        let ids = parse_path_identities(&["get /api/test".to_string()]).expect("should parse");
        assert_eq!(ids[0].0, "GET");
    }

    #[test]
    fn formats_entry_identity() {
        assert_eq!(entry_identity("get", "/api/users"), "GET /api/users");
        assert_eq!(entry_identity("POST", "/api/orders"), "POST /api/orders");
    }

    #[test]
    fn load_har_single_json_entry() {
        let path = std::path::Path::new("testdata/har/single-entry.har");
        let (recordings, skips) = load_har(&path, &[], &[]).expect("should load");
        assert_eq!(recordings.len(), 1);
        assert!(skips.is_empty());
        let key = recordings.keys().next().unwrap();
        assert_eq!(key, "GET /users/42");
        assert_eq!(recordings[key].len(), 1);
    }
}
