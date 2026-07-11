# apiwatch Verify Local Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `apiwatch verify <OPENAPI> --name <NAME> --lock <PATH>` to compare one local OpenAPI contract against a named v1 `api.lock` entry.

**Architecture:** Extend `lockfile` to deserialize and validate v1 lockfiles, select one named OpenAPI target, and compare ordered operation sets against the existing normalized `ApiContract`. Add a focused output renderer and a thin CLI route that returns `0` for a match, `1` for deterministic drift, and `2` for input or validation failures.

**Tech Stack:** Rust 2021, `clap`, `anyhow`, `serde`, `serde_yaml`, `assert_cmd`, and `predicates`.

## Global Constraints

- CLI shape is exactly `apiwatch verify <OPENAPI> --name <NAME> --lock <PATH>`.
- `<OPENAPI>` and `--lock` are local files only; do not fetch remote contracts or read a config file.
- The trimmed API name must be non-empty and must select exactly one entry under `apis`.
- Only v1 lockfiles with `source: openapi` are accepted.
- Verification compares only normalized uppercase HTTP method and path pairs.
- A matching set prints `Verified <NAME>` and exits `0`.
- Drift exits `1`; all `REMOVED METHOD path` lines precede all `ADDED METHOD path` lines, and each group is ordered lexicographically by method and path.
- Input, parse, lockfile, and validation errors exit `2` through the existing top-level error handler.
- Do not mutate the lockfile, compare schemas or authentication, or verify multiple entries in one invocation.
- Update README and CHANGELOG when the command is implemented.

---

## File Structure

- `src/lockfile/mod.rs`: v1 YAML reading, target selection, and deterministic operation-set comparison.
- `src/output/mod.rs`: rendering for `VerifyChange` values.
- `src/cli.rs`: `Verify` Clap subcommand and arguments.
- `src/main.rs`: Verify command routing and exit-code selection.
- `tests/cli_verify.rs`: end-to-end match, drift, and input-error coverage.
- `testdata/openapi/verify_*.yaml`: small current-contract fixtures.
- `testdata/lock/verify_*.lock`: v1 lockfile and invalid-lock fixtures.
- `README.md` and `CHANGELOG.md`: implemented command documentation.
- `implementation-log/`: ignored high-level task notes; never stage these files.

---

### Task 1: Read and Compare Version 1 Lockfiles

**Files:**
- Modify: `src/lockfile/mod.rs`
- Create: `testdata/openapi/verify_current.yaml`
- Create: `testdata/lock/verify_users.lock`
- Create: `testdata/lock/verify_unsupported_version.lock`

**Interfaces:**
- Produces: `lockfile::load(path: &Path) -> anyhow::Result<ApiLock>`.
- Produces: `lockfile::select_verify_target(lock: &ApiLock, name: &str) -> anyhow::Result<VerifyTarget>`.
- Produces: `lockfile::compare_verify_target(target: &VerifyTarget, current: &ApiContract) -> Vec<VerifyChange>`.
- Produces: `VerifyTarget::name(&self) -> &str`.
- Produces: `VerifyChange { kind: VerifyChangeKind, method: String, path: String }`.

- [ ] **Step 1: Add the failing lockfile comparison tests and fixtures**

Append this test module to `src/lockfile/mod.rs` after the existing production code:

```rust
#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;

    use super::*;

    #[test]
    fn compare_verify_target_reports_removed_before_added_in_order() {
        let lock = ApiLock {
            version: 1,
            apis: BTreeMap::from([(
                "users".to_string(),
                LockedApi {
                    source: "openapi".to_string(),
                    operations: vec![
                        LockedOperation {
                            method: "GET".to_string(),
                            path: "/zeta".to_string(),
                        },
                        LockedOperation {
                            method: "GET".to_string(),
                            path: "/users".to_string(),
                        },
                    ],
                },
            )]),
        };
        let current = crate::openapi::load_contract(Path::new(
            "testdata/openapi/verify_current.yaml",
        ))
        .expect("fixture should load");

        let target = select_verify_target(&lock, "users").expect("target should select");

        assert_eq!(target.name(), "users");
        assert_eq!(
            compare_verify_target(&target, &current),
            vec![
                VerifyChange {
                    kind: VerifyChangeKind::Removed,
                    method: "GET".to_string(),
                    path: "/users".to_string(),
                },
                VerifyChange {
                    kind: VerifyChangeKind::Removed,
                    method: "GET".to_string(),
                    path: "/zeta".to_string(),
                },
                VerifyChange {
                    kind: VerifyChangeKind::Added,
                    method: "POST".to_string(),
                    path: "/users".to_string(),
                },
                VerifyChange {
                    kind: VerifyChangeKind::Added,
                    method: "POST".to_string(),
                    path: "/zeta".to_string(),
                },
            ]
        );
    }

    #[test]
    fn load_rejects_an_unsupported_lockfile_version() {
        let error = load(Path::new("testdata/lock/verify_unsupported_version.lock"))
            .expect_err("version 2 lockfile should be rejected");

        assert!(
            error
                .to_string()
                .contains("unsupported api.lock version 2")
        );
    }
}
```

Create `testdata/openapi/verify_current.yaml`:

```yaml
openapi: 3.0.3
info:
  title: Verify current
  version: 1.0.0
paths:
  /users:
    post:
      responses:
        "200":
          description: OK
  /zeta:
    post:
      responses:
        "200":
          description: OK
```

Create `testdata/lock/verify_users.lock`:

```yaml
version: 1
apis:
  users:
    source: openapi
    operations:
      - method: GET
        path: /users
      - method: GET
        path: /zeta
```

Create `testdata/lock/verify_unsupported_version.lock`:

```yaml
version: 2
apis:
  users:
    source: openapi
    operations: []
```

- [ ] **Step 2: Run the lockfile tests to verify RED**

Run:

```bash
cargo test lockfile::tests::compare_verify_target_reports_removed_before_added_in_order
```

Expected: compilation fails because `VerifyChange`, `VerifyChangeKind`, `select_verify_target`, and `compare_verify_target` do not exist.

- [ ] **Step 3: Implement v1 deserialization, target selection, and comparison**

Replace `src/lockfile/mod.rs` with:

```rust
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::contract::ApiContract;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiLock {
    version: u8,
    apis: BTreeMap<String, LockedApi>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct LockedApi {
    source: String,
    operations: Vec<LockedOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct LockedOperation {
    method: String,
    path: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct VerifyTarget {
    name: String,
    operations: BTreeSet<LockedOperation>,
}

impl VerifyTarget {
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyChangeKind {
    Removed,
    Added,
}

impl VerifyChangeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Removed => "REMOVED",
            Self::Added => "ADDED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyChange {
    pub kind: VerifyChangeKind,
    pub method: String,
    pub path: String,
}

pub fn from_contract(name: &str, contract: &ApiContract) -> Result<ApiLock> {
    let name = normalized_name(name)?;
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

pub fn load(path: &Path) -> Result<ApiLock> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read api.lock {}", path.display()))?;
    let lock: ApiLock =
        serde_yaml::from_str(&contents).context("failed to parse api.lock YAML")?;

    if lock.version != 1 {
        return Err(anyhow!("unsupported api.lock version {}", lock.version));
    }

    Ok(lock)
}

pub fn select_verify_target(lock: &ApiLock, name: &str) -> Result<VerifyTarget> {
    let name = normalized_name(name)?;
    let api = lock
        .apis
        .get(name)
        .ok_or_else(|| anyhow!("api {name} not found in lockfile"))?;

    if api.source != "openapi" {
        return Err(anyhow!("unsupported api.lock source {}", api.source));
    }

    Ok(VerifyTarget {
        name: name.to_string(),
        operations: api.operations.iter().cloned().collect(),
    })
}

pub fn compare_verify_target(target: &VerifyTarget, current: &ApiContract) -> Vec<VerifyChange> {
    let current_operations: BTreeSet<_> = current
        .operations
        .keys()
        .map(|key| LockedOperation {
            method: key.method.as_str().to_string(),
            path: key.path.clone(),
        })
        .collect();
    let mut changes = Vec::new();

    for operation in target.operations.difference(&current_operations) {
        changes.push(VerifyChange {
            kind: VerifyChangeKind::Removed,
            method: operation.method.clone(),
            path: operation.path.clone(),
        });
    }

    for operation in current_operations.difference(&target.operations) {
        changes.push(VerifyChange {
            kind: VerifyChangeKind::Added,
            method: operation.method.clone(),
            path: operation.path.clone(),
        });
    }

    changes
}

fn normalized_name(name: &str) -> Result<&str> {
    let name = name.trim();
    if name.is_empty() {
        return Err(anyhow!("api name cannot be empty"));
    }

    Ok(name)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;

    use super::*;

    #[test]
    fn compare_verify_target_reports_removed_before_added_in_order() {
        let lock = ApiLock {
            version: 1,
            apis: BTreeMap::from([(
                "users".to_string(),
                LockedApi {
                    source: "openapi".to_string(),
                    operations: vec![
                        LockedOperation {
                            method: "GET".to_string(),
                            path: "/zeta".to_string(),
                        },
                        LockedOperation {
                            method: "GET".to_string(),
                            path: "/users".to_string(),
                        },
                    ],
                },
            )]),
        };
        let current = crate::openapi::load_contract(Path::new(
            "testdata/openapi/verify_current.yaml",
        ))
        .expect("fixture should load");

        let target = select_verify_target(&lock, "users").expect("target should select");

        assert_eq!(target.name(), "users");
        assert_eq!(
            compare_verify_target(&target, &current),
            vec![
                VerifyChange {
                    kind: VerifyChangeKind::Removed,
                    method: "GET".to_string(),
                    path: "/users".to_string(),
                },
                VerifyChange {
                    kind: VerifyChangeKind::Removed,
                    method: "GET".to_string(),
                    path: "/zeta".to_string(),
                },
                VerifyChange {
                    kind: VerifyChangeKind::Added,
                    method: "POST".to_string(),
                    path: "/users".to_string(),
                },
                VerifyChange {
                    kind: VerifyChangeKind::Added,
                    method: "POST".to_string(),
                    path: "/zeta".to_string(),
                },
            ]
        );
    }

    #[test]
    fn load_rejects_an_unsupported_lockfile_version() {
        let error = load(Path::new("testdata/lock/verify_unsupported_version.lock"))
            .expect_err("version 2 lockfile should be rejected");

        assert!(
            error
                .to_string()
                .contains("unsupported api.lock version 2")
        );
    }
}
```

- [ ] **Step 4: Run the focused lockfile tests to verify GREEN**

Run:

```bash
cargo test lockfile::tests
```

Expected: both new lockfile tests pass.

- [ ] **Step 5: Run the full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: all commands exit `0`.

- [ ] **Step 6: Update the ignored implementation log, commit, and push Task 1**

Create `implementation-log/2026-07-11-apiwatch-verify-lockfile.md` with the task goal, design decisions, files touched, test results, and push status. Do not stage it.

Run:

```bash
git add src/lockfile/mod.rs testdata/openapi/verify_current.yaml testdata/lock/verify_users.lock testdata/lock/verify_unsupported_version.lock
git commit -m "Add lockfile verification support"
git push origin main
```

Expected: the tracked implementation and fixtures commit and push successfully.

---

### Task 2: Add the Verify CLI and End-to-End Coverage

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/main.rs`
- Modify: `src/output/mod.rs`
- Create: `tests/cli_verify.rs`
- Create: `testdata/openapi/verify_matching.yaml`
- Create: `testdata/openapi/verify_added.yaml`
- Create: `testdata/openapi/verify_removed.yaml`
- Create: `testdata/lock/verify_invalid_yaml.lock`
- Create: `testdata/lock/verify_unsupported_source.lock`

**Interfaces:**
- Consumes: `lockfile::load`, `lockfile::select_verify_target`, `lockfile::compare_verify_target`, and `VerifyChange` from Task 1.
- Produces CLI: `apiwatch verify <OPENAPI> --name <NAME> --lock <PATH>`.
- Produces: `output::render_verify_changes(changes: &[VerifyChange]) -> String`.

- [ ] **Step 1: Add the failing CLI integration tests and fixtures**

Create `testdata/openapi/verify_matching.yaml`:

```yaml
openapi: 3.0.3
info:
  title: Verify matching
  version: 1.0.0
paths:
  /users:
    get:
      responses:
        "200":
          description: OK
  /zeta:
    get:
      responses:
        "200":
          description: OK
```

Create `testdata/openapi/verify_added.yaml`:

```yaml
openapi: 3.0.3
info:
  title: Verify added
  version: 1.0.0
paths:
  /users:
    get:
      responses:
        "200":
          description: OK
    post:
      responses:
        "201":
          description: Created
  /zeta:
    get:
      responses:
        "200":
          description: OK
```

Create `testdata/openapi/verify_removed.yaml`:

```yaml
openapi: 3.0.3
info:
  title: Verify removed
  version: 1.0.0
paths:
  /zeta:
    get:
      responses:
        "200":
          description: OK
```

Create `testdata/lock/verify_invalid_yaml.lock`:

```yaml
version: [
```

Create `testdata/lock/verify_unsupported_source.lock`:

```yaml
version: 1
apis:
  users:
    source: remote
    operations: []
```

Create `tests/cli_verify.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;

fn verify_command(openapi: &str, name: &str, lock: &str) -> Command {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");
    command.args(["verify", openapi, "--name", name, "--lock", lock]);
    command
}

#[test]
fn verify_exits_zero_for_matching_locked_operations() {
    verify_command(
        "testdata/openapi/verify_matching.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .assert()
    .success()
    .stdout("Verified users\n");
}

#[test]
fn verify_exits_one_for_an_added_operation() {
    verify_command(
        "testdata/openapi/verify_added.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .assert()
    .code(1)
    .stdout("ADDED POST /users\n");
}

#[test]
fn verify_exits_one_for_a_removed_operation() {
    verify_command(
        "testdata/openapi/verify_removed.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .assert()
    .code(1)
    .stdout("REMOVED GET /users\n");
}

#[test]
fn verify_renders_removed_operations_before_added_operations() {
    verify_command(
        "testdata/openapi/verify_current.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .assert()
    .code(1)
    .stdout(
        "\
REMOVED GET /users
REMOVED GET /zeta
ADDED POST /users
ADDED POST /zeta
",
    );
}

#[test]
fn verify_exits_two_for_an_empty_api_name() {
    verify_command(
        "testdata/openapi/verify_matching.yaml",
        "",
        "testdata/lock/verify_users.lock",
    )
    .assert()
    .code(2)
    .stdout(predicate::str::is_empty())
    .stderr(predicate::str::contains("api name cannot be empty"));
}

#[test]
fn verify_exits_two_for_a_missing_api_name() {
    verify_command(
        "testdata/openapi/verify_matching.yaml",
        "payments",
        "testdata/lock/verify_users.lock",
    )
    .assert()
    .code(2)
    .stdout(predicate::str::is_empty())
    .stderr(predicate::str::contains("api payments not found in lockfile"));
}

#[test]
fn verify_exits_two_for_invalid_lockfile_yaml() {
    verify_command(
        "testdata/openapi/verify_matching.yaml",
        "users",
        "testdata/lock/verify_invalid_yaml.lock",
    )
    .assert()
    .code(2)
    .stdout(predicate::str::is_empty())
    .stderr(predicate::str::contains("failed to parse api.lock YAML"));
}

#[test]
fn verify_exits_two_for_an_unsupported_lockfile_version() {
    verify_command(
        "testdata/openapi/verify_matching.yaml",
        "users",
        "testdata/lock/verify_unsupported_version.lock",
    )
    .assert()
    .code(2)
    .stdout(predicate::str::is_empty())
    .stderr(predicate::str::contains("unsupported api.lock version 2"));
}

#[test]
fn verify_exits_two_for_an_unsupported_lockfile_source() {
    verify_command(
        "testdata/openapi/verify_matching.yaml",
        "users",
        "testdata/lock/verify_unsupported_source.lock",
    )
    .assert()
    .code(2)
    .stdout(predicate::str::is_empty())
    .stderr(predicate::str::contains("unsupported api.lock source remote"));
}
```

- [ ] **Step 2: Run the Verify CLI tests to verify RED**

Run:

```bash
cargo test --test cli_verify
```

Expected: every test fails because Clap does not yet recognize the `verify` subcommand.

- [ ] **Step 3: Add the CLI variant, renderer, and command route**

Add this `Verify` variant to `Command` in `src/cli.rs` after `Lock`:

```rust
    /// Verify one OpenAPI contract against a named api.lock entry.
    Verify {
        /// Current OpenAPI YAML or JSON file to verify.
        openapi: PathBuf,
        /// API name to verify from the lockfile.
        #[arg(long)]
        name: String,
        /// api.lock file to compare against.
        #[arg(long)]
        lock: PathBuf,
    },
```

Add this import beside the existing `Change` and `Severity` import in `src/output/mod.rs`:

```rust
use crate::lockfile::VerifyChange;
```

Append this renderer to `src/output/mod.rs`:

```rust

pub fn render_verify_changes(changes: &[VerifyChange]) -> String {
    let mut rendered = String::new();

    for change in changes {
        rendered.push_str(change.kind.as_str());
        rendered.push(' ');
        rendered.push_str(&change.method);
        rendered.push(' ');
        rendered.push_str(&change.path);
        rendered.push('\n');
    }

    rendered
}
```

Add this match arm to `run()` in `src/main.rs`:

```rust
        Command::Verify {
            openapi,
            name,
            lock,
        } => {
            let lock = lockfile::load(&lock)?;
            let target = lockfile::select_verify_target(&lock, &name)?;
            let contract = openapi::load_contract(&openapi)?;
            let changes = lockfile::compare_verify_target(&target, &contract);

            if changes.is_empty() {
                println!("Verified {}", target.name());
                Ok(0)
            } else {
                print!("{}", output::render_verify_changes(&changes));
                Ok(1)
            }
        }
```

- [ ] **Step 4: Run the Verify CLI tests to verify GREEN**

Run:

```bash
cargo test --test cli_verify
```

Expected: all nine Verify CLI tests pass.

- [ ] **Step 5: Run the full verification gate**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: all commands exit `0`.

- [ ] **Step 6: Update the ignored implementation log, commit, and push Task 2**

Create `implementation-log/2026-07-11-apiwatch-verify-cli.md` with the task goal, test coverage, verification results, and push status. Do not stage it.

Run:

```bash
git add src/cli.rs src/main.rs src/output/mod.rs tests/cli_verify.rs testdata/openapi/verify_matching.yaml testdata/openapi/verify_added.yaml testdata/openapi/verify_removed.yaml testdata/lock/verify_invalid_yaml.lock testdata/lock/verify_unsupported_source.lock
git commit -m "Add local verify command"
git push origin main
```

Expected: the Verify CLI, integration tests, and fixtures commit and push successfully.

---

### Task 3: Document the Verify Command

**Files:**
- Modify: `README.md`
- Modify: `CHANGELOG.md`

**Interfaces:**
- Documents: `apiwatch verify <OPENAPI> --name <NAME> --lock <PATH>`.
- Documents: exit `0` for a match and exit `1` for operation drift.

- [ ] **Step 1: Update the README CLI examples**

Replace the CLI section command block and remove the now-empty planned-command section so it is:

````markdown
## CLI

```bash
apiwatch diff old.openapi.yaml new.openapi.yaml
apiwatch lock openapi.yaml --name users --output api.lock
apiwatch verify openapi.yaml --name users --lock api.lock
```
````

Add this paragraph immediately after the block:

```markdown
`apiwatch verify` compares the normalized operation set in a local OpenAPI file with one named `api.lock` entry. It exits `0` when they match and `1` when operations have drifted.
```

- [ ] **Step 2: Add the Unreleased changelog entry**

Add this bullet under `## Unreleased` / `### Added` in `CHANGELOG.md`:

```markdown
- `apiwatch verify <OPENAPI> --name <NAME> --lock <PATH>` compares a local OpenAPI contract to one named v1 `api.lock` entry and exits `1` for deterministic operation drift.
```

- [ ] **Step 3: Run documentation and full verification checks**

Run:

```bash
git diff --check
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: all commands exit `0`.

- [ ] **Step 4: Update the ignored implementation log, commit, and push Task 3**

Create `implementation-log/2026-07-11-apiwatch-verify-docs.md` with the task goal, documentation scope, verification results, and push status. Do not stage it.

Run:

```bash
git add README.md CHANGELOG.md
git commit -m "Document local verify command"
git push origin main
```

Expected: the documentation commit and push succeed.

---

## Final Completion Check

- [ ] Run `git status --short --branch` and confirm `main...origin/main` with no tracked changes.
- [ ] Run `cargo fmt --all -- --check` and confirm exit `0`.
- [ ] Run `cargo clippy --all-targets --all-features -- -D warnings` and confirm exit `0`.
- [ ] Run `cargo test` and confirm all tests pass.
- [ ] Run:

```bash
cargo run -- verify testdata/openapi/verify_matching.yaml --name users --lock testdata/lock/verify_users.lock
```

Confirm exit `0` and exact output `Verified users`.

- [ ] Run:

```bash
cargo run -- verify testdata/openapi/verify_current.yaml --name users --lock testdata/lock/verify_users.lock
```

Confirm exit `1` and output:

```text
REMOVED GET /users
REMOVED GET /zeta
ADDED POST /users
ADDED POST /zeta
```

- [ ] Confirm every Verify commit has been pushed to `origin/main`.
