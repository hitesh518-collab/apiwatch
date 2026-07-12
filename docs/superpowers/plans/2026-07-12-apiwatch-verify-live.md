# APIWatch Verify Live Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let `apiwatch verify` compare a named v1 `api.lock` entry against a local OpenAPI file or an HTTP/HTTPS OpenAPI URL.

**Architecture:** Keep lockfile comparison and rendering unchanged. Add a narrow remote source module that classifies URL inputs and performs bounded blocking fetches. Refactor OpenAPI parsing so local files and fetched document text share the same format-aware validation and normalization path.

**Tech Stack:** Rust 2021, `clap`, `anyhow`, `openapiv3`, `serde_json`, `serde_yaml`, `reqwest` 0.12 (`blocking`, `rustls-tls`), and standard-library `TcpListener` test servers.

## Global Constraints

- Command shape: `apiwatch verify <OPENAPI_OR_URL> --name <NAME> --lock <PATH>`.
- Only `http://` and `https://` are remote. Inputs with any other `://` scheme are exit-`2` errors. All other values remain local paths.
- Network policy: 10-second timeout, at most five redirects, response body at most 10 MiB.
- Preserve output: match exits `0`, deterministic drift exits `1`, and input/fetch/parse/lock errors exit `2` with no success output.
- Do not send auth, cookies, custom headers, or user configuration.
- No caching, lockfile mutation, schema/auth comparison beyond v1 operations, multi-entry verification, or CI work.
- Use ignored `implementation-log/` for high-level progress; never stage it.

---

## File Structure

- `src/remote.rs`: URL classification, bounded HTTP fetch, response-format inference.
- `src/openapi/mod.rs`: shared local/remote OpenAPI text parsing and normalization.
- `src/cli.rs`: Verify source becomes `String`.
- `src/main.rs`: Verify routes through the shared local-or-remote loader.
- `tests/cli_verify.rs`: one-shot local HTTP server and live Verify integration coverage.
- `Cargo.toml`, `Cargo.lock`: Rustls blocking HTTP client.
- `README.md`, `CHANGELOG.md`: live URL behavior and bounds.

### Task 1: Remote Source And Shared Parser

**Files:**
- Create: `src/remote.rs`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `src/main.rs:1-7`
- Modify: `src/openapi/mod.rs:19-37`
- Test: `src/remote.rs`

**Interfaces:**
- Produces `remote::fetch(input: &str) -> Result<Option<RemoteOpenApi>>`.
- Produces `RemoteOpenApi { text: String, is_json: bool }`.
- Produces `openapi::load_contract_text(text: &str, is_json: bool, location: &str) -> Result<ApiContract>`.
- Produces `openapi::load_contract_input(input: &str) -> Result<ApiContract>`.

- [ ] **Step 1: Write failing remote unit tests**

Create `src/remote.rs` with this test module before adding production code:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetch_rejects_an_unsupported_url_scheme() {
        let error = fetch("ftp://example.test/openapi.yaml")
            .expect_err("unsupported scheme should be rejected");
        assert!(error.to_string().contains("unsupported OpenAPI URL scheme"));
    }

    #[test]
    fn read_body_rejects_more_than_ten_mebibytes() {
        let body = vec![b'x'; MAX_RESPONSE_BYTES + 1];
        let error = read_limited_body(std::io::Cursor::new(body))
            .expect_err("oversized body should be rejected");
        assert!(error.to_string().contains("remote OpenAPI response exceeds 10 MiB"));
    }
}
```

- [ ] **Step 2: Run the tests red**

Run: `cargo test remote::tests`

Expected: compilation fails because the remote module and its functions do not exist.

- [ ] **Step 3: Add the bounded fetcher**

Add this dependency to `Cargo.toml`:

```toml
reqwest = { version = "0.12", default-features = false, features = ["blocking", "rustls-tls"] }
```

Implement the module with these exact components:

```rust
pub const MAX_RESPONSE_BYTES: usize = 10 * 1024 * 1024;

pub struct RemoteOpenApi {
    pub text: String,
    pub is_json: bool,
}

pub fn fetch(input: &str) -> anyhow::Result<Option<RemoteOpenApi>>;
fn remote_url(input: &str) -> anyhow::Result<Option<reqwest::Url>>;
fn read_limited_body(reader: impl std::io::Read) -> anyhow::Result<String>;
```

`remote_url` splits once at `:`. If no scheme exists, or the remainder does not begin `//`, return `Ok(None)` for a local path. Accept case-insensitive `http` and `https`, parse with `reqwest::Url::parse`, and report malformed URLs as `invalid OpenAPI URL`. Reject every other `://` scheme as `unsupported OpenAPI URL scheme`.

`fetch` builds `reqwest::blocking::Client` with `timeout(Duration::from_secs(10))` and `redirect(Policy::limited(5))`; it sends a GET, rejects every non-success status with `remote OpenAPI request returned a non-success status`, and never includes response bodies in errors. Determine `is_json` from a JSON media type or the final response URL ending in `.json`. `read_limited_body` reads no more than `MAX_RESPONSE_BYTES + 1`, rejects larger responses, and returns UTF-8 text only.

Add `mod remote;` to `src/main.rs`.

- [ ] **Step 4: Factor OpenAPI parsing into text loading**

Retain the public local loader and add this shared boundary:

```rust
pub fn load_contract_text(text: &str, is_json: bool, location: &str) -> Result<ApiContract> {
    validate_raw_openapi_paths(text, is_json)?;
    let document: OpenAPI = if is_json {
        serde_json::from_str(text).with_context(|| format!("failed to parse OpenAPI JSON {location}"))?
    } else {
        serde_yaml::from_str(text).with_context(|| format!("failed to parse OpenAPI YAML {location}"))?
    };
    ensure_openapi_3(&document)?;
    normalize(document)
}

pub fn load_contract_input(input: &str) -> Result<ApiContract> {
    if let Some(remote) = crate::remote::fetch(input)? {
        return load_contract_text(&remote.text, remote.is_json, "remote document");
    }
    load_contract(Path::new(input))
}
```

Make `load_contract` read the local file as it does now, choose JSON from its extension, and call `load_contract_text` with `path.to_string_lossy().as_ref()`. This preserves the existing `failed to parse OpenAPI JSON <PATH>` and `failed to parse OpenAPI YAML <PATH>` wording. Fetched text uses the fixed `remote document` location so no raw URL is included in parse errors.

- [ ] **Step 5: Run the unit tests green**

Run: `cargo test remote::tests`

Expected: both remote tests pass and `Cargo.lock` contains the resolved `reqwest` graph.

- [ ] **Step 6: Commit Task 1**

```bash
git add Cargo.toml Cargo.lock src/remote.rs src/openapi/mod.rs src/main.rs
git commit -m "Add bounded remote OpenAPI loader"
git push origin main
```

### Task 2: Live Verify Route And Integration Tests

**Files:**
- Modify: `src/cli.rs:33-42`
- Modify: `src/main.rs:59-79`
- Modify: `tests/cli_verify.rs`
- Create: `testdata/openapi/verify_matching.json`

**Interfaces:**
- Consumes `openapi::load_contract_input(input: &str) -> Result<ApiContract>` from Task 1.
- Produces unchanged Verify `0`/`1`/`2` behavior for local files and URLs.

- [ ] **Step 1: Add failing live CLI tests and one-shot server helper**

Add this helper to `tests/cli_verify.rs`:

```rust
fn serve_once(status: &str, content_type: &str, body: &'static str, suffix: &str) -> String {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
    let address = listener.local_addr().expect("test server should have an address");
    let status = status.to_string();
    let content_type = content_type.to_string();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("test server should accept");
        let mut request = [0_u8; 1024];
        stream.read(&mut request).expect("test server should read request");
        write!(stream, "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
            .expect("test server should write response");
    });
    format!("http://{address}/{suffix}")
}
```

Add tests with these assertions:

```rust
#[test]
fn verify_exits_zero_for_matching_remote_operations() {
    let url = serve_once("200 OK", "application/yaml", include_str!("../testdata/openapi/verify_matching.yaml"), "openapi.yaml");
    verify_command(&url, "users", "testdata/lock/verify_users.lock")
        .assert().success().stdout("Verified users\n");
}

#[test]
fn verify_exits_one_for_remote_operation_drift() {
    let url = serve_once("200 OK", "application/yaml", include_str!("../testdata/openapi/verify_current.yaml"), "openapi.yaml");
    verify_command(&url, "users", "testdata/lock/verify_users.lock")
        .assert().code(1).stdout("REMOVED GET /users\nREMOVED GET /zeta\nADDED POST /users\nADDED POST /zeta\n");
}

#[test]
fn verify_exits_two_for_a_remote_non_success_status() {
    let url = serve_once("503 Service Unavailable", "text/plain", "unavailable", "openapi.yaml");
    verify_command(&url, "users", "testdata/lock/verify_users.lock")
        .assert().code(2).stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("remote OpenAPI request returned a non-success status"));
}

#[test]
fn verify_exits_two_for_an_unsupported_remote_url_scheme() {
    verify_command("ftp://example.test/openapi.yaml", "users", "testdata/lock/verify_users.lock")
        .assert().code(2).stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("unsupported OpenAPI URL scheme"));
}
```

Create `verify_matching.json` as the JSON equivalent of `verify_matching.yaml`. Add a fifth test that serves this JSON fixture with `Content-Type: application/json` from a `.yaml` URL and expects `Verified users`.

- [ ] **Step 2: Run a focused test red**

Run: `cargo test --test cli_verify verify_exits_zero_for_matching_remote_operations`

Expected: FAIL because Verify still treats its positional source as a local `PathBuf`.

- [ ] **Step 3: Route Verify through the shared input loader**

Change `src/cli.rs`:

```rust
/// Current local OpenAPI YAML/JSON file or HTTP(S) URL to verify.
openapi: String,
```

Change the Verify route in `src/main.rs` to:

```rust
let contract = openapi::load_contract_input(&openapi)?;
```

Do not change lock loading, target selection, comparison, rendering, or the existing exit branches.

- [ ] **Step 4: Run all Verify CLI tests green**

Run: `cargo test --test cli_verify verify_`

Expected: all local checks and remote match, drift, non-success, unsupported-scheme, and JSON-content-type tests pass.

- [ ] **Step 5: Commit Task 2**

```bash
git add src/cli.rs src/main.rs tests/cli_verify.rs testdata/openapi/verify_matching.json
git commit -m "Add live verify command"
git push origin main
```

### Task 3: Documentation And Final Verification

**Files:**
- Modify: `README.md:19-27`
- Modify: `CHANGELOG.md:5-10`
- Create: `implementation-log/2026-07-12-apiwatch-verify-live.md` (ignored)

**Interfaces:**
- Documents `apiwatch verify <OPENAPI_OR_URL> --name <NAME> --lock <PATH>`.
- Documents HTTP/HTTPS support, 10-second timeout, 10 MiB response limit, and existing exit codes.

- [ ] **Step 1: Update README and changelog**

Add this README example:

```bash
apiwatch verify https://api.example.com/openapi.yaml --name users --lock api.lock
```

State that Verify accepts local YAML/JSON files plus HTTP/HTTPS URLs, exits `0` for a match, `1` for drift, and `2` for invalid local or remote input. Document the 10-second timeout and 10 MiB limit. State that auth, custom headers, and configuration files are not included. Add a changelog entry for URL-backed verification and fetch failures exiting `2`.

- [ ] **Step 2: Run release gates**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
git diff --check
```

Expected: every command exits `0`.

- [ ] **Step 3: Run direct smoke checks**

```bash
cargo run --quiet -- verify testdata/openapi/verify_matching.yaml --name users --lock testdata/lock/verify_users.lock
cargo run --quiet -- verify testdata/openapi/verify_current.yaml --name users --lock testdata/lock/verify_users.lock
```

Expected: the local match prints `Verified users` and exits `0`; local drift prints removed operations before added operations and exits `1`. Run the focused remote-match integration test and confirm exit `0`.

- [ ] **Step 4: Write the implementation log**

Record URL support, network limits, changed files, verification, commits, and remaining non-goals in the ignored log file.

- [ ] **Step 5: Commit Task 3**

```bash
git add README.md CHANGELOG.md
git commit -m "Document live verify command"
git push origin main
```

## Plan Self-Review

- Spec coverage: Task 1 covers source detection, Rustls fetches, network bounds, response-format inference, and shared parsing. Task 2 covers command routing plus local-server match, drift, error, scheme, and JSON cases. Task 3 covers docs and release verification.
- Placeholder scan: every implementation step identifies functions, files, test assertions, and expected commands.
- Type consistency: Task 1 defines `RemoteOpenApi`, `remote::fetch`, `openapi::load_contract_text`, and `openapi::load_contract_input`; Task 2 consumes the shared loader with the same signatures.
