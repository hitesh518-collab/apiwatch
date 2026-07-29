use apiwatch::lock_size::{ContractMeasurement, Recommendation};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub schema_version: u8,
    pub apiwatch_version: String,
    pub max_lock_bytes: u64,
    pub corpus: Vec<CorpusResult>,
    pub privacy: PrivacyResult,
    pub recommendation: Recommendation,
}

#[derive(Clone, Debug, Serialize)]
pub struct CorpusResult {
    pub name: String,
    pub source_commit: String,
    pub sha256: String,
    pub source_bytes: u64,
    pub normalization_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measurements: Option<ContractMeasurement>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub v4_contract_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PrivacyResult {
    pub passed: bool,
    pub candidate_count: u8,
}

pub fn render_json(report: &Report) -> Result<Vec<u8>, serde_json::Error> {
    let mut rendered = serde_json::to_string_pretty(report)?.into_bytes();
    rendered.push(b'\n');
    Ok(rendered)
}

pub fn render_markdown(report: &Report) -> Vec<u8> {
    let mut rendered = String::new();
    rendered.push_str("# APIWatch Phase 1 Lock-Size Report\n\n");
    rendered.push_str(&format!(
        "- Report schema: {}\n- APIWatch: {}\n- Ceiling: {} bytes\n\n",
        report.schema_version, report.apiwatch_version, report.max_lock_bytes
    ));
    rendered.push_str(
        "| Corpus | Commit | Source bytes | Status | Operations | Expanded YAML | Canonical JSON | Deduplicated YAML |\n",
    );
    rendered.push_str("|---|---|---:|---|---:|---:|---:|---:|\n");
    for item in &report.corpus {
        let (operations, expanded, json, deduplicated) = match &item.measurements {
            Some(measurement) => (
                measurement.operation_count.to_string(),
                size_cell(&measurement.expanded_yaml),
                size_cell(&measurement.canonical_json),
                size_cell(&measurement.deduplicated_yaml),
            ),
            None => ("—".into(), "—".into(), "—".into(), "—".into()),
        };
        rendered.push_str(&format!(
            "| {} | `{}` | {} | {} | {} | {} | {} | {} |\n",
            escape(&item.name),
            item.source_commit,
            item.source_bytes,
            escape(&item.normalization_status),
            operations,
            expanded,
            json,
            deduplicated,
        ));
        if let Some(error) = &item.expected_error {
            rendered.push_str(&format!(
                "\nExpected `{}` failure: `{}`\n\n",
                escape(&item.name),
                escape(error)
            ));
        }
    }
    rendered.push_str(&format!(
        "\n- Privacy sentinels: passed across {} candidates\n- Recommendation: `{}`\n",
        report.privacy.candidate_count,
        recommendation_name(report.recommendation)
    ));
    rendered.into_bytes()
}

#[derive(Debug, Serialize)]
struct V4Report<'a> {
    schema_version: u8,
    apiwatch_version: &'a str,
    max_lock_bytes: u64,
    corpus: Vec<V4CorpusResult<'a>>,
}

#[derive(Debug, Serialize)]
struct V4CorpusResult<'a> {
    name: &'a str,
    source_commit: &'a str,
    sha256: &'a str,
    source_bytes: u64,
    normalization_status: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    operation_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    v4_contract_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    within_ceiling: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected_error: Option<&'a str>,
}

pub fn render_v4_json(report: &Report) -> Result<Vec<u8>, serde_json::Error> {
    let report = v4_report(report);
    let mut rendered = serde_json::to_string_pretty(&report)?.into_bytes();
    rendered.push(b'\n');
    Ok(rendered)
}

pub fn render_v4_markdown(report: &Report) -> Vec<u8> {
    let mut rendered = String::new();
    rendered.push_str("# APIWatch Phase 2 v4 Lock-Size Report\n\n");
    rendered.push_str(&format!(
        "- Report schema: {}\n- APIWatch: {}\n- Ceiling: {} bytes\n\n",
        report.schema_version, report.apiwatch_version, report.max_lock_bytes
    ));
    rendered.push_str(
        "| Corpus | Commit | SHA-256 | Source bytes | Status | Operations | v4 contract payload |\n",
    );
    rendered.push_str("|---|---|---|---:|---|---:|---:|\n");
    for item in &report.corpus {
        let operations = item
            .operation_count
            .map_or_else(|| "—".to_owned(), |value| value.to_string());
        let payload = item.v4_contract_bytes.map_or_else(
            || "—".to_owned(),
            |bytes| {
                format!(
                    "{} ({})",
                    bytes,
                    if bytes <= report.max_lock_bytes {
                        "fits"
                    } else {
                        "over"
                    }
                )
            },
        );
        rendered.push_str(&format!(
            "| {} | `{}` | `{}` | {} | {} | {} | {} |\n",
            escape(&item.name),
            item.source_commit,
            item.sha256,
            item.source_bytes,
            escape(&item.normalization_status),
            operations,
            payload,
        ));
        if let Some(error) = &item.expected_error {
            rendered.push_str(&format!(
                "\nExpected `{}` failure: `{}`\n\n",
                escape(&item.name),
                escape(error)
            ));
        }
    }
    while rendered.ends_with("\n\n") {
        rendered.pop();
    }
    rendered.into_bytes()
}

fn v4_report(report: &Report) -> V4Report<'_> {
    V4Report {
        schema_version: report.schema_version,
        apiwatch_version: &report.apiwatch_version,
        max_lock_bytes: report.max_lock_bytes,
        corpus: report
            .corpus
            .iter()
            .map(|item| V4CorpusResult {
                name: &item.name,
                source_commit: &item.source_commit,
                sha256: &item.sha256,
                source_bytes: item.source_bytes,
                normalization_status: &item.normalization_status,
                operation_count: item.operation_count,
                v4_contract_bytes: item.v4_contract_bytes,
                within_ceiling: item
                    .v4_contract_bytes
                    .map(|bytes| bytes <= report.max_lock_bytes),
                expected_error: item.expected_error.as_deref(),
            })
            .collect(),
    }
}

fn size_cell(measurement: &apiwatch::lock_size::CandidateMeasurement) -> String {
    format!(
        "{} ({})",
        measurement.bytes,
        if measurement.within_ceiling {
            "fits"
        } else {
            "over"
        }
    )
}

fn recommendation_name(recommendation: Recommendation) -> &'static str {
    match recommendation {
        Recommendation::ExpandedYaml => "expanded_yaml",
        Recommendation::DeduplicatedYaml => "deduplicated_yaml",
        Recommendation::CanonicalJson => "canonical_json",
        Recommendation::OperationScopingRequired => "operation_scoping_required",
    }
}

fn escape(value: &str) -> String {
    value.replace('|', "\\|").replace(['\r', '\n'], " ")
}
