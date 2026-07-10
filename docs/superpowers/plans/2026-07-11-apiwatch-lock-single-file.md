# apiwatch Lock Single-File Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `apiwatch lock <OPENAPI> --name <NAME> --output <PATH>` so one local OpenAPI file can produce a deterministic `api.lock` YAML file.

**Architecture:** Reuse `openapi::load_contract` to normalize the input contract, then add a focused `lockfile` module that converts the normalized `ApiContract` into serializable lockfile structs. Route a new `Lock` CLI subcommand through `main`, write the rendered YAML to disk, and keep lockfile behavior independent from raw OpenAPI so future input sources can reuse it.

**Tech Stack:** Rust 2021, `clap` for CLI parsing, `anyhow` for error context, `serde` and `serde_yaml` for deterministic YAML rendering, `assert_cmd` and `predicates` for CLI integration tests.

## Global Constraints

- `<OPENAPI>` is a local OpenAPI 3.x YAML or JSON file.
- `--name <NAME>` is the key used under `apis` in the lockfile. Empty names are invalid.
- `--output <PATH>` is the lockfile path to write. The command overwrites an existing file, but the parent directory must already exist.
- Success exits with code `0` and prints `Wrote <PATH>`.
- Input, parse, normalization, validation, and write failures exit with code `2`, matching current CLI error behavior.
- The first lockfile version stores normalized operation metadata only.
- Operations are sorted by the existing normalized `OperationKey` ordering, so output is deterministic.
- The first slice intentionally does not store raw OpenAPI fragments, secrets, headers, examples, request or response schemas, or hashes.
- No `apiwatch lock --config apiwatch.yaml`.
- No remote URL fetching.
- No schema hashing.
- No multi-API merge behavior.
- No compatibility check between an existing lockfile and a new OpenAPI file.

---

## File Structure

- `Cargo.toml`: add direct `serde` dependency with derive support.
- `src/lockfile/mod.rs`: new lockfile conversion and YAML rendering module.
- `src/cli.rs`: add the `Lock` subcommand and arguments.
- `src/main.rs`: register the `lockfile` module and route the `Lock` subcommand.
- `tests/cli_lock.rs`: new integration tests for lock command success and error behavior.
- `testdata/openapi/lock_ordering.yaml`: fixture proving deterministic operation ordering.
- `README.md`: document the new command.
- `docs/lockfile-spec.md`: replace the draft shape with the implemented first lockfile version.
- `CHANGELOG.md`: add an Unreleased entry for the lock command.
- `implementation-log/YYYY-MM-DD-*.md`: update local ignored implementation logs for each work session.

---

### Task 1: Add the Working Single-File Lock Command

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/cli.rs`
- Modify: `src/main.rs`
- Create: `src/lockfile/mod.rs`
- Create: `tests/cli_lock.rs`
- Create: `testdata/openapi/lock_ordering.yaml`

**Interfaces:**
- Consumes: `openapi::load_contract(path: &Path) -> anyhow::Result<ApiContract>`
- Produces: `lockfile::from_contract(name: &str, contract: &ApiContract) -> anyhow::Result<ApiLock>`
- Produces: `lockfile::render(lock: &ApiLock) -> anyhow::Result<String>`
- Produces CLI: `apiwatch lock <OPENAPI> --name <NAME> --output <PATH>`

- [ ] **Step 1: Write the failing CLI test and ordering fixture**

Create `testdata/openapi/lock_ordering.yaml`:

```yaml
openapi: 3.0.3
info:
  title: Example
  version: 1.0.0
paths:
  /users:
    post:
      responses:
        "201":
          description: Created
    get:
      responses:
        "200":
          description: OK
```

Create `tests/cli_lock.rs`:

```rust
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use assert_cmd::Command;
use predicates::prelude::*;

fn temp_lock_path(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_nanos();
    let mut path = std::env::temp_dir();
    path.push(format!(
        "apiwatch-{name}-{}-{suffix}.lock",
        std::process::id()
    ));
    path
}

#[test]
fn lock_writes_deterministic_single_api_lockfile() {
    let output_path = temp_lock_path("single-api");
    let output_arg = output_path
        .to_str()
        .expect("temp path should be valid UTF-8");

    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");
    command
        .args([
            "lock",
            "testdata/openapi/lock_ordering.yaml",
            "--name",
            "users",
            "--output",
            output_arg,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "Wrote {}",
            output_path.display()
        )));

    let rendered = fs::read_to_string(&output_path).expect("lockfile should be written");
    fs::remove_file(&output_path).ok();

    assert_eq!(
        rendered,
        "\
version: 1
apis:
  users:
    source: openapi
    operations:
    - method: GET
      path: /users
    - method: POST
      path: /users
"
    );
}
```

- [ ] **Step 2: Run the focused test to verify it fails for the missing subcommand**

Run:

```bash
cargo test --test cli_lock lock_writes_deterministic_single_api_lockfile
```

Expected: FAIL because `apiwatch` does not yet have a `lock` subcommand. The assertion should show a non-zero exit with a Clap error mentioning the unrecognized subcommand `lock`.

- [ ] **Step 3: Add the serialization dependency**

Modify `Cargo.toml` dependencies to include direct serde derive support:

```toml
[dependencies]
anyhow = "1"
clap = { version = "4", features = ["derive"] }
openapiv3 = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
```

- [ ] **Step 4: Add the lockfile module**

Create `src/lockfile/mod.rs`:

```rust
use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::contract::ApiContract;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct ApiLock {
    version: u8,
    apis: BTreeMap<String, LockedApi>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct LockedApi {
    source: String,
    operations: Vec<LockedOperation>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct LockedOperation {
    method: String,
    path: String,
}

pub fn from_contract(name: &str, contract: &ApiContract) -> Result<ApiLock> {
    let operations = contract
        .operations
        .keys()
        .map(|key| LockedOperation {
            method: key.method.as_str().to_string(),
            path: key.path.clone(),
        })
        .collect();

    let mut apis = BTreeMap::new();
    apis.insert(
        name.to_string(),
        LockedApi {
            source: "openapi".to_string(),
            operations,
        },
    );

    Ok(ApiLock { version: 1, apis })
}

pub fn render(lock: &ApiLock) -> Result<String> {
    serde_yaml::to_string(lock).context("failed to serialize lockfile")
}
```

- [ ] **Step 5: Add the CLI parser variant**

Modify `src/cli.rs` so `Command` includes `Lock`:

```rust
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Compare two OpenAPI contracts.
    Diff {
        /// Old OpenAPI YAML or JSON file.
        old: PathBuf,
        /// New OpenAPI YAML or JSON file.
        new: PathBuf,
    },
    /// Create an api.lock file from one OpenAPI contract.
    Lock {
        /// OpenAPI YAML or JSON file to lock.
        openapi: PathBuf,
        /// API name to use as the lockfile key.
        #[arg(long)]
        name: String,
        /// Lockfile path to write.
        #[arg(long)]
        output: PathBuf,
    },
}
```

- [ ] **Step 6: Route the lock command**

Modify `src/main.rs`:

```rust
mod cli;
mod contract;
mod diff;
mod lockfile;
mod openapi;
mod output;

use std::fs;

use anyhow::{Context, Result};
use clap::Parser;

use crate::cli::{Cli, Command};
use crate::diff::Severity;
```

Add the new match arm:

```rust
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
```

- [ ] **Step 7: Run the focused test to verify it passes**

Run:

```bash
cargo test --test cli_lock lock_writes_deterministic_single_api_lockfile
```

Expected: PASS with 1 test passing in `tests/cli_lock.rs`.

- [ ] **Step 8: Run the full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: all commands exit `0`.

- [ ] **Step 9: Commit and push Task 1**

Run:

```bash
git add Cargo.toml Cargo.lock src/cli.rs src/main.rs src/lockfile/mod.rs tests/cli_lock.rs testdata/openapi/lock_ordering.yaml
git commit -m "Add single-file lock command"
git push origin main
```

Expected: commit and push succeed.

---

### Task 2: Cover Lock Command Error Paths

**Files:**
- Modify: `tests/cli_lock.rs`
- Optionally modify: `src/lockfile/mod.rs`
- Optionally modify: `src/main.rs`

**Interfaces:**
- Consumes: `apiwatch lock <OPENAPI> --name <NAME> --output <PATH>`
- Relies on: `lockfile::from_contract` returning `api name cannot be empty` for empty or whitespace-only names.
- Relies on: `main` returning exit code `2` for propagated errors.

- [ ] **Step 1: Add failing CLI tests for empty name and invalid OpenAPI input**

Append to `tests/cli_lock.rs`:

```rust
#[test]
fn lock_exits_two_for_empty_api_name() {
    let output_path = temp_lock_path("empty-name");
    let output_arg = output_path
        .to_str()
        .expect("temp path should be valid UTF-8");

    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");
    command
        .args([
            "lock",
            "testdata/openapi/lock_ordering.yaml",
            "--name",
            "",
            "--output",
            output_arg,
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("api name cannot be empty"));

    assert!(
        !output_path.exists(),
        "lockfile should not be written when the api name is invalid"
    );
}

#[test]
fn lock_exits_two_for_invalid_openapi_input() {
    let output_path = temp_lock_path("invalid-input");
    let output_arg = output_path
        .to_str()
        .expect("temp path should be valid UTF-8");

    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");
    command
        .args([
            "lock",
            "testdata/openapi/invalid_yaml.yaml",
            "--name",
            "users",
            "--output",
            output_arg,
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("failed to parse OpenAPI YAML"));

    assert!(
        !output_path.exists(),
        "lockfile should not be written when OpenAPI parsing fails"
    );
}
```

- [ ] **Step 2: Run the focused tests to verify any missing behavior fails**

Run:

```bash
cargo test --test cli_lock lock_exits_two
```

Expected: the invalid input case should pass because Task 1 routes parsing before writing. The empty name case should fail because Task 1 has not added API name validation yet.

- [ ] **Step 3: Fix empty-name validation if needed**

Update `src/lockfile/mod.rs` so the imports and `from_contract` trim and validate the name exactly as follows:

```rust
use anyhow::{anyhow, Context, Result};
```

```rust
pub fn from_contract(name: &str, contract: &ApiContract) -> Result<ApiLock> {
    let name = name.trim();
    if name.is_empty() {
        return Err(anyhow!("api name cannot be empty"));
    }

    let operations = contract
        .operations
        .keys()
        .map(|key| LockedOperation {
            method: key.method.as_str().to_string(),
            path: key.path.clone(),
        })
        .collect();

    let mut apis = BTreeMap::new();
    apis.insert(
        name.to_string(),
        LockedApi {
            source: "openapi".to_string(),
            operations,
        },
    );

    Ok(ApiLock { version: 1, apis })
}
```

- [ ] **Step 4: Run focused error tests again**

Run:

```bash
cargo test --test cli_lock lock_exits_two
```

Expected: PASS with 2 matching tests passing in `tests/cli_lock.rs`.

- [ ] **Step 5: Run the full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: all commands exit `0`.

- [ ] **Step 6: Commit and push Task 2**

Run:

```bash
git add tests/cli_lock.rs src/lockfile/mod.rs src/main.rs
git commit -m "Cover lock command error paths"
git push origin main
```

Expected: commit and push succeed. If only `tests/cli_lock.rs` changed, stage only that file.

---

### Task 3: Document the Lock Command and Lockfile Shape

**Files:**
- Modify: `README.md`
- Modify: `docs/lockfile-spec.md`
- Modify: `CHANGELOG.md`

**Interfaces:**
- Consumes: implemented CLI `apiwatch lock <OPENAPI> --name <NAME> --output <PATH>`
- Documents: lockfile version `1`, `apis.<name>.source`, and `apis.<name>.operations[]`

- [ ] **Step 1: Update README CLI examples**

Modify the `README.md` CLI section to show both implemented commands:

````markdown
## CLI

```bash
apiwatch diff old.openapi.yaml new.openapi.yaml
apiwatch lock openapi.yaml --name users --output api.lock
```

Planned future commands:

```bash
apiwatch verify
```
````

- [ ] **Step 2: Replace the lockfile draft with the implemented shape**

Replace `docs/lockfile-spec.md` with:

````markdown
# api.lock

`api.lock` is a repository-level lockfile for external API contracts.

The first lockfile version is intentionally small and stores normalized operation metadata from one or more APIs.

## Version 1

```yaml
version: 1
apis:
  users:
    source: openapi
    operations:
      - method: GET
        path: /users
      - method: POST
        path: /users
```

## Fields

- `version`: lockfile format version. The initial format uses `1`.
- `apis`: map of API names to locked API metadata.
- `apis.<name>.source`: source kind used to produce the lock. The initial command writes `openapi`.
- `apis.<name>.operations`: deterministic list of normalized operations.
- `method`: uppercase HTTP method.
- `path`: normalized OpenAPI path template.

## Privacy

The lockfile avoids secrets, sensitive raw payloads, examples, headers, and raw OpenAPI fragments. Future versions may add schema metadata or hashes while keeping sensitive input out of the file.
````

- [ ] **Step 3: Add an Unreleased changelog entry**

Modify the top of `CHANGELOG.md` so it begins:

```markdown
# Changelog

## Unreleased

### Added

- `apiwatch lock <OPENAPI> --name <NAME> --output <PATH>` writes a deterministic v1 `api.lock` file with normalized operation metadata.

## v0.1.0
```

- [ ] **Step 4: Run documentation diff checks**

Run:

```bash
git diff --check
```

Expected: command exits `0`.

- [ ] **Step 5: Run the full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: all commands exit `0`.

- [ ] **Step 6: Commit and push Task 3**

Run:

```bash
git add README.md docs/lockfile-spec.md CHANGELOG.md
git commit -m "Document single-file lock command"
git push origin main
```

Expected: commit and push succeed.

---

## Final Completion Check

- [ ] Run `git status --short --branch` and confirm `main...origin/main` with no tracked changes.
- [ ] Run `cargo fmt --all -- --check` and confirm exit `0`.
- [ ] Run `cargo clippy --all-targets --all-features -- -D warnings` and confirm exit `0`.
- [ ] Run `cargo test` and confirm exit `0`.
- [ ] Run `apiwatch lock testdata/openapi/lock_ordering.yaml --name users --output C:\tmp\apiwatch-final-check.lock` from the built binary or through `cargo run -- lock ...`, then inspect that the lockfile contains `version: 1`, `source: openapi`, `GET /users`, and `POST /users`.
- [ ] Remove `C:\tmp\apiwatch-final-check.lock` after inspection.
- [ ] Confirm all commits for the slice have been pushed to `origin/main`.
