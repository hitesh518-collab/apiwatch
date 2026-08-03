# Phase 5 — HAR Import: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users import HAR capture files into value-free observed lock entries via `apiwatch record --from-har`.

**Architecture:** New `src/har.rs` module parses HAR JSON, filters entries by content-type and status, groups them by user-provided path identities, and returns grouped recordings. `main.rs` feeds each group through the existing `observed::infer()` / `observed::merge()` / `lockfile::record_observed()` pipeline.

**Tech Stack:** Rust 1.88+, serde_json (HAR parsing), url (path extraction), mime (content-type detection)

## Global Constraints

- MSRV 1.88
- No new dependencies
- All existing tests must remain green
- No lockfile version bump
- Determinisitic output: same HAR + same flags = byte-identical lock
- No scalar values, credentials, or dynamic map keys in serialized output

---

### Task 1: Create `src/har.rs` — types, parsing, and `load_har` skeleton

**Files:**
- Create: `src/har.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Produces: `pub(crate) struct HarRecording { pub method: String, pub path: String, pub body: serde_json::Value }`
- Produces: `pub(crate) enum HarSkipReason { .. }`
- Produces: `pub(crate) type HarRecordings = BTreeMap<String, Vec<HarRecording>>`
- Produces: `pub(crate) fn load_har(path: &Path, path_identities: &[String], status_filter: &[u16]) -> Result<(HarRecordings, Vec<(String, HarSkipReason)>)>`

**Description:** Define all HAR types, deserialization, the public API function skeleton, and the skip-reason enum. Wire up the module in `lib.rs`. No filtering logic yet — just structs and deserialization.

- [ ] **Step 1: Create `src/har.rs` with HAR structs**

```rust
use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Har {
    log: HarLog,
}

#[derive(Debug, Deserialize)]
struct HarLog {
    entries: Vec<HarEntry>,
}

#[derive(Debug, Deserialize)]
struct HarEntry {
    request: HarRequest,
    response: HarResponse,
}

#[derive(Debug, Deserialize)]
struct HarRequest {
    method: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct HarResponse {
    status: u16,
    content: HarContent,
}

#[derive(Debug, Deserialize)]
struct HarContent {
    #[serde(default)]
    #[serde(rename = "mimeType")]
    mime_type: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    encoding: Option<String>,
}

#[derive(Debug)]
pub(crate) struct HarRecording {
    pub method: String,
    pub path: String,
    pub body: serde_json::Value,
}

#[derive(Debug)]
pub(crate) enum HarSkipReason {
    NonJsonContentType(String),
    NonMatchingStatus { status: u16, path: String },
    EmptyBody,
    JsonParseError(String),
    Base64Encoded,
}

pub(crate) type HarRecordings = BTreeMap<String, Vec<HarRecording>>;

pub(crate) fn load_har(
    path: &Path,
    path_identities: &[String],
    status_filter: &[u16],
) -> Result<(HarRecordings, Vec<(String, HarSkipReason)>)> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read HAR file {}", path.display()))?;
    let har: Har = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse HAR file {}", path.display()))?;
    if har.log.entries.is_empty() {
        anyhow::bail!("HAR file contains no entries");
    }

    let mut recordings: HarRecordings = BTreeMap::new();
    let mut skips: Vec<(String, HarSkipReason)> = Vec::new();

    // Placeholder: iterate entries, filter, group — implemented in Task 2

    if recordings.is_empty() {
        anyhow::bail!("no HAR entries matched the recording criteria");
    }

    Ok((recordings, skips))
}
```

- [ ] **Step 2: Declare the module in `src/lib.rs`**

Add after `pub mod observed;`:

```rust
#[doc(hidden)]
pub(crate) mod har;
```

- [ ] **Step 3: Run cargo check to verify it compiles**

```powershell
cargo check
```

Expected: compiles (with warnings about unused imports — fine).

- [ ] **Step 4: Commit**

```bash
git add src/har.rs src/lib.rs
git commit -m "feat: add har module types and load_har skeleton"
```

---

### Task 2: Implement HAR entry filtering and grouping

**Files:**
- Modify: `src/har.rs`

**Interfaces:**
- Produces: full `load_har()` implementation
- Produces: `fn is_json_content_type(mime_type: &str) -> bool`
- Produces: `fn parse_path_identities(identities: &[String]) -> Result<Vec<(String, String)>>` — returns vec of (method, path_prefix)
- Produces: `fn entry_identity(method: &str, path: &str) -> String` — formats "METHOD /path"

**Description:** Implement status filtering, content-type detection, path extraction via the `url` crate, path identity matching, and entry grouping.

- [ ] **Step 1: Write unit test for `is_json_content_type`**

Add at the bottom of `src/har.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_json_content_types() {
        assert!(super::is_json_content_type("application/json"));
        assert!(super::is_json_content_type("application/json; charset=utf-8"));
        assert!(super::is_json_content_type("APPLICATION/JSON"));
        assert!(super::is_json_content_type("application/json+hal"));
        assert!(!super::is_json_content_type("text/plain"));
        assert!(!super::is_json_content_type("application/xml"));
        assert!(!super::is_json_content_type(""));
    }
}
```

- [ ] **Step 2: Run test — expect FAIL**

```powershell
cargo test is_json_content_type
```

- [ ] **Step 3: Implement `is_json_content_type`**

Add after the struct definitions, before `load_har`:

```rust
fn is_json_content_type(mime_type: &str) -> bool {
    let mime_type = mime_type.trim();
    if mime_type.is_empty() {
        return false;
    }
    let lower = mime_type.to_lowercase();
    lower.starts_with("application/json")
        || lower.starts_with("application/vnd.")
}
```

- [ ] **Step 4: Run test — expect PASS**

```powershell
cargo test is_json_content_type
```

- [ ] **Step 5: Write unit test for `parse_path_identities`**

```rust
#[test]
fn parses_valid_path_identities() {
    let ids = parse_path_identities(&[
        "GET /api/users".to_string(),
        "POST /api/orders".to_string(),
    ])
    .expect("should parse");
    assert_eq!(ids, vec![
        ("GET".to_string(), "/api/users".to_string()),
        ("POST".to_string(), "/api/orders".to_string()),
    ]);
}

#[test]
fn parses_path_identity_without_space_as_error() {
    assert!(parse_path_identities(&["no-space".to_string()]).is_err());
}

#[test]
fn normalizes_method_to_uppercase() {
    let ids = parse_path_identities(&["get /api/test".to_string()]).expect("should parse");
    assert_eq!(ids[0].0, "GET");
}
```

- [ ] **Step 6: Run tests — expect FAIL**

```powershell
cargo test parse_path_identity
```

- [ ] **Step 7: Implement `parse_path_identities`**

```rust
fn parse_path_identities(identities: &[String]) -> Result<Vec<(String, String)>> {
    let mut result = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for identity in identities {
        let (method, path) = identity
            .split_once(' ')
            .ok_or_else(|| anyhow::anyhow!(
                "invalid --path-identity '{}': expected 'METHOD /path'",
                identity
            ))?;
        let method = method.to_uppercase();
        let path = path.trim().to_string();
        if path.is_empty() {
            anyhow::bail!("invalid --path-identity '{}': path part is empty", identity);
        }
        if !seen.insert((method.clone(), path.clone())) {
            anyhow::bail!("duplicate --path-identity '{}'", identity);
        }
        result.push((method, path));
    }
    Ok(result)
}
```

Add `use std::collections::HashSet;` to imports.

- [ ] **Step 8: Run tests — expect PASS**

```powershell
cargo test parse_path_identity
```

- [ ] **Step 9: Write unit test for `entry_identity`**

```rust
#[test]
fn formats_entry_identity() {
    assert_eq!(entry_identity("get", "/api/users"), "GET /api/users");
    assert_eq!(entry_identity("POST", "/api/orders"), "POST /api/orders");
}
```

- [ ] **Step 10: Run test — expect FAIL**

```powershell
cargo test entry_identity
```

- [ ] **Step 11: Implement `entry_identity`**

```rust
fn entry_identity(method: &str, path: &str) -> String {
    format!("{} {}", method.to_uppercase(), path)
}
```

- [ ] **Step 12: Run test — expect PASS**

```powershell
cargo test entry_identity
```

- [ ] **Step 13: Write unit test for `load_har` with fixture**

Create `testdata/har/single-entry.har`:

```json
{"log":{"version":"1.2","entries":[{"request":{"method":"GET","url":"https://api.example.com/users/42"},"response":{"status":200,"content":{"mimeType":"application/json","text":"{\"id\":42,\"name\":\"alice\"}"}}}]}}
```

Add test:

```rust
#[test]
fn load_har_single_json_entry() {
    let path = std::path::Path::new("testdata/har/single-entry.har");
    let (recordings, skips) = load_har(&path, &[], &[]).expect("should load");
    assert_eq!(recordings.len(), 1);
    assert!(skips.is_empty());
    let key = recordings.keys().next().unwrap();
    assert_eq!(key, "GET /users/42");
    assert_eq!(recordings[key].len(), 1);
}
```

- [ ] **Step 14: Run test — expect FAIL**

```powershell
cargo test load_har_single_json_entry
```

- [ ] **Step 15: Implement full `load_har` filtering and grouping**

Replace the placeholder body in `load_har` with:

```rust
    let identities = if path_identities.is_empty() {
        None
    } else {
        Some(parse_path_identities(path_identities)?)
    };

    for entry in &har.log.entries {
        let method = entry.request.method.trim().to_uppercase();
        if method.is_empty() {
            continue;
        }

        let parsed_url = match url::Url::parse(&entry.request.url) {
            Ok(u) => u,
            Err(_) => {
                skips.push((
                    format!("{} (invalid URL)", entry.request.url),
                    HarSkipReason::EmptyBody,
                ));
                continue;
            }
        };
        let path = parsed_url.path().to_string();
        let skip_label = format!("{} {}", method, path);

        // Status filter
        if !status_filter.is_empty() {
            if !status_filter.contains(&entry.response.status) {
                skips.push((skip_label, HarSkipReason::NonMatchingStatus {
                    status: entry.response.status,
                    path: path.clone(),
                }));
                continue;
            }
        } else if entry.response.status < 200 || entry.response.status >= 300 {
            skips.push((skip_label, HarSkipReason::NonMatchingStatus {
                status: entry.response.status,
                path: path.clone(),
            }));
            continue;
        }

        // Encoding check
        if let Some(ref enc) = entry.response.content.encoding {
            if enc == "base64" {
                skips.push((skip_label, HarSkipReason::Base64Encoded));
                continue;
            }
        }

        // Content-type check
        if !is_json_content_type(&entry.response.content.mime_type) {
            skips.push((skip_label, HarSkipReason::NonJsonContentType(
                entry.response.content.mime_type.clone(),
            )));
            continue;
        }

        // Body check
        let text = entry.response.content.text.trim().to_string();
        if text.is_empty() {
            skips.push((skip_label, HarSkipReason::EmptyBody));
            continue;
        }

        // JSON parse
        let body: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                skips.push((skip_label, HarSkipReason::JsonParseError(e.to_string())));
                continue;
            }
        };

        // Determine entry key
        let key = if let Some(ref ids) = identities {
            let mut matched = None;
            for (ident_method, ident_path) in ids {
                if method == ident_method && path.starts_with(ident_path.as_str()) {
                    matched = Some(entry_identity(ident_method, ident_path));
                    break;
                }
            }
            match matched {
                Some(k) => k,
                None => {
                    skips.push((skip_label, HarSkipReason::NonMatchingStatus {
                        status: entry.response.status,
                        path: path.clone(),
                    }));
                    continue;
                }
            }
        } else {
            entry_identity(&method, &path)
        };

        recordings
            .entry(key)
            .or_default()
            .push(HarRecording { method, path, body });
    }
```

Add `use url;` to imports.

- [ ] **Step 16: Run test — expect PASS**

```powershell
cargo test load_har_single_json_entry
```

- [ ] **Step 17: Run all har module tests**

```powershell
cargo test har
```

- [ ] **Step 18: Commit**

```bash
git add src/har.rs testdata/har/
git commit -m "feat: implement HAR entry filtering, grouping, and path identity matching"
```

---

### Task 3: Add `--from-har`, `--path-identity`, `--status` flags to CLI

**Files:**
- Modify: `src/cli.rs`

**Interfaces:**
- Produces: updated `Command::Record` variant with new fields

**Description:** Add three optional flags to the Record subcommand. `--from-har` and `--from-json` are mutually exclusive. `--name` becomes optional.

- [ ] **Step 1: Add an argument group for source and new fields to `Command::Record`**

Replace the existing `Record` variant (lines 59-78) with:

```rust
    /// Record the observed shape of one JSON body.
    Record {
        /// HAR file to import (mutually exclusive with --from-json).
        #[arg(long)]
        from_har: Option<PathBuf>,
        /// Local JSON body to record.
        #[arg(long)]
        from_json: Option<PathBuf>,
        /// API name to use as the lockfile key. Required for --from-json;
        /// optional for --from-har (entries auto-keyed by method+path).
        #[arg(long)]
        name: Option<String>,
        /// api.lock path to write.
        #[arg(long)]
        output: PathBuf,
        /// Merge the JSON shape into an existing observed entry.
        #[arg(long)]
        merge: bool,
        /// Mark a JSON object path as a dynamic-key map.
        #[arg(long = "map-at")]
        map_at: Vec<String>,
        /// Observation ratio (0.0-1.0) required before a field hardens.
        #[arg(long = "required-threshold")]
        required_threshold: Option<f64>,
        /// Group HAR entries under this key (repeatable METHOD /path).
        #[arg(long = "path-identity", value_name = "METHOD /path")]
        path_identity: Vec<String>,
        /// Only record responses with these HTTP status codes (repeatable).
        /// When absent, only 2xx responses are recorded.
        #[arg(long = "status", value_name = "CODE")]
        status: Vec<u16>,
    },
```

- [ ] **Step 2: Add validation for source mutual exclusion in main.rs later; check compilation**

```powershell
cargo check
```

Expected: compiles. (Validation for `--from-json`/`--from-har` exclusivity happens in main.rs.)

- [ ] **Step 3: Commit**

```bash
git add src/cli.rs
git commit -m "feat: add --from-har, --path-identity, --status flags to record CLI"
```

---

### Task 4: Wire HAR import path in `main.rs`

**Files:**
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `har::load_har()`, `har::HarRecording`, `har::HarSkipReason`
- Consumes: `observed::infer()`, `observed::merge()`, `lockfile::record_observed()`, `lockfile::render()`, `lockfile::load_or_create_for_record()`

**Description:** Update the `Command::Record` handler to support two branches: `--from-json` (existing behavior) and `--from-har` (new). Validate mutual exclusion and `--name` requirement. Print skip diagnostics.

- [ ] **Step 1: Replace the `Command::Record` match arm (lines 83-104) in main.rs**

```rust
        Command::Record {
            from_har,
            from_json,
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

            match (from_har, from_json) {
                (None, None) => anyhow::bail!("either --from-json or --from-har is required"),
                (Some(_), Some(_)) => {
                    anyhow::bail!("--from-json and --from-har are mutually exclusive")
                }
                (None, Some(ref json_path)) => {
                    let name = name.ok_or_else(|| {
                        anyhow::anyhow!("--name is required for --from-json")
                    })?;
                    let shape = observed::load_shape(json_path)?;
                    let mut lock = lockfile::load_or_create_for_record(&output)?;
                    lockfile::record_observed(
                        &mut lock, &name, shape, merge, &map_at, required_threshold,
                    )?;
                    let rendered = lockfile::render(&lock)?;
                    fs::write(&output, rendered)
                        .with_context(|| format!("failed to write lockfile {}", output.display()))?;
                    println!("Wrote {}", output.display());
                }
                (Some(ref har_path), None) => {
                    let (recordings, skips) =
                        har::load_har(har_path, &path_identity, &status)?;

                    let mut lock = lockfile::load_or_create_for_record(&output)?;

                    let effective_name = name.as_deref();
                    if let Some(single_name) = effective_name {
                        // --name overrides grouping: all entries under one key
                        let mut first = true;
                        for (_key, recs) in &recordings {
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
                                        true, // merge into same name
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
                                if merge {
                                    true
                                } else {
                                    false
                                },
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
                }
            }

            Ok(0)
        }
```

- [ ] **Step 2: Run cargo check to verify it compiles**

```powershell
cargo check
```

- [ ] **Step 3: Run all existing tests — must all pass**

```powershell
cargo test
```

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire HAR import path through record command"
```

---

### Task 5: Integration tests for HAR import

**Files:**
- Modify: `tests/cli_record.rs`
- Create: `testdata/har/single-entry.har` (if not already created)
- Create: `testdata/har/multi-entry.har`
- Create: `testdata/har/mixed-content.har`
- Create: `testdata/har/non-json-entry.har`

**Description:** Integration tests using `assert_cmd` that exercise the full HAR import pipeline end-to-end.

- [ ] **Step 1: Create HAR test fixtures**

Create `testdata/har/single-entry.har`:

```json
{"log":{"version":"1.2","entries":[{"request":{"method":"GET","url":"https://api.example.com/users/42"},"response":{"status":200,"content":{"mimeType":"application/json","text":"{\"id\":42,\"name\":\"alice\"}"}}}]}}
```

Create `testdata/har/multi-entry.har`:

```json
{"log":{"version":"1.2","entries":[{"request":{"method":"GET","url":"https://api.example.com/users/42"},"response":{"status":200,"content":{"mimeType":"application/json","text":"{\"id\":42,\"name\":\"alice\"}"}}},{"request":{"method":"GET","url":"https://api.example.com/orders/7"},"response":{"status":200,"content":{"mimeType":"application/json","text":"{\"order_id\":7,\"total\":99.99}"}}},{"request":{"method":"GET","url":"https://api.example.com/users/99"},"response":{"status":200,"content":{"mimeType":"application/json","text":"{\"id\":99,\"name\":\"bob\"}"}}}]}}
```

Create `testdata/har/mixed-content.har`:

```json
{"log":{"version":"1.2","entries":[{"request":{"method":"GET","url":"https://api.example.com/data"},"response":{"status":200,"content":{"mimeType":"application/json","text":"{\"ok\":true}"}}},{"request":{"method":"GET","url":"https://api.example.com/image"},"response":{"status":200,"content":{"mimeType":"image/png","text":"binary"}}},{"request":{"method":"GET","url":"https://api.example.com/broken"},"response":{"status":200,"content":{"mimeType":"application/json","text":"not json"}}},{"request":{"method":"GET","url":"https://api.example.com/empty"},"response":{"status":200,"content":{"mimeType":"application/json","text":""}}}]}}
```

Create `testdata/har/non-json-entry.har`:

```json
{"log":{"version":"1.2","entries":[{"request":{"method":"GET","url":"https://api.example.com/image"},"response":{"status":200,"content":{"mimeType":"image/png","text":"binary"}}}]}}
```

- [ ] **Step 2: Add integration tests to `tests/cli_record.rs`**

Add after the last test, before the closing of the file:

```rust
#[test]
fn record_from_har_single_entry_creates_observed_lock() {
    let output = temp_lock_path("har-single");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-har",
            "testdata/har/single-entry.har",
            "--output",
            output_arg,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Recorded 1 endpoints:"))
        .stdout(predicate::str::contains("GET /users/42"));

    let lock = fs::read_to_string(&output).expect("lock should exist");
    fs::remove_file(&output).ok();

    assert!(lock.starts_with("version: 2\n"));
    assert!(lock.contains("provenance: observed"));
    assert!(lock.contains("GET /users/42"));
    assert!(lock.contains("\"alice\"") == false);
}

#[test]
fn record_from_har_multi_entry_groups_by_path() {
    let output = temp_lock_path("har-multi");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-har",
            "testdata/har/multi-entry.har",
            "--output",
            output_arg,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Recorded 2 endpoints:"))
        .stdout(predicate::str::contains("GET /users/42: 1 sample(s)"))
        .stdout(predicate::str::contains("GET /users/99: 1 sample(s)"))
        .stdout(predicate::str::contains("GET /orders/7: 1 sample(s)"));

    let lock = fs::read_to_string(&output).expect("lock should exist");
    fs::remove_file(&output).ok();

    assert!(lock.starts_with("version: 2\n"));
    assert!(lock.contains("provenance: observed"));
}

#[test]
fn record_from_har_with_path_identity_groups_entries() {
    let output = temp_lock_path("har-identity");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-har",
            "testdata/har/multi-entry.har",
            "--output",
            output_arg,
            "--path-identity",
            "GET /api/users",
            "--path-identity",
            "GET /orders",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Recorded 3 endpoints:"))
        .stdout(predicate::str::contains("GET /orders: 1 sample(s)"))
        .stdout(predicate::str::contains("GET /api/users: 2 sample(s)"));

    let lock = fs::read_to_string(&output).expect("lock should exist");
    fs::remove_file(&output).ok();

    assert!(lock.contains("GET /api/users"));
    assert!(lock.contains("GET /orders"));
}

#[test]
fn record_from_har_reports_skipped_entries() {
    let output = temp_lock_path("har-mixed");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-har",
            "testdata/har/mixed-content.har",
            "--output",
            output_arg,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Recorded 1 endpoints:"))
        .stdout(predicate::str::contains("Skipped 3 response(s):"))
        .stdout(predicate::str::contains("non-JSON content type"))
        .stdout(predicate::str::contains("JSON parse error"))
        .stdout(predicate::str::contains("empty body"));

    let lock = fs::read_to_string(&output).expect("lock should exist");
    fs::remove_file(&output).ok();

    assert!(lock.starts_with("version: 2\n"));
}

#[test]
fn record_from_har_no_json_entries_fails() {
    let output = temp_lock_path("har-no-json");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-har",
            "testdata/har/non-json-entry.har",
            "--output",
            output_arg,
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("no HAR entries matched"));

    assert!(!output.exists());
}

#[test]
fn record_from_har_with_status_filter() {
    let output = temp_lock_path("har-status");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-har",
            "testdata/har/single-entry.har",
            "--output",
            output_arg,
            "--status",
            "200",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Recorded 1 endpoints:"));

    let lock = fs::read_to_string(&output).expect("lock should exist");
    fs::remove_file(&output).ok();
    assert!(lock.starts_with("version: 2\n"));
}

#[test]
fn record_from_har_with_name_merges_all_under_single_key() {
    let output = temp_lock_path("har-name");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-har",
            "testdata/har/multi-entry.har",
            "--output",
            output_arg,
            "--name",
            "my-api",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Recorded 3 endpoints:"));

    let lock = fs::read_to_string(&output).expect("lock should exist");
    fs::remove_file(&output).ok();

    assert!(lock.contains("my-api"));
    // should only see the --name key, not auto-derived method+path keys
    assert_eq!(lock.matches("provenance: observed").count(), 1);
}

#[test]
fn record_from_har_file_not_found() {
    let output = temp_lock_path("har-missing");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-har",
            "testdata/har/does-not-exist.har",
            "--output",
            output_arg,
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("failed to read HAR file"));
}

#[test]
fn record_from_har_mutual_exclusion_with_from_json() {
    let output = temp_lock_path("har-exclusive");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-har",
            "testdata/har/single-entry.har",
            "--from-json",
            "testdata/observed/portfolio-empty.json",
            "--output",
            output_arg,
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("mutually exclusive"));
}

#[test]
fn record_from_har_existing_tests_still_pass() {
    // Verify --from-json path still works (same as record_creates_a_value_free_v2_observed_lock)
    let output = temp_lock_path("har-backcompat");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "record",
            "--from-json",
            "testdata/observed/portfolio-empty.json",
            "--name",
            "portfolio",
            "--output",
            output_arg,
        ])
        .assert()
        .success();

    let lock = fs::read_to_string(&output).expect("lock should exist");
    fs::remove_file(&output).ok();

    assert!(lock.starts_with("version: 2\n"));
    assert!(lock.contains("provenance: observed"));
}
```

- [ ] **Step 3: Run integration tests — expect PASS**

```powershell
cargo test --test cli_record
```

Note: the existing test `record_creates_a_value_free_v2_observed_lock` will fail because `--name` is now `Option<String>` but the test passes it as a positional (it was previously required positional). Actually, let me check: in clap derive, `--name` is `#[arg(long)]` so it's always an option flag, not a positional. Looking at the existing struct definition, `name: String` with `#[arg(long)]` means it's a required long option. Changing to `Option<String>` with `#[arg(long)]` means the long arg becomes optional. This should be backward-compatible — the old test passes `--name portfolio` which will still set `Some("portfolio")`. Good.

But wait: what about the error message if `--name` is missing for `--from-json`? Currently main.rs does `let name = name.ok_or_else(|| anyhow::anyhow!("--name is required for --from-json"))?;` which handles it.

- [ ] **Step 4: Run ALL existing tests — all must pass**

```powershell
cargo test
cargo clippy -- -D warnings
cargo fmt -- --check
```

- [ ] **Step 5: Commit**

```bash
git add tests/cli_record.rs testdata/har/
git commit -m "test: add integration tests for HAR import"
```

---

## Final Verification

After all tasks complete:

```powershell
cargo test
cargo clippy -- -D warnings
cargo fmt -- --check
```

All tests must pass. Verify that:
1. `apiwatch record --from-har capture.har --output api.lock` produces a valid v2+ lock
2. `apiwatch record --from-har capture.har --output api.lock --path-identity "GET /api/users"` groups entries
3. `apiwatch record --from-har capture.har --output api.lock --status 200` filters by status
4. Non-JSON responses are skipped with diagnostic output
5. No values leak into the lockfile
6. `apiwatch record --from-json` path is unchanged (backward compat)
7. Empty HAR or all-skipped HAR exits code 2
