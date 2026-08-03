use std::collections::BTreeMap;
use std::fs;

use anyhow::{Context, Result};
use apiwatch::cli::{Cli, Command, OutputFormat};
use apiwatch::{config, diff, har, lockfile, observed, openapi, output, remote};
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
        Command::Init { output } => {
            if output.exists() {
                anyhow::bail!(
                    "{} already exists; use --output to specify a different path",
                    output.display()
                );
            }

            let empty_lock = "version: 4\napis: {}\n";
            fs::write(&output, empty_lock)
                .with_context(|| format!("failed to write {}", output.display()))?;
            println!("Created {}", output.display());

            let workflows_dir = std::path::Path::new(".github/workflows");
            let workflow_path = workflows_dir.join("apiwatch.yml");
            if !workflow_path.exists() {
                fs::create_dir_all(workflows_dir)
                    .context("failed to create .github/workflows directory")?;
                let workflow = r#"name: apiwatch
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
jobs:
  verify:
    uses: hitesh518-collab/apiwatch/.github/workflows/action.yml@main
"#;
                fs::write(&workflow_path, workflow)
                    .with_context(|| format!("failed to write {}", workflow_path.display()))?;
                println!("Created {}", workflow_path.display());
            } else {
                println!("Skipped {} (already exists)", workflow_path.display());
            }

            println!("\nNext steps:");
            println!(
                "  1. Record: apiwatch record --from-har capture.har --output {}",
                output.display()
            );
            println!(
                "  2. Or lock: apiwatch lock --openapi spec.yaml --name my-api --output {}",
                output.display()
            );
            println!(
                "  3. CI: git add {} .github/workflows/ && git commit -m \"add apiwatch\"",
                output.display()
            );

            Ok(0)
        }
        Command::Coverage {
            lock: lock_path,
            name,
        } => {
            let lock = lockfile::load(&lock_path)?;
            let observed = lock.observed_entries();
            if observed.is_empty() {
                println!("no observed entries in lock");
                return Ok(0);
            }

            let entries: Vec<_> = if let Some(ref filter) = name {
                if let Some(entry) = observed.get(filter) {
                    vec![(filter.as_str(), entry)]
                } else {
                    anyhow::bail!("observed entry '{}' not found in lock", filter);
                }
            } else {
                observed.iter().map(|(k, v)| (k.as_str(), v)).collect()
            };

            for (entry_name, entry) in &entries {
                println!(
                    "{} (threshold {:.2}, {} total observations)",
                    entry_name,
                    entry.threshold,
                    total_observations(&entry.shape),
                );
                if !entry.first_seen.is_empty() {
                    println!("  first seen: {}", entry.first_seen);
                }
                if !entry.last_seen.is_empty() {
                    println!("  last seen:  {}", entry.last_seen);
                }
                println!("\n  Fields:");

                let mut fields = Vec::new();
                collect_fields(&entry.shape, "$", entry.threshold, &mut fields);
                for field in &fields {
                    println!("    {} {}", field.path, field.status);
                }
                if fields.is_empty() {
                    println!("    (no fields)");
                }
                println!();
            }

            Ok(0)
        }
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
            from_har,
            from_json,
            from_url,
            method,
            header,
            name,
            output,
            merge,
            map_at,
            required_threshold,
            path_identity,
            status,
        } => {
            if let Some(t) = required_threshold {
                if !(0.0..=1.0).contains(&t) {
                    anyhow::bail!("--required-threshold must be between 0.0 and 1.0");
                }
            }

            let source_count =
                from_har.is_some() as u8 + from_json.is_some() as u8 + from_url.is_some() as u8;
            if source_count == 0 {
                anyhow::bail!("a source is required: --from-json, --from-har, or --from-url");
            }
            if source_count > 1 {
                anyhow::bail!("only one source may be specified");
            }

            if let Some(ref json_path) = from_json {
                let name =
                    name.ok_or_else(|| anyhow::anyhow!("--name is required for --from-json"))?;
                let shape = observed::load_shape(json_path)?;
                let mut lock = lockfile::load_or_create_for_record(&output)?;
                lockfile::record_observed(
                    &mut lock,
                    &name,
                    shape,
                    merge,
                    &map_at,
                    required_threshold,
                )?;
                let rendered = lockfile::render(&lock)?;
                fs::write(&output, rendered)
                    .with_context(|| format!("failed to write lockfile {}", output.display()))?;
                println!("Wrote {}", output.display());
            } else if let Some(ref har_path) = from_har {
                let (recordings, skips) = har::load_har(har_path, &path_identity, &status)?;

                if recordings.is_empty() {
                    anyhow::bail!("no HAR entries matched");
                }

                let mut lock = lockfile::load_or_create_for_record(&output)?;

                let effective_name = name.as_deref();
                if let Some(single_name) = effective_name {
                    let mut first = true;
                    for recs in recordings.values() {
                        for rec in recs {
                            let shape = observed::infer(&rec.body);
                            if first {
                                lockfile::record_observed(
                                    &mut lock,
                                    single_name,
                                    shape,
                                    false,
                                    &map_at,
                                    required_threshold,
                                )?;
                                first = false;
                            } else {
                                lockfile::record_observed(
                                    &mut lock,
                                    single_name,
                                    shape,
                                    true,
                                    &map_at,
                                    required_threshold,
                                )?;
                            }
                        }
                    }
                } else {
                    for (key, recs) in &recordings {
                        if recs.is_empty() {
                            continue;
                        }
                        let merged_shape = {
                            let mut shape = observed::infer(&recs[0].body);
                            for rec in &recs[1..] {
                                observed::merge(&mut shape, &observed::infer(&rec.body));
                            }
                            shape
                        };
                        lockfile::record_observed(
                            &mut lock,
                            key,
                            merged_shape,
                            merge,
                            &map_at,
                            required_threshold,
                        )?;
                    }
                }

                let rendered = lockfile::render(&lock)?;
                fs::write(&output, rendered)
                    .with_context(|| format!("failed to write lockfile {}", output.display()))?;

                println!("Wrote {}", output.display());

                if !recordings.is_empty() {
                    println!("\nRecorded {} endpoints:", recordings.len());
                    for (key, recs) in &recordings {
                        println!("  {key}: {} sample(s)", recs.len());
                    }
                }
                if !skips.is_empty() {
                    println!("\nSkipped {} response(s):", skips.len());
                    for (label, reason) in &skips {
                        let detail = match reason {
                            har::HarSkipReason::NonJsonContentType(mime_type) => {
                                format!("non-JSON content type ({})", mime_type)
                            }
                            har::HarSkipReason::NonMatchingStatus { status, .. } => {
                                format!("non-matching status ({})", status)
                            }
                            har::HarSkipReason::EmptyBody => "empty body".to_string(),
                            har::HarSkipReason::JsonParseError(e) => {
                                format!("JSON parse error: {}", e)
                            }
                            har::HarSkipReason::Base64Encoded => "base64 encoded".to_string(),
                        };
                        println!("  - {}: {}", label, detail);
                    }
                }
            } else if let Some(ref url) = from_url {
                let method = method.trim().to_uppercase();
                if method.is_empty() {
                    anyhow::bail!("--method must not be empty");
                }

                let remote_headers = {
                    let resolved = config::resolve_headers(&BTreeMap::new(), &header)?;
                    if resolved.is_empty() {
                        None
                    } else {
                        Some(resolved)
                    }
                };

                let body = remote::fetch_json(url, &method, remote_headers.as_ref())?;
                let shape = observed::infer(&body);

                let parsed_url =
                    url::Url::parse(url).with_context(|| format!("invalid URL: {url}"))?;
                let path = parsed_url.path().to_string();

                let entry_name = if let Some(ref n) = name {
                    n.clone()
                } else if !path_identity.is_empty() {
                    let mut matched = None;
                    for identity in &path_identity {
                        let (ident_method, ident_path) =
                            identity.split_once(' ').ok_or_else(|| {
                                anyhow::anyhow!(
                                    "invalid --path-identity '{}': expected 'METHOD /path'",
                                    identity
                                )
                            })?;
                        let ident_method = ident_method.to_uppercase();
                        if method == ident_method && path.starts_with(ident_path) {
                            matched = Some(format!("{} {}", ident_method, ident_path));
                            break;
                        }
                    }
                    matched.unwrap_or_else(|| format!("{} {}", method, path))
                } else {
                    format!("{} {}", method, path)
                };

                let mut lock = lockfile::load_or_create_for_record(&output)?;
                lockfile::record_observed(
                    &mut lock,
                    &entry_name,
                    shape,
                    merge,
                    &map_at,
                    required_threshold,
                )?;
                let rendered = lockfile::render(&lock)?;
                fs::write(&output, rendered)
                    .with_context(|| format!("failed to write lockfile {}", output.display()))?;
                println!("Wrote {}", output.display());
                println!("Recorded {} from {}", entry_name, url);
            }

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
            all,
            source_url,
        } => {
            let lock = lockfile::load(&lock_path)?;
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

            if all {
                let base_url = source_url
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("--source-url is required with --all"))?;
                let observed_entries: Vec<_> = lock
                    .observed_entries()
                    .iter()
                    .map(|(name, entry)| (name.clone(), entry.clone()))
                    .collect();
                if observed_entries.is_empty() {
                    anyhow::bail!("no observed entries in lockfile");
                }

                let mut any_breaking = false;
                let mut any_success = false;
                for (entry_name, entry) in &observed_entries {
                    let (method, path) = entry_name.split_once(' ').unwrap_or(("GET", entry_name));
                    let url = format!("{base_url}{path}");

                    let body = match remote::fetch_json(&url, method, remote_headers.as_ref()) {
                        Ok(b) => {
                            any_success = true;
                            b
                        }
                        Err(e) => {
                            eprintln!("error: {entry_name}: {e:#}");
                            continue;
                        }
                    };

                    let current = observed::infer(&body);
                    let report =
                        observed::verify_with_tiers(&entry.shape, &current, entry.threshold);
                    let has_changes = !report.changes.is_empty();
                    let has_tiered = !report.tiered.is_empty();

                    let rendered = match format {
                        OutputFormat::Text if !has_changes && !has_tiered => {
                            format!(
                                "=== {} ===\nVerified {}\n  first seen: {}\n  last seen:  {}\n\n",
                                entry_name, entry_name, entry.first_seen, entry.last_seen
                            )
                        }
                        OutputFormat::Text => format!(
                            "=== {} ===\n{}",
                            entry_name,
                            output::render_observed_verify_with_tiers(
                                entry_name,
                                entry.threshold,
                                &entry.first_seen,
                                &entry.last_seen,
                                &report,
                            )
                        ),
                        OutputFormat::Json => output::render_observed_verify_with_tiers_json(
                            entry_name,
                            entry.threshold,
                            &entry.first_seen,
                            &entry.last_seen,
                            &report,
                        )?,
                        OutputFormat::Sarif => output::render_observed_verify_with_tiers_sarif(
                            &lock_path, entry_name, &report,
                        )?,
                    };
                    print!("{rendered}");

                    if has_changes {
                        any_breaking = true;
                    }
                }

                if !any_success {
                    anyhow::bail!("all observed entries failed to fetch from {base_url}");
                }

                return Ok(if any_breaking { 1 } else { 0 });
            }

            let openapi = openapi.ok_or_else(|| {
                anyhow::anyhow!("OPENAPI source is required for single-entry verify")
            })?;
            let name =
                name.ok_or_else(|| anyhow::anyhow!("--name is required for single-entry verify"))?;
            let target = lockfile::select_verify_target(&lock, &name)?;

            match target.kind() {
                lockfile::VerifyTargetKind::Observed {
                    shape: expected,
                    threshold,
                    first_seen,
                    last_seen,
                } => {
                    if openapi.starts_with("http://") || openapi.starts_with("https://") {
                        anyhow::bail!("observed verification requires a local JSON file");
                    }
                    let current = observed::load_shape(std::path::Path::new(&openapi))?;
                    let report = observed::verify_with_tiers(expected, &current, *threshold);
                    let has_changes = !report.changes.is_empty();
                    let has_tiered = !report.tiered.is_empty();
                    let rendered = match format {
                        OutputFormat::Text if !has_changes && !has_tiered => {
                            format!("Verified {}\n  first seen: {first_seen}\n  last seen:  {last_seen}\n", target.name())
                        }
                        OutputFormat::Text => output::render_observed_verify_with_tiers(
                            target.name(),
                            *threshold,
                            first_seen,
                            last_seen,
                            &report,
                        ),
                        OutputFormat::Json => output::render_observed_verify_with_tiers_json(
                            target.name(),
                            *threshold,
                            first_seen,
                            last_seen,
                            &report,
                        )?,
                        OutputFormat::Sarif => output::render_observed_verify_with_tiers_sarif(
                            &lock_path,
                            target.name(),
                            &report,
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

fn total_observations(shape: &observed::Shape) -> u64 {
    match shape {
        observed::Shape::Object { observations, .. } => *observations,
        _ => 0,
    }
}

struct FieldInfo {
    path: String,
    status: String,
}

fn collect_fields(
    shape: &observed::Shape,
    path: &str,
    threshold: f64,
    entries: &mut Vec<FieldInfo>,
) {
    match shape {
        observed::Shape::Object {
            observations,
            properties,
        } => {
            for (name, property) in properties {
                let property_path = format!("{path}.{name}");
                let hardened =
                    observed::is_hardened(*observations, property.observations, threshold);
                let kind = shape_kind_name(&property.shape);
                let status = if hardened {
                    format!(
                        "{}  {}/{} observations  hardened",
                        kind, property.observations, observations
                    )
                } else {
                    let reason = if *observations < observed::MINIMUM_OBSERVATION_FLOOR {
                        format!(
                            "below floor ({} < {})",
                            observations,
                            observed::MINIMUM_OBSERVATION_FLOOR
                        )
                    } else {
                        let ratio = property.observations as f64 / *observations as f64;
                        format!("{:.2} < {:.2} threshold", ratio, threshold)
                    };
                    format!(
                        "{}  {}/{} observations  lenient ({})",
                        kind, property.observations, observations, reason
                    )
                };
                entries.push(FieldInfo {
                    path: property_path,
                    status,
                });
                collect_fields(
                    &property.shape,
                    &format!("{path}.{name}"),
                    threshold,
                    entries,
                );
            }
        }
        observed::Shape::Array { items } => {
            collect_fields(items, &format!("{path}[]"), threshold, entries);
        }
        observed::Shape::Map { values } => {
            collect_fields(values, &format!("{path}.<map-value>"), threshold, entries);
        }
        observed::Shape::Union { variants } => {
            for variant in variants {
                collect_fields(variant, path, threshold, entries);
            }
        }
        _ => {}
    }
}

fn shape_kind_name(shape: &observed::Shape) -> &'static str {
    match shape {
        observed::Shape::Null => "null",
        observed::Shape::Boolean => "boolean",
        observed::Shape::Number => "number",
        observed::Shape::String => "string",
        observed::Shape::Object { .. } => "object",
        observed::Shape::Map { .. } => "map",
        observed::Shape::Array { .. } => "array",
        observed::Shape::Union { .. } => "union",
        observed::Shape::Unknown => "unknown",
    }
}
