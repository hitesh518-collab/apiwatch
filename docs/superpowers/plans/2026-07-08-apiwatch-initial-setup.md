# apiwatch Initial Setup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create the public `apiwatch` repository, scaffold a Rust CLI, and implement the first high-confidence OpenAPI semantic diff slice.

**Architecture:** The CLI parses commands with `clap`, loads OpenAPI YAML/JSON into a normalized contract model, diffs normalized contracts, and renders deterministic terminal output. Diffing stays independent of raw OpenAPI so future lockfiles, JSON samples, GraphQL, and runtime captures can reuse the same core model.

**Tech Stack:** Rust, `clap`, `serde`, `serde_yaml`, `serde_json`, `openapiv3`, `anyhow`, `assert_cmd`, `predicates`, GitHub CLI.

## Global Constraints

- Project name: `apiwatch`.
- Repository visibility: public.
- License: Apache-2.0.
- Default branch: `main`.
- Positioning: `API lockfiles for external services.`
- Repository description: `Lock, diff, and verify the APIs your code depends on.`
- MVP input: local OpenAPI 3.x YAML and JSON files.
- MVP command: `apiwatch diff old.openapi.yaml new.openapi.yaml`.
- Exit code `0`: no breaking changes.
- Exit code `1`: breaking changes found.
- Exit code `2`: invalid input, parse failure, unsupported OpenAPI shape, or internal error.
- v0.1 supported OpenAPI subset: paths, HTTP methods, parameters, JSON request bodies, JSON responses, status codes, content types, object properties, required fields, enum values, primitive types, nullable, and format.
- Rule philosophy: fewer high-confidence rules are better than noisy broad detection.
- Do not add dashboards, accounts, cloud backend, static code scanning, runtime monitoring, or AI features.

---

## File Structure

- Create `Cargo.toml`: Rust package metadata, dependencies, and dev-dependencies.
- Create `src/main.rs`: binary entrypoint and process exit handling.
- Create `src/cli.rs`: `clap` command and argument definitions.
- Create `src/contract/mod.rs`: normalized contract structs shared by loaders and diff rules.
- Create `src/openapi/mod.rs`: OpenAPI file loading and normalization into `contract`.
- Create `src/diff/mod.rs`: typed change model and high-confidence diff rules.
- Create `src/output/mod.rs`: human-readable grouped change rendering.
- Create `tests/cli_diff.rs`: CLI integration tests and exit-code checks.
- Create `testdata/openapi/endpoint_removed_old.yaml`: old fixture for endpoint removal.
- Create `testdata/openapi/endpoint_removed_new.yaml`: new fixture for endpoint removal.
- Create `testdata/openapi/no_breaking_old.yaml`: old fixture for non-breaking endpoint addition.
- Create `testdata/openapi/no_breaking_new.yaml`: new fixture for non-breaking endpoint addition.
- Create `README.md`: project positioning, status, and first CLI example.
- Create `IDEA.md`: concise product idea summary derived from the original idea brief.
- Create `DESIGN.md`: short pointer to the approved design spec.
- Create `CONTRIBUTING.md`: local development workflow.
- Create `CODE_OF_CONDUCT.md`: Contributor Covenant reference text.
- Create `LICENSE`: Apache-2.0 license text.
- Create `docs/change-rules.md`: early breaking, warning, and non-breaking rule catalog.
- Create `docs/lockfile-spec.md`: draft `api.lock` direction.
- Create `.github/workflows/ci.yml`: Rust formatting, clippy, and test checks.
- Create `.github/ISSUE_TEMPLATE/bug_report.md`: bug report template.
- Create `.github/ISSUE_TEMPLATE/feature_request.md`: feature request template.

---

### Task 1: Public Repository Setup

**Files:**
- Modify: `.git/config`
- Remote: GitHub repository `apiwatch`

**Interfaces:**
- Consumes: local git repository on branch `main`.
- Produces: remote named `origin` pointing at the public GitHub repository.

- [ ] **Step 1: Check GitHub CLI authentication**

Run:

```bash
gh auth status
```

Expected: PASS with an authenticated GitHub account that can create repositories. If the command fails because `gh` is not installed or not authenticated, stop and ask the project owner to authenticate GitHub CLI.

- [ ] **Step 2: Check whether `apiwatch` already exists**

Run:

```bash
gh repo view apiwatch --json name,owner,visibility,url
```

Expected when the repo does not exist: FAIL with a not-found message. Expected when the repo exists: PASS with repository JSON; if it exists and is owned by the project owner, reuse it instead of creating a duplicate.

- [ ] **Step 3: Create the public GitHub repository**

Run only if Step 2 confirms the repo does not exist:

```bash
gh repo create apiwatch --public --description "Lock, diff, and verify the APIs your code depends on." --source . --remote origin
```

Expected: PASS and `.git/config` contains an `origin` remote.

- [ ] **Step 4: Add repository topics**

Run:

```bash
gh repo edit apiwatch --add-topic api --add-topic openapi --add-topic cli --add-topic rust --add-topic contract-testing --add-topic developer-tools --add-topic ci
```

Expected: PASS.

- [ ] **Step 5: Push the current design commit**

Run:

```bash
git push -u origin main
```

Expected: PASS and branch `main` tracks `origin/main`.

- [ ] **Step 6: Verify remote configuration**

Run:

```bash
git remote -v
```

Expected: output includes `origin` for fetch and push.

---

### Task 2: Open-Source Project Documentation

**Files:**
- Create: `README.md`
- Create: `IDEA.md`
- Create: `DESIGN.md`
- Create: `CONTRIBUTING.md`
- Create: `CODE_OF_CONDUCT.md`
- Create: `LICENSE`
- Create: `docs/change-rules.md`
- Create: `docs/lockfile-spec.md`
- Create: `.github/ISSUE_TEMPLATE/bug_report.md`
- Create: `.github/ISSUE_TEMPLATE/feature_request.md`

**Interfaces:**
- Consumes: approved design at `docs/superpowers/specs/2026-07-08-apiwatch-design.md`.
- Produces: contributor-facing project documentation and issue templates.

- [ ] **Step 1: Create README**

Create `README.md` with:

````markdown
# apiwatch

API lockfiles for external services.

`apiwatch` is a CLI-first open-source tool for locking, diffing, and verifying the APIs your code depends on.

The mental model:

```text
package-lock.json : packages
api.lock          : external APIs
```

## Status

`apiwatch` is in early development. The first milestone is semantic diffing for local OpenAPI 3.x files.

## Planned CLI

```bash
apiwatch diff old.openapi.yaml new.openapi.yaml
apiwatch lock --config apiwatch.yaml
apiwatch verify
```

## MVP Scope

- Parse local OpenAPI 3.x YAML and JSON files.
- Normalize API operations into an internal contract model.
- Detect high-confidence breaking changes.
- Print CI-friendly output.

## Non-Goals For The MVP

- Dashboard
- User accounts
- Cloud backend
- Static code scanning
- Runtime monitoring
- AI features

## License

Apache-2.0
````

- [ ] **Step 2: Create IDEA.md**

Create `IDEA.md` with:

```markdown
# apiwatch Idea

`apiwatch` prevents third-party API changes from silently breaking applications.

Most API monitoring answers whether an API is up. `apiwatch` focuses on whether the API contract still matches what a repository expects.

The first version starts with OpenAPI because it gives structured contracts:

1. Import two OpenAPI 3.x files.
2. Normalize them into contract snapshots.
3. Compare the snapshots.
4. Report breaking, warning, and non-breaking changes.

Longer term, `apiwatch` can support lockfiles, CI verification, remote specs, runtime JSON samples, and language scanners.
```

- [ ] **Step 3: Create DESIGN.md**

Create `DESIGN.md` with:

```markdown
# apiwatch Design

The approved design lives in:

`docs/superpowers/specs/2026-07-08-apiwatch-design.md`

Short version:

- Rust CLI.
- OpenAPI-first MVP.
- Normalized contract model.
- Semantic diff rules.
- CI-friendly output and exit codes.
- Apache-2.0 public open-source repository.
```

- [ ] **Step 4: Create contribution docs**

Create `CONTRIBUTING.md` with:

````markdown
# Contributing

Thanks for helping build `apiwatch`.

## Local Development

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## Project Direction

The first milestone is a trustworthy OpenAPI semantic diff engine. Prefer small, well-tested rules over broad noisy detection.

## Pull Requests

- Keep changes focused.
- Add fixtures for diff behavior.
- Update docs when changing CLI behavior or rule classification.
````

- [ ] **Step 5: Create code of conduct**

Create `CODE_OF_CONDUCT.md` with:

```markdown
# Code of Conduct

This project follows the Contributor Covenant Code of Conduct.

Be respectful, constructive, and considerate. The goal is to make `apiwatch` a useful and welcoming open-source developer tool.
```

- [ ] **Step 6: Create Apache-2.0 license**

Create `LICENSE` using the official Apache License 2.0 text from:

```text
https://www.apache.org/licenses/LICENSE-2.0.txt
```

Expected: `LICENSE` begins with `Apache License` and `Version 2.0, January 2004`.

- [ ] **Step 7: Create change rules doc**

Create `docs/change-rules.md` with:

```markdown
# Change Rules

`apiwatch` classifies semantic API changes as breaking, warning, or non-breaking.

## Breaking

- Endpoint removed.
- HTTP method removed.
- Required request field added.
- Response field removed.
- Response field type changed.
- Enum value removed.
- Successful status code removed.
- Content type changed.

## Warning

- Nullable changed.
- Numeric type widened or narrowed.
- Format changed.
- Response field became optional.
- New error status code added.
- Unsupported or ambiguous OpenAPI shape.

## Non-Breaking

- Endpoint added.
- Optional response field added.
- Optional request parameter added.

## Philosophy

Rules should be high-confidence and explainable. False positives reduce trust, so uncertain cases should be warnings before they become breaking changes.
```

- [ ] **Step 8: Create draft lockfile spec**

Create `docs/lockfile-spec.md` with:

````markdown
# api.lock Draft

`api.lock` is planned as a repository-level lockfile for external API contracts.

This format is intentionally unstable during early development.

Possible shape:

```yaml
version: 1
apis:
  github:
    source: openapi
    base_url: https://api.github.com
    endpoints:
      - method: GET
        path: /repos/{owner}/{repo}
        response_schema_hash: sha256:example
```

The lockfile should avoid secrets and sensitive raw payloads. It should store normalized contract metadata and hashes.
````

- [ ] **Step 9: Create issue templates**

Create `.github/ISSUE_TEMPLATE/bug_report.md` with:

````markdown
---
name: Bug report
about: Report incorrect behavior in apiwatch
title: ""
labels: bug
assignees: ""
---

## What happened?

## What did you expect?

## Command

```bash
apiwatch diff old.openapi.yaml new.openapi.yaml
```

## OpenAPI fixtures

Attach or link the smallest OpenAPI files that reproduce the behavior.
````

Create `.github/ISSUE_TEMPLATE/feature_request.md` with:

````markdown
---
name: Feature request
about: Suggest a focused improvement for apiwatch
title: ""
labels: enhancement
assignees: ""
---

## Problem

## Proposed behavior

## Example

```bash
apiwatch diff old.openapi.yaml new.openapi.yaml
```
````

- [ ] **Step 10: Verify docs**

Run:

```bash
git diff --check
```

Expected: PASS with no whitespace errors.

- [ ] **Step 11: Commit documentation**

Run:

```bash
git add README.md IDEA.md DESIGN.md CONTRIBUTING.md CODE_OF_CONDUCT.md LICENSE docs/change-rules.md docs/lockfile-spec.md .github/ISSUE_TEMPLATE/bug_report.md .github/ISSUE_TEMPLATE/feature_request.md
git commit -m "docs: add open-source project docs"
```

Expected: PASS.

---

### Task 3: Rust CLI Skeleton

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/cli.rs`
- Create: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: project name and CLI command choices from the design.
- Produces:
  - `cli::Cli`
  - `cli::Command::Diff { old: PathBuf, new: PathBuf }`
  - binary command `apiwatch diff <OLD> <NEW>`

- [ ] **Step 1: Create Cargo manifest**

Create `Cargo.toml` with:

```toml
[package]
name = "apiwatch"
version = "0.1.0"
edition = "2021"
license = "Apache-2.0"
description = "Lock, diff, and verify the APIs your code depends on."
repository = "https://github.com/hitesh0861/apiwatch"

[dependencies]
anyhow = "1"
clap = { version = "4", features = ["derive"] }

[dev-dependencies]
assert_cmd = "2"
predicates = "3"
```

- [ ] **Step 2: Create CLI parser**

Create `src/cli.rs` with:

```rust
use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "apiwatch")]
#[command(about = "Lock, diff, and verify the APIs your code depends on.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Compare two OpenAPI contracts.
    Diff {
        /// Old OpenAPI YAML or JSON file.
        old: PathBuf,
        /// New OpenAPI YAML or JSON file.
        new: PathBuf,
    },
}
```

- [ ] **Step 3: Create binary entrypoint**

Create `src/main.rs` with:

```rust
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
```

- [ ] **Step 4: Create CI workflow**

Create `.github/workflows/ci.yml` with:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

jobs:
  rust:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - run: cargo fmt --all -- --check
      - run: cargo clippy --all-targets --all-features -- -D warnings
      - run: cargo test
```

- [ ] **Step 5: Verify skeleton**

Run:

```bash
cargo fmt
cargo test
cargo run -- diff old.yaml new.yaml
```

Expected: tests pass and the run command prints `diffing old.yaml -> new.yaml`.

- [ ] **Step 6: Commit skeleton**

Run:

```bash
git add Cargo.toml src/main.rs src/cli.rs .github/workflows/ci.yml
git commit -m "feat: add Rust CLI skeleton"
```

Expected: PASS.

---

### Task 4: Normalized Contract Model

**Files:**
- Create: `src/contract/mod.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: no earlier domain types.
- Produces:
  - `contract::ApiContract`
  - `contract::Operation`
  - `contract::HttpMethod`
  - `contract::Response`
  - `contract::Schema`
  - `contract::SchemaKind`
  - `contract::Property`

- [ ] **Step 1: Create contract module**

Create `src/contract/mod.rs` with:

```rust
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiContract {
    pub operations: BTreeMap<OperationKey, Operation>,
}

impl ApiContract {
    pub fn new() -> Self {
        Self {
            operations: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct OperationKey {
    pub method: HttpMethod,
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Options,
    Head,
    Trace,
}

impl HttpMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Options => "OPTIONS",
            Self::Head => "HEAD",
            Self::Trace => "TRACE",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    pub responses: BTreeMap<String, Response>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub content: BTreeMap<String, Schema>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    pub kind: SchemaKind,
    pub nullable: bool,
    pub format: Option<String>,
    pub enum_values: Vec<String>,
    pub properties: BTreeMap<String, Property>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaKind {
    Object,
    Array,
    String,
    Integer,
    Number,
    Boolean,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    pub required: bool,
    pub schema: Box<Schema>,
}
```

- [ ] **Step 2: Wire module into binary**

Modify `src/main.rs` so the module list is:

```rust
mod cli;
mod contract;
```

- [ ] **Step 3: Verify contract module compiles**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 4: Commit contract model**

Run:

```bash
git add src/main.rs src/contract/mod.rs
git commit -m "feat: add normalized contract model"
```

Expected: PASS.

---

### Task 5: OpenAPI Loader For Paths And Responses

**Files:**
- Modify: `Cargo.toml`
- Create: `src/openapi/mod.rs`
- Modify: `src/main.rs`
- Create: `testdata/openapi/endpoint_removed_old.yaml`
- Create: `testdata/openapi/endpoint_removed_new.yaml`

**Interfaces:**
- Consumes:
  - `contract::ApiContract`
  - `contract::HttpMethod`
  - `contract::Operation`
  - `contract::OperationKey`
  - `contract::Response`
  - `contract::Schema`
  - `contract::SchemaKind`
- Produces:
  - `openapi::load_contract(path: &Path) -> anyhow::Result<ApiContract>`

- [ ] **Step 1: Add parsing dependencies**

Modify `Cargo.toml` dependencies to:

```toml
[dependencies]
anyhow = "1"
clap = { version = "4", features = ["derive"] }
openapiv3 = "2"
serde_json = "1"
serde_yaml = "0.9"
```

- [ ] **Step 2: Create endpoint removal fixtures**

Create `testdata/openapi/endpoint_removed_old.yaml` with:

```yaml
openapi: 3.0.3
info:
  title: Example
  version: 1.0.0
paths:
  /users:
    get:
      responses:
        "200":
          description: OK
          content:
            application/json:
              schema:
                type: object
                properties:
                  id:
                    type: string
```

Create `testdata/openapi/endpoint_removed_new.yaml` with:

```yaml
openapi: 3.0.3
info:
  title: Example
  version: 1.0.0
paths: {}
```

- [ ] **Step 3: Create OpenAPI loader**

Create `src/openapi/mod.rs` with:

```rust
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use openapiv3::{
    MediaType, OpenAPI, Operation as OpenApiOperation, PathItem, ReferenceOr, Response as OpenApiResponse,
    Schema as OpenApiSchema, SchemaKind as OpenApiSchemaKind, StatusCode, Type,
};

use crate::contract::{
    ApiContract, HttpMethod, Operation, OperationKey, Property, Response, Schema, SchemaKind,
};

pub fn load_contract(path: &Path) -> Result<ApiContract> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read OpenAPI file {}", path.display()))?;
    let document: OpenAPI = if path.extension().and_then(|value| value.to_str()) == Some("json") {
        serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse OpenAPI JSON {}", path.display()))?
    } else {
        serde_yaml::from_str(&raw)
            .with_context(|| format!("failed to parse OpenAPI YAML {}", path.display()))?
    };

    normalize(document)
}

fn normalize(document: OpenAPI) -> Result<ApiContract> {
    let mut contract = ApiContract::new();

    for (path, item) in document.paths.paths {
        let item = resolve_path_item(item)?;
        insert_operation(&mut contract, &path, HttpMethod::Get, item.get.as_ref())?;
        insert_operation(&mut contract, &path, HttpMethod::Post, item.post.as_ref())?;
        insert_operation(&mut contract, &path, HttpMethod::Put, item.put.as_ref())?;
        insert_operation(&mut contract, &path, HttpMethod::Patch, item.patch.as_ref())?;
        insert_operation(&mut contract, &path, HttpMethod::Delete, item.delete.as_ref())?;
        insert_operation(&mut contract, &path, HttpMethod::Options, item.options.as_ref())?;
        insert_operation(&mut contract, &path, HttpMethod::Head, item.head.as_ref())?;
        insert_operation(&mut contract, &path, HttpMethod::Trace, item.trace.as_ref())?;
    }

    Ok(contract)
}

fn resolve_path_item(item: ReferenceOr<PathItem>) -> Result<PathItem> {
    match item {
        ReferenceOr::Item(item) => Ok(item),
        ReferenceOr::Reference { reference } => Err(anyhow!(
            "path item references are not supported yet: {reference}"
        )),
    }
}

fn insert_operation(
    contract: &mut ApiContract,
    path: &str,
    method: HttpMethod,
    operation: Option<&OpenApiOperation>,
) -> Result<()> {
    let Some(operation) = operation else {
        return Ok(());
    };

    let mut responses = BTreeMap::new();
    for (status, response) in &operation.responses.responses {
        let status = match status {
            StatusCode::Code(code) => code.to_string(),
            StatusCode::Range(range) => format!("{range:?}"),
        };
        let response = normalize_response(response)?;
        responses.insert(status, response);
    }

    contract.operations.insert(
        OperationKey {
            method,
            path: path.to_string(),
        },
        Operation { responses },
    );

    Ok(())
}

fn normalize_response(response: &ReferenceOr<OpenApiResponse>) -> Result<Response> {
    let response = match response {
        ReferenceOr::Item(response) => response,
        ReferenceOr::Reference { reference } => {
            return Err(anyhow!("response references are not supported yet: {reference}"));
        }
    };

    let mut content = BTreeMap::new();
    for (content_type, media_type) in &response.content {
        content.insert(content_type.clone(), normalize_media_type(media_type)?);
    }

    Ok(Response { content })
}

fn normalize_media_type(media_type: &MediaType) -> Result<Schema> {
    match &media_type.schema {
        Some(schema) => normalize_schema(schema),
        None => Ok(unknown_schema()),
    }
}

fn normalize_schema(schema: &ReferenceOr<OpenApiSchema>) -> Result<Schema> {
    let schema = match schema {
        ReferenceOr::Item(schema) => schema,
        ReferenceOr::Reference { reference } => {
            return Err(anyhow!("schema references are not supported yet: {reference}"));
        }
    };

    let mut normalized = unknown_schema();
    normalized.nullable = schema.schema_data.nullable;

    match &schema.schema_kind {
        OpenApiSchemaKind::Type(Type::Object(object)) => {
            normalized.kind = SchemaKind::Object;
            normalized.properties = object
                .properties
                .iter()
                .map(|(name, schema)| {
                    let required = object.required.contains(name);
                    let schema = normalize_schema(schema)?;
                    Ok((
                        name.clone(),
                        Property {
                            required,
                            schema: Box::new(schema),
                        },
                    ))
                })
                .collect::<Result<BTreeMap<_, _>>>()?;
        }
        OpenApiSchemaKind::Type(Type::Array(array)) => {
            normalized.kind = SchemaKind::Array;
            if let Some(items) = &array.items {
                normalized.properties.insert(
                    "items".to_string(),
                    Property {
                        required: true,
                        schema: Box::new(normalize_schema(items)?),
                    },
                );
            }
        }
        OpenApiSchemaKind::Type(Type::String(string)) => {
            normalized.kind = SchemaKind::String;
            normalized.format = string.format.clone().map(|format| format.to_string());
            normalized.enum_values = string.enumeration.iter().flatten().cloned().collect();
        }
        OpenApiSchemaKind::Type(Type::Integer(integer)) => {
            normalized.kind = SchemaKind::Integer;
            normalized.format = integer.format.clone().map(|format| format.to_string());
            normalized.enum_values = integer
                .enumeration
                .iter()
                .flatten()
                .map(|value| value.to_string())
                .collect();
        }
        OpenApiSchemaKind::Type(Type::Number(number)) => {
            normalized.kind = SchemaKind::Number;
            normalized.format = number.format.clone().map(|format| format.to_string());
            normalized.enum_values = number
                .enumeration
                .iter()
                .flatten()
                .map(|value| value.to_string())
                .collect();
        }
        OpenApiSchemaKind::Type(Type::Boolean {}) => {
            normalized.kind = SchemaKind::Boolean;
        }
        _ => {
            normalized.kind = SchemaKind::Unknown;
        }
    }

    Ok(normalized)
}

fn unknown_schema() -> Schema {
    Schema {
        kind: SchemaKind::Unknown,
        nullable: false,
        format: None,
        enum_values: Vec::new(),
        properties: BTreeMap::new(),
    }
}
```

- [ ] **Step 4: Wire OpenAPI module**

Modify the module list in `src/main.rs` to:

```rust
mod cli;
mod contract;
mod openapi;
```

- [ ] **Step 5: Add loader unit test**

Append this test module to `src/openapi/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::contract::HttpMethod;

    use super::load_contract;

    #[test]
    fn loads_openapi_operations() {
        let contract = load_contract(Path::new("testdata/openapi/endpoint_removed_old.yaml"))
            .expect("fixture should parse");

        let key = contract
            .operations
            .keys()
            .find(|key| key.path == "/users" && key.method == HttpMethod::Get)
            .expect("GET /users should be normalized");

        let operation = contract.operations.get(key).expect("operation should exist");
        assert!(operation.responses.contains_key("200"));
    }
}
```

- [ ] **Step 6: Verify loader**

Run:

```bash
cargo test openapi::tests::loads_openapi_operations
```

Expected: PASS.

- [ ] **Step 7: Commit loader**

Run:

```bash
git add Cargo.toml src/main.rs src/openapi/mod.rs testdata/openapi/endpoint_removed_old.yaml testdata/openapi/endpoint_removed_new.yaml
git commit -m "feat: load OpenAPI contracts"
```

Expected: PASS.

---

### Task 6: Diff Engine And Human Output

**Files:**
- Create: `src/diff/mod.rs`
- Create: `src/output/mod.rs`
- Modify: `src/main.rs`
- Create: `tests/cli_diff.rs`
- Create: `testdata/openapi/no_breaking_old.yaml`
- Create: `testdata/openapi/no_breaking_new.yaml`

**Interfaces:**
- Consumes:
  - `openapi::load_contract(path: &Path) -> anyhow::Result<ApiContract>`
  - `contract::ApiContract`
  - `contract::OperationKey`
- Produces:
  - `diff::Change`
  - `diff::Severity`
  - `diff::diff_contracts(old: &ApiContract, new: &ApiContract) -> Vec<Change>`
  - `output::render_changes(changes: &[Change]) -> String`

- [ ] **Step 1: Create no-breaking fixtures**

Create `testdata/openapi/no_breaking_old.yaml` with:

```yaml
openapi: 3.0.3
info:
  title: Example
  version: 1.0.0
paths:
  /users:
    get:
      responses:
        "200":
          description: OK
```

Create `testdata/openapi/no_breaking_new.yaml` with:

```yaml
openapi: 3.0.3
info:
  title: Example
  version: 1.0.0
paths:
  /users:
    get:
      responses:
        "200":
          description: OK
  /teams:
    get:
      responses:
        "200":
          description: OK
```

- [ ] **Step 2: Create diff module**

Create `src/diff/mod.rs` with:

```rust
use crate::contract::{ApiContract, OperationKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Breaking,
    Warning,
    NonBreaking,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub severity: Severity,
    pub operation: OperationKey,
    pub message: String,
}

pub fn diff_contracts(old: &ApiContract, new: &ApiContract) -> Vec<Change> {
    let mut changes = Vec::new();

    for key in old.operations.keys() {
        if !new.operations.contains_key(key) {
            changes.push(Change {
                severity: Severity::Breaking,
                operation: key.clone(),
                message: "endpoint removed".to_string(),
            });
        }
    }

    for key in new.operations.keys() {
        if !old.operations.contains_key(key) {
            changes.push(Change {
                severity: Severity::NonBreaking,
                operation: key.clone(),
                message: "endpoint added".to_string(),
            });
        }
    }

    changes
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{diff_contracts, Severity};
    use crate::openapi::load_contract;

    #[test]
    fn detects_removed_endpoint_as_breaking() {
        let old = load_contract(Path::new("testdata/openapi/endpoint_removed_old.yaml"))
            .expect("old fixture should parse");
        let new = load_contract(Path::new("testdata/openapi/endpoint_removed_new.yaml"))
            .expect("new fixture should parse");

        let changes = diff_contracts(&old, &new);

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].severity, Severity::Breaking);
        assert_eq!(changes[0].operation.method.as_str(), "GET");
        assert_eq!(changes[0].operation.path, "/users");
        assert_eq!(changes[0].message, "endpoint removed");
    }

    #[test]
    fn detects_added_endpoint_as_non_breaking() {
        let old = load_contract(Path::new("testdata/openapi/no_breaking_old.yaml"))
            .expect("old fixture should parse");
        let new = load_contract(Path::new("testdata/openapi/no_breaking_new.yaml"))
            .expect("new fixture should parse");

        let changes = diff_contracts(&old, &new);

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].severity, Severity::NonBreaking);
        assert_eq!(changes[0].operation.method.as_str(), "GET");
        assert_eq!(changes[0].operation.path, "/teams");
        assert_eq!(changes[0].message, "endpoint added");
    }
}
```

- [ ] **Step 3: Create output renderer**

Create `src/output/mod.rs` with:

```rust
use crate::diff::{Change, Severity};

pub fn render_changes(changes: &[Change]) -> String {
    if changes.is_empty() {
        return "No changes detected.\n".to_string();
    }

    let mut rendered = String::new();
    render_group(&mut rendered, "Breaking changes", changes, Severity::Breaking);
    render_group(&mut rendered, "Warnings", changes, Severity::Warning);
    render_group(
        &mut rendered,
        "Non-breaking changes",
        changes,
        Severity::NonBreaking,
    );
    rendered
}

fn render_group(rendered: &mut String, title: &str, changes: &[Change], severity: Severity) {
    let group: Vec<_> = changes
        .iter()
        .filter(|change| change.severity == severity)
        .collect();

    if group.is_empty() {
        return;
    }

    if !rendered.is_empty() {
        rendered.push('\n');
    }

    rendered.push_str(title);
    rendered.push('\n');

    for change in group {
        rendered.push_str("- ");
        rendered.push_str(change.operation.method.as_str());
        rendered.push(' ');
        rendered.push_str(&change.operation.path);
        rendered.push_str(": ");
        rendered.push_str(&change.message);
        rendered.push('\n');
    }
}
```

- [ ] **Step 4: Wire diff command**

Modify `src/main.rs` to:

```rust
mod cli;
mod contract;
mod diff;
mod openapi;
mod output;

use anyhow::Result;
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
    }
}
```

- [ ] **Step 5: Add CLI integration tests**

Create `tests/cli_diff.rs` with:

```rust
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn diff_exits_one_for_breaking_change() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/endpoint_removed_old.yaml",
            "testdata/openapi/endpoint_removed_new.yaml",
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Breaking changes"))
        .stdout(predicate::str::contains("GET /users: endpoint removed"));
}

#[test]
fn diff_exits_zero_for_non_breaking_change() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/no_breaking_old.yaml",
            "testdata/openapi/no_breaking_new.yaml",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Non-breaking changes"))
        .stdout(predicate::str::contains("GET /teams: endpoint added"));
}
```

- [ ] **Step 6: Verify diff slice**

Run:

```bash
cargo fmt
cargo test
cargo run -- diff testdata/openapi/endpoint_removed_old.yaml testdata/openapi/endpoint_removed_new.yaml
```

Expected: tests pass, and the run command prints a breaking change for `GET /users`.

- [ ] **Step 7: Commit diff slice**

Run:

```bash
git add src/main.rs src/diff/mod.rs src/output/mod.rs tests/cli_diff.rs testdata/openapi/no_breaking_old.yaml testdata/openapi/no_breaking_new.yaml
git commit -m "feat: report endpoint-level OpenAPI diffs"
```

Expected: PASS.

---

### Task 7: Push Initial Public Setup

**Files:**
- Modify: remote branch `main`

**Interfaces:**
- Consumes: local commits from Tasks 2 through 6.
- Produces: public GitHub repository with initial docs, Rust CLI skeleton, and first diff behavior.

- [ ] **Step 1: Run final local checks**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
git status --short --branch
```

Expected: formatting passes, clippy passes, tests pass, and git status shows a clean working tree on `main`.

- [ ] **Step 2: Push commits**

Run:

```bash
git push
```

Expected: PASS.

- [ ] **Step 3: Verify GitHub repository**

Run:

```bash
gh repo view apiwatch --json name,visibility,description,url
```

Expected: JSON shows name `apiwatch`, visibility `PUBLIC`, and description `Lock, diff, and verify the APIs your code depends on.`

- [ ] **Step 4: Share the repository URL**

Report the URL printed by Step 3 to the project owner.
