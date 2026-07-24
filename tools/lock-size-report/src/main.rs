mod report;

use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use apiwatch::lock_size::{
    encode_canonical_json, encode_deduplicated_yaml, encode_expanded_yaml, measure_contract,
    recommend, scope_contract, PRIVACY_SENTINELS,
};
use clap::Parser;
use report::{CorpusResult, PrivacyResult, Report};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    manifest: PathBuf,
    #[arg(long)]
    compat_dir: PathBuf,
    #[arg(long)]
    privacy_fixture: PathBuf,
    #[arg(long, default_value_t = 5_242_880)]
    max_lock_bytes: u64,
    #[arg(long = "include-operation")]
    include_operations: Vec<String>,
    #[arg(long)]
    json_out: PathBuf,
    #[arg(long)]
    markdown_out: PathBuf,
    #[arg(long)]
    check: bool,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    version: u8,
    max_total_bytes: u64,
    specs: Vec<ManifestEntry>,
}

#[derive(Debug, Deserialize)]
struct ManifestEntry {
    name: String,
    file: String,
    url: String,
    sha256: String,
    max_bytes: u64,
    status: String,
    expected_error: Option<String>,
}

#[derive(Debug)]
struct Failure {
    code: i32,
    message: String,
}

impl Failure {
    fn behavior(message: impl Into<String>) -> Self {
        Self {
            code: 1,
            message: message.into(),
        }
    }

    fn input(message: impl Into<String>) -> Self {
        Self {
            code: 2,
            message: message.into(),
        }
    }
}

fn main() {
    let args = match Args::try_parse() {
        Ok(args) => args,
        Err(error) => {
            let _ = error.print();
            std::process::exit(2);
        }
    };
    if let Err(failure) = run(&args) {
        eprintln!("error: {}", failure.message);
        std::process::exit(failure.code);
    }
}

fn run(args: &Args) -> Result<(), Failure> {
    if args.max_lock_bytes == 0 {
        return Err(Failure::input("max-lock-bytes must be positive"));
    }
    let manifest_bytes = fs::read(&args.manifest)
        .map_err(|error| Failure::input(format!("failed to read manifest: {error}")))?;
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| Failure::input(format!("failed to parse manifest: {error}")))?;
    if manifest.version != 1 {
        return Err(Failure::input("unsupported compatibility manifest version"));
    }

    let mut corpus = Vec::new();
    let mut measurements = Vec::new();
    let mut total_bytes = 0_u64;
    for entry in &manifest.specs {
        let source_commit = validate_entry(entry)?;
        let path = args.compat_dir.join(&entry.file);
        let bytes = fs::read(&path).map_err(|error| {
            Failure::input(format!("{}: failed to read cache: {error}", entry.name))
        })?;
        let source_bytes = u64::try_from(bytes.len())
            .map_err(|_| Failure::input(format!("{}: source size overflow", entry.name)))?;
        if source_bytes > entry.max_bytes {
            return Err(Failure::input(format!(
                "{}: cached file exceeds {} bytes",
                entry.name, entry.max_bytes
            )));
        }
        total_bytes = total_bytes
            .checked_add(source_bytes)
            .ok_or_else(|| Failure::input("compatibility corpus size overflow"))?;
        if total_bytes > manifest.max_total_bytes {
            return Err(Failure::input(format!(
                "compatibility corpus exceeds {} bytes",
                manifest.max_total_bytes
            )));
        }
        let actual_hash = format!("{:x}", Sha256::digest(&bytes));
        if actual_hash != entry.sha256 {
            return Err(Failure::input(format!("{}: SHA-256 mismatch", entry.name)));
        }

        match entry.status.as_str() {
            "passing" => {
                let contract = apiwatch::openapi::load_contract(&path).map_err(|error| {
                    Failure::behavior(format!(
                        "{}: expected normalization success, got {error:#}",
                        entry.name
                    ))
                })?;
                let contract = scope_contract(&contract, &args.include_operations)
                    .map_err(|error| Failure::input(format!("{}: {error}", entry.name)))?;
                let measurement = measure_contract(&contract, args.max_lock_bytes)
                    .map_err(|error| Failure::behavior(format!("{}: {error:#}", entry.name)))?;
                measurements.push(measurement.clone());
                corpus.push(CorpusResult {
                    name: entry.name.clone(),
                    source_commit,
                    sha256: entry.sha256.clone(),
                    source_bytes,
                    normalization_status: "passing".into(),
                    operation_count: Some(measurement.operation_count),
                    measurements: Some(measurement),
                    expected_error: None,
                });
            }
            "known_failing" => {
                let expected = entry.expected_error.as_deref().ok_or_else(|| {
                    Failure::input(format!("{}: expected_error is required", entry.name))
                })?;
                match apiwatch::openapi::load_contract(&path) {
                    Ok(_) => {
                        return Err(Failure::behavior(format!(
                            "{}: unexpectedly normalized successfully",
                            entry.name
                        )))
                    }
                    Err(error) if format!("{error:#}").contains(expected) => {}
                    Err(_) => {
                        return Err(Failure::behavior(format!(
                            "{}: parser failure no longer matches expectation",
                            entry.name
                        )))
                    }
                }
                corpus.push(CorpusResult {
                    name: entry.name.clone(),
                    source_commit,
                    sha256: entry.sha256.clone(),
                    source_bytes,
                    normalization_status: "known_failing".into(),
                    operation_count: None,
                    measurements: None,
                    expected_error: Some(expected.to_owned()),
                });
            }
            _ => {
                return Err(Failure::input(format!(
                    "{}: unsupported compatibility status",
                    entry.name
                )))
            }
        }
    }

    verify_privacy(&args.privacy_fixture)?;
    let report = Report {
        schema_version: 1,
        apiwatch_version: apiwatch::VERSION.to_owned(),
        max_lock_bytes: args.max_lock_bytes,
        corpus,
        privacy: PrivacyResult {
            passed: true,
            candidate_count: 3,
        },
        recommendation: recommend(&measurements, args.max_lock_bytes),
    };
    let json = report::render_json(&report)
        .map_err(|error| Failure::behavior(format!("failed to render JSON report: {error}")))?;
    let markdown = report::render_markdown(&report);
    write_or_check(&args.json_out, &json, args.check)?;
    write_or_check(&args.markdown_out, &markdown, args.check)?;
    Ok(())
}

fn validate_entry(entry: &ManifestEntry) -> Result<String, Failure> {
    let path = Path::new(&entry.file);
    if path.file_name().and_then(|value| value.to_str()) != Some(entry.file.as_str())
        || path.components().count() != 1
        || !matches!(path.components().next(), Some(Component::Normal(_)))
    {
        return Err(Failure::input(format!(
            "{}: file must be a plain filename",
            entry.name
        )));
    }
    if entry.max_bytes == 0 {
        return Err(Failure::input(format!(
            "{}: max_bytes must be positive",
            entry.name
        )));
    }
    if entry.sha256.len() != 64
        || !entry
            .sha256
            .bytes()
            .all(|value| value.is_ascii_digit() || (b'a'..=b'f').contains(&value))
    {
        return Err(Failure::input(format!(
            "{}: sha256 must be lowercase hexadecimal",
            entry.name
        )));
    }
    let prefix = "https://raw.githubusercontent.com/";
    let remainder = entry.url.strip_prefix(prefix).ok_or_else(|| {
        Failure::input(format!("{}: URL must use immutable raw GitHub", entry.name))
    })?;
    let parts: Vec<_> = remainder.split('/').collect();
    if parts.len() < 4
        || parts[2].len() != 40
        || !parts[2]
            .bytes()
            .all(|value| value.is_ascii_digit() || (b'a'..=b'f').contains(&value))
    {
        return Err(Failure::input(format!(
            "{}: URL must contain an immutable 40-character commit",
            entry.name
        )));
    }
    Ok(parts[2].to_owned())
}

fn verify_privacy(path: &Path) -> Result<(), Failure> {
    let contract = apiwatch::openapi::load_contract(path)
        .map_err(|error| Failure::input(format!("failed to load privacy fixture: {error:#}")))?;
    let candidates: [ContractEncoder; 3] = [
        encode_expanded_yaml,
        encode_canonical_json,
        encode_deduplicated_yaml,
    ];
    for encode in candidates {
        let rendered = encode(&contract)
            .map_err(|error| Failure::behavior(format!("privacy encoding failed: {error:#}")))?;
        if PRIVACY_SENTINELS.iter().any(|sentinel| {
            rendered
                .windows(sentinel.len())
                .any(|part| part == sentinel.as_bytes())
        }) {
            return Err(Failure::behavior(
                "privacy sentinel leaked into a candidate representation",
            ));
        }
    }
    Ok(())
}

type ContractEncoder = fn(&apiwatch::contract::ApiContract) -> anyhow::Result<Vec<u8>>;

fn write_or_check(path: &Path, bytes: &[u8], check: bool) -> Result<(), Failure> {
    if check {
        let existing = fs::read(path)
            .map_err(|error| Failure::input(format!("failed to read report: {error}")))?;
        if existing != bytes {
            return Err(Failure::behavior(format!(
                "report differs: {}",
                path.display()
            )));
        }
        return Ok(());
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|error| Failure::input(format!("failed to create report directory: {error}")))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| Failure::input(format!("failed to create temporary report: {error}")))?;
    temporary
        .write_all(bytes)
        .and_then(|_| temporary.flush())
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|error| Failure::input(format!("failed to write temporary report: {error}")))?;
    temporary
        .persist(path)
        .map_err(|error| Failure::input(format!("failed to replace report: {}", error.error)))?;
    Ok(())
}
