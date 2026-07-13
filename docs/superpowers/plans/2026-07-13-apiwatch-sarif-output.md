# APIWatch SARIF Output Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic SARIF 2.1.0 output for Diff and Verify, plus opt-in GitHub Code Scanning upload from the reusable Verify action.

**Architecture:** Extend the shared output-format enum with `sarif`, then use private owned Serde structures in `output/mod.rs` to produce one SARIF document per command. The action’s optional `sarif-file` path selects a capture-upload-restore-exit flow; the existing text-mode action path remains unchanged when the input is empty.

**Tech Stack:** Rust 2021, Clap `ValueEnum`, Serde/serde_json, SARIF 2.1.0, GitHub composite actions, `github/codeql-action/upload-sarif@v4`, assert_cmd.

## Global Constraints

- Support only `--format text|json|sarif` on `diff` and `verify`; text remains default and `lock` stays text-only.
- SARIF stdout is one compact JSON document plus one trailing newline, with top-level `$schema` `https://json.schemastore.org/sarif-2.1.0.json`, `version` `2.1.0`, and exactly one run.
- Every SARIF document includes all five rules in this order: `apiwatch/diff-breaking`, `apiwatch/diff-warning`, `apiwatch/diff-non-breaking`, `apiwatch/verify-removed`, `apiwatch/verify-added`.
- Diff levels map Breaking/Warning/NonBreaking to `error`/`warning`/`note`; Verify Removed/Added map to `error`/`warning`.
- Diff results point to the new OpenAPI path. Verify results point to the lock path. Each result includes the exact `partialFingerprints.apiwatch/v1` value defined in the design.
- Preserve current deterministic Diff/Verify ordering, default text output, existing JSON output, and all `0`/`1`/`2` exits. Operational and validation errors remain stderr-only with no SARIF output.
- `sarif-file` is optional. An empty value preserves the current direct text Verify action. A nonempty value must be a relative path with no `..` segment inside `working-directory`.
- For valid SARIF Verify results, upload with `github/codeql-action/upload-sarif@v4` and category `apiwatch-${{ inputs.name }}`, then return the captured Verify exit code.
- Verify exit `2` stops before upload. Upload failures fail the action. The composite action has no new outputs.
- Repository CI keeps SARIF upload disabled to avoid fixture-driven code-scanning alerts.
- Keep agent records in ignored `implementation-log/` files.

---

### Task 1: Add SARIF Rendering to Diff and Verify

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/main.rs`
- Modify: `src/output/mod.rs`
- Modify: `tests/cli_diff.rs`
- Modify: `tests/cli_verify.rs`

**Interfaces:**
- Produces `OutputFormat::Sarif` alongside existing `Text` and `Json`.
- Produces `pub fn render_changes_sarif(artifact_path: &Path, changes: &[Change]) -> anyhow::Result<String>`.
- Produces `pub fn render_verify_changes_sarif(artifact_path: &Path, name: &str, changes: &[VerifyChange]) -> anyhow::Result<String>`.
- Both renderer functions return compact SARIF plus a trailing newline and use the existing `anyhow::Result<String>` error boundary.

- [ ] **Step 1: Write failing Diff and Verify SARIF integration tests**

  Add `Value` parsing helpers in `tests/cli_diff.rs` and `tests/cli_verify.rs`; `serde_json` is already a normal dependency and available to integration tests. Add the tests before production changes.

  In `tests/cli_diff.rs`, add a breaking-result test that runs:

  ```rust
  let output = Command::cargo_bin("apiwatch")
      .expect("binary should build")
      .args([
          "diff",
          "testdata/openapi/endpoint_removed_old.yaml",
          "testdata/openapi/endpoint_removed_new.yaml",
          "--format",
          "sarif",
      ])
      .output()
      .expect("Diff command should run");
  ```

  Assert exit `1`, empty stderr, trailing newline, `$schema`, `version: "2.1.0"`, one run, tool driver name `apiwatch`, semantic version equal to `env!("CARGO_PKG_VERSION")`, and the five fixed rule IDs in order. Assert the sole result equals these semantic fields:

  ```json
  {
    "ruleId": "apiwatch/diff-breaking",
    "level": "error",
    "message": { "text": "endpoint removed" },
    "locations": [{
      "physicalLocation": {
        "artifactLocation": { "uri": "testdata/openapi/endpoint_removed_new.yaml" }
      }
    }],
    "partialFingerprints": {
      "apiwatch/v1": "diff:apiwatch/diff-breaking:GET:/users:endpoint removed"
    }
  }
  ```

  Add a warning-only SARIF test using `status_error_added_old.yaml` and `status_error_added_new.yaml`; assert exit `0`, `ruleId` `apiwatch/diff-warning`, level `warning`, artifact URI `testdata/openapi/status_error_added_new.yaml`, and fingerprint `diff:apiwatch/diff-warning:GET:/users:response status 429 added`.

  Add a no-change SARIF test that compares `no_breaking_old.yaml` to itself and asserts exit `0`, all five rules present, and `results: []`.

  In `tests/cli_verify.rs`, add a drift test for `verify_current.yaml` with `--format sarif`; assert exit `1`, all five rules in fixed order, and these ordered result rule IDs:

  ```text
  apiwatch/verify-removed
  apiwatch/verify-removed
  apiwatch/verify-added
  apiwatch/verify-added
  ```

  Assert the first result has level `error`, message `locked operation removed: GET /users`, artifact URI `testdata/lock/verify_users.lock`, and fingerprint `verify:users:apiwatch/verify-removed:GET:/users`. Assert the first added result has level `warning`, message `unlocked operation added: POST /users`, and fingerprint `verify:users:apiwatch/verify-added:POST:/users`.

  Add a matching Verify SARIF test using `verify_matching.yaml`; assert exit `0`, all five rules, and `results: []`.

  Add invalid-format tests for both commands with `--format yaml`; assert exit `2`, empty stdout, and existing Clap invalid-value stderr text. Existing text and JSON regression tests must remain untouched.

- [ ] **Step 2: Run one new test per command and verify the expected red state**

  Run:

  ```powershell
  cargo test --test cli_diff diff_sarif_reports_breaking_change_and_exit_one
  cargo test --test cli_verify verify_sarif_reports_drift_and_exit_one
  ```

  Expected: both fail because Clap rejects `sarif` as an invalid value for `--format <FORMAT>`. Do not implement until this failure is observed.

- [ ] **Step 3: Extend the shared CLI format enum and command dispatch**

  In `src/cli.rs`, add the enum variant:

  ```rust
  pub enum OutputFormat {
      Text,
      Json,
      Sarif,
  }
  ```

  In `src/main.rs`, add the SARIF branches without moving loading, comparison, or exit-code decisions. Diff uses `&new` as its artifact path:

  ```rust
  OutputFormat::Sarif => output::render_changes_sarif(&new, &changes)?,
  ```

  Preserve the Verify artifact path before loading the lockfile by destructuring `lock: lock_path`, then load with `lockfile::load(&lock_path)?`. In both Verify branches, add:

  ```rust
  OutputFormat::Sarif => {
      output::render_verify_changes_sarif(&lock_path, target.name(), &changes)?
  }
  ```

  Do not change the existing Text and Json renderer calls or their output.

- [ ] **Step 4: Add private SARIF serializer types and rule helpers**

  In `src/output/mod.rs`, retain all JSON/text types and add private owned serializer types. Use `#[serde(rename = "$schema")]` for the top-level schema field and `#[serde(rename = "ruleId")]`, `#[serde(rename = "partialFingerprints")]`, `#[serde(rename = "physicalLocation")]`, `#[serde(rename = "artifactLocation")]`, `#[serde(rename = "semanticVersion")]`, `#[serde(rename = "shortDescription")]`, `#[serde(rename = "defaultConfiguration")]`, and `#[serde(rename = "problem.severity")]` where required.

  Define the renderer boundary exactly as:

  ```rust
  pub fn render_changes_sarif(artifact_path: &Path, changes: &[Change]) -> Result<String>;
  pub fn render_verify_changes_sarif(
      artifact_path: &Path,
      name: &str,
      changes: &[VerifyChange],
  ) -> Result<String>;
  ```

  Have both functions build `SarifLog` with `schema` `https://json.schemastore.org/sarif-2.1.0.json`, version `2.1.0`, one run, `tool.driver.name` `apiwatch`, `semanticVersion: env!("CARGO_PKG_VERSION").to_string()`, and `sarif_rules()`.

  `sarif_rules()` returns descriptors in the exact global-constraint order. Each descriptor contains the rule ID, a short name, a short description, help text, `defaultConfiguration.level`, `properties.precision: "high"`, and problem severity `error`, `warning`, or `recommendation`.

  Map Diff values exhaustively:

  ```rust
  Severity::Breaking => ("apiwatch/diff-breaking", "error"),
  Severity::Warning => ("apiwatch/diff-warning", "warning"),
  Severity::NonBreaking => ("apiwatch/diff-non-breaking", "note"),
  ```

  Use `change.message` as the result message and construct fingerprint `diff:{rule_id}:{method}:{path}:{message}`. Map Verify values exhaustively:

  ```rust
  VerifyChangeKind::Removed => ("apiwatch/verify-removed", "error", "locked operation removed"),
  VerifyChangeKind::Added => ("apiwatch/verify-added", "warning", "unlocked operation added"),
  ```

  Construct Verify message `{prefix}: {method} {path}` and fingerprint `verify:{name}:{rule_id}:{method}:{path}`. Build a single location with the artifact URI from `artifact_path.to_string_lossy().into_owned()`. Preserve source-slice iteration order. Serialize through a shared `render_sarif(results)` helper with `serde_json::to_string`, append `\n`, and contextualize failures as `failed to serialize SARIF output`.

- [ ] **Step 5: Run focused SARIF tests and format/lint checks**

  Run:

  ```powershell
  cargo fmt --all -- --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test --test cli_diff
  cargo test --test cli_verify
  ```

  Expected: all new SARIF tests and all existing Diff/Verify text/JSON tests pass with no Clippy warnings.

- [ ] **Step 6: Inspect both commands manually**

  Run:

  ```powershell
  cargo run -- diff testdata/openapi/endpoint_removed_old.yaml testdata/openapi/endpoint_removed_new.yaml --format sarif
  cargo run -- verify testdata/openapi/verify_current.yaml --name users --lock testdata/lock/verify_users.lock --format sarif
  ```

  Expected: each prints one compact SARIF document; Diff exits `1` and Verify exits `1` after rendering.

- [ ] **Step 7: Commit the complete CLI SARIF slice**

  ```powershell
  git add src/cli.rs src/main.rs src/output/mod.rs tests/cli_diff.rs tests/cli_verify.rs
  git commit -m "Add SARIF output"
  ```

### Task 2: Add Opt-In SARIF Upload to the GitHub Action and Document It

**Files:**
- Modify: `action.yml`
- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Create (ignored): `implementation-log/2026-07-13-apiwatch-sarif-output.md`

**Interfaces:**
- Consumes `apiwatch verify ... --format sarif` from Task 1.
- Produces optional action input `sarif-file`, empty by default.
- No GitHub Action outputs are added.

- [ ] **Step 1: Add the action input and preserve the existing default Verify step**

  In root `action.yml`, add after `working-directory`:

  ```yaml
  sarif-file:
    description: Relative SARIF output path within working-directory; enables Code Scanning upload when set.
    required: false
    default: ""
  ```

  Add `if: ${{ inputs.sarif-file == '' }}` to the existing `Verify API contract` step. Keep its command and environment otherwise unchanged, so the current `action-smoke` path remains exact.

- [ ] **Step 2: Add the SARIF capture step with safe path validation**

  Add a `Generate SARIF` composite step after the direct Verify step:

  ```yaml
  - name: Generate SARIF
    if: ${{ inputs.sarif-file != '' }}
    shell: bash
    working-directory: ${{ inputs.working-directory }}
    env:
      ACTION_PATH: ${{ github.action_path }}
      OPENAPI: ${{ inputs.openapi }}
      API_NAME: ${{ inputs.name }}
      LOCK: ${{ inputs.lock }}
      SARIF_FILE: ${{ inputs.sarif-file }}
    run: |
      case "$SARIF_FILE" in
        /*|..|../*|*/..|*/../*)
          echo "error: sarif-file must be a relative path within working-directory" >&2
          exit 2
          ;;
      esac
      mkdir -p -- "$(dirname "$SARIF_FILE")"
      set +e
      "$ACTION_PATH/target/release/apiwatch" verify "$OPENAPI" --name "$API_NAME" --lock "$LOCK" --format sarif > "$SARIF_FILE"
      status=$?
      set -e
      if [ "$status" -eq 2 ]; then
        exit 2
      fi
      echo "APIWATCH_SARIF_EXIT_CODE=$status" >> "$GITHUB_ENV"
  ```

  The capture step must exit `0` for a match or drift to let upload run. It must exit `2` directly for an operational/validation error, so no empty or partial SARIF is uploaded.

- [ ] **Step 3: Upload SARIF, then restore Verify's captured exit code**

  Add these steps after `Generate SARIF`:

  ```yaml
  - name: Upload SARIF
    if: ${{ inputs.sarif-file != '' && env.APIWATCH_SARIF_EXIT_CODE != '' }}
    uses: github/codeql-action/upload-sarif@v4
    with:
      sarif_file: ${{ inputs.working-directory }}/${{ inputs.sarif-file }}
      category: apiwatch-${{ inputs.name }}
  - name: Report Verify result
    if: ${{ inputs.sarif-file != '' && env.APIWATCH_SARIF_EXIT_CODE != '' }}
    shell: bash
    env:
      APIWATCH_SARIF_EXIT_CODE: ${{ env.APIWATCH_SARIF_EXIT_CODE }}
    run: exit "$APIWATCH_SARIF_EXIT_CODE"
  ```

  Do not add action outputs, `continue-on-error`, or a SARIF upload to repository CI. An upload failure must naturally fail the composite action before `Report Verify result` runs.

- [ ] **Step 4: Document CLI SARIF and action permissions**

  In `README.md`, add `--format sarif` examples for Diff and Verify in the existing JSON Output area. Explain that SARIF 2.1.0 is intended for Code Scanning and preserves the same exit codes.

  Extend the GitHub Action section with this consumer workflow example:

  ```yaml
  permissions:
    contents: read
    security-events: write

  steps:
    - uses: actions/checkout@v4
    - uses: hitesh518-collab/apiwatch@<commit-sha>
      with:
        openapi: https://api.example.com/openapi.yaml
        name: users
        lock: api.lock
        sarif-file: apiwatch.sarif
  ```

  State that `sarif-file` is relative to `working-directory`, enables upload, and needs `security-events: write`. State that an action drift report uploads before the action returns exit `1`.

  In `CHANGELOG.md` under Unreleased Added, add:

  ```markdown
  - SARIF 2.1.0 output for `apiwatch diff` and `apiwatch verify`, plus opt-in GitHub Code Scanning upload from the reusable action.
  ```

- [ ] **Step 5: Write the ignored implementation log and run the full local release gate**

  Record the SARIF v2.1.0/rule/fingerprint decisions, output locations, action opt-in behavior, validation, deferred scope, GitHub run, and any upload limitation. Do not stage the log.

  Run:

  ```powershell
  cargo fmt --all -- --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test
  git diff --check
  git status --short --branch
  ```

  Expected: all local gates pass, with only `action.yml`, `README.md`, and `CHANGELOG.md` tracked before committing. The existing text-mode `action-smoke` remains the regression check for the default action path.

- [ ] **Step 6: Commit, push, and inspect CI**

  ```powershell
  git add action.yml README.md CHANGELOG.md
  git commit -m "Add SARIF GitHub Action support"
  git push origin main
  ```

  Confirm the pushed CI run has green `rust` and `action-smoke` jobs. Do not add a SARIF-enabled smoke invocation because it would create fixture-derived Code Scanning alerts in this repository.

## Final Verification

- [ ] Both commands accept `--format sarif`, retain their existing text/JSON modes, and reject invalid formats with exit `2`.
- [ ] SARIF documents conform to the v2.1.0 contract, contain one run, fixed rule order, correct levels/messages/locations/fingerprints, and preserve result order.
- [ ] No SARIF document is emitted for operational or validation errors.
- [ ] Empty SARIF results exit `0`; breaking Diff and any Verify drift render SARIF then exit `1`.
- [ ] Empty `sarif-file` preserves the current action behavior. A valid nonempty path captures, uploads, and restores the Verify exit code; invalid paths and Verify exit `2` never upload.
- [ ] README documents consumer permissions and action behavior; the changelog is updated; logs remain ignored.
- [ ] Full local release gate and final GitHub `rust` plus text-mode `action-smoke` are green.
