mod cli;
mod contract;
mod diff;
mod lockfile;
mod observed;
mod openapi;
mod output;
mod remote;

use std::fs;

use anyhow::{Context, Result};
use clap::Parser;

use crate::cli::{Cli, Command, OutputFormat};
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
        Command::Diff { old, new, format } => {
            let old = openapi::load_contract(&old)?;
            let new_contract = openapi::load_contract(&new)?;
            let changes = diff::diff_contracts(&old, &new_contract);
            let rendered = match format {
                OutputFormat::Text => output::render_changes(&changes),
                OutputFormat::Json => output::render_changes_json(&changes)?,
                OutputFormat::Sarif => output::render_changes_sarif(&new, &changes)?,
            };
            print!("{rendered}");

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
            lock: lock_path,
            format,
        } => {
            let lock = lockfile::load(&lock_path)?;
            let target = lockfile::select_verify_target(&lock, &name)?;
            let contract = openapi::load_contract_input(&openapi)?;
            let changes = lockfile::compare_verify_target(&target, &contract);

            if changes.is_empty() {
                match format {
                    OutputFormat::Text => println!("Verified {}", target.name()),
                    OutputFormat::Json => print!(
                        "{}",
                        output::render_verify_changes_json(target.name(), &changes)?
                    ),
                    OutputFormat::Sarif => print!(
                        "{}",
                        output::render_verify_changes_sarif(&lock_path, target.name(), &changes)?
                    ),
                }
                Ok(0)
            } else {
                let rendered = match format {
                    OutputFormat::Text => output::render_verify_changes(&changes),
                    OutputFormat::Json => {
                        output::render_verify_changes_json(target.name(), &changes)?
                    }
                    OutputFormat::Sarif => {
                        output::render_verify_changes_sarif(&lock_path, target.name(), &changes)?
                    }
                };
                print!("{rendered}");
                Ok(1)
            }
        }
    }
}
