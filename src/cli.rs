use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "apiwatch")]
#[command(about = "Lock, diff, and verify the APIs your code depends on.")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
    Sarif,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Compare two OpenAPI contracts.
    Diff {
        /// Old OpenAPI YAML or JSON file.
        old: PathBuf,
        /// New OpenAPI YAML or JSON file.
        new: PathBuf,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
        #[arg(long, value_hint = clap::ValueHint::DirPath)]
        ref_root: Option<PathBuf>,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// Create an api.lock file from one OpenAPI contract.
    Lock {
        /// OpenAPI YAML or JSON file to lock.
        openapi: PathBuf,
        /// API name to use as the lockfile key.
        #[arg(long)]
        name: String,
        /// Lockfile path to write.
        #[arg(long)]
        output: PathBuf,
        /// Replace a named entry in an existing lockfile.
        #[arg(long)]
        update: bool,
        /// Include only this operation (repeatable METHOD /path).
        #[arg(long = "include-operation")]
        include_operations: Vec<String>,
        /// Maximum serialized contract payload size in bytes.
        #[arg(long, default_value_t = crate::lockfile::DEFAULT_MAX_LOCK_BYTES)]
        max_lock_bytes: u64,
        #[arg(long, value_hint = clap::ValueHint::DirPath)]
        ref_root: Option<PathBuf>,
    },
    /// Record the observed shape of one JSON body.
    Record {
        /// HAR file to import (mutually exclusive with --from-json).
        #[arg(long)]
        from_har: Option<PathBuf>,
        /// Local JSON body to record.
        #[arg(long)]
        from_json: Option<PathBuf>,
        /// API name to use as the lockfile key. Required for --from-json;
        /// optional for --from-har (entries auto-keyed by method+path).
        #[arg(long)]
        name: Option<String>,
        /// Live URL to fetch and record (mutually exclusive with --from-json, --from-har).
        #[arg(long)]
        from_url: Option<String>,
        /// HTTP method for --from-url (default GET).
        #[arg(long, default_value = "GET")]
        method: String,
        /// Request headers for --from-url (NAME:${ENV_VAR}).
        #[arg(long = "header", value_name = "NAME:${ENV_VAR}")]
        header: Vec<String>,
        /// api.lock path to write.
        #[arg(long)]
        output: PathBuf,
        /// Merge the JSON shape into an existing observed entry.
        #[arg(long)]
        merge: bool,
        /// Mark a JSON object path as a dynamic-key map.
        #[arg(long = "map-at")]
        map_at: Vec<String>,
        /// Observation ratio (0.0-1.0) required before a field hardens.
        #[arg(long = "required-threshold")]
        required_threshold: Option<f64>,
        /// Group HAR entries under this key (repeatable METHOD /path).
        #[arg(long = "path-identity", value_name = "METHOD /path")]
        path_identity: Vec<String>,
        /// Only record responses with these HTTP status codes (repeatable).
        /// When absent, only 2xx responses are recorded.
        #[arg(long = "status", value_name = "CODE")]
        status: Vec<u16>,
    },
    /// Verify one OpenAPI contract against a named api.lock entry.
    Verify {
        /// Current local OpenAPI YAML/JSON file or HTTP(S) URL to verify.
        /// Required unless --all is set.
        openapi: Option<String>,
        /// API name to verify from the lockfile.
        #[arg(long)]
        name: Option<String>,
        /// api.lock file to compare against.
        #[arg(long)]
        lock: PathBuf,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
        #[arg(long, value_hint = clap::ValueHint::DirPath)]
        ref_root: Option<PathBuf>,
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long = "header", value_name = "NAME:${ENV_VAR}")]
        header: Vec<String>,
        /// Verify all observed entries in the lock.
        #[arg(long)]
        all: bool,
        /// Base URL for --all: each entry's path is appended.
        #[arg(long = "source-url", requires = "all")]
        source_url: Option<String>,
    },
}
