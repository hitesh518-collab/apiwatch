use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "apiwatch")]
#[command(about = "Lock, diff, and verify the APIs your code depends on.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Compare two OpenAPI contracts.
    Diff {
        /// Old OpenAPI YAML or JSON file.
        old: PathBuf,
        /// New OpenAPI YAML or JSON file.
        new: PathBuf,
    },
}
