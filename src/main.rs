use std::fs;

use anyhow::{Context, Result};
use apiwatch::cli::{Cli, Command, OutputFormat};
use apiwatch::diff::Severity;
use apiwatch::{diff, lockfile, observed, openapi, output};
use clap::Parser;

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
            update,
            include_operations,
            max_lock_bytes,
        } => {
            let contract = openapi::load_contract(&openapi)?;
            let scoped = apiwatch::lock_size::scope_contract(&contract, &include_operations)?;
            let scope = lockfile::scope_from_selectors(&include_operations)?;
            let entry = lockfile::build_v4_declared(&scoped, scope, max_lock_bytes)?;

            let rendered = if update {
                if !output.exists() {
                    anyhow::bail!("--update requires an existing lockfile");
                }
                let existing = lockfile::load(&output)?;
                let updated = lockfile::replace_declared_v4(existing, &name, entry)?;
                lockfile::render(&updated)?
            } else {
                let created = lockfile::new_v4(&name, entry)?;
                lockfile::render(&created)?
            };

            if update {
                lockfile::atomic_replace(&output, rendered.as_bytes())?;
            } else {
                lockfile::atomic_write_new(&output, rendered.as_bytes())?;
            }
            println!("Wrote {}", output.display());
            Ok(0)
        }
        Command::Record {
            from_json,
            name,
            output,
            merge,
            map_at,
        } => {
            let shape = observed::load_shape(&from_json)?;
            let mut lock = lockfile::load_or_create_for_record(&output)?;
            lockfile::record_observed(&mut lock, &name, shape, merge, &map_at)?;
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
            match target.kind() {
                lockfile::VerifyTargetKind::Observed { shape: expected } => {
                    if openapi.starts_with("http://") || openapi.starts_with("https://") {
                        anyhow::bail!("observed verification requires a local JSON file");
                    }
                    let current = observed::load_shape(std::path::Path::new(&openapi))?;
                    let changes = observed::compare(expected, &current);
                    let has_changes = !changes.is_empty();
                    let rendered = match format {
                        OutputFormat::Text if changes.is_empty() => {
                            format!("Verified {}\n", target.name())
                        }
                        OutputFormat::Text => output::render_observed_verify_changes(&changes),
                        OutputFormat::Json => {
                            output::render_observed_verify_changes_json(target.name(), &changes)?
                        }
                        OutputFormat::Sarif => output::render_observed_verify_changes_sarif(
                            &lock_path,
                            target.name(),
                            &changes,
                        )?,
                    };
                    print!("{rendered}");
                    Ok(if has_changes { 1 } else { 0 })
                }
                lockfile::VerifyTargetKind::Declared {
                    contract: locked,
                    scope,
                    coverage,
                } => {
                    let current = openapi::load_contract_input(&openapi)?;
                    let current = lockfile::scope_current_for_verify(&current, scope)?;
                    let changes = diff::diff_contracts(locked, &current);
                    let (coverage, limitation) = match coverage {
                        lockfile::DeclaredCoverage::PartialV3 => (
                            output::Coverage::Partial,
                            Some(output::Limitation::Phase2RelockRequired),
                        ),
                        lockfile::DeclaredCoverage::FullV4 => (output::Coverage::Full, None),
                    };
                    let rendered = match format {
                        OutputFormat::Text => {
                            if limitation.is_some() {
                                eprintln!(
                                    "warning: api.lock v3 lacks Phase 2 contract fields; re-lock from the original OpenAPI source for full coverage"
                                );
                            }
                            output::render_declared_verify_text(target.name(), &changes)
                        }
                        OutputFormat::Json => output::render_declared_verify_json(
                            target.name(),
                            coverage,
                            limitation,
                            &changes,
                        )?,
                        OutputFormat::Sarif => output::render_declared_verify_sarif(
                            &lock_path,
                            target.name(),
                            limitation,
                            &changes,
                        )?,
                    };
                    print!("{rendered}");
                    Ok(
                        if changes
                            .iter()
                            .any(|change| change.severity == Severity::Breaking)
                        {
                            1
                        } else {
                            0
                        },
                    )
                }
                lockfile::VerifyTargetKind::LegacyDeclared { .. } => {
                    let contract = openapi::load_contract_input(&openapi)?;
                    let changes = lockfile::compare_verify_target(&target, &contract)?;
                    let limitation = Some(output::Limitation::RouteOnlyLock);
                    let rendered = match format {
                        OutputFormat::Text => {
                            eprintln!(
                                "warning: api.lock v1/v2 declared entry is route-only; full contract changes are not verified"
                            );
                            output::render_declared_verify_text(target.name(), &changes)
                        }
                        OutputFormat::Json => output::render_declared_verify_json(
                            target.name(),
                            output::Coverage::Routes,
                            limitation,
                            &changes,
                        )?,
                        OutputFormat::Sarif => output::render_declared_verify_sarif(
                            &lock_path,
                            target.name(),
                            limitation,
                            &changes,
                        )?,
                    };
                    print!("{rendered}");
                    Ok(
                        if changes
                            .iter()
                            .any(|change| change.severity == Severity::Breaking)
                        {
                            1
                        } else {
                            0
                        },
                    )
                }
            }
        }
    }
}
