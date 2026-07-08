mod cli;

use anyhow::Result;
use clap::Parser;

use crate::cli::{Cli, Command};

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
            println!("diffing {} -> {}", old.display(), new.display());
            Ok(0)
        }
    }
}
