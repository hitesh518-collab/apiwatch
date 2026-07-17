# APIWatch Observed Contracts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add local JSON shape recording, monotonic merging, and provenance-aware observed-contract verification while preserving every existing declared OpenAPI lock and Verify behavior.

**Architecture:** Add an OpenAPI-independent `observed` module that reduces JSON values to value-free shape trees, merges those trees monotonically, and compares a current tree directionally against a locked tree. Refactor the lockfile into an internal version-aware model that reads v1 declared files and v2 declared/observed files; the CLI selects parsing and output from a selected entry's provenance.

**Tech Stack:** Rust 2021, `clap`, `serde`, `serde_yaml`, `serde_json`, `anyhow`, existing CLI integration tests, and YAML/JSON fixtures.

## Global Constraints

- Do not add dependencies; the required JSON, YAML, CLI, and error crates already exist in `Cargo.toml`.
- Continue reading version-1 `source: openapi` locks and preserve all declared Verify text, JSON, SARIF, remote-input, and `0`/`1`/`2` behavior byte-for-byte.
- Write version 2 only when `record` creates or updates an observed entry; v2 entries use explicit `provenance: declared|observed`.
- Observed locks and diagnostics may contain field names, JSON paths, and shape names only. They must never contain input scalar values, examples, headers, tokens, credentials, or entire bodies.
- `record --merge` widens only an existing observed entry. Without `--merge`, an existing requested name is an error. A declared entry can never be replaced or merged as observed.
- In this slice, observed inputs are local JSON files only. Defer map annotations, coverage, HAR and live recording, enum inference, runtime monitoring, and source discovery.
- Keep a concise ignored record in `implementation-log/2026-07-17-apiwatch-observed-contracts.md`; do not stage it.
- Run `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, and `git diff --check` before handoff.

## File Structure

- Create `src/observed/mod.rs`: value-free JSON shape inference, merging, directional compatibility, JSON loading, and observed change types.
- Modify `src/lockfile/mod.rs`: version-aware lockfile parsing/rendering, v1-to-v2 conversion, observed-entry storage, record updates, and declared/observed Verify targets.
- Modify `src/cli.rs` and `src/main.rs`: add `Record`; route Verify by selected target provenance.
- Modify `src/output/mod.rs`: observed Verify text, v2 JSON, and SARIF serializers without changing declared serializers.
- Create `testdata/observed/*.json`: stable JSON samples for recording, merging, compatible verification, drift, empty arrays, and secret-retention assertions.
- Create `tests/cli_record.rs` and extend `tests/cli_verify.rs`: public CLI and output contracts.
- Modify `README.md`, `docs/lockfile-spec.md`, `CHANGELOG.md`, and `action.yml`: document the v2 format, commands, safety boundary, and provenance-neutral Verify input.

---

### Task 1: Build the Value-Free Observed Shape Engine

**Files:**
- Create: `src/observed/mod.rs`
- Modify: `src/main.rs` (add `mod observed;` only)

**Interfaces:**
- Consumes: `serde_json::Value` and local JSON files.
- Produces: `Shape`, `ObservedProperty`, `ObservedChange`, `ObservedChangeKind`, `load_shape`, `infer`, `merge`, and `compare` for the lockfile, CLI, and output layers.

- [ ] **Step 1: Write the failing shape and privacy unit tests in `src/observed/mod.rs`**

  Add a test module that exercises the public engine through these cases:

  ```rust
  #[test]
  fn merge_marks_late_fields_optional_and_sorts_a_scalar_union() {
      let mut shape = infer(&json!({"live_price": 12, "holdings": []}));
      merge(&mut shape, &infer(&json!({
          "live_price": null,
          "holdings": [{"ticker": "APW"}],
          "error": "temporary"
      })));

      assert!(compare(&shape, &infer(&json!({
          "live_price": 3,
          "holdings": [{"ticker": "DIFFERENT"}]
      }))).is_empty());
      assert!(compare(&shape, &infer(&json!({"holdings": []})))
          .iter()
          .any(|change| change.path == "$.live_price"));
  }

  #[test]
  fn inferred_shapes_never_serialize_source_values() {
      let shape = infer(&json!({"token": "super-secret-token", "amount": 42}));
      let rendered = serde_yaml::to_string(&shape).expect("shape should serialize");

      assert!(!rendered.contains("super-secret-token"));
      assert!(!rendered.contains("42"));
      assert!(rendered.contains("token"));
      assert!(rendered.contains("string"));
  }
  ```

  Add focused tests for an empty array accepting a populated array, required
  field removal, a string replacing a locked number, and deterministic union
  ordering (`null` before `number`).

- [ ] **Step 2: Run the new unit tests and confirm the module does not yet compile**

  Run: `cargo test observed::tests -- --nocapture`

  Expected: compilation fails because `mod observed`, `Shape`, `infer`,
  `merge`, and `compare` do not exist yet.

- [ ] **Step 3: Implement the shape model and local JSON loader**

  Create the following serializable public model, using `BTreeMap` for all
  object-property order and `Vec` only after union variants have been sorted:

  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
  #[serde(tag = "kind", rename_all = "snake_case")]
  pub enum Shape {
      Null,
      Boolean,
      Number,
      String,
      Object {
          observations: u64,
          properties: BTreeMap<String, ObservedProperty>,
      },
      Array { items: Box<Shape> },
      Union { variants: Vec<Shape> },
      Unknown,
  }

  #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
  pub struct ObservedProperty {
      pub observations: u64,
      pub shape: Box<Shape>,
  }

  #[derive(Debug, Clone, PartialEq, Eq)]
  pub enum ObservedChangeKind {
      MissingRequiredField,
      IncompatibleShape,
  }

  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct ObservedChange {
      pub kind: ObservedChangeKind,
      pub path: String,
      pub expected: Option<String>,
      pub actual: Option<String>,
  }
  ```

  Implement `pub fn load_shape(path: &Path) -> Result<Shape>` with
  `fs::read_to_string` and `serde_json::from_str`; wrap only a path in the
  contextual error (`failed to parse observed JSON <path>`), never the body.
  Implement `infer(&Value)` so arrays fold all elements into one item shape,
  using `Unknown` for zero elements; objects set `observations: 1` and each
  property count to `1`; and numeric JSON values become `Number`.

- [ ] **Step 4: Implement monotonic merge and directional comparison**

  Add these functions and helpers:

  ```rust
  pub fn merge(existing: &mut Shape, incoming: &Shape);
  pub fn compare(expected: &Shape, actual: &Shape) -> Vec<ObservedChange>;
  pub fn shape_name(shape: &Shape) -> String;
  ```

  `merge` increments object observation counts, keeps every existing and new
  property, merges same-kind recursive nodes, replaces an existing `Unknown`
  with a concrete incoming shape, and otherwise canonicalizes distinct shapes
  into `Union { variants }`. `compare` begins at `$`; reports a missing
  property only when its `observations` equals its parent object count; accepts
  extra properties and a concrete actual value for `Unknown`; recursively
  checks all actual array items; and accepts an actual shape if any expected
  union branch accepts it. Sort changes by JSON path, then kind.

- [ ] **Step 5: Run the focused tests and the full current suite**

  Run: `cargo test observed::tests -- --nocapture`

  Expected: all new shape tests pass.

  Run: `cargo test`

  Expected: the existing 111 tests and all new observed-module tests pass.

- [ ] **Step 6: Commit the engine in isolation**

  ```powershell
  git add src/observed/mod.rs src/main.rs
  git commit -m "Add observed JSON shape engine"
  ```

  Expected: the commit contains only the new module and module declaration.

---

### Task 2: Add Version-2 Lockfile Entries and Provenance Targets

**Files:**
- Modify: `src/lockfile/mod.rs`
- Modify: `src/observed/mod.rs` (only visibility adjustments required by the lockfile)

**Interfaces:**
- Consumes: `ApiContract`, `Shape`, and existing v1 YAML fixtures.
- Produces: version-aware `ApiLock`, `VerifyTarget::{Declared, Observed}`,
  `load_or_create_for_record`, `record_observed`, and deterministic `render`.

- [ ] **Step 1: Write failing v1/v2 lockfile tests**

  In `src/lockfile/mod.rs` tests, add a v1 fixture load assertion followed by
  an observed insertion assertion:

  ```rust
  #[test]
  fn recording_into_v1_preserves_declared_operations_and_renders_v2() {
      let mut lock = load(Path::new("testdata/lock/verify_users.lock"))
          .expect("v1 lock should load");
      let shape = crate::observed::infer(&serde_json::json!({"id": 1}));

      record_observed(&mut lock, "portfolio", shape, false)
          .expect("new observed entry should be recorded");
      let rendered = render(&lock).expect("v2 lock should render");

      assert!(rendered.starts_with("version: 2\n"));
      assert!(rendered.contains("provenance: declared"));
      assert!(rendered.contains("provenance: observed"));
      assert!(rendered.contains("path: /users"));
      assert!(!rendered.contains(" 1\n"));
  }
  ```

  Add tests that `record_observed(..., false)` rejects an existing name,
  `record_observed(..., true)` rejects a declared entry, a v2 observed target
  is selected as `VerifyTarget::Observed`, and the existing v1 declared target
  remains `VerifyTarget::Declared` with its current operation ordering.

- [ ] **Step 2: Run the new lockfile tests and confirm they fail**

  Run: `cargo test lockfile::tests -- --nocapture`

  Expected: compilation fails because `record_observed` and the provenance
  target variants do not exist.

- [ ] **Step 3: Refactor the internal lockfile representation**

  Replace the single v1-only serialized shape with an internal model that
  retains its source format and a tagged entry enum:

  ```rust
  enum LockVersion { V1, V2 }

  enum LockedApi {
      Declared { source: String, operations: Vec<LockedOperation> },
      Observed { shape: Shape },
  }

  pub enum VerifyTarget {
      Declared(DeclaredVerifyTarget),
      Observed(ObservedVerifyTarget),
  }

  pub struct ObservedVerifyTarget {
      name: String,
      shape: Shape,
  }
  ```

  Keep `from_contract` and its caller writing a v1 declared lock. Parse the
  first YAML document into a small `version` header, deserialize it into
  version-specific raw structs, then convert it to the internal model. Reject
  all versions except 1 and 2. Serialize v1 only for an all-declared V1 model;
  serialize every V2 entry with `provenance`, retaining declared `source` and
  `operations` or observed `shape` as appropriate.

- [ ] **Step 4: Add recording and target-selection functions**

  Implement these exact public functions:

  ```rust
  pub fn load_or_create_for_record(path: &Path) -> Result<ApiLock>;
  pub fn record_observed(
      lock: &mut ApiLock,
      name: &str,
      incoming: Shape,
      merge_existing: bool,
  ) -> Result<()>;
  pub fn select_verify_target(lock: &ApiLock, name: &str) -> Result<VerifyTarget>;
  ```

  `load_or_create_for_record` returns an empty V2 lock only when the path does
  not exist; it propagates every other file error. `record_observed` validates
  the normalized name, preserves unrelated entries, upgrades a V1 model to
  V2 before adding an observed entry, rejects an occupied name without merge,
  rejects declared entries with merge, and calls `observed::merge` only for an
  existing observed entry. `select_verify_target` retains the existing source,
  control-character, method, and path validation for declared entries.

- [ ] **Step 5: Run compatibility tests**

  Run: `cargo test lockfile::tests -- --nocapture`

  Expected: new v1/v2 tests and all existing lockfile tests pass.

  Run: `cargo test --test cli_lock --test cli_verify`

  Expected: existing public `lock` and declared `verify` behavior remains
  unchanged.

- [ ] **Step 6: Commit lockfile compatibility work**

  ```powershell
  git add src/lockfile/mod.rs src/observed/mod.rs
  git commit -m "Support observed api lock entries"
  ```

  Expected: the commit contains the version-aware lockfile model and its unit
  tests, with no CLI behavior added yet.

---

### Task 3: Deliver Record and Text-Mode Observed Verify

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/main.rs`
- Modify: `src/output/mod.rs`
- Create: `tests/cli_record.rs`
- Modify: `tests/cli_verify.rs`
- Create: `testdata/observed/portfolio-empty.json`
- Create: `testdata/observed/portfolio-populated.json`
- Create: `testdata/observed/portfolio-matching.json`
- Create: `testdata/observed/portfolio-missing-required.json`
- Create: `testdata/observed/portfolio-type-drift.json`

**Interfaces:**
- Consumes: the Task 1 shape engine and Task 2 `record_observed` and
  `VerifyTarget` APIs.
- Produces: public `record` command, provenance-aware text Verify, and stable
  CLI exit behavior.

- [ ] **Step 1: Create JSON fixtures that contain representative private values**

  Use `portfolio-empty.json` with an empty `holdings` array and a field such as
  `"session_token": "recording-secret-001"`. Use `portfolio-populated.json`
  with a holding object, `"live_price": null`, and an `error` field. Make the
  matching fixture vary every scalar value while retaining the learned shape;
  make the missing fixture omit a field that is present in every recording;
  make the type-drift fixture change a locked number to a string.

- [ ] **Step 2: Write failing record and observed Verify integration tests**

  Create `tests/cli_record.rs` with the existing temporary-lock-path pattern
  from `tests/cli_lock.rs`. Add these tests before CLI implementation:

  ```rust
  #[test]
  fn record_creates_a_value_free_v2_observed_lock() {
      let output = temp_lock_path("observed-record");
      Command::cargo_bin("apiwatch").unwrap()
          .args(["record", "--from-json", "testdata/observed/portfolio-empty.json",
                 "--name", "portfolio", "--output", output.to_str().unwrap()])
          .assert().success();

      let lock = fs::read_to_string(&output).unwrap();
      assert!(lock.starts_with("version: 2\n"));
      assert!(lock.contains("provenance: observed"));
      assert!(!lock.contains("recording-secret-001"));
      fs::remove_file(output).ok();
  }
  ```

  Add tests for byte-identical repeated record output, v1 migration while
  retaining `GET /users`, merge widening `live_price` to a null/number union,
  and rejected duplicate or declared entry names. In `tests/cli_verify.rs`,
  add tests that an observed matching body exits `0` with `Verified portfolio`,
  a missing required field exits `1` with
  `BREAKING $.summary.current_value: required field missing`, and type drift
  exits `1` with the expected/found type line. Assert no observed stderr or
  stdout contains `recording-secret-001` or other fixture values.

- [ ] **Step 3: Run the new integration tests and confirm they fail**

  Run: `cargo test --test cli_record --test cli_verify`

  Expected: Clap rejects the unknown `record` command and observed Verify has
  no route yet.

- [ ] **Step 4: Add CLI parsing and Record orchestration**

  Add this `Command::Record` variant in `src/cli.rs`:

  ```rust
  Record {
      #[arg(long)]
      from_json: PathBuf,
      #[arg(long)]
      name: String,
      #[arg(long)]
      output: PathBuf,
      #[arg(long)]
      merge: bool,
  }
  ```

  Rename Verify's internal positional field to `input: String` while retaining
  its positional syntax. In `main.rs`, route Record through
  `observed::load_shape`, `lockfile::load_or_create_for_record`,
  `lockfile::record_observed`, and `lockfile::render`; write only after every
  validation succeeds and print `Wrote <path>` on success.

- [ ] **Step 5: Route Verify by provenance and render observed text**

  Match `VerifyTarget` after lock selection. Keep the existing declared arm
  unchanged except for matching `VerifyTarget::Declared`. For an observed
  target, reject `http://` and `https://` inputs with an error containing
  `observed verification requires a local JSON file`; load the local shape;
  compare it; print `Verified <name>` for no changes; otherwise use this new
  renderer:

  ```rust
  pub fn render_observed_verify_changes(changes: &[ObservedChange]) -> String {
      changes.iter().map(|change| {
          match change.kind {
              ObservedChangeKind::MissingRequiredField =>
                  format!("BREAKING {}: required field missing\n", change.path),
              ObservedChangeKind::IncompatibleShape => format!(
                  "BREAKING {}: expected {}, found {}\n",
                  change.path,
                  change.expected.as_deref().unwrap(),
                  change.actual.as_deref().unwrap(),
              ),
          }
      }).collect()
  }
  ```

  Preserve deterministic comparison order and never render a value.

- [ ] **Step 6: Run text-mode CLI coverage**

  Run: `cargo test --test cli_record --test cli_verify`

  Expected: every Record, merge, migration, observed text Verify, and existing
  declared Verify test passes.

- [ ] **Step 7: Commit the first end-to-end workflow**

  ```powershell
  git add src/cli.rs src/main.rs src/output/mod.rs tests/cli_record.rs tests/cli_verify.rs testdata/observed
  git commit -m "Add observed contract recording and verification"
  ```

  Expected: the commit contains the public Record command and text-mode
  observed Verify coverage.

---

### Task 4: Add Observed JSON and SARIF Verify Output

**Files:**
- Modify: `src/output/mod.rs`
- Modify: `src/main.rs`
- Modify: `tests/cli_verify.rs`

**Interfaces:**
- Consumes: `ObservedChange` from Task 1 and the established `OutputFormat`.
- Produces: `render_observed_verify_changes_json` and
  `render_observed_verify_changes_sarif` for the observed Verify arm only.

- [ ] **Step 1: Write failing output-format integration tests**

  Add an observed JSON assertion that parses stdout and expects:

  ```rust
  assert_eq!(rendered, json!({
      "version": 2,
      "command": "verify",
      "name": "portfolio",
      "provenance": "observed",
      "summary": {"breaking": 1},
      "changes": [{
          "kind": "missing_required_field",
          "path": "$.summary.current_value"
      }]
  }));
  ```

  Add a type-drift JSON test that includes only `kind`, `path`, `expected`,
  and `actual`; assert it excludes fixture secret values. Add a SARIF test that
  checks SARIF `2.1.0`, the local lock artifact URI, an observed missing-field
  rule ID, `error` level, and a fingerprint containing no values. Retain the
  existing declared JSON/SARIF tests unchanged.

- [ ] **Step 2: Run format tests and confirm they fail**

  Run: `cargo test --test cli_verify observed -- --nocapture`

  Expected: JSON and SARIF assertions fail because observed Verify currently
  emits text for every format.

- [ ] **Step 3: Add observed JSON serialization**

  Add private serializer structs in `src/output/mod.rs` and this public
  function:

  ```rust
  pub fn render_observed_verify_changes_json(
      name: &str,
      changes: &[ObservedChange],
  ) -> Result<String>;
  ```

  Serialize the exact version-2 envelope from Step 1 with `serde_json`, append
  exactly one newline, and map the two change kinds to
  `missing_required_field` and `incompatible_shape`. Omit `expected` and
  `actual` for missing fields; include only type names for incompatible shapes.

- [ ] **Step 4: Add observed SARIF serialization**

  Add:

  ```rust
  pub fn render_observed_verify_changes_sarif(
      artifact_path: &Path,
      name: &str,
      changes: &[ObservedChange],
  ) -> Result<String>;
  ```

  Reuse `render_artifact_uri` and the private SARIF document types, but give
  observed output a fixed two-rule list:
  `apiwatch/verify-observed-missing-required-field` and
  `apiwatch/verify-observed-incompatible-shape`. Use `error` for both results;
  point locations at the lockfile; build fingerprints as
  `verify-observed:<name>:<rule-id>:<path>:<expected>:<actual>`, omitting
  absent type segments. Do not modify the declared five-rule list or declared
  renderers.

- [ ] **Step 5: Select observed renderers in `main.rs`**

  In the observed Verify arm, select text, JSON, or SARIF with the new
  renderer functions. Preserve `0` for a match, `1` for all observed drift,
  and `2` for loading/serialization errors. Leave the declared arm's calls to
  `render_verify_changes_json` and `render_verify_changes_sarif` unchanged.

- [ ] **Step 6: Run all format and regression tests**

  Run: `cargo test --test cli_verify`

  Expected: observed text/JSON/SARIF and every pre-existing declared Verify
  JSON/SARIF test passes.

  Run: `cargo test`

  Expected: the complete suite passes.

- [ ] **Step 7: Commit output contracts**

  ```powershell
  git add src/output/mod.rs src/main.rs tests/cli_verify.rs
  git commit -m "Add observed verification machine output"
  ```

  Expected: the commit adds only observed result serializers and their tests.

---

### Task 5: Document the Workflow and Run the Release Gate

**Files:**
- Modify: `README.md`
- Modify: `docs/lockfile-spec.md`
- Modify: `CHANGELOG.md`
- Modify: `action.yml`
- Create: `implementation-log/2026-07-17-apiwatch-observed-contracts.md` (ignored)

**Interfaces:**
- Consumes: the complete Record, v2 lockfile, and observed Verify CLI.
- Produces: accurate public documentation and recorded validation evidence.

- [ ] **Step 1: Add documentation assertions before editing prose**

  Add a short PowerShell validation script to the implementation log plan and
  run it after editing. It must assert README contains each of:

  ```powershell
  'apiwatch record --from-json body.json --name portfolio --output api.lock'
  'apiwatch record --from-json updated.json --name portfolio --output api.lock --merge'
  'apiwatch verify body.json --name portfolio --lock api.lock'
  'APIWatch records JSON structure, never captured values'
  ```

  It must also assert `docs/lockfile-spec.md` contains `## Version 2`,
  `provenance: observed`, and `provenance: declared`; `CHANGELOG.md` has one
  observed-contract bullet under Unreleased; and `action.yml` describes its
  input as provenance-selected rather than OpenAPI-only.

- [ ] **Step 2: Update the README and lockfile specification**

  Add a concise `## Observed JSON Contracts` README section after the CLI
  examples. Include Record, merge, and observed Verify commands; explain that
  values are never retained; distinguish Record's mutating learning mode from
  Verify's read-only checking mode; state that local JSON is the only observed
  input in this release; and list map annotations, coverage, HAR, and live
  recording as deferred.

  Extend `docs/lockfile-spec.md` with a Version 2 example matching the design:
  explicit provenance, observed shape node kinds, property observation counts,
  union ordering, v1 read compatibility, and the no-value privacy guarantee.

- [ ] **Step 3: Update changelog and composite action metadata**

  Add a single Unreleased bullet to `CHANGELOG.md` for versioned observed JSON
  recording, merge, and verification. Change `action.yml`'s `openapi` input
  description to `OpenAPI input or local JSON body, selected by the named lock
  entry provenance.` Preserve the action input name and every command line so
  existing consumers remain compatible.

- [ ] **Step 4: Run the documentation assertions and public CLI smoke tests**

  Run the PowerShell assertions from Step 1.

  Expected: it prints `Observed-contract documentation is valid.`

  Run:

  ```powershell
  cargo run -- record --from-json testdata/observed/portfolio-empty.json --name portfolio --output C:\tmp\apiwatch-observed.lock
  cargo run -- record --from-json testdata/observed/portfolio-populated.json --name portfolio --output C:\tmp\apiwatch-observed.lock --merge
  cargo run -- verify testdata/observed/portfolio-matching.json --name portfolio --lock C:\tmp\apiwatch-observed.lock
  ```

  Expected: both Record commands write the lock and Verify prints `Verified
  portfolio`. Remove only the explicit temporary file after checking it.

- [ ] **Step 5: Run the final quality gate and update the ignored log**

  Run:

  ```powershell
  cargo fmt --all -- --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test
  git diff --check
  git status --short
  ```

  Expected: every command succeeds; status lists only intended tracked docs
  plus the ignored implementation log. Record the goal, v1/v2 decision,
  files touched, test totals, privacy checks, deferred map/coverage scope, and
  any blockers in `implementation-log/2026-07-17-apiwatch-observed-contracts.md`.

- [ ] **Step 6: Commit documentation separately**

  ```powershell
  git add README.md docs/lockfile-spec.md CHANGELOG.md action.yml
  git commit -m "Document observed contract workflow"
  ```

  Expected: the final tracked commit contains documentation and metadata only;
  the implementation log remains ignored.
