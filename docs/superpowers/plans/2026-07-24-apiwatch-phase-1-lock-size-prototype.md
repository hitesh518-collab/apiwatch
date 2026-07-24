# APIWatch Phase 1 Lock-Size Prototype Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a permanent, deterministic Rust tool that measures three candidate complete-contract lock representations against APIWatch's pinned real-world corpus and proves the normalized boundary excludes source-only values.

**Architecture:** Extract the existing modules into the root library without changing CLI behavior, then add a pure `lock_size` module for selectors, encoders, measurements, and recommendations. A separate workspace binary orchestrates offline corpus verification and atomically writes deterministic JSON and Markdown reports.

**Tech Stack:** Rust 2021, Rust 1.86 MSRV, Cargo workspace, `serde`, `serde_json`, `serde_yaml`, `sha2`, `clap`, `anyhow`, existing OpenAPI normalizer, Python corpus fetcher, GitHub Actions.

## Global Constraints

- Keep the end-user crate name and binary name `apiwatch`.
- Keep `rust-version = "1.86"` for every workspace package.
- Do not change current `apiwatch diff`, `lock`, `record`, or `verify` behavior or output.
- Do not implement or emit `api.lock` v3 in this slice.
- The default measurement ceiling is exactly 5,242,880 bytes.
- Candidate encoders consume only normalized `ApiContract`.
- Exact scoping uses repeatable `--include-operation "METHOD /path"` values.
- Later migration uses `lock --update`; this prototype records but does not
  implement that production interface.
- Never include raw OpenAPI fragments, examples, defaults, descriptions, extensions, credentials, headers, or source payload values.
- Keep the standard root test suite offline.
- Preserve deterministic `BTreeMap` ordering and one trailing newline in every rendered candidate/report.
- Real-corpus files remain under gitignored `.compat-cache/`.
- Write one high-level ignored implementation log entry before completion.

---

### Task 1: Extract a Shared Library Without Behavior Changes

**Files:**
- Create: `src/lib.rs`
- Modify: `src/main.rs`
- Modify: `Cargo.toml`
- Verify: `tests/cli_diff.rs`
- Verify: `tests/cli_lock.rs`
- Verify: `tests/cli_metadata.rs`
- Verify: `tests/cli_record.rs`
- Verify: `tests/cli_verify.rs`

**Interfaces:**
- Consumes: existing modules currently declared by `src/main.rs`
- Produces: documentation-hidden library modules used by the CLI and later tool

- [ ] **Step 1: Capture the green characterization baseline**

Run:

```powershell
cargo test
```

Expected: 142 active tests pass and 5 compatibility tests are ignored.

- [ ] **Step 2: Declare one workspace and the SHA-256 dependency**

Add to `Cargo.toml`:

```toml
[workspace]
members = ["tools/lock-size-report"]
resolver = "2"
```

Do not create the tool package yet. Add:

```toml
sha2 = "0.10"
```

Run:

```powershell
cargo metadata --no-deps
```

Expected: failure because the declared workspace member does not exist. This
is the intentional red state proving the workspace boundary is not yet
complete.

- [ ] **Step 3: Add the shared library root**

Create `src/lib.rs`:

```rust
#![doc = "Internal APIWatch library. Public interfaces are pre-v1 and unstable."]

#[doc(hidden)]
pub mod cli;
#[doc(hidden)]
pub mod contract;
#[doc(hidden)]
pub mod diff;
#[doc(hidden)]
pub mod lockfile;
#[doc(hidden)]
pub mod observed;
#[doc(hidden)]
pub mod openapi;
#[doc(hidden)]
pub mod output;
#[doc(hidden)]
pub mod remote;
```

Remove the eight `mod` declarations from `src/main.rs`. Import modules from the
library:

```rust
use apiwatch::{cli, diff, lockfile, observed, openapi, output};
use apiwatch::cli::{Cli, Command, OutputFormat};
use apiwatch::diff::Severity;
```

- [ ] **Step 4: Add the minimal workspace tool package**

Create `tools/lock-size-report/Cargo.toml`:

```toml
[package]
name = "apiwatch-lock-size-report"
version = "0.1.0"
edition = "2021"
rust-version = "1.86"
publish = false

[dependencies]
apiwatch = { path = "../.." }
```

Create `tools/lock-size-report/src/main.rs`:

```rust
fn main() {
    println!("apiwatch lock-size report prototype");
}
```

- [ ] **Step 5: Verify the refactor**

Run:

```powershell
cargo fmt --all
cargo metadata --no-deps
cargo test --workspace
cargo run -p apiwatch-lock-size-report
```

Expected: metadata succeeds, all existing tests pass unchanged, and the tool
prints `apiwatch lock-size report prototype`.

- [ ] **Step 6: Commit the library boundary**

```powershell
git add Cargo.toml Cargo.lock src/lib.rs src/main.rs tools/lock-size-report
git commit -m "refactor: share apiwatch modules with internal tools"
```

---

### Task 2: Add Exact Operation Selection

**Files:**
- Create: `src/lock_size.rs`
- Modify: `src/lib.rs`
- Test: `src/lock_size.rs`

**Interfaces:**
- Consumes: `contract::{ApiContract, HttpMethod, OperationKey}`
- Produces:
  - `pub fn parse_operation_selector(value: &str) -> Result<OperationKey>`
  - `pub fn scope_contract(contract: &ApiContract, selectors: &[String]) -> Result<ApiContract>`

- [ ] **Step 1: Add failing selector tests**

Add `pub mod lock_size;` with `#[doc(hidden)]` to `src/lib.rs`. Create
`src/lock_size.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{parse_operation_selector, scope_contract};
    use crate::contract::HttpMethod;
    use crate::openapi::load_contract;

    #[test]
    fn selector_normalizes_method_and_preserves_exact_path() {
        let key = parse_operation_selector("get /users/{id}").unwrap();
        assert_eq!(key.method, HttpMethod::Get);
        assert_eq!(key.path, "/users/{id}");
    }

    #[test]
    fn selector_rejects_ambiguous_whitespace_and_invalid_paths() {
        for value in [
            "GET  /users",
            " GET /users",
            "GET /users ",
            "GET users",
            "BOGUS /users",
            "GET /users\u{0001}",
        ] {
            assert!(parse_operation_selector(value).is_err(), "{value:?}");
        }
    }

    #[test]
    fn scope_rejects_duplicates_and_missing_operations() {
        let contract =
            load_contract(Path::new("testdata/openapi/verify_matching.yaml")).unwrap();
        assert!(scope_contract(
            &contract,
            &["get /users".into(), "GET /users".into()]
        )
        .unwrap_err()
        .to_string()
        .contains("duplicate operation selector"));
        assert!(scope_contract(&contract, &["DELETE /missing".into()])
            .unwrap_err()
            .to_string()
            .contains("operation selector was not found"));
    }

    #[test]
    fn empty_selectors_clone_the_full_contract() {
        let contract =
            load_contract(Path::new("testdata/openapi/verify_matching.yaml")).unwrap();
        assert_eq!(scope_contract(&contract, &[]).unwrap(), contract);
    }
}
```

- [ ] **Step 2: Run the tests to verify red**

Run:

```powershell
cargo test lock_size::tests
```

Expected: compilation fails because the selector functions do not exist.

- [ ] **Step 3: Implement exact selection**

Add:

```rust
use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Result};

use crate::contract::{ApiContract, HttpMethod, OperationKey};

pub fn parse_operation_selector(value: &str) -> Result<OperationKey> {
    let Some((method, path)) = value.split_once(' ') else {
        return Err(anyhow!("operation selector must be METHOD /path"));
    };
    if method.is_empty() || path.is_empty() || path.contains(' ') {
        return Err(anyhow!("operation selector must contain one ASCII space"));
    }
    let method = match method.to_ascii_uppercase().as_str() {
        "GET" => HttpMethod::Get,
        "POST" => HttpMethod::Post,
        "PUT" => HttpMethod::Put,
        "PATCH" => HttpMethod::Patch,
        "DELETE" => HttpMethod::Delete,
        "OPTIONS" => HttpMethod::Options,
        "HEAD" => HttpMethod::Head,
        "TRACE" => HttpMethod::Trace,
        _ => return Err(anyhow!("unsupported operation selector method")),
    };
    if !path.starts_with('/') || path.chars().any(char::is_control) {
        return Err(anyhow!("operation selector path must be a safe absolute path"));
    }
    Ok(OperationKey {
        method,
        path: path.to_owned(),
    })
}

pub fn scope_contract(contract: &ApiContract, selectors: &[String]) -> Result<ApiContract> {
    if selectors.is_empty() {
        return Ok(contract.clone());
    }
    let mut selected = BTreeSet::new();
    for selector in selectors {
        let key = parse_operation_selector(selector)?;
        if !selected.insert(key) {
            return Err(anyhow!("duplicate operation selector"));
        }
    }
    let mut operations = BTreeMap::new();
    for key in selected {
        let operation = contract
            .operations
            .get(&key)
            .ok_or_else(|| anyhow!("operation selector was not found"))?;
        operations.insert(key, operation.clone());
    }
    Ok(ApiContract { operations })
}
```

- [ ] **Step 4: Verify selectors**

Run:

```powershell
cargo test lock_size::tests
cargo test --workspace
```

Expected: selector tests and the full workspace pass.

- [ ] **Step 5: Commit**

```powershell
git add src/lib.rs src/lock_size.rs
git commit -m "feat: add exact contract operation selection"
```

---

### Task 3: Encode Expanded Candidates and Prove Privacy

**Files:**
- Modify: `src/contract/mod.rs`
- Modify: `src/lock_size.rs`
- Create: `testdata/openapi/privacy_sentinels.yaml`
- Test: `src/lock_size.rs`

**Interfaces:**
- Produces:
  - `pub const PRIVACY_SENTINELS: &[&str]`
  - `pub enum CandidateKind { ExpandedYaml, CanonicalJson, DeduplicatedYaml }`
  - `pub fn encode_expanded_yaml(contract: &ApiContract) -> Result<Vec<u8>>`
  - `pub fn encode_canonical_json(contract: &ApiContract) -> Result<Vec<u8>>`

- [ ] **Step 1: Add the privacy fixture**

Create `testdata/openapi/privacy_sentinels.yaml`:

```yaml
openapi: 3.0.3
info:
  title: Privacy sentinels
  version: 1.0.0
  description: APIWATCH_DESCRIPTION_SENTINEL
x-private-note: APIWATCH_EXTENSION_SENTINEL
paths:
  /accounts:
    get:
      description: APIWATCH_OPERATION_DESCRIPTION_SENTINEL
      security:
        - bearerAuth: []
      responses:
        "200":
          description: APIWATCH_RESPONSE_DESCRIPTION_SENTINEL
          content:
            application/json:
              schema:
                type: object
                properties:
                  token:
                    type: string
                    default: APIWATCH_DEFAULT_SENTINEL
                    example: APIWATCH_EXAMPLE_SENTINEL
components:
  securitySchemes:
    bearerAuth:
      type: http
      scheme: bearer
      description: APIWATCH_CREDENTIAL_SENTINEL
```

- [ ] **Step 2: Add failing expanded-encoder tests**

Add tests:

```rust
#[test]
fn expanded_encoders_are_deterministic_and_value_free() {
    let contract =
        load_contract(Path::new("testdata/openapi/privacy_sentinels.yaml")).unwrap();
    for rendered in [
        encode_expanded_yaml(&contract).unwrap(),
        encode_canonical_json(&contract).unwrap(),
    ] {
        assert_eq!(rendered.last(), Some(&b'\n'));
        let second = if rendered.starts_with(b"{") {
            encode_canonical_json(&contract).unwrap()
        } else {
            encode_expanded_yaml(&contract).unwrap()
        };
        assert_eq!(rendered, second);
        let text = String::from_utf8(rendered).unwrap();
        for sentinel in PRIVACY_SENTINELS {
            assert!(!text.contains(sentinel));
        }
        assert!(text.contains("/accounts"));
        assert!(text.contains("token"));
    }
}
```

- [ ] **Step 3: Run red**

```powershell
cargo test expanded_encoders_are_deterministic_and_value_free
```

Expected: compilation fails because the encoder functions do not exist.

- [ ] **Step 4: Add deterministic serialization to the normalized model**

Add `Serialize` to the existing derive lists for `ApiContract`, `Operation`,
`AuthRequirement`, `Parameter`, `RequestBody`, `Response`, `Schema`, and
`Property` without changing their fields. Add `Serialize` to the existing
derive lists for `HttpMethod`, `ParameterLocation`, `AuthSchemeKind`, and
`SchemaKind`. Apply `#[serde(rename_all = "lowercase")]` to `HttpMethod` and
`ParameterLocation`. Apply `#[serde(rename_all = "camelCase")]` to
`AuthSchemeKind` and `SchemaKind`. Import `serde::Serialize`.

Define the shared sentinel list in `src/lock_size.rs`:

```rust
pub const PRIVACY_SENTINELS: &[&str] = &[
    "APIWATCH_DESCRIPTION_SENTINEL",
    "APIWATCH_EXTENSION_SENTINEL",
    "APIWATCH_OPERATION_DESCRIPTION_SENTINEL",
    "APIWATCH_RESPONSE_DESCRIPTION_SENTINEL",
    "APIWATCH_DEFAULT_SENTINEL",
    "APIWATCH_EXAMPLE_SENTINEL",
    "APIWATCH_CREDENTIAL_SENTINEL",
];
```

Implement string-key serialization for `OperationKey` and `ParameterKey`:

```rust
impl Serialize for OperationKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("{} {}", self.method.as_str(), self.path))
    }
}

impl Serialize for ParameterKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("{}:{}", self.location.as_str(), self.name))
    }
}
```

- [ ] **Step 5: Implement expanded encoders**

Add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateKind {
    ExpandedYaml,
    CanonicalJson,
    DeduplicatedYaml,
}

pub fn encode_expanded_yaml(contract: &ApiContract) -> Result<Vec<u8>> {
    let mut rendered = serde_yaml::to_string(contract)
        .context("failed to encode expanded YAML")?
        .into_bytes();
    if !rendered.ends_with(b"\n") {
        rendered.push(b'\n');
    }
    Ok(rendered)
}

pub fn encode_canonical_json(contract: &ApiContract) -> Result<Vec<u8>> {
    let mut rendered =
        serde_json::to_vec(contract).context("failed to encode canonical JSON")?;
    rendered.push(b'\n');
    Ok(rendered)
}
```

- [ ] **Step 6: Verify privacy and compatibility**

```powershell
cargo test expanded_encoders_are_deterministic_and_value_free
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: all pass and existing CLI outputs remain unchanged.

- [ ] **Step 7: Commit**

```powershell
git add src/contract/mod.rs src/lock_size.rs testdata/openapi/privacy_sentinels.yaml
git commit -m "feat: encode deterministic expanded contract candidates"
```

---

### Task 4: Add Deduplicated Schemas and Stable Digests

**Files:**
- Modify: `src/lock_size.rs`
- Test: `src/lock_size.rs`

**Interfaces:**
- Produces:
  - `pub fn sha256_id(bytes: &[u8]) -> String`
  - `pub fn encode_deduplicated_yaml(contract: &ApiContract) -> Result<Vec<u8>>`

- [ ] **Step 1: Add failing digest and deduplication tests**

Add:

```rust
#[test]
fn sha256_ids_are_stable_and_prefixed() {
    assert_eq!(
        sha256_id(b"schema"),
        "sha256:df0ad6e43880f09c90ebf95f19110178aba6890df0010ebda7485029e2b543b4"
    );
}

#[test]
fn deduplicated_yaml_interns_repeated_schemas_and_is_private() {
    let contract =
        load_contract(Path::new("testdata/openapi/privacy_sentinels.yaml")).unwrap();
    let first = encode_deduplicated_yaml(&contract).unwrap();
    let second = encode_deduplicated_yaml(&contract).unwrap();
    assert_eq!(first, second);
    let text = String::from_utf8(first).unwrap();
    assert!(text.contains("schemas:"));
    assert!(text.contains("sha256:"));
    for sentinel in PRIVACY_SENTINELS {
        assert!(!text.contains(sentinel));
    }
}

#[test]
fn deduplication_rejects_a_forced_digest_collision() {
    let first = Schema {
        kind: SchemaKind::String,
        nullable: false,
        format: None,
        enum_values: Vec::new(),
        properties: BTreeMap::new(),
    };
    let second = Schema {
        kind: SchemaKind::Boolean,
        nullable: false,
        format: None,
        enum_values: Vec::new(),
        properties: BTreeMap::new(),
    };
    let error = intern_schemas_for_test(&[first, second], |_| "sha256:forced".into())
        .unwrap_err();
    assert!(error.to_string().contains("schema digest collision"));
}
```

In the actual test, construct both complete `Schema` values explicitly using
`SchemaKind`, `nullable`, `format`, `enum_values`, and `BTreeMap::new()`.

- [ ] **Step 2: Run red**

```powershell
cargo test lock_size::tests::sha256_ids_are_stable_and_prefixed
cargo test lock_size::tests::deduplicated_yaml_interns_repeated_schemas_and_is_private
```

Expected: compilation fails because digest and deduplicated encoding are absent.

- [ ] **Step 3: Define the deduplicated wire structures**

Add private serializable structures:

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct DeduplicatedContract {
    operations: BTreeMap<OperationKey, DeduplicatedOperation>,
    schemas: BTreeMap<String, WireSchema>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct DeduplicatedOperation {
    auth: BTreeMap<String, AuthRequirement>,
    parameters: BTreeMap<ParameterKey, WireParameter>,
    request_body: Option<BTreeMap<String, String>>,
    responses: BTreeMap<String, BTreeMap<String, String>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct WireParameter {
    name: String,
    required: bool,
    schema: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct WireSchema {
    kind: SchemaKind,
    nullable: bool,
    format: Option<String>,
    enum_values: Vec<String>,
    properties: BTreeMap<String, WireProperty>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct WireProperty {
    required: bool,
    schema: String,
}
```

- [ ] **Step 4: Implement bottom-up interning**

Implement:

```rust
pub fn sha256_id(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn intern_schema<F>(
    schema: &Schema,
    schemas: &mut BTreeMap<String, WireSchema>,
    canonical: &mut BTreeMap<String, Vec<u8>>,
    digest: &F,
) -> Result<String>
where
    F: Fn(&[u8]) -> String,
{
    let mut properties = BTreeMap::new();
    for (name, property) in &schema.properties {
        let id = intern_schema(&property.schema, schemas, canonical, digest)?;
        properties.insert(
            name.clone(),
            WireProperty {
                required: property.required,
                schema: id,
            },
        );
    }
    let wire = WireSchema {
        kind: schema.kind.clone(),
        nullable: schema.nullable,
        format: schema.format.clone(),
        enum_values: schema.enum_values.clone(),
        properties,
    };
    let bytes = serde_json::to_vec(&wire).context("failed to canonicalize schema")?;
    let id = digest(&bytes);
    if let Some(existing) = canonical.get(&id) {
        if existing != &bytes {
            return Err(anyhow!("schema digest collision"));
        }
    } else {
        canonical.insert(id.clone(), bytes);
        schemas.insert(id.clone(), wire);
    }
    Ok(id)
}
```

Convert every parameter, request-body media type, and response media type to a
schema ID using this function. The test-only collision helper calls the same
internal function with an injected digest function.

- [ ] **Step 5: Render deduplicated YAML**

```rust
pub fn encode_deduplicated_yaml(contract: &ApiContract) -> Result<Vec<u8>> {
    let wire = deduplicate_with(contract, &sha256_id)?;
    let mut rendered = serde_yaml::to_string(&wire)
        .context("failed to encode deduplicated YAML")?
        .into_bytes();
    if !rendered.ends_with(b"\n") {
        rendered.push(b'\n');
    }
    Ok(rendered)
}
```

- [ ] **Step 6: Verify**

```powershell
cargo fmt --all
cargo test lock_size::tests
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: all pass, including collision and privacy tests.

- [ ] **Step 7: Commit**

```powershell
git add src/lock_size.rs
git commit -m "feat: measure deduplicated contract schemas"
```

---

### Task 5: Add Measurements and the Representation Decision Rule

**Files:**
- Modify: `src/lock_size.rs`
- Test: `src/lock_size.rs`

**Interfaces:**
- Produces:
  - `pub struct CandidateMeasurement { pub bytes: u64, pub within_ceiling: bool }`
  - `pub struct ContractMeasurement`
  - `pub enum Recommendation`
  - `pub fn measure_contract(contract: &ApiContract, ceiling: u64) -> Result<ContractMeasurement>`
  - `pub fn recommend(measurements: &[ContractMeasurement], ceiling: u64) -> Recommendation`

- [ ] **Step 1: Add failing boundary tests**

Add table-driven tests with synthetic `ContractMeasurement` values:

```rust
#[test]
fn recommendation_requires_twenty_percent_yaml_headroom() {
    assert_eq!(
        recommend(&[measurement(4_194_304, 3_000_000, 2_000_000)], 5_242_880),
        Recommendation::ExpandedYaml
    );
    assert_eq!(
        recommend(&[measurement(4_194_305, 3_000_000, 2_000_000)], 5_242_880),
        Recommendation::DeduplicatedYaml
    );
}

#[test]
fn recommendation_falls_back_in_the_approved_order() {
    assert_eq!(
        recommend(&[measurement(6_000_000, 5_000_000, 4_000_000)], 5_242_880),
        Recommendation::DeduplicatedYaml
    );
    assert_eq!(
        recommend(&[measurement(6_000_000, 5_500_000, 4_000_000)], 5_242_880),
        Recommendation::CanonicalJson
    );
    assert_eq!(
        recommend(&[measurement(6_000_000, 5_500_000, 5_400_000)], 5_242_880),
        Recommendation::OperationScopingRequired
    );
}

fn measurement(
    expanded_yaml: u64,
    deduplicated_yaml: u64,
    canonical_json: u64,
) -> ContractMeasurement {
    fn candidate(bytes: u64) -> CandidateMeasurement {
        CandidateMeasurement {
            bytes,
            within_ceiling: bytes <= 5_242_880,
        }
    }
    ContractMeasurement {
        operation_count: 1,
        expanded_yaml: candidate(expanded_yaml),
        canonical_json: candidate(canonical_json),
        deduplicated_yaml: candidate(deduplicated_yaml),
    }
}
```

- [ ] **Step 2: Run red**

```powershell
cargo test recommendation_
```

Expected: compilation fails because measurement and recommendation types do
not exist.

- [ ] **Step 3: Implement measurement types**

Use serializable stable names:

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CandidateMeasurement {
    pub bytes: u64,
    pub within_ceiling: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ContractMeasurement {
    pub operation_count: usize,
    pub expanded_yaml: CandidateMeasurement,
    pub canonical_json: CandidateMeasurement,
    pub deduplicated_yaml: CandidateMeasurement,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Recommendation {
    ExpandedYaml,
    DeduplicatedYaml,
    CanonicalJson,
    OperationScopingRequired,
}
```

`measure_contract` renders all three candidates once, counts exact byte-vector
lengths, and compares each with the positive ceiling.

- [ ] **Step 4: Implement the exact recommendation order**

Use an expanded-YAML headroom boundary calculated without floating point:

```rust
let expanded_headroom_limit = ceiling.saturating_mul(4) / 5;
```

Recommend a candidate only when every supplied successful measurement meets
its boundary. Reject an empty slice with `OperationScopingRequired`.

- [ ] **Step 5: Verify**

```powershell
cargo test recommendation_
cargo test lock_size::tests
cargo test --workspace
```

Expected: every boundary and full suite passes.

- [ ] **Step 6: Commit**

```powershell
git add src/lock_size.rs
git commit -m "feat: classify contract lock-size candidates"
```

---

### Task 6: Build the Offline Atomic Report Tool

**Files:**
- Modify: `tools/lock-size-report/Cargo.toml`
- Replace: `tools/lock-size-report/src/main.rs`
- Create: `tools/lock-size-report/src/report.rs`
- Create: `tools/lock-size-report/tests/cli.rs`
- Modify: `Cargo.lock`

**Interfaces:**
- Consumes: `apiwatch::{lock_size, openapi}`
- Produces:
  - offline CLI flags from the design
  - report schema version 1
  - atomic write and `--check` behavior
  - exit codes 0, 1, and 2

- [ ] **Step 1: Add tool dependencies and a failing CLI integration test**

Use:

```toml
[dependencies]
anyhow = "1"
apiwatch = { path = "../.." }
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"
tempfile = "3"
```

In the integration test, copy the repository fixture
`testdata/openapi/verify_matching.yaml` into a temporary cache, compute its
SHA-256 with `sha2`, and write a temporary manifest whose URL contains the
immutable dummy commit
`0123456789abcdef0123456789abcdef01234567`. Use the repository privacy fixture
for `--privacy-fixture`.

Invoke `CARGO_BIN_EXE_apiwatch-lock-size-report` and assert:

- exit 0;
- JSON and Markdown files exist;
- neither contains an absolute temporary directory;
- a second `--check` invocation exits 0;
- changing the JSON file makes `--check` exit 1.

- [ ] **Step 2: Run red**

```powershell
cargo test -p apiwatch-lock-size-report --test cli
```

Expected: failure because the minimal tool does not accept the reporting
arguments.

- [ ] **Step 3: Define CLI and report data**

In `main.rs`:

```rust
#[derive(clap::Parser)]
struct Args {
    #[arg(long)]
    manifest: PathBuf,
    #[arg(long)]
    compat_dir: PathBuf,
    #[arg(long)]
    privacy_fixture: PathBuf,
    #[arg(long, default_value_t = 5_242_880)]
    max_lock_bytes: u64,
    #[arg(long = "include-operation")]
    include_operations: Vec<String>,
    #[arg(long)]
    json_out: PathBuf,
    #[arg(long)]
    markdown_out: PathBuf,
    #[arg(long)]
    check: bool,
}
```

In `report.rs`, define serializable `Report`, `CorpusResult`,
`NormalizationResult`, and `PrivacyResult`. Store source commit by extracting
the 40-character URL segment. Store the manifest's expected error for known
failures, not an OS-specific error chain.

- [ ] **Step 4: Verify corpus files before normalization**

Validate:

- manifest version is 1;
- filename is plain and remains beneath `compat_dir`;
- URL matches an immutable raw GitHub commit;
- SHA-256 is lowercase 64-hex;
- file size is positive and no larger than `max_bytes`;
- computed SHA-256 equals the pin;
- `max_lock_bytes` is positive.

Classify these as invocation/input failures with exit 2.

- [ ] **Step 5: Implement compatibility expectation handling**

For `status: "passing"`, normalize and measure. Any parser error is a behavior
failure with exit 1.

For `status: "known_failing"`, require the error chain to contain
`expected_error`. Unexpected success or a different error is exit 1. Add a
deterministic report row with that manifest error.

- [ ] **Step 6: Implement deterministic rendering and atomic output**

Before rendering the report, load `--privacy-fixture`, encode it with all three
candidates, and reject the run with exit 1 if any byte output contains any
value from `apiwatch::lock_size::PRIVACY_SENTINELS`. Record
`privacy: { passed: true, candidate_count: 3 }` only after all three scans
succeed. Never include a matched sentinel in an error message.

JSON uses:

```rust
serde_json::to_string_pretty(&report)? + "\n"
```

Markdown uses fixed headings and tables in manifest order, no timestamps or
paths. Escape Markdown pipe characters in error text.

Implement `write_or_check` with `tempfile::NamedTempFile::new_in` in the
destination directory. Write and flush all bytes, call `as_file().sync_all()`,
then use `persist(path)` for atomic replacement:

```rust
fn write_or_check(path: &Path, bytes: &[u8], check: bool) -> Result<(), Failure>
```

The temporary handle removes itself on error. In check mode, compare existing
bytes and return exit 1 on mismatch without creating a temporary file.

- [ ] **Step 7: Add failure-preservation tests**

Extend the CLI test:

- prewrite both output files with `preserve-me`;
- invoke with a deliberately wrong corpus hash;
- assert exit 2;
- assert both files still contain `preserve-me`;
- assert the temporary output directory contains only the two preserved files,
  the manifest, and its cache/fixture inputs.

- [ ] **Step 8: Verify**

```powershell
cargo fmt --all
cargo test -p apiwatch-lock-size-report
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: tool and workspace tests pass without warnings.

- [ ] **Step 9: Commit**

```powershell
git add Cargo.toml Cargo.lock tools/lock-size-report
git commit -m "feat: add offline contract size reporting tool"
```

---

### Task 7: Generate and Lock the Real-Corpus Evidence

**Files:**
- Create: `docs/benchmarks/phase-1-lock-size-report.json`
- Create: `docs/benchmarks/phase-1-lock-size-report.md`
- Modify: `src/lock_size.rs` only if a test-first bug fix is required
- Modify: `tools/lock-size-report/` only if a test-first bug fix is required

**Interfaces:**
- Consumes: `.compat-cache`, `compat/specs.json`, report tool
- Produces: deterministic committed evidence and one concrete recommendation

- [ ] **Step 1: Verify the pinned cache**

```powershell
python scripts/fetch_compat_specs.py
```

Expected: all five cached specifications verify and total 25,626,695 bytes.

- [ ] **Step 2: Generate reports**

```powershell
cargo run -p apiwatch-lock-size-report -- `
  --manifest compat/specs.json `
  --compat-dir .compat-cache `
  --privacy-fixture testdata/openapi/privacy_sentinels.yaml `
  --max-lock-bytes 5242880 `
  --json-out docs/benchmarks/phase-1-lock-size-report.json `
  --markdown-out docs/benchmarks/phase-1-lock-size-report.md
```

Expected: GitHub, Asana, and Box have operation counts and all three size
measurements; Stripe and DigitalOcean reproduce their exact expected errors;
privacy passes; the report contains exactly one recommendation.

- [ ] **Step 3: Inspect the evidence**

Run:

```powershell
$report = Get-Content -Raw docs/benchmarks/phase-1-lock-size-report.json | ConvertFrom-Json
if ($report.schema_version -ne 1) { throw 'unexpected report schema' }
if (($report.corpus | Where-Object normalization_status -eq 'passing').Count -ne 3) { throw 'expected three passing specifications' }
if (($report.corpus | Where-Object normalization_status -eq 'known_failing').Count -ne 2) { throw 'expected two known failures' }
if (-not $report.privacy.passed) { throw 'privacy check failed' }
if (-not $report.recommendation) { throw 'missing recommendation' }
```

- [ ] **Step 4: Prove determinism**

```powershell
cargo run -p apiwatch-lock-size-report -- `
  --manifest compat/specs.json `
  --compat-dir .compat-cache `
  --privacy-fixture testdata/openapi/privacy_sentinels.yaml `
  --max-lock-bytes 5242880 `
  --json-out docs/benchmarks/phase-1-lock-size-report.json `
  --markdown-out docs/benchmarks/phase-1-lock-size-report.md `
  --check
```

Expected: exit 0 and no file changes.

- [ ] **Step 5: Handle any discovered defect with TDD**

If generation exposes a tool defect, first add the smallest local fixture or
unit test reproducing it, run that test red, implement the minimal correction,
and rerun the focused and full suites. Do not alter corpus expectations merely
to make the report pass.

- [ ] **Step 6: Commit evidence**

```powershell
git add docs/benchmarks/phase-1-lock-size-report.json docs/benchmarks/phase-1-lock-size-report.md
git commit -m "docs: record Phase 1 lock-size evidence"
```

---

### Task 8: Enforce Reports in CI and Complete the Prototype Slice

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `docs/lockfile-spec.md`
- Modify: `ROADMAP.md`
- Create/update: `implementation-log/2026-07-24-phase-1-lock-size-prototype.md`

**Interfaces:**
- Consumes: committed tool and reports
- Produces: CI regression gate and handoff to the measured v3 schema design

- [ ] **Step 1: Add the tool to Rust and MSRV gates**

Change Rust commands to:

```yaml
- run: cargo fmt --all -- --check
- run: cargo clippy --workspace --all-targets --all-features -- -D warnings
- run: cargo test --workspace
```

Change MSRV to:

```yaml
- run: cargo check --workspace --locked
```

- [ ] **Step 2: Add report verification to the compatibility job**

After the compatibility tests, add:

```yaml
- run: >-
    cargo run -p apiwatch-lock-size-report --
    --manifest compat/specs.json
    --compat-dir .compat-cache
    --privacy-fixture testdata/openapi/privacy_sentinels.yaml
    --max-lock-bytes 5242880
    --json-out docs/benchmarks/phase-1-lock-size-report.json
    --markdown-out docs/benchmarks/phase-1-lock-size-report.md
    --check
```

- [ ] **Step 3: Update lockfile and roadmap documentation**

In `docs/lockfile-spec.md`, add a Phase 1 prototype-results subsection linking
both benchmark reports, stating the measured recommendation verbatim, and
repeating that v3 remains unimplemented pending schema approval.

In `ROADMAP.md`, mark ordered-scope item 1 as completed with the same report
links. Do not mark item 2 or Phase 1 complete.

- [ ] **Step 4: Run the full verification gate**

Use a consistent Rustup stable toolchain for Clippy if local Cargo and Clippy
come from different installations:

```powershell
cargo fmt --all -- --check
$stableRustc = rustup which --toolchain stable rustc
$env:RUSTC = $stableRustc
$env:CARGO_TARGET_DIR = 'target\phase1-clippy-stable'
rustup run stable cargo clippy --workspace --all-targets --all-features -- -D warnings
Remove-Item Env:RUSTC
Remove-Item Env:CARGO_TARGET_DIR
cargo test --workspace
$msrvRustc = rustup which --toolchain 1.86.0 rustc
$env:RUSTC = $msrvRustc
$env:CARGO_TARGET_DIR = 'target\phase1-msrv-1.86'
rustup run 1.86.0 cargo check --workspace --locked
Remove-Item Env:RUSTC
Remove-Item Env:CARGO_TARGET_DIR
python -m unittest discover -s scripts/tests -p "test_*.py"
python scripts/fetch_compat_specs.py
cargo test --test compat -- --ignored --nocapture
cargo run -p apiwatch-lock-size-report -- --manifest compat/specs.json --compat-dir .compat-cache --privacy-fixture testdata/openapi/privacy_sentinels.yaml --max-lock-bytes 5242880 --json-out docs/benchmarks/phase-1-lock-size-report.json --markdown-out docs/benchmarks/phase-1-lock-size-report.md --check
python scripts/release_smoke.py
git diff --check main...HEAD
```

Expected: formatting and Clippy pass; all workspace and Python tests pass;
Rust 1.86 check passes; five corpus cases match expectations; reports are
unchanged; release smoke passes; branch diff has no whitespace errors.

- [ ] **Step 5: Write the ignored implementation log**

Record:

- approved scope and policy decisions;
- candidate representations;
- corpus commits and hashes;
- measured sizes and recommendation;
- red/green evidence;
- verification results;
- known parser failures;
- next step: exact v3 schema design.

Do not stage the log.

- [ ] **Step 6: Commit the CI and documentation gate**

```powershell
git add .github/workflows/ci.yml docs/lockfile-spec.md ROADMAP.md
git commit -m "ci: enforce Phase 1 lock-size evidence"
```

- [ ] **Step 7: Final repository audit**

```powershell
git status --short
git log --oneline main..HEAD
git diff --stat main...HEAD
```

Expected: tracked working tree clean; only Phase 1 prototype design, plan,
library/tooling, fixtures, evidence, CI, and documentation commits are present.

- [ ] **Step 8: Stop at the v3 schema design gate**

Present the measured sizes, concrete representation recommendation, exact
verification results, branch, and HEAD commit. Do not implement `api.lock` v3
until the user approves the measured schema direction.
