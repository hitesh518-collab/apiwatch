# Phase 5 Remaining Items: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the remaining 5 Phase 5 features: live URL recording, multi-entry verify, init scaffold, coverage command, and onboarding.

**Architecture:** Each feature extends the existing CLI + main.rs dispatch pattern. Live recording and multi-entry verify both leverage `reqwest` (already in deps) for HTTP fetching, reusing `config::resolve_headers` for auth. Init writes template files. Coverage walks the shape tree with existing `is_hardened`. Backward-compatible — no lockfile changes.

**Tech Stack:** Rust 1.88+, reqwest (blocking, already available), serde_json

## Global Constraints

- MSRV 1.88
- No new dependencies
- All existing 336 tests must remain green
- No lockfile version bump
- Deterministic output
- No scalar values, credentials, or dynamic map keys in serialized output

---

### Task 1: Live URL Recording — `remote::fetch_json` helper

**Files:**
- Modify: `src/remote.rs`

**Interfaces:**
- Produces: `pub fn fetch_json(url: &str, method: &str, headers: Option<&BTreeMap<String, String>>) -> Result<serde_json::Value>`

**Description:** Add a dedicated JSON-fetching function to `remote.rs`. It fetches a URL with the given HTTP method and headers, validates the response is JSON, and parses it.

- [ ] **Step 1: Add `fetch_json` to `src/remote.rs`**

Add after `fetch` function (after line 52):

```rust
pub fn fetch_json(
    url: &str,
    method: &str,
    headers: Option<&BTreeMap<String, String>>,
) -> Result<serde_json::Value> {
    let parsed_url =
        reqwest::Url::parse(url).map_err(|error| anyhow!("invalid URL: {error}"))?;
    if !parsed_url.username().is_empty() || parsed_url.password().is_some() {
        return Err(anyhow!("URL credentials are not allowed"));
    }

    let client = Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(10))
        .redirect(Policy::limited(5))
        .build()
        .context("failed to build HTTP client")?;

    let method = reqwest::Method::from_bytes(method.to_uppercase().as_bytes())
        .map_err(|_| anyhow!("invalid HTTP method: {method}"))?;
    let mut request = client.request(method, parsed_url);
    if let Some(hdrs) = headers {
        for (name, value) in hdrs {
            request = request.header(name.as_str(), value.as_str());
        }
    }

    let response = request
        .send()
        .with_context(|| format!("failed to fetch {url}"))?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "server returned {} for {}",
            response.status().as_u16(),
            url
        ));
    }

    if !response_is_json(&response) {
        let ct = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown");
        return Err(anyhow!("response is not JSON (content-type: {ct})"));
    }

    let body = read_limited_body(response)?;
    let value =
        serde_json::from_str(&body).context("failed to parse JSON response")?;

    Ok(value)
}
```

Add `use reqwest::Method;` to imports.

- [ ] **Step 2: Add unit test**

```rust
#[test]
fn fetch_json_rejects_non_json_content_type() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let address = listener.local_addr().expect("listener should have an address");
    let url = format!("http://{}/data", address);

    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            use std::io::Write;
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nhello"
            );
        }
    });

    let result = fetch_json(&url, "GET", None);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("not JSON"));
}
```

- [ ] **Step 3: Run tests — expect PASS**

```powershell
cargo test fetch_json
cargo test
```

- [ ] **Step 4: Commit**

```bash
git add src/remote.rs
git commit -m "feat: add fetch_json helper for live URL recording"
```

---

### Task 2: Live URL Recording — CLI + main.rs wiring

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `remote::fetch_json()`
- Consumes: `observed::infer()`, `lockfile::record_observed()`, `config::resolve_headers()`

**Description:** Add `--from-url` and `--method` flags to the Record command, wire the live recording branch in main.rs, and handle mutual exclusivity with other sources.

- [ ] **Step 1: Update CLI in `src/cli.rs`**

Add to the `Record` variant after the `name` field:

```rust
        /// Live URL to fetch and record (mutually exclusive with --from-json, --from-har).
        #[arg(long)]
        from_url: Option<String>,
        /// HTTP method for --from-url (default GET).
        #[arg(long, default_value = "GET")]
        method: String,
        /// Request headers for --from-url (NAME:${ENV_VAR}).
        #[arg(long = "header", value_name = "NAME:${ENV_VAR}")]
        header: Vec<String>,
```

- [ ] **Step 2: Update main.rs Command::Record destructure**

Add `from_url`, `method`, `header` to the destructure:

```rust
        Command::Record {
            from_har,
            from_json,
            from_url,
            name,
            output,
            merge,
            map_at,
            required_threshold,
            path_identity,
            status,
            method,
            header,
        } => {
```

- [ ] **Step 3: Update source mutual exclusion check**

Change the match from `(from_har, from_json)` to a 3-way check:

```rust
            let source_count = from_har.is_some() as u8
                + from_json.is_some() as u8
                + from_url.is_some() as u8;
            if source_count == 0 {
                anyhow::bail!("a source is required: --from-json, --from-har, or --from-url");
            }
            if source_count > 1 {
                anyhow::bail!("only one source may be specified: --from-json, --from-har, or --from-url");
            }
```

- [ ] **Step 4: Add `--from-url` branch**

Add after the `--from-har` branch and before the closing `}` of the source match:

```rust
                _ if from_url.is_some() => {
                    let url = from_url.as_ref().unwrap();
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

                    let parsed_url = url::Url::parse(url)
                        .with_context(|| format!("invalid URL: {url}"))?;
                    let path = parsed_url.path().to_string();

                    let entry_name = if let Some(ref n) = name {
                        n.clone()
                    } else if !path_identity.is_empty() {
                        // Find matching path identity
                        let mut matched = None;
                        for identity in &path_identity {
                            let (ident_method, ident_path) = identity
                                .split_once(' ')
                                .ok_or_else(|| anyhow::anyhow!(
                                    "invalid --path-identity '{}': expected 'METHOD /path'",
                                    identity
                                ))?;
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
                    fs::write(&output, rendered).with_context(|| {
                        format!("failed to write lockfile {}", output.display())
                    })?;
                    println!("Wrote {}", output.display());
                    println!("Recorded {} from {}", entry_name, url);
                }
```

Add `use url;` to main.rs imports.

- [ ] **Step 5: Update the remaining existing branches**

The `--from-json` and `--from-har` branches need minor adjustments to work with the new 3-way check. Remove their specific match arms and replace with the source_count approach. The existing match arms for `(None, Some(ref json_path))` and `(Some(ref har_path), None)` should be replaced with `if let Some(ref json_path) = from_json` and `if let Some(ref har_path) = from_har` inside the `_ if source_count == 1` block, since `from_har` and `from_json` are now `Option` and we can't match them as a pair directly after adding `from_url`.

Actually simpler: keep the pair match but nest the 3-way count. Move the count check before the pair match:

```rust
            let source_count = from_har.is_some() as u8
                + from_json.is_some() as u8
                + from_url.is_some() as u8;
            if source_count == 0 {
                anyhow::bail!("a source is required: --from-json, --from-har, or --from-url");
            }
            if source_count > 1 {
                anyhow::bail!("only one source may be specified");
            }

            // Use if-let chains for each source since we can't match a 3-tuple of Options cleanly
            if let Some(ref json_path) = from_json {
                let name = name.ok_or_else(|| anyhow::anyhow!("--name is required for --from-json"))?;
                let shape = observed::load_shape(json_path)?;
                let mut lock = lockfile::load_or_create_for_record(&output)?;
                lockfile::record_observed(
                    &mut lock, &name, shape, merge, &map_at, required_threshold,
                )?;
                let rendered = lockfile::render(&lock)?;
                fs::write(&output, rendered)
                    .with_context(|| format!("failed to write lockfile {}", output.display()))?;
                println!("Wrote {}", output.display());
            } else if let Some(ref har_path) = from_har {
                // ... existing HAR code ...
            } else if let Some(ref url) = from_url {
                // ... new URL code ...
            }
```

This is cleaner. Replace the entire inner `match (from_har, from_json)` block with this if-let chain.

- [ ] **Step 6: Run cargo check and tests**

```powershell
cargo check
cargo test
```

- [ ] **Step 7: Commit**

```bash
git add src/cli.rs src/main.rs
git commit -m "feat: add live URL recording with --from-url flag"
```

---

### Task 3: Multi-Entry Verify

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/main.rs`
- Modify: `src/output/mod.rs`

**Interfaces:**
- Consumes: `remote::fetch_json()`, `observed::verify_with_tiers()`, `lockfile::load()`, `output::render_observed_verify_with_tiers()`
- Produces: `pub fn render_multi_verify_text(name: &str, threshold: f64, ...) -> String`

**Description:** Add `--all` and `--source-url` to Verify. When `--all` is set, verify all observed entries against the provided base URL.

- [ ] **Step 1: Update CLI in `src/cli.rs`**

Modify the `Verify` variant:

```rust
    /// Verify one OpenAPI contract against a named api.lock entry.
    Verify {
        /// Current local OpenAPI YAML/JSON file or HTTP(S) URL to verify.
        /// Required unless --all is set.
        openapi: Option<String>,
        /// API name to verify from the lockfile.
        #[arg(long)]
        name: Option<String>,
        /// api.lock file to compare against.
        #[arg(long)]
        lock: PathBuf,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
        #[arg(long, value_hint = clap::ValueHint::DirPath)]
        ref_root: Option<PathBuf>,
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long = "header", value_name = "NAME:${ENV_VAR}")]
        header: Vec<String>,
        /// Verify all observed entries in the lock.
        #[arg(long)]
        all: bool,
        /// Base URL for --all: each entry's path is appended.
        #[arg(long = "source-url", requires = "all")]
        source_url: Option<String>,
    },
```

- [ ] **Step 2: Add `--all` branch in `src/main.rs`**

In the `Command::Verify` handler, before the existing match:

```rust
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
                let observed_entries: Vec<_> = lock.observed_entries()
                    .iter()
                    .map(|(name, entry)| (name.clone(), entry.clone()))
                    .collect();
                if observed_entries.is_empty() {
                    anyhow::bail!("no observed entries in lockfile");
                }

                let mut any_breaking = false;
                for (entry_name, entry) in &observed_entries {
                    let path = entry_name
                        .split_once(' ')
                        .map(|(_, p)| p)
                        .unwrap_or("");
                    let url = format!("{base_url}{path}");

                    let body = match remote::fetch_json(
                        &url,
                        "GET",
                        remote_headers.as_ref(),
                    ) {
                        Ok(b) => b,
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
                                entry_name,
                                entry_name,
                                entry.first_seen,
                                entry.last_seen
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
                            &lock_path,
                            entry_name,
                            &report,
                        )?,
                    };
                    print!("{rendered}");

                    if has_changes {
                        any_breaking = true;
                    }
                }

                return Ok(if any_breaking { 1 } else { 0 });
            }

            // Existing single-entry verify continues...
            let openapi = openapi.ok_or_else(|| {
                anyhow::anyhow!("OPENAPI source is required for single-entry verify")
            })?;
            let target_name = name.ok_or_else(|| {
                anyhow::anyhow!("--name is required for single-entry verify")
            })?;
            // ... rest of existing code unchanged ...
```

- [ ] **Step 3: Add `observed_entries()` accessor to `ApiLock`**

In `src/lockfile/mod.rs`, add:

```rust
impl ApiLock {
    pub fn observed_entries(&self) -> &BTreeMap<String, observed::ObservedEntry> {
        &self.observed
    }
}
```

- [ ] **Step 4: Run cargo check and tests**

```powershell
cargo check
cargo test
```

- [ ] **Step 5: Commit**

```bash
git add src/cli.rs src/main.rs src/lockfile/mod.rs src/output/mod.rs
git commit -m "feat: add multi-entry verify with --all and --source-url"
```

---

### Task 4: `apiwatch init` command

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/main.rs`

**Description:** Add an `Init` command that creates an empty v4 lockfile and a GitHub Actions workflow.

- [ ] **Step 1: Add `Init` to CLI in `src/cli.rs`**

Add after the `Record` variant:

```rust
    /// Scaffold a new api.lock and CI workflow.
    Init {
        /// Lockfile path to create.
        #[arg(long, default_value = "api.lock")]
        output: PathBuf,
    },
```

- [ ] **Step 2: Add `Init` handler in `src/main.rs`**

Add a new match arm before `Command::Diff`:

```rust
        Command::Init { output } => {
            if output.exists() {
                if atty::is(atty::Stream::Stdout) {
                    eprint!("{} already exists. Overwrite? [y/N] ", output.display());
                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input)?;
                    if !input.trim().eq_ignore_ascii_case("y") {
                        anyhow::bail!("aborted");
                    }
                } else {
                    anyhow::bail!(
                        "{} already exists; use --output to specify a different path",
                        output.display()
                    );
                }
            }

            let empty_lock = "version: 4\napis: {}\n";
            fs::write(&output, empty_lock)
                .with_context(|| format!("failed to write {}", output.display()))?;
            println!("Created {}", output.display());

            // Write GitHub Actions workflow
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
            println!("  1. Record: apiwatch record --from-har capture.har --output {}", output.display());
            println!("  2. Or lock: apiwatch lock --openapi spec.yaml --name my-api --output {}", output.display());
            println!("  3. CI: git add {} .github/workflows/ && git commit -m \"add apiwatch\"", output.display());

            Ok(0)
        }
```

Note: `atty` is NOT in Cargo.toml. Instead of adding a dependency, use a simple approach. Replace the `atty::is` check with checking if stdin is a TTY via the standard library:

```rust
            if output.exists() {
                let is_tty = std::io::stdin().lock().lines().next().is_some(); // not great
                // Actually, just use --force flag approach:
```

Let's keep it simple - no interactive prompts. Just error if file exists:

```rust
            if output.exists() {
                anyhow::bail!(
                    "{} already exists; use --output to specify a different path",
                    output.display()
                );
            }
```

- [ ] **Step 3: Run cargo check and tests**

```powershell
cargo check
cargo test
```

- [ ] **Step 4: Commit**

```bash
git add src/cli.rs src/main.rs
git commit -m "feat: add apiwatch init command to scaffold lock and CI workflow"
```

---

### Task 5: Coverage command

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `lockfile::load()`, `observed::is_hardened()`, `observed::Shape`
- Produces: coverage text output

**Description:** Add a `Coverage` command that lists all observed entries in a lock with per-field hardening status.

- [ ] **Step 1: Add `Coverage` to CLI in `src/cli.rs`**

Add after `Init`:

```rust
    /// Report endpoint and field coverage for observed entries.
    Coverage {
        /// api.lock file to inspect.
        #[arg(long)]
        lock: PathBuf,
        /// Filter to a specific observed entry.
        #[arg(long)]
        name: Option<String>,
    },
```

- [ ] **Step 2: Add `Coverage` handler in `src/main.rs`**

```rust
        Command::Coverage { lock: lock_path, name } => {
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
```

- [ ] **Step 3: Add helper functions to `src/main.rs`**

Before the `Covery` handler (before `fn run()`), add these helper functions at the bottom of the file:

```rust
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
            for (name, property) in observations_and_properties(observations, properties) {
                let property_path = format!("{path}.{name}");
                let hardened = observed::is_hardened(
                    *observations,
                    property.observations,
                    threshold,
                );
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
                            observations, observed::MINIMUM_OBSERVATION_FLOOR
                        )
                    } else {
                        let ratio =
                            property.observations as f64 / *observations as f64;
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
                collect_fields(&property.shape, &property_path, threshold, entries);
            }
        }
        observed::Shape::Array { items } => {
            collect_fields(items, &format!("{path}[]"), threshold, entries);
        }
        observed::Shape::Map { values } => {
            collect_fields(
                values,
                &format!("{path}.<map-value>"),
                threshold,
                entries,
            );
        }
        observed::Shape::Union { variants } => {
            for variant in variants {
                collect_fields(variant, path, threshold, entries);
            }
        }
        _ => {}
    }
}

fn observations_and_properties<'a>(
    observations: &'a u64,
    properties: &'a BTreeMap<String, observed::ObservedProperty>,
) -> impl Iterator<Item = (&'a u64, &'a observed::ObservedProperty)> {
    properties.values().map(move |p| (observations, p))
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
```

Add `use std::collections::BTreeMap;` to imports if not present (it already is at the top).

Actually, `observed::ObservedProperty` may not be pub. Let me check... In `src/observed/mod.rs`, `ObservedProperty` is `pub struct ObservedProperty`. Good.

Wait, but `MINIMUM_OBSERVATION_FLOOR` needs to be `pub`. Let me check — it's defined as `pub const MINIMUM_OBSERVATION_FLOOR: u64 = 3;`. Good.

- [ ] **Step 4: Run cargo check and tests**

```powershell
cargo check
cargo test
```

If `observed::ObservedProperty` or `observed::MINIMUM_OBSERVATION_FLOOR` are not accessible from main.rs, make them `pub` in `src/observed/mod.rs`.

- [ ] **Step 5: Commit**

```bash
git add src/cli.rs src/main.rs
git commit -m "feat: add coverage command for observed entries"
```

---

### Task 6: Onboarding — README quickstart + sample HAR

**Files:**
- Modify: `README.md`
- Create: `testdata/har/example-quickstart.har`

**Description:** Add a 3-step quickstart to the README and a sample HAR fixture for newcomers to try.

- [ ] **Step 1: Create sample HAR fixture**

Create `testdata/har/example-quickstart.har`:

```json
{"log":{"version":"1.2","entries":[{"request":{"method":"GET","url":"https://api.example.com/users"},"response":{"status":200,"content":{"mimeType":"application/json","text":"[{\"id\":1,\"name\":\"Alice\",\"email\":\"alice@example.com\"},{\"id\":2,\"name\":\"Bob\",\"email\":\"bob@example.com\"}]"}}},{"request":{"method":"GET","url":"https://api.example.com/orders?status=open"},"response":{"status":200,"content":{"mimeType":"application/json","text":"[{\"order_id\":101,\"total\":29.99,\"items\":[{\"sku\":\"A-1\",\"qty\":2}]},{\"order_id\":102,\"total\":14.50,\"items\":[{\"sku\":\"B-7\",\"qty\":1}]}]"}}}]}}
```

- [ ] **Step 2: Update README with quickstart**

At the top of README, replace or augment the usage section with:

```markdown
## Quickstart

```bash
# 1. Record from browser traffic (export HAR from DevTools)
apiwatch record --from-har traffic.har --output api.lock
# Try it now with our example: testdata/har/example-quickstart.har

# 2. Verify all observed entries against the live API
apiwatch verify --all --lock api.lock --source-url https://api.example.com

# 3. Scaffold CI
apiwatch init --output api.lock
git add api.lock .github/workflows/
git commit -m "add apiwatch contract evidence"
```
```

- [ ] **Step 3: Update known limitations table**

Remove the "Observed inputs (D-19)" row since HAR import is now done. Add:

```
| Live recording (D-19) | Live URL and HAR import implemented; passive proxy is post-v1. | Phase 5 |
```

Actually, just remove the D-19 row entirely since HAR import and live recording are both done now.

- [ ] **Step 4: Run cargo test to confirm no regressions**

```powershell
cargo test
cargo clippy -- -D warnings
cargo fmt -- --check
```

- [ ] **Step 5: Commit**

```bash
git add README.md testdata/har/example-quickstart.har
git commit -m "docs: add quickstart guide and sample HAR fixture"
```

---

## Final Verification

After all tasks complete:

```powershell
cargo test
cargo clippy -- -D warnings
cargo fmt -- --check
```

All tests must pass. Verify:
1. `apiwatch record --from-url https://httpbin.org/json --output test.lock` produces a valid observed lock
2. `apiwatch verify --all --lock api.lock --source-url https://api.example.com` verifies all observed entries
3. `apiwatch init --output api.lock` creates an empty v4 lock + CI workflow
4. `apiwatch coverage --lock api.lock` lists entries with hardening status
5. Quickstart example works end-to-end with the sample HAR
