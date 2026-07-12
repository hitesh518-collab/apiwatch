# APIWatch JSON Output Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic, versioned JSON output to `apiwatch diff` and `apiwatch verify` without changing their default text output or exit codes.

**Architecture:** Add one Clap `OutputFormat` value enum and opt-in `--format text|json` fields on the Diff and Verify commands. Keep comparison and exit-code logic in `main.rs`; `output/mod.rs` adapts existing result values to private serializable JSON envelopes instead of adding broad serialization derives to domain types.

**Tech Stack:** Rust 2021, Clap derive/value enums, Serde, serde_json, assert_cmd, existing Rust GitHub Actions CI.

## Global Constraints

- Support only `--format text|json` on `diff` and `verify`; `text` is the default and `lock` remains text-only.
- `--format json` writes exactly one compact JSON object plus a trailing newline to stdout for successful results and drift results.
- Existing operational and validation errors remain human-readable stderr output with exit code `2`; emit no partial JSON error document.
- Preserve all current exit codes: Diff is `1` only with breaking changes; Verify is `1` for drift; both are `0` for successful non-drift/non-breaking results.
- Diff JSON fields are exactly `version`, `command`, `summary`, and `changes`; `summary` keys are `breaking`, `warning`, `non_breaking` in that order.
- Diff change fields are exactly `severity`, `method`, `path`, and `message`; severity values are `breaking`, `warning`, and `non_breaking`.
- Verify JSON fields are exactly `version`, `command`, `name`, `summary`, and `changes`; `summary` keys are `removed`, `added` in that order.
- Verify change fields are exactly `kind`, `method`, and `path`; kind values are `removed` and `added`.
- Preserve the existing deterministic Diff and Verify change ordering.
- Do not add SARIF, output files, action outputs, custom error JSON, or JSON support to `lock`.
- Keep agent records in ignored `implementation-log/` files.

---

### Task 1: Add Diff JSON Output

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/main.rs`
- Modify: `src/output/mod.rs`
- Modify: `tests/cli_diff.rs`

**Interfaces:**
- Produces `pub enum OutputFormat { Text, Json }` in `cli.rs`, derived with `ValueEnum`, `Clone`, `Copy`, `Debug`, `PartialEq`, and `Eq`.
- Produces `pub fn render_changes_json(changes: &[Change]) -> anyhow::Result<String>` in `output/mod.rs`.
- Adds `format: OutputFormat` to only the `Command::Diff` variant in this task.

- [ ] **Step 1: Write the failing Diff JSON integration tests**

  Add `use serde_json::{json, Value};` to `tests/cli_diff.rs`. Add these tests before changing production code:

  ```rust
  #[test]
  fn diff_json_reports_breaking_changes_and_exit_one() {
      let output = Command::cargo_bin("apiwatch")
          .expect("binary should build")
          .args([
              "diff",
              "testdata/openapi/endpoint_removed_old.yaml",
              "testdata/openapi/endpoint_removed_new.yaml",
              "--format",
              "json",
          ])
          .output()
          .expect("Diff command should run");

      assert_eq!(output.status.code(), Some(1));
      assert!(output.stderr.is_empty());
      assert!(output.stdout.ends_with(b"\n"));
      let rendered: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
      assert_eq!(
          rendered,
          json!({
              "version": 1,
              "command": "diff",
              "summary": { "breaking": 1, "warning": 0, "non_breaking": 0 },
              "changes": [{
                  "severity": "breaking",
                  "method": "GET",
                  "path": "/users",
                  "message": "endpoint removed"
              }]
          })
      );
  }
  ```

  Add a warning-only test using `status_error_added_old.yaml` and `status_error_added_new.yaml`; assert exit `0`, summary `{ "breaking": 0, "warning": 1, "non_breaking": 0 }`, and the one change `warning GET /users` with message `response status 429 added`.

  Add a no-change test that diffs `testdata/openapi/no_breaking_old.yaml` against itself and asserts exit `0`, a zero-valued Diff summary, and `changes: []`.

  Add a default-text compatibility test that runs the endpoint-removed fixture without `--format`, asserts exit `1`, and asserts stdout is exactly:

  ```text
  Breaking changes
  - GET /users: endpoint removed
  ```

  Add an invalid-format test that runs `diff testdata/openapi/no_breaking_old.yaml testdata/openapi/no_breaking_old.yaml --format yaml`, asserts exit `2`, empty stdout, and stderr containing `invalid value 'yaml' for '--format <FORMAT>'`.

- [ ] **Step 2: Run the new tests and verify they fail because the flag is absent**

  Run:

  ```powershell
  cargo test --test cli_diff diff_json_reports_breaking_changes_and_exit_one
  ```

  Expected: FAIL with Clap reporting that `--format` is an unexpected argument. The default-text test may pass before implementation; the new JSON and invalid-format behavior must not all pass at this point.

- [ ] **Step 3: Add the shared output-format enum and Diff command field**

  In `src/cli.rs`, import `ValueEnum`, define the enum above `Command`, and add the field to Diff:

  ```rust
  use clap::{Parser, Subcommand, ValueEnum};

  #[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
  pub enum OutputFormat {
      Text,
      Json,
  }

  // Inside Command::Diff after `new`:
  #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
  format: OutputFormat,
  ```

- [ ] **Step 4: Add the private serializable Diff envelope**

  In `src/output/mod.rs`, retain text renderers and add `anyhow::{Context, Result}` plus `serde::Serialize`. Define private structs in field order and the JSON renderer:

  ```rust
  #[derive(Serialize)]
  struct DiffJson<'a> {
      version: u8,
      command: &'static str,
      summary: DiffSummary,
      changes: Vec<DiffJsonChange<'a>>,
  }

  #[derive(Serialize)]
  struct DiffSummary {
      breaking: usize,
      warning: usize,
      non_breaking: usize,
  }

  #[derive(Serialize)]
  struct DiffJsonChange<'a> {
      severity: &'static str,
      method: &'static str,
      path: &'a str,
      message: &'a str,
  }
  ```

  Implement `render_changes_json` by mapping each `Change` without sorting or cloning its strings. Count each `Severity` once while mapping. Convert severities with an exhaustive helper returning `breaking`, `warning`, or `non_breaking`; use `change.operation.method.as_str()` for `method`. Serialize `DiffJson { version: 1, command: "diff", ... }` with `serde_json::to_string`, append `\n`, and apply `.context("failed to serialize Diff JSON output")` to the serializer result.

- [ ] **Step 5: Select Diff rendering in `main.rs` without changing exit logic**

  Import `OutputFormat` with `Cli` and `Command`, destructure `Command::Diff { old, new, format }`, and select the renderer after `diff_contracts`:

  ```rust
  let rendered = match format {
      OutputFormat::Text => output::render_changes(&changes),
      OutputFormat::Json => output::render_changes_json(&changes)?,
  };
  print!("{rendered}");
  ```

  Leave the following breaking-severity exit-code check unchanged.

- [ ] **Step 6: Run the focused Diff suite and inspect the new behavior**

  Run:

  ```powershell
  cargo test --test cli_diff
  cargo run -- diff testdata/openapi/endpoint_removed_old.yaml testdata/openapi/endpoint_removed_new.yaml --format json
  ```

  Expected: all Diff integration tests pass; the manual command writes the compact Diff JSON object and exits `1`.

- [ ] **Step 7: Commit the Diff JSON slice**

  ```powershell
  git add src/cli.rs src/main.rs src/output/mod.rs tests/cli_diff.rs
  git commit -m "Add JSON output for diff"
  ```

### Task 2: Add Verify JSON Output

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/main.rs`
- Modify: `src/output/mod.rs`
- Modify: `tests/cli_verify.rs`

**Interfaces:**
- Consumes `OutputFormat` and `render_changes_json` from Task 1.
- Produces `pub fn render_verify_changes_json(name: &str, changes: &[VerifyChange]) -> anyhow::Result<String>` in `output/mod.rs`.
- Adds `format: OutputFormat` to `Command::Verify`.

- [ ] **Step 1: Write the failing Verify JSON integration tests**

  Add `use serde_json::{json, Value};` to `tests/cli_verify.rs`. Add a drift test before production changes:

  ```rust
  #[test]
  fn verify_json_reports_drift_and_exit_one() {
      let output = verify_command(
          "testdata/openapi/verify_current.yaml",
          "users",
          "testdata/lock/verify_users.lock",
      )
      .args(["--format", "json"])
      .output()
      .expect("Verify command should run");

      assert_eq!(output.status.code(), Some(1));
      assert!(output.stderr.is_empty());
      assert!(output.stdout.ends_with(b"\n"));
      let rendered: Value = serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
      assert_eq!(
          rendered,
          json!({
              "version": 1,
              "command": "verify",
              "name": "users",
              "summary": { "removed": 2, "added": 2 },
              "changes": [
                  { "kind": "removed", "method": "GET", "path": "/users" },
                  { "kind": "removed", "method": "GET", "path": "/zeta" },
                  { "kind": "added", "method": "POST", "path": "/users" },
                  { "kind": "added", "method": "POST", "path": "/zeta" }
              ]
          })
      );
  }
  ```

  Add a matching-contract test using `verify_matching.yaml` and `verify_users.lock` with `--format json`; assert exit `0`, `name: "users"`, summary `{ "removed": 0, "added": 0 }`, and `changes: []`.

  Add a default-text compatibility test for `verify_current.yaml` that asserts exit `1` and the existing ordered lines. Add an invalid-format test that supplies `--format yaml`, asserts exit `2`, empty stdout, and stderr containing `invalid value 'yaml' for '--format <FORMAT>'`.

- [ ] **Step 2: Run the new Verify JSON test and verify it fails because Verify lacks the flag**

  Run:

  ```powershell
  cargo test --test cli_verify verify_json_reports_drift_and_exit_one
  ```

  Expected: FAIL with Clap reporting `--format` as an unexpected argument.

- [ ] **Step 3: Add the Verify command format field**

  In `src/cli.rs`, add the same field after `lock` in `Command::Verify`:

  ```rust
  #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
  format: OutputFormat,
  ```

- [ ] **Step 4: Add the private serializable Verify envelope**

  In `src/output/mod.rs`, define these private structs in the listed field order and implement `render_verify_changes_json`:

  ```rust
  #[derive(Serialize)]
  struct VerifyJson<'a> {
      version: u8,
      command: &'static str,
      name: &'a str,
      summary: VerifySummary,
      changes: Vec<VerifyJsonChange<'a>>,
  }

  #[derive(Serialize)]
  struct VerifySummary {
      removed: usize,
      added: usize,
  }

  #[derive(Serialize)]
  struct VerifyJsonChange<'a> {
      kind: &'static str,
      method: &'a str,
      path: &'a str,
  }
  ```

  Map `VerifyChangeKind::Removed` to `removed` and `VerifyChangeKind::Added` to `added` with an exhaustive helper. Count each kind while preserving input ordering. Serialize `VerifyJson { version: 1, command: "verify", name, ... }` with the same compact-string-plus-newline pattern and context message `failed to serialize Verify JSON output`.

- [ ] **Step 5: Select Verify rendering in `main.rs` without changing lookup or exit behavior**

  Destructure `format` in `Command::Verify`. After calculating `changes`, replace the separate text-print branches with a rendering selection that passes `target.name()` only to JSON:

  ```rust
  if changes.is_empty() {
      match format {
          OutputFormat::Text => println!("Verified {}", target.name()),
          OutputFormat::Json => print!("{}", output::render_verify_changes_json(target.name(), &changes)?),
      }
      Ok(0)
  } else {
      let rendered = match format {
          OutputFormat::Text => output::render_verify_changes(&changes),
          OutputFormat::Json => output::render_verify_changes_json(target.name(), &changes)?,
      };
      print!("{rendered}");
      Ok(1)
  }
  ```

  Keep lock loading, target selection, remote/local contract loading, and all error propagation unchanged.

- [ ] **Step 6: Run the focused Verify suite and inspect a matching JSON result**

  Run:

  ```powershell
  cargo test --test cli_verify
  cargo run -- verify testdata/openapi/verify_matching.yaml --name users --lock testdata/lock/verify_users.lock --format json
  ```

  Expected: all Verify integration tests pass; the manual command writes the compact zero-drift Verify JSON object and exits `0`.

- [ ] **Step 7: Commit the Verify JSON slice**

  ```powershell
  git add src/cli.rs src/main.rs src/output/mod.rs tests/cli_verify.rs
  git commit -m "Add JSON output for verify"
  ```

### Task 3: Document JSON Output and Run the Release Gate

**Files:**
- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Create (ignored): `implementation-log/2026-07-13-apiwatch-json-output.md`

**Interfaces:**
- Documents the completed `--format text|json` interfaces from Tasks 1 and 2 exactly.
- No code, CLI, workflow, or action metadata changes in this task.

- [ ] **Step 1: Add concise README usage and contract documentation**

  Add these examples immediately after the existing Diff and Verify CLI examples:

  ```bash
  apiwatch diff old.openapi.yaml new.openapi.yaml --format json
  apiwatch verify openapi.yaml --name users --lock api.lock --format json
  ```

  Add a short `## JSON Output` section before `## GitHub Action` stating:

  ```markdown
  `apiwatch diff` and `apiwatch verify` support `--format text|json`; text is the default. JSON output is a versioned, deterministic result document written to stdout. Diff reports `breaking`, `warning`, and `non_breaking` summary counts with operation messages; Verify reports the named lock entry and `removed`/`added` operation drift. Exit codes remain `0` for a clean result, `1` for detected breaking changes or Verify drift, and `2` for operational or validation errors.
  ```

  Do not document SARIF, JSON error documents, output files, or JSON support for `lock`.

- [ ] **Step 2: Add the Unreleased changelog entry**

  Add under `## Unreleased` / `### Added`:

  ```markdown
  - Deterministic, versioned JSON output for `apiwatch diff` and `apiwatch verify` via `--format json`.
  ```

- [ ] **Step 3: Write the ignored implementation log**

  Record the dual-command `--format` decision, version `1` envelopes, text compatibility, exit-code preservation, local quality gate, GitHub CI result, and deferred SARIF/error-document scope. Do not stage the log.

- [ ] **Step 4: Run the complete local quality gate**

  Run:

  ```powershell
  cargo fmt --all -- --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test
  git diff --check
  git status --short --branch
  ```

  Expected: format and Clippy exit `0`, all tests pass, no diff whitespace errors, and only README/CHANGELOG are tracked changes before committing.

- [ ] **Step 5: Commit and push the documentation**

  ```powershell
  git add README.md CHANGELOG.md
  git commit -m "Document JSON output"
  git push origin main
  ```

- [ ] **Step 6: Inspect CI after the final push**

  Confirm the `rust` and existing `action-smoke` jobs pass. The smoke action intentionally exercises Verify's unchanged default text format, demonstrating backward compatibility for the reusable action.

## Final Verification

- [ ] `apiwatch diff ... --format json` emits the exact v1 Diff schema for breaking, warning-only, and no-change fixtures.
- [ ] `apiwatch verify ... --format json` emits the exact v1 Verify schema for drift and matching fixtures.
- [ ] Omitting `--format` preserves representative existing text output byte-for-byte.
- [ ] Invalid format values exit `2` before normal command work and emit no stdout JSON.
- [ ] Existing operational errors retain stderr and exit `2` without a partial JSON document.
- [ ] The full Rust quality gate passes and final GitHub CI is green.
- [ ] Only intended tracked files are committed; implementation logs stay ignored.
