mod cli;
mod contract;
mod diff;
mod openapi;
mod output;

use anyhow::Result;
use clap::Parser;

use crate::cli::{Cli, Command};
use crate::diff::Severity;

fn main() {
    let exit_code = match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error:#}");
            2
        }
    };

    std::process::exit(exit_code);
}

fn run() -> Result<i32> {
    let cli = Cli::parse();

    match cli.command {
        Command::Diff { old, new } => {
            let old = openapi::load_contract(&old)?;
            let new = openapi::load_contract(&new)?;
            let changes = diff::diff_contracts(&old, &new);
            print!("{}", output::render_changes(&changes));

            if changes
                .iter()
                .any(|change| change.severity == Severity::Breaking)
            {
                Ok(1)
            } else {
                Ok(0)
            }
        }
    }
}
