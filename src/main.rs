mod cli;
mod contract;
mod diff;
mod lockfile;
mod openapi;
mod output;

use std::fs;

use anyhow::{Context, Result};
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
        Command::Lock {
            openapi,
            name,
            output,
        } => {
            let contract = openapi::load_contract(&openapi)?;
            let lock = lockfile::from_contract(&name, &contract)?;
            let rendered = lockfile::render(&lock)?;
            fs::write(&output, rendered)
                .with_context(|| format!("failed to write lockfile {}", output.display()))?;
            println!("Wrote {}", output.display());
            Ok(0)
        }
        Command::Verify {
            openapi,
            name,
            lock,
        } => {
            let lock = lockfile::load(&lock)?;
            let target = lockfile::select_verify_target(&lock, &name)?;
            let contract = openapi::load_contract(&openapi)?;
            let changes = lockfile::compare_verify_target(&target, &contract);

            if changes.is_empty() {
                println!("Verified {}", target.name());
                Ok(0)
            } else {
                print!("{}", output::render_verify_changes(&changes));
                Ok(1)
            }
        }
    }
}
