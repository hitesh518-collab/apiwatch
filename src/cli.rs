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
        /// Local JSON body to record.
        #[arg(long)]
        from_json: PathBuf,
        /// API name to use as the lockfile key.
        #[arg(long)]
        name: String,
        /// api.lock path to write.
        #[arg(long)]
        output: PathBuf,
        /// Merge the JSON shape into an existing observed entry.
        #[arg(long)]
        merge: bool,
        /// Mark a JSON object path as a dynamic-key map.
        #[arg(long = "map-at")]
        map_at: Vec<String>,
    },
    /// Verify one OpenAPI contract against a named api.lock entry.
    Verify {
        /// Current local OpenAPI YAML/JSON file or HTTP(S) URL to verify.
        openapi: String,
        /// API name to verify from the lockfile.
        #[arg(long)]
        name: String,
        /// api.lock file to compare against.
        #[arg(long)]
        lock: PathBuf,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
        #[arg(long, value_hint = clap::ValueHint::DirPath)]
        ref_root: Option<PathBuf>,
        #[arg(long)]
        config: Option<PathBuf>,
    },
}
