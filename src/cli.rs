use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "apiwatch")]
#[command(about = "Lock, diff, and verify the APIs your code depends on.")]
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
    },
}
