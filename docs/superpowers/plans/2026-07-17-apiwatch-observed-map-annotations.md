# APIWatch Observed Map Annotations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow `apiwatch record` to explicitly model dynamic-key JSON objects as value-free maps, so key churn is accepted while every dynamic value remains structurally verified.

**Architecture:** Extend the observed-shape engine with a serializable `Shape::Map` node and a strict, property-only JSONPath annotation routine. Record applies the repeatable annotations atomically to the incoming shape and, on merge, the prior observed shape before using the existing merge pipeline. Verification, including text, JSON, and SARIF output, reuses existing observed changes because map comparison produces the same path-and-type diagnostics.

**Tech Stack:** Rust 2021, `clap`, `serde`, `serde_yaml`, `serde_json`, `anyhow`, `assert_cmd`, and the existing Rust integration-test fixtures.

## Global Constraints

- Do not add dependencies; the JSON, YAML, CLI, and error crates required by this work already exist in `Cargo.toml`.
- Keep the existing version-2 lockfile format; `kind: map` is a backward-compatible observed-shape node, not a lockfile version bump.
- Preserve all version-1 declared and version-2 declared/observed verification behavior that does not use an explicit `--map-at` annotation.
- Accept exactly `$` plus zero or more named property segments (`$.by_broker`, `$.state.by_region`); a segment starts with ASCII letter or `_` and continues with ASCII letters, digits, or `_`.
- Reject duplicate paths, missing paths, non-object targets, empty segments, bracket notation, arrays, wildcards, filters, scripts, and malformed paths as ordinary CLI input errors (exit `2`).
- Apply annotations to the incoming shape before recording; with `--merge`, apply them to a cloned existing observed shape too, and mutate the lock only after both transformations succeed.
- A map stores no dynamic keys or scalar input values. Locks and all diagnostics may contain only field names, JSON paths, and shape names.
- Do not implement automatic map inference, coverage reporting, HAR/live recording, enum inference, or advanced JSONPath in this slice.
- Keep `implementation-log/2026-07-17-apiwatch-observed-map-annotations.md` concise and ignored; do not stage it.
- Before handoff, run `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, and `git diff --check`.

## File Structure

- Modify `src/observed/mod.rs`: add `Shape::Map`, strict annotation parsing and conversion, map-aware merging, and map-aware directional comparison.
- Modify `src/cli.rs`: make `--map-at` a repeatable `Record` option.
- Modify `src/main.rs`: forward Record annotations to the lockfile layer.
- Modify `src/lockfile/mod.rs`: atomically annotate new and existing observed shapes before record/merge.
- Modify `tests/cli_record.rs`: cover public annotation parsing, recording, merging, and error atomicity.
- Modify `tests/cli_verify.rs`: cover dynamic key churn, map value drift, and map-to-scalar drift in public Verify output.
- Create `testdata/observed/portfolio-map-initial.json`, `testdata/observed/portfolio-map-merged.json`, `testdata/observed/portfolio-map-matching.json`, `testdata/observed/portfolio-map-value-drift.json`, and `testdata/observed/portfolio-map-scalar-drift.json`: stable value-containing bodies used only as local inputs.
- Modify `README.md`, `docs/lockfile-spec.md`, and `CHANGELOG.md`: document explicit map annotations, `kind: map`, safety, and deferred scope.

---

### Task 1: Add Explicit Map Shapes and Strict Annotations

**Files:**
- Modify: `src/observed/mod.rs`

**Interfaces:**
- Consumes: existing `serde_json::Value`, `Shape`, `ObservedProperty`, `merge`, and `compare` behavior.
- Produces:

  ```rust
  pub fn apply_map_annotations(shape: &mut Shape, paths: &[String]) -> Result<()>;
  ```

  and the additional shape node:

  ```rust
  Shape::Map { values: Box<Shape> }
  ```

  for the lockfile and recording tasks that follow.

- [ ] **Step 1: Write failing focused unit tests in `src/observed/mod.rs`**

  Add these tests to the existing module test block. They pin down the public annotation contract without relying on CLI parsing:

  ```rust
  #[test]
  fn annotation_converts_an_object_to_a_value_free_map() {
      let mut shape = infer(&json!({
          "by_broker": {
              "acme": {"pnl_pct": 1.2, "session_token": "secret-one"},
              "globex": {"pnl_pct": 3.4, "session_token": "secret-two"}
          }
      }));

      apply_map_annotations(&mut shape, &["$.by_broker".to_owned()])
          .expect("annotation should succeed");
      let rendered = serde_yaml::to_string(&shape).expect("shape should serialize");

      assert!(rendered.contains("kind: map"));
      assert!(rendered.contains("pnl_pct"));
      assert!(!rendered.contains("acme"));
      assert!(!rendered.contains("globex"));
      assert!(!rendered.contains("secret-one"));
      assert!(!rendered.contains("secret-two"));
  }

  #[test]
  fn annotation_accepts_root_and_nested_named_property_paths() {
      let mut root = infer(&json!({"acme": 1, "globex": 2}));
      apply_map_annotations(&mut root, &["$".to_owned()]).expect("root map should work");
      assert!(matches!(root, Shape::Map { .. }));

      let mut nested = infer(&json!({"state": {"by_region": {"in": true}}}));
      apply_map_annotations(&mut nested, &["$.state.by_region".to_owned()])
          .expect("nested map should work");
      let Shape::Object { properties, .. } = nested else { panic!("root should remain object") };
      let state = &properties["state"].shape;
      let Shape::Object { properties, .. } = state.as_ref() else { panic!("state should be object") };
      assert!(matches!(properties["by_region"].shape.as_ref(), Shape::Map { .. }));
  }

  #[test]
  fn annotation_rejects_invalid_duplicate_missing_and_non_object_targets() {
      let base = infer(&json!({"by_broker": {"acme": 1}, "scalar": 1}));
      for paths in [
          vec!["$.by_broker".to_owned(), "$.by_broker".to_owned()],
          vec!["$".to_owned(), "$.by_broker".to_owned()],
          vec!["$.missing".to_owned()],
          vec!["$.scalar".to_owned()],
          vec!["$.by-broker".to_owned()],
          vec!["$.by_broker[0]".to_owned()],
          vec!["$.by_broker.*".to_owned()],
          vec!["$..by_broker".to_owned()],
      ] {
          let mut shape = base.clone();
          assert!(apply_map_annotations(&mut shape, &paths).is_err(), "{paths:?}");
          assert_eq!(shape, base, "invalid paths must leave the shape unchanged");
      }
  }

  #[test]
  fn map_merges_later_plain_objects_and_verify_ignores_key_churn() {
      let mut expected = infer(&json!({"by_broker": {"acme": {"pnl_pct": 1}}}));
      apply_map_annotations(&mut expected, &["$.by_broker".to_owned()])
          .expect("annotation should succeed");
      merge(&mut expected, &infer(&json!({
          "by_broker": {"globex": {"pnl_pct": 2}}
      })));

      assert!(compare(&expected, &infer(&json!({
          "by_broker": {"other": {"pnl_pct": 3}}
      }))).is_empty());
      assert!(compare(&expected, &infer(&json!({"by_broker": {}}))).is_empty());

      let changes = compare(&expected, &infer(&json!({
          "by_broker": {"acme": {"pnl_pct": "wrong"}}
      })));
      assert!(changes.iter().any(|change| {
          change.path == "$.by_broker.<map-value>.pnl_pct"
              && change.expected.as_deref() == Some("number")
              && change.actual.as_deref() == Some("string")
      }));
  }
  ```

- [ ] **Step 2: Run the focused tests and verify they fail before implementation**

  Run:

  ```powershell
  cargo test observed::tests -- --nocapture
  ```

  Expected: compilation fails because `Shape::Map` and `apply_map_annotations` do not exist.

- [ ] **Step 3: Add the map model and deterministic helpers**

  Extend the existing serde-tagged `Shape` definition and its deterministic sort key with this exact variant and name:

  ```rust
  Map {
      values: Box<Shape>,
  },
  ```

  Add `Map` after `Object` in the enum so YAML is rendered as `kind: map` with
  a nested `values` shape. Make `shape_name` return `"map"`. In the existing
  canonical union-order helper, give `Map` a unique order between `Object` and
  `Array`; do not use debug output or dynamic key names as a sort key.

  Add these private helpers beside `infer`:

  ```rust
  use std::collections::BTreeSet;

  fn parse_map_path(raw: &str) -> Result<Vec<String>>;
  fn paths_overlap(left: &[String], right: &[String]) -> bool;
  fn object_value_shape(properties: &BTreeMap<String, ObservedProperty>) -> Shape;
  fn annotate_map_at(shape: &mut Shape, raw: &str, segments: &[String]) -> Result<()>;
  ```

  `parse_map_path` must return an empty segment vector only for `$`. For all
  other paths, scan from byte zero: require `$.`, then consume one or more
  valid named segments. Validate a first byte with
  `is_ascii_alphabetic() || byte == b'_'` and later bytes with
  `is_ascii_alphanumeric() || byte == b'_'`. Return exact sanitized errors of
  the form `invalid map annotation path <raw>: expected $ followed by named property segments`.
  Never echo a body or a JSON value in an error.

  Before transforming, reject any two distinct parsed paths where one segment
  list is a prefix of the other with `overlapping map annotation path <raw>`.
  This makes `$` plus `$.by_broker` (and any ancestor/descendant pair) a
  deterministic input error instead of allowing flag order to change the
  recorded contract.

  `object_value_shape` must start with `Shape::Unknown`, merge each
  `ObservedProperty.shape` in `BTreeMap` order, and return `Unknown` for an
  empty object. It must not retain property keys or observations.

- [ ] **Step 4: Implement all-or-nothing annotation, map merge, and comparison**

  Implement the public entry point as a validate-then-transform operation:

  ```rust
  pub fn apply_map_annotations(shape: &mut Shape, paths: &[String]) -> Result<()> {
      let parsed = paths
          .iter()
          .map(|path| parse_map_path(path).map(|segments| (path, segments)))
          .collect::<Result<Vec<_>>>()?;
      let mut seen = BTreeSet::new();
      for (raw, segments) in &parsed {
          if !seen.insert(segments.clone()) {
              bail!("duplicate map annotation path {raw}");
          }
      }
      for (index, (raw, segments)) in parsed.iter().enumerate() {
          if parsed[..index]
              .iter()
              .any(|(_, previous)| paths_overlap(previous, segments))
          {
              bail!("overlapping map annotation path {raw}");
          }
      }

      let mut annotated = shape.clone();
      for (raw, segments) in parsed {
          annotate_map_at(&mut annotated, raw, &segments)?;
      }
      *shape = annotated;
      Ok(())
  }
  ```

  `annotate_map_at` must recursively follow only `Shape::Object` properties.
  At the target it replaces an `Object { properties, .. }` with
  `Map { values: Box::new(object_value_shape(&properties)) }`. An already-map
  target is a no-op so repeated `--merge --map-at` commands remain valid after
  the first recording. A missing nested property returns
  `map annotation path $.a.b does not exist`; a target or intermediate value
  that is not an object/map returns
  `map annotation path $.a.b must target an object`. Use the original path
  text in these messages, never a serialized shape.

  Extend `merge` with these cases before the normal same-kind/union fallback:

  ```rust
  (Shape::Map { values: existing }, Shape::Map { values: incoming }) => {
      merge(existing, incoming);
  }
  (Shape::Map { values }, Shape::Object { properties, .. }) => {
      let incoming_values = object_value_shape(properties);
      merge(values, &incoming_values);
  }
  ```

  Keep a `Map` with any scalar, array, null, boolean, number, or string in
  the existing deterministic union fallback. Do not convert an ordinary
  object to a map inside `merge`.

  Extend directional `compare` so an expected map accepts an actual object by
  comparing `values` with each actual `ObservedProperty.shape` at
  `join_path(path, key)`. It accepts an empty object, compares an actual map's
  `values` directly for internal/unit-test use, and reports one
  `IncompatibleShape` at the current path when actual is another kind. This
  preserves array, union, required-field, and Unknown comparison behavior.

- [ ] **Step 5: Run unit and regression coverage**

  Run:

  ```powershell
  cargo test observed::tests -- --nocapture
  cargo test lockfile::tests -- --nocapture
  ```

  Expected: the new annotation tests pass, and pre-existing lockfile behavior
  remains unchanged.

- [ ] **Step 6: Commit the self-contained shape engine change**

  ```powershell
  git add src/observed/mod.rs
  git commit -m "Add observed map annotations"
  ```

  Expected: the commit contains only map-shape behavior and its unit tests.

---

### Task 2: Expose Repeatable Record Annotations and Preserve Atomicity

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/main.rs`
- Modify: `src/lockfile/mod.rs`
- Modify: `tests/cli_record.rs`
- Create: `testdata/observed/portfolio-map-initial.json`
- Create: `testdata/observed/portfolio-map-merged.json`

**Interfaces:**
- Consumes: `observed::apply_map_annotations`, existing `load_shape`, and the
  v2 observed-entry `ApiLock` model.
- Produces the new lockfile API:

  ```rust
  pub fn record_observed(
      lock: &mut ApiLock,
      name: &str,
      incoming: Shape,
      merge_existing: bool,
      map_paths: &[String],
  ) -> Result<()>;
  ```

  and this `Record` CLI field:

  ```rust
  #[arg(long = "map-at")]
  map_at: Vec<String>,
  ```

- [ ] **Step 1: Create fixtures and write public failing CLI tests**

  Create `testdata/observed/portfolio-map-initial.json`:

  ```json
  {
    "by_broker": {
      "acme": {"pnl_pct": 1.2, "session_token": "map-secret-initial"},
      "globex": {"pnl_pct": 3.4, "session_token": "map-secret-initial-2"}
    },
    "state": {"by_region": {"in": {"active": true}}}
  }
  ```

  Create `testdata/observed/portfolio-map-merged.json`:

  ```json
  {
    "by_broker": {"initech": {"pnl_pct": 5.6, "session_token": "map-secret-merged"}},
    "state": {"by_region": {"us": {"active": false}}}
  }
  ```

  Add a helper in `tests/cli_record.rs` that calls Record with a temporary
  output path, then add these tests:

  ```rust
  #[test]
  fn record_repeatable_map_at_writes_value_free_maps() {
      let output = temp_lock_path("observed-map");
      Command::cargo_bin("apiwatch").unwrap()
          .args([
              "record", "--from-json", "testdata/observed/portfolio-map-initial.json",
              "--name", "portfolio", "--output", output.to_str().unwrap(),
              "--map-at", "$.by_broker", "--map-at", "$.state.by_region",
          ])
          .assert().success();

      let lock = fs::read_to_string(&output).unwrap();
      fs::remove_file(&output).ok();
      assert_eq!(lock.matches("kind: map").count(), 2);
      assert!(!lock.contains("acme"));
      assert!(!lock.contains("globex"));
      assert!(!lock.contains("map-secret-initial"));
  }

  #[test]
  fn merge_into_recorded_map_needs_no_repeated_annotation() {
      let output = temp_lock_path("observed-map-merge");
      let output_arg = output.to_str().unwrap();
      Command::cargo_bin("apiwatch").unwrap()
          .args([
              "record", "--from-json", "testdata/observed/portfolio-map-initial.json",
              "--name", "portfolio", "--output", output_arg, "--map-at", "$.by_broker",
          ])
          .assert().success();
      Command::cargo_bin("apiwatch").unwrap()
          .args([
              "record", "--from-json", "testdata/observed/portfolio-map-merged.json",
              "--name", "portfolio", "--output", output_arg, "--merge",
          ])
          .assert().success();

      let lock = fs::read_to_string(&output).unwrap();
      fs::remove_file(&output).ok();
      assert!(lock.contains("kind: map"));
      assert!(!lock.contains("initech"));
      assert!(!lock.contains("map-secret-merged"));
  }
  ```

  Add table-driven failure cases for `$.by-broker`, `$.by_broker[0]`,
  `$..by_broker`, `$.missing`, `$.state.by_region.in.active`, and two copies
  of `$.by_broker`.
  For each, assert exit code `2`, an empty stdout, a stderr substring naming
  `map annotation`, and that a pre-existing lock's bytes are unchanged.

- [ ] **Step 2: Run the new CLI tests and verify they fail before plumbing**

  Run:

  ```powershell
  cargo test --test cli_record -- --nocapture
  ```

  Expected: Clap rejects `--map-at` as an unknown argument; the existing
  Record tests still pass.

- [ ] **Step 3: Add the repeatable CLI argument and forward it**

  In the existing `Command::Record` definition in `src/cli.rs`, add exactly:

  ```rust
  #[arg(long = "map-at")]
  map_at: Vec<String>,
  ```

  Keep it next to `merge` so its help describes Record mutation options. In
  the `Command::Record` match arm in `src/main.rs`, destructure `map_at` and
  forward `&map_at` to `lockfile::record_observed`. Do not parse or normalize
  paths in CLI code; the observed module owns that contract.

- [ ] **Step 4: Apply annotations atomically in `record_observed`**

  Change the signature to the interface above. Keep the existing name,
  duplicate-entry, declared-entry, and v1-to-v2 validation order. Replace
  direct mutation with this staged flow:

  ```rust
  let mut annotated_incoming = incoming;
  apply_map_annotations(&mut annotated_incoming, map_paths)?;

  match lock.apis.get(name) {
      None => {
          lock.version = LockVersion::V2;
          lock.apis.insert(name.to_owned(), LockedApi::Observed {
              shape: annotated_incoming,
          });
      }
      Some(LockedApi::Observed { shape }) if merge_existing => {
          let mut annotated_existing = shape.clone();
          apply_map_annotations(&mut annotated_existing, map_paths)?;
          merge(&mut annotated_existing, &annotated_incoming);
          *lock.apis.get_mut(name).expect("entry was checked") = LockedApi::Observed {
              shape: annotated_existing,
          };
      }
      Some(LockedApi::Observed { .. }) => bail!("api {name} already exists; pass --merge to update it"),
      Some(LockedApi::Declared { .. }) => bail!("api {name} is declared and cannot be recorded as observed"),
  }
  ```

  Move the version upgrade into the successful insertion/update branches if
  necessary so an invalid annotation cannot convert an otherwise untouched v1
  lock. If current validation normalizes `name` before lookup, preserve that
  normalized value in every message and `BTreeMap` operation.

  Add lockfile unit tests for both atomic cases: invalid annotation on a new
  entry leaves the empty lock unmodified, and invalid annotation with merge
  leaves the serialized existing observed entry byte-identical.

- [ ] **Step 5: Run CLI and lockfile coverage**

  Run:

  ```powershell
  cargo test --test cli_record -- --nocapture
  cargo test lockfile::tests -- --nocapture
  cargo test --test cli_verify
  ```

  Expected: two repeatable annotations render two maps; later unannotated
  merges retain the first map; invalid flags return code `2` without changing
  locks; all existing Verify behavior remains green.

- [ ] **Step 6: Commit record plumbing and public contract coverage**

  ```powershell
  git add src/cli.rs src/main.rs src/lockfile/mod.rs tests/cli_record.rs testdata/observed/portfolio-map-initial.json testdata/observed/portfolio-map-merged.json
  git commit -m "Record explicit observed maps"
  ```

  Expected: the commit contains the public repeatable option, atomic
  lockfile mutation, two source fixtures, and record tests only.

---

### Task 3: Verify Dynamic Maps, Document the Contract, and Run the Release Gate

**Files:**
- Modify: `tests/cli_verify.rs`
- Create: `testdata/observed/portfolio-map-matching.json`
- Create: `testdata/observed/portfolio-map-value-drift.json`
- Create: `testdata/observed/portfolio-map-scalar-drift.json`
- Modify: `README.md`
- Modify: `docs/lockfile-spec.md`
- Modify: `CHANGELOG.md`
- Modify: `implementation-log/2026-07-17-apiwatch-observed-map-annotations.md` (ignored)

**Interfaces:**
- Consumes: Task 1 map comparison, Task 2 Record command, and the existing
  text/JSON/SARIF observed Verify renderers.
- Produces: public verification guarantees for key churn and complete,
  accurate docs for `--map-at` and `kind: map`.

- [ ] **Step 1: Create verify fixtures and write failing end-to-end tests**

  Create `testdata/observed/portfolio-map-matching.json` with an empty
  `by_broker` object and a changed `state.by_region` key; it proves removed,
  new, and empty dynamic maps are compatible:

  ```json
  {
    "by_broker": {},
    "state": {"by_region": {"eu": {"active": true}}}
  }
  ```

  Create `testdata/observed/portfolio-map-value-drift.json`:

  ```json
  {
    "by_broker": {"acme": {"pnl_pct": "not-a-number", "session_token": "verify-secret"}},
    "state": {"by_region": {"in": {"active": true}}}
  }
  ```

  Create `testdata/observed/portfolio-map-scalar-drift.json`:

  ```json
  {
    "by_broker": "unavailable",
    "state": {"by_region": {"in": {"active": true}}}
  }
  ```

  In `tests/cli_verify.rs`, add a `record_map_portfolio(&Path)` helper that
  records the initial fixture with both `--map-at` paths, then add these
  tests:

  ```rust
  #[test]
  fn verify_observed_map_accepts_dynamic_key_churn_and_empty_maps() {
      let lock = observed_lock_path();
      record_map_portfolio(&lock);
      verify_command(
          "testdata/observed/portfolio-map-matching.json",
          "portfolio",
          lock.to_str().unwrap(),
      )
      .assert().success().stdout("Verified portfolio\n");
      fs::remove_file(lock).ok();
  }

  #[test]
  fn verify_observed_map_reports_dynamic_value_type_drift_without_values() {
      let lock = observed_lock_path();
      record_map_portfolio(&lock);
      verify_command(
          "testdata/observed/portfolio-map-value-drift.json",
          "portfolio",
          lock.to_str().unwrap(),
      )
      .assert().code(1)
      .stdout(predicate::str::contains(
          "BREAKING $.by_broker.<map-value>.pnl_pct: expected number, found string\n",
      ))
      .stdout(predicate::str::contains("verify-secret").not());
      fs::remove_file(lock).ok();
  }

  #[test]
  fn verify_observed_map_reports_map_to_scalar_drift() {
      let lock = observed_lock_path();
      record_map_portfolio(&lock);
      verify_command(
          "testdata/observed/portfolio-map-scalar-drift.json",
          "portfolio",
          lock.to_str().unwrap(),
      )
      .assert().code(1)
      .stdout("BREAKING $.by_broker: expected map, found string\n");
      fs::remove_file(lock).ok();
  }
  ```

  Add one `--format json` assertion that the value-drift change includes only
  `kind: "incompatible_shape"`, the dynamic-key path, `expected: "number"`,
  and `actual: "string"`. Add one SARIF assertion that the same dynamic path
  appears in the result message/fingerprint and `verify-secret` does not
  appear anywhere in stdout.

- [ ] **Step 2: Run the end-to-end tests and verify they fail before map comparison is complete**

  Run:

  ```powershell
  cargo test --test cli_verify observed_map -- --nocapture
  ```

  Expected: before Task 1/2 integration is complete, map Record or Verify
  assertions fail; after implementing prior tasks, this command must pass.

- [ ] **Step 3: Complete any map comparison/output integration exposed by the tests**

  Do not create a new output format. If a failure reveals that the observed
  renderer excludes nested map paths, preserve the existing change object and
  fix its producer in `src/observed/mod.rs` so all formats receive:

  ```rust
  ObservedChange {
      kind: ObservedChangeKind::IncompatibleShape,
      path: "$.by_broker.<map-value>.pnl_pct".to_owned(),
      expected: Some("number".to_owned()),
      actual: Some("string".to_owned()),
  }
  ```

  If the expected map receives a scalar, produce the same change kind at the
  annotated path with `expected: Some("map".to_owned())`. Do not add map keys,
  fixture values, or a map-specific result schema to `src/output/mod.rs`.

- [ ] **Step 4: Run all public verification tests**

  Run:

  ```powershell
  cargo test --test cli_verify -- --nocapture
  cargo test --test cli_record -- --nocapture
  cargo test
  ```

  Expected: the new map text, JSON, and SARIF tests pass alongside every
  existing declared and observed verification test.

- [ ] **Step 5: Update the README, lockfile spec, and changelog**

  Add a concise `Observed JSON Maps` subsection in `README.md` after the
  observed contract workflow. It must include this real command:

  ```text
  apiwatch record --from-json portfolio.json --name portfolio --output api.lock --map-at $.by_broker --map-at $.state.by_region
  ```

  State that `--map-at` is repeatable, keys are treated as dynamic data rather
  than API fields, every value is still checked, annotations are explicit,
  and stored locks/diagnostics never retain captured values. State that
  automatic map inference, advanced JSONPath, and coverage reporting remain
  deferred.

  In `docs/lockfile-spec.md`, add a version-2 observed example exactly shaped
  like:

  ```yaml
  shape:
    kind: object
    observations: 1
    properties:
      by_broker:
        observations: 1
        shape:
          kind: map
          values:
            kind: object
            observations: 2
            properties:
              pnl_pct:
                observations: 2
                shape:
                  kind: number
  ```

  Document accepted paths (`$`, named property segments), the rejection of
  bracket/wildcard/filter expressions, `Map + Object` merge behavior, and
  directional Verify semantics. Add one Unreleased `CHANGELOG.md` bullet:
  `Add explicit repeatable --map-at annotations for dynamic-key observed JSON maps.`

- [ ] **Step 6: Validate documentation and run the release gate**

  Run:

  ```powershell
  rg -F -- '--map-at $.by_broker --map-at $.state.by_region' README.md
  rg -F -- 'kind: map' docs/lockfile-spec.md
  rg -F -- 'automatic map inference' README.md docs/lockfile-spec.md
  cargo fmt --all -- --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test
  git diff --check
  ```

  Expected: the three documentation searches return matches; formatting,
  linting, all tests, and whitespace checks succeed.

- [ ] **Step 7: Update the ignored implementation log and commit docs/tests**

  In `implementation-log/2026-07-17-apiwatch-observed-map-annotations.md`,
  record the completed goal, explicit-only decision, files touched, exact
  final verification commands, no-value checks, and any blocker. Do not stage
  the log.

  ```powershell
  git add tests/cli_verify.rs testdata/observed/portfolio-map-matching.json testdata/observed/portfolio-map-value-drift.json testdata/observed/portfolio-map-scalar-drift.json README.md docs/lockfile-spec.md CHANGELOG.md
  git commit -m "Document observed map annotations"
  ```

  Expected: the final tracked commit contains verification fixtures/tests and
  documentation; `implementation-log/` remains ignored.
