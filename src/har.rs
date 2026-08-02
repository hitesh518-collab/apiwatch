use std::collections::BTreeMap;
use std::path::Path;

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
pub(crate) struct HarRecording {
    pub method: String,
    pub path: String,
    pub body: serde_json::Value,
}

#[derive(Debug)]
pub(crate) enum HarSkipReason {
    NonJsonContentType(String),
    NonMatchingStatus { status: u16, path: String },
    EmptyBody,
    JsonParseError(String),
    Base64Encoded,
}

pub(crate) type HarRecordings = BTreeMap<String, Vec<HarRecording>>;

pub(crate) fn load_har(
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

    let mut recordings: HarRecordings = BTreeMap::new();
    let mut skips: Vec<(String, HarSkipReason)> = Vec::new();

    // Placeholder: iterate entries, filter, group — implemented in Task 2

    if recordings.is_empty() {
        anyhow::bail!("no HAR entries matched the recording criteria");
    }

    Ok((recordings, skips))
}
