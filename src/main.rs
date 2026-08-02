use std::collections::BTreeMap;
use std::fs;

use anyhow::{Context, Result};
use apiwatch::cli::{Cli, Command, OutputFormat};
use apiwatch::{config, diff, lockfile, observed, openapi, output};
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
        Command::Diff {
            old,
            new,
            format,
            ref_root,
            config: config_path,
        } => {
            let old = openapi::load_contract_with_ref_root(&old, ref_root.clone())?;
            let new_contract = openapi::load_contract_with_ref_root(&new, ref_root)?;
            let mut changes = diff::diff_contracts(&old, &new_contract);
            let cfg = load_optional_config(config_path.as_deref())?;
            if let Some(ref cfg) = cfg {
                config::apply_config(&mut changes, cfg);
            }
            let exit_code =
                config::compute_exit_code(&changes, cfg.as_ref().and_then(|c| c.fail_on.as_ref()));
            let rendered = match format {
                OutputFormat::Text => output::render_changes(&changes),
                OutputFormat::Json => output::render_changes_json(&changes)?,
                OutputFormat::Sarif => output::render_changes_sarif(&new, &changes)?,
            };
            print!("{rendered}");
            Ok(exit_code)
        }
        Command::Lock {
            openapi,
            name,
            output,
            update,
            include_operations,
            max_lock_bytes,
            ref_root,
        } => {
            let contract = openapi::load_contract_with_ref_root(&openapi, ref_root)?;
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
            required_threshold,
        } => {
            if let Some(t) = required_threshold {
                if !(0.0..=1.0).contains(&t) {
                    anyhow::bail!("--required-threshold must be between 0.0 and 1.0");
                }
            }
            let shape = observed::load_shape(&from_json)?;
            let mut lock = lockfile::load_or_create_for_record(&output)?;
            lockfile::record_observed(&mut lock, &name, shape, merge, &map_at, required_threshold)?;
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
            ref_root,
            config: config_path,
            header,
        } => {
            let lock = lockfile::load(&lock_path)?;
            let target = lockfile::select_verify_target(&lock, &name)?;
            let cfg =
                load_optional_config_with_discovery(config_path.as_deref(), Some(&lock_path))?;
            let remote_headers = config::resolve_headers(
                cfg.as_ref()
                    .map(|c| &c.remote.headers)
                    .unwrap_or(&BTreeMap::new()),
                &header,
            )?;
            let remote_headers = if remote_headers.is_empty() {
                None
            } else {
                Some(remote_headers)
            };
            match target.kind() {
                lockfile::VerifyTargetKind::Observed {
                    shape: expected,
                    threshold,
                } => {
                    if openapi.starts_with("http://") || openapi.starts_with("https://") {
                        anyhow::bail!("observed verification requires a local JSON file");
                    }
                    let current = observed::load_shape(std::path::Path::new(&openapi))?;
                    let changes = observed::compare(expected, &current, *threshold);
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
                    let current = openapi::load_contract_input_with_ref_root(
                        &openapi,
                        ref_root,
                        remote_headers.as_ref(),
                    )?;
                    let current = lockfile::scope_current_for_verify(&current, scope)?;
                    let mut changes = diff::diff_contracts(locked, &current);
                    if let Some(ref cfg) = cfg {
                        config::apply_config(&mut changes, cfg);
                    }
                    let exit_code = config::compute_exit_code(
                        &changes,
                        cfg.as_ref().and_then(|c| c.fail_on.as_ref()),
                    );
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
                    Ok(exit_code)
                }
                lockfile::VerifyTargetKind::LegacyDeclared { .. } => {
                    let contract = openapi::load_contract_input_with_ref_root(
                        &openapi,
                        ref_root,
                        remote_headers.as_ref(),
                    )?;
                    let mut changes = lockfile::compare_verify_target(&target, &contract)?;
                    if let Some(ref cfg) = cfg {
                        config::apply_config(&mut changes, cfg);
                    }
                    let exit_code = config::compute_exit_code(
                        &changes,
                        cfg.as_ref().and_then(|c| c.fail_on.as_ref()),
                    );
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
                    Ok(exit_code)
                }
            }
        }
    }
}

fn load_optional_config(explicit_path: Option<&std::path::Path>) -> Result<Option<config::Config>> {
    match explicit_path {
        Some(path) => {
            let cfg = config::Config::load(path)?;
            Ok(Some(cfg))
        }
        None => Ok(None),
    }
}

fn load_optional_config_with_discovery(
    explicit_path: Option<&std::path::Path>,
    discover_root: Option<&std::path::Path>,
) -> Result<Option<config::Config>> {
    match explicit_path {
        Some(path) => {
            let cfg = config::Config::load(path)?;
            Ok(Some(cfg))
        }
        None => match discover_root {
            Some(root) => match config::Config::discover(root) {
                Ok(cfg) => Ok(Some(cfg)),
                Err(_) => Ok(None),
            },
            None => Ok(None),
        },
    }
}
