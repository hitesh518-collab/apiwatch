# Phase 3 Real-World Compatibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make APIWatch's declared contracts work against real-world specifications and delivery patterns — recursive schemas, malformed metadata, multi-file refs, OpenAPI 3.1, and a globally representative compatibility corpus.

**Architecture:** Each task builds on the existing v4 normalization pipeline. D-14 adds a `CycleRef` schema variant to the contract model that the comparison engine treats as an opaque leaf. D-13 wraps OpenAPI deserialization in a tolerant value-tree layer. D-15 adds file-level `$ref` resolution. D-12 normalizes 3.1 constructs into the v4 model. Configuration and auth headers are independent modules. The corpus expands from 5 to 10 globally diverse specs.

**Tech Stack:** Rust 1.86+, openapiv3 2, serde_yml (replaces serde_yaml 0.9), serde_json, sha2, clap 4, reqwest 0.12

## Global Constraints

- MSRV: Rust 1.86
- diff and declared Verify share one `diff_contracts` comparison path
- Deterministic ordering and byte-stable lock output must be preserved
- Verify is read-only
- No observed values, credentials, or dynamic map keys in lockfiles/diagnostics
- Each defect gets a failing regression fixture before its fix
- Lockfile versions 1-3 remain readable (v3 reports partial Phase 2 coverage)
- Production v4 lock payloads must stay within the 5,242,880-byte ceiling

---

## File Structure

| File | Role | Task |
|---|---|---|
| `Cargo.toml` | Replace serde_yaml → serde_yml dependency | 1 |
| `src/lib.rs` | Add new `config` module to public exports | 6 |
| `src/contract/mod.rs` | Add `SchemaKind::CycleRef` variant, update structural_key/shape_key encoding, `CycleRefTarget` type | 2 |
| `src/openapi/mod.rs` | Cycle detection in SchemaResolver (D-14), tolerant YAML parsing (D-13), external file ref resolution (D-15), OpenAPI 3.1 normalization (D-12), `--ref-root` CLI arg passthrough | 2,3,4,5 |
| `src/openapi/identity.rs` | Unchanged (no new identity logic) | — |
| `src/lockfile/v4/schema.rs` | Handle CycleRef in intern_schema/expand_schema/validate_schema_table | 2 |
| `src/lockfile/v4/mod.rs` | Handle CycleRef in validation | 2 |
| `src/lockfile/v4/canonical.rs` | Handle CycleRef in canonical schema encoding | 2 |
| `src/lockfile/v3/schema.rs` | Handle CycleRef in v3 intern/expand (graceful fallback for v3) | 2 |
| `src/diff/mod.rs` | Handle CycleRef in diff_schema (equal-if-same-target), CycleRef in schema_kind_name, apply config filters | 2,6 |
| `src/cli.rs` | Add `--ref-root`, `--header`, `--config` CLI flags | 4,6,7 |
| `src/main.rs` | Pass new CLI flags to normalizer/verify, load config | 4,6,7 |
| `src/config.rs` | New: .apiwatch.yaml parser and rule engine | 6 |
| `src/remote.rs` | Add auth header injection with env-var resolution | 7 |
| `tools/lock-size-report/Cargo.toml` | Update serde_yaml → serde_yml in workspace | 1 |
| `tools/lock-size-report/src/main.rs` | Update serde_yaml import → serde_yml | 1 |
| `tools/lock-size-report/src/report.rs` | Update serde_yaml import → serde_yml | 1 |
| `tests/cli_diff.rs` | CycleRef regression, 3.1 nullable types, config ignore/severity/fail_on tests | 2,5,6 |
| `tests/cli_lock.rs` | CycleRef lock round-trip, 3.1 lock, external ref lock | 2,4,5 |
| `tests/cli_verify.rs` | CycleRef verify, 3.1 verify, config-filtered verify, auth header verify | 2,5,6,7 |
| `tests/compat.rs` | Updated corpus with new specs, Stripe/DigitalOcean passing | 3,8 |
| `testdata/openapi/phase3_d14_cycle.yaml` | Cycle-breaking regression fixture (Stripe-like recursive schema) | 2 |
| `testdata/openapi/phase3_d13_metadata_old.yaml` | Malformed metadata fixture (map where string expected) | 3 |
| `testdata/openapi/phase3_d13_metadata_new.yaml` | Same spec with one field changed | 3 |
| `testdata/openapi/phase3_d15_api.yaml` | External ref: references schemas.yaml | 4 |
| `testdata/openapi/phase3_d15_schemas.yaml` | External ref: target schemas file | 4 |
| `testdata/openapi/phase3_d12_31_nullable_old.yaml` | OpenAPI 3.1 nullable-type spec (old) | 5 |
| `testdata/openapi/phase3_d12_31_nullable_new.yaml` | OpenAPI 3.1 nullable-type spec (new, type changed) | 5 |
| `testdata/lock/v4_cycle.lock` | Golden v4 lockfile with CycleRef schema | 2 |
| `testdata/lock/v4_31.lock` | Golden v4 lockfile from 3.1 source | 5 |
| `testdata/lock/v4_d15.lock` | Golden v4 lockfile from split spec | 4 |
| `compat/specs.json` | Add 5 new corpus entries, update Stripe/DigitalOcean status | 8 |

---

### Task 1: D-23 — Replace `serde_yaml` with `serde_yml`

**Files:**
- Modify: `Cargo.toml:14`
- Modify: `tools/lock-size-report/Cargo.toml`
- Modify: `tools/lock-size-report/src/main.rs`
- Modify: `tools/lock-size-report/src/report.rs`
- Modify: `src/lockfile/v4/mod.rs:193-199`

**Interfaces:**
- Consumes: (none — this is first, mechanical)
- Produces: `serde_yml::from_reader`, `serde_yml::to_string` — drop-in replacements for `serde_yaml`

#### Steps

- [ ] **Step 1: Update root Cargo.toml dependency**

Replace `serde_yaml = "0.9"` with `serde_yml = "0.0.1"` at `Cargo.toml:14`:

```toml
serde_yml = "0.0.1"
```

- [ ] **Step 2: Update lock-size-report Cargo.toml**

Read `tools/lock-size-report/Cargo.toml`, find the `serde_yaml` line, replace with `serde_yml = "0.0.1"`. Run `cargo check` to verify the dependency resolves (it will fail on import paths until step 3).

- [ ] **Step 3: Update all import paths**

Search the entire workspace for `serde_yaml` with: `rg "serde_yaml" --type rust`. Replace every occurrence:
- `serde_yaml::from_reader` → `serde_yml::from_reader`
- `serde_yaml::to_string` → `serde_yml::to_string`
- `serde_yaml::from_str` → `serde_yml::from_str`
- `serde_yaml::from_slice` → `serde_yml::from_slice`

Expected files (confirm with `rg`):
- `src/lockfile/v4/mod.rs:194` — `serde_yaml::to_string`
- `tools/lock-size-report/src/main.rs` — any serde_yaml imports
- `tools/lock-size-report/src/report.rs` — any serde_yaml imports
- `src/openapi/mod.rs` — if any direct serde_yaml calls
- `tests/*.rs` — if any test files use serde_yaml directly

- [ ] **Step 4: Build and run tests**

Run: `cargo build --workspace`
Expected: success (no compile errors, no serde_yaml references remain)

Run: `cargo test --workspace`
Expected: all 263 tests pass, 5 intentional ignores

- [ ] **Step 5: Verify deterministic lockfile output**

Run the lock-size-report with `--check` to confirm byte-identical output:

```powershell
cargo run -p apiwatch-lock-size-report -- --manifest compat/specs.json --compat-dir .compat-cache --privacy-fixture testdata/openapi/privacy_sentinels.yaml --max-lock-bytes 5242880 --json-out docs/benchmarks/phase-1-lock-size-report.json --markdown-out docs/benchmarks/phase-1-lock-size-report.md --v4-json-out docs/benchmarks/phase-2-v4-lock-size-report.json --v4-markdown-out docs/benchmarks/phase-2-v4-lock-size-report.md --check
```

Expected: no "report differs" error (exit code 0)

Also verify a golden v4 lockfile round-trips correctly:
```powershell
cargo run -- lock testdata/openapi/privacy_sentinels.yaml --name private --output testdata/lock/v4_private.lock
```

Expected: produces byte-identical lockfile (no diff against committed golden)

- [ ] **Step 6: Run strict Clippy and formatting**

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: both pass with zero warnings

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "fix: replace deprecated serde_yaml with serde_yml (D-23)"
```

---

### Task 2: D-14 — Cycle-Breaking References

**Files:**
- Modify: `src/contract/mod.rs:186-308` (Schema struct, SchemaKind enum, structural_key, shape_key, schema_kind_tag)
- Modify: `src/openapi/mod.rs:633-765` (SchemaResolver, resolve method)
- Modify: `src/lockfile/v4/schema.rs:196-264,364-418,419-479` (intern_schema, expand_schema, validate_schema_table)
- Modify: `src/lockfile/v4/mod.rs:533-556` (validation in load path)
- Modify: `src/lockfile/v4/canonical.rs` (canonical schema encoding for CycleRef)
- Modify: `src/lockfile/v3/schema.rs:395` (v3 items handling, CycleRef fallback)
- Modify: `src/diff/mod.rs:514-525,929-942` (diff_schema kind change, schema_kind_name)
- Create: `testdata/openapi/phase3_d14_cycle_old.yaml`
- Create: `testdata/openapi/phase3_d14_cycle_new.yaml`
- Create: `testdata/lock/v4_cycle.lock` (golden, after code working)
- Modify: `tests/cli_diff.rs` (phase2_d14 regression test)
- Modify: `tests/cli_lock.rs` (cycle lock round-trip test)
- Modify: `tests/cli_verify.rs` (cycle verify test)
- Modify: `tests/compat.rs` (if needed for Stripe passing)

**Interfaces:**
- Produces:
  - `SchemaKind::CycleRef { target: Box<Schema> }` — new variant in contract model
  - `CycleRefTarget` — resolved path like `#/cycles/components/schemas/File`
  - Updated `schema_kind_tag` handling for CycleRef

#### Steps

- [ ] **Step 1: Write failing test for cycle-breaking lock**

Create `testdata/openapi/phase3_d14_cycle_old.yaml`:

```yaml
openapi: "3.0.3"
info:
  title: Cycle Test
  version: "1.0.0"
paths:
  /files/{id}:
    get:
      operationId: getFile
      responses:
        "200":
          description: A file
          content:
            application/json:
              schema:
                $ref: "#/components/schemas/File"
components:
  schemas:
    File:
      type: object
      properties:
        id:
          type: string
        parent:
          $ref: "#/components/schemas/File"
```

Create `testdata/openapi/phase3_d14_cycle_new.yaml` with one property added to `File`:

```yaml
openapi: "3.0.3"
info:
  title: Cycle Test
  version: "2.0.0"
paths:
  /files/{id}:
    get:
      operationId: getFile
      responses:
        "200":
          description: A file
          content:
            application/json:
              schema:
                $ref: "#/components/schemas/File"
components:
  schemas:
    File:
      type: object
      properties:
        id:
          type: string
        name:
          type: string
        parent:
          $ref: "#/components/schemas/File"
```

Add regression test in `tests/cli_lock.rs` (at the end of the file, before the closing module boundary):

```rust
#[test]
fn phase2_d14_lock_stores_cycle_breaking_references() {
    let dir = tempfile::tempdir().expect("tempdir");
    let lock = dir.path().join("api.lock");
    Command::cargo_bin("apiwatch")
        .expect("binary")
        .args(&[
            "lock",
            "testdata/openapi/phase3_d14_cycle_old.yaml",
            "--name", "cycle-test",
            "--output", lock.to_str().unwrap(),
            "--max-lock-bytes", "5242880",
        ])
        .assert()
        .success();
    let lock_bytes = std::fs::read_to_string(&lock).expect("read lock");
    assert!(lock_bytes.contains("cycle_ref"), "lockfile should contain cycle_ref schema kind");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test phase2_d14_lock_stores_cycle_breaking_references`
Expected: FAIL with "circular schema reference detected" or assertion failure

- [ ] **Step 3: Add `CycleRef` to contract model**

In `src/contract/mod.rs`, after line 307 (`SchemaKind::Unknown => "unknown"`), add the new variant and change the diff-only equality logic:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Schema {
    pub kind: SchemaKind,
    pub nullable: bool,
    pub format: Option<String>,
    pub enum_values: Vec<String>,
    pub properties: BTreeMap<String, Property>,
    pub items: Option<Box<Schema>>,
    pub additional_properties: AdditionalProperties,
    pub branches: Vec<Schema>,
    pub cycle_target: Option<String>,  // NEW
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SchemaKind {
    Object,
    Array,
    OneOf,
    AllOf,
    AnyOf,
    String,
    Integer,
    Number,
    Boolean,
    Unknown,
    CycleRef,  // NEW
}
```

Update `schema_kind_tag`:

```rust
fn schema_kind_tag(kind: &SchemaKind) -> &'static str {
    match kind {
        SchemaKind::Object => "object",
        SchemaKind::Array => "array",
        SchemaKind::OneOf => "oneOf",
        SchemaKind::AllOf => "allOf",
        SchemaKind::AnyOf => "anyOf",
        SchemaKind::String => "string",
        SchemaKind::Integer => "integer",
        SchemaKind::Number => "number",
        SchemaKind::Boolean => "boolean",
        SchemaKind::Unknown => "unknown",
        SchemaKind::CycleRef => "cycle_ref",  // NEW
    }
}
```

Update `encode_structural` to handle CycleRef — append the cycle_target path:

```rust
fn encode_structural(&self, encoded: &mut String) {
    // ... existing code ...
    match &self.items {
        Some(items) => {
            encode_field(encoded, "items");
            items.encode_structural(encoded);
        }
        None => encode_field(encoded, ""),
    }
    // NEW: encode cycle_target
    match &self.cycle_target {
        Some(target) => encode_field(encoded, target),
        None => encode_field(encoded, ""),
    }
    // ... rest of existing code ...
}
```

Also update `Schema::new()` helper functions if any exist — CycleRef schemas should be created with minimal defaults (kind=CycleRef, all other fields empty/None).

- [ ] **Step 4: Implement cycle detection in SchemaResolver**

In `src/openapi/mod.rs`, modify the `resolve` method (around line 751) to detect cycles:

```rust
fn resolve(&self, reference: &str, visiting: &mut BTreeSet<String>) -> Result<Schema> {
    let name = component_name(reference, "schemas")?;
    if !visiting.insert(name.to_owned()) {
        // Cycle detected — produce CycleRef terminal
        let cycle_path = format!("#/cycles/components/schemas/{name}");
        return Ok(Schema {
            kind: SchemaKind::CycleRef,
            nullable: false,
            format: None,
            enum_values: vec![],
            properties: BTreeMap::new(),
            items: None,
            additional_properties: AdditionalProperties::Forbidden,
            branches: vec![],
            cycle_target: Some(cycle_path),
        });
    }
    let schema = self.schemas.get(&name).ok_or_else(|| anyhow!("schema {} not found", name))?;
    let normalized = self.normalize_schema(schema, visiting)?;
    visiting.remove(&name);
    Ok(normalized)
}
```

The same pattern applies to `resolve_parameter`, `resolve_request_body`, `resolve_response`, `resolve_path_item`, and `resolve_security_scheme` — each needs the `visiting.insert/remove` guard already present; the only change is returning `CycleRef` instead of an error on re-visit.

- [ ] **Step 5: Update v4 lockfile intern/expand for CycleRef**

In `src/lockfile/v4/schema.rs`, `intern_schema` — add CycleRef handling:

```rust
(crate::contract::SchemaKind::CycleRef) => {
    WireSchema {
        kind: crate::contract::SchemaKind::CycleRef,
        nullable: false,
        format: None,
        enum_values: vec![],
        properties: BTreeMap::new(),
        items: None,
        additional_properties: WireAdditionalProperties::Forbidden,
        branches: vec![],
    }
}
```

In `expand_schema`, handle CycleRef:

```rust
crate::contract::SchemaKind::CycleRef => {
    Schema {
        kind: crate::contract::SchemaKind::CycleRef,
        nullable: false,
        format: None,
        enum_values: vec![],
        properties: BTreeMap::new(),
        items: None,
        additional_properties: AdditionalProperties::Forbidden,
        branches: vec![],
        cycle_target: Some("cycle_ref_ignored_on_expand".to_owned()), // or store the target in wire
    }
}
```

Actually — CycleRef should carry its target path through the wire to be useful. Add `cycle_target: Option<String>` to `WireSchema`:

```rust
pub(super) struct WireSchema {
    kind: SchemaKind,
    nullable: bool,
    format: Option<String>,
    enum_values: Vec<String>,
    properties: BTreeMap<String, WireProperty>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    items: Option<String>,
    additional_properties: WireAdditionalProperties,
    #[serde(default)]
    branches: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cycle_target: Option<String>,  // NEW
}
```

In `validate_schema_table` (line 455-456), skip cycle checking for CycleRef schemas (they are terminal leaves, not cycles in the expansion sense):

```rust
if !matches!(wire.kind, crate::contract::SchemaKind::CycleRef) {
    // existing cycle detection code
}
```

- [ ] **Step 6: Update canonical encoding for CycleRef**

In `src/lockfile/v4/canonical.rs`, find where schema IDs are computed. If CycleRef schemas have their target path, encode it. If they're terminals with no contents, assign a fixed canonical ID or derive from the target.

- [ ] **Step 7: Update v3 lockfile for CycleRef (graceful fallback)**

In `src/lockfile/v3/schema.rs`, filter out CycleRef schemas during v3 interning — v3 doesn't support them, so operations that reference recursive schemas should document partial coverage:

```rust
crate::contract::SchemaKind::CycleRef => {
    // v3 cannot represent cycle-breaking references — produce Unknown terminal
    WireSchema {
        kind: crate::contract::SchemaKind::Unknown,
        // ... defaults ...
    }
}
```

- [ ] **Step 8: Update diff engine for CycleRef**

In `src/diff/mod.rs`, modify `diff_schema` kind change check (line 514):

```rust
fn eq_kind(a: &SchemaKind, b: &SchemaKind) -> bool {
    match (a, b) {
        (SchemaKind::CycleRef, SchemaKind::CycleRef) => true,
        (a, b) => a == b,
    }
}
```

Replace `if old.kind != new.kind` with `if !eq_kind(&old.kind, &new.kind)`.

In `schema_kind_name` function (around line 929):

```rust
SchemaKind::CycleRef => "cycle_ref",
```

- [ ] **Step 9: Run the test**

Run: `cargo test phase2_d14_lock_stores_cycle_breaking_references`
Expected: PASS — lockfile contains `cycle_ref`

Add more regression tests:

```rust
#[test]
fn phase2_d14_cycle_diff_detects_property_addition() {
    Command::cargo_bin("apiwatch")
        .expect("binary")
        .args(&["diff",
            "testdata/openapi/phase3_d14_cycle_old.yaml",
            "testdata/openapi/phase3_d14_cycle_new.yaml",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("name").and(predicate::str::contains("added")));
}

#[test]
fn phase2_d14_cycle_diff_equal_specs_produces_no_changes() {
    Command::cargo_bin("apiwatch")
        .expect("binary")
        .args(&["diff",
            "testdata/openapi/phase3_d14_cycle_old.yaml",
            "testdata/openapi/phase3_d14_cycle_old.yaml",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("0 breaking").and(
            predicate::str::contains("0 non-breaking")
        ));
}
```

- [ ] **Step 10: Full test suite**

Run: `cargo test --workspace`
Expected: all tests pass, Stripe compat test transitions from `known_failing` to `passing` (update compat/specs.json status if this is the case — deferred to Task 3/8 if needed)

- [ ] **Step 11: Clippy and formatting**

```powershell
cargo fmt --check; cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 12: Commit**

```bash
git add -A
git commit -m "feat: add cycle-breaking schema references (D-14)"
```

---

### Task 3: D-13 — Malformed Metadata Tolerance

**Files:**
- Modify: `src/openapi/mod.rs:23-58` (load_contract, tolerant parsing layer)
- Create: `testdata/openapi/phase3_d13_metadata_old.yaml`
- Create: `testdata/openapi/phase3_d13_metadata_new.yaml`
- Modify: `tests/cli_diff.rs` (D-13 regression)
- Modify: `tests/compat.rs` or `compat/specs.json` (DigitalOcean status update)

**Interfaces:**
- Consumes: `(none — stands alone)`
- Produces: tolerant YAML-to-OpenAPI parsing that skips non-consumed metadata fields

#### Steps

- [ ] **Step 1: Write failing test**

Create `testdata/openapi/phase3_d13_metadata_old.yaml` — a valid spec with malformed tag description (map instead of string, matching DigitalOcean's actual bug):

```yaml
openapi: "3.0.3"
info:
  title: Metadata Test
  version: "1.0.0"
tags:
  - name: test
    description:
      en: "A test tag"
      fr: "Un tag de test"
paths:
  /items:
    get:
      operationId: listItems
      tags:
        - test
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

Create `testdata/openapi/phase3_d13_metadata_new.yaml` — same spec with one property added:

```yaml
openapi: "3.0.3"
info:
  title: Metadata Test
  version: "2.0.0"
tags:
  - name: test
    description:
      en: "A test tag"
      fr: "Un tag de test"
paths:
  /items:
    get:
      operationId: listItems
      tags:
        - test
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
                  name:
                    type: string
```

Add regression test in `tests/cli_diff.rs`:

```rust
#[test]
fn phase2_d13_tolerates_malformed_tag_metadata() {
    Command::cargo_bin("apiwatch")
        .expect("binary")
        .args(&["diff",
            "testdata/openapi/phase3_d13_metadata_old.yaml",
            "testdata/openapi/phase3_d13_metadata_new.yaml",
        ])
        .assert()
        .success();
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test phase2_d13_tolerates_malformed_tag_metadata`
Expected: FAIL with "invalid type: map, expected a string" (from openapiv3 parser)

- [ ] **Step 3: Implement tolerant YAML parsing**

In `src/openapi/mod.rs`, modify `load_contract` / `load_contract_text` to strip malformed metadata before passing to `openapiv3`:

Add a new function `tolerant_openapi_yaml` that:
1. Parses the raw bytes into `serde_yml::Value`
2. Walks the value tree and removes fields the normalizer does not consume
3. Re-serializes to bytes
4. Passes cleaned bytes to `openapiv3::OpenAPI::deserialize`

```rust
fn tolerant_openapi_yaml(bytes: &[u8]) -> Result<OpenAPI> {
    let mut value: serde_yml::Value = serde_yml::from_slice(bytes)
        .context("failed to parse OpenAPI document as YAML")?;

    // Strip malformed metadata from tags
    if let Some(tags) = value.get_mut("tags") {
        if let Some(tags) = tags.as_sequence_mut() {
            for tag in tags {
                if let Some(tag) = tag.as_mapping_mut() {
                    tag.remove("description");
                    tag.remove("externalDocs");
                    tag.remove("x-");
                }
            }
        }
    }

    // Strip external docs from info, operations, schemas
    strip_deep(&mut value, "externalDocs");
    strip_deep(&mut value, "example");
    strip_deep(&mut value, "examples");
    strip_deep(&mut value, "callbacks");
    strip_deep_by_prefix(&mut value, "x-");  // vendor extensions

    let cleaned = serde_yml::to_string(&value)
        .context("failed to re-serialize cleaned OpenAPI document")?;

    let document: OpenAPI = serde_yml::from_str(&cleaned)
        .context("failed to parse cleaned OpenAPI 3.0 document")?;

    Ok(document)
}

fn strip_deep(value: &mut serde_yml::Value, key: &str) {
    if let Some(mapping) = value.as_mapping_mut() {
        mapping.remove(&serde_yml::Value::String(key.to_owned()));
        for (_, v) in mapping.iter_mut() {
            strip_deep(v, key);
        }
    } else if let Some(seq) = value.as_sequence_mut() {
        for item in seq {
            strip_deep(item, key);
        }
    }
}

fn strip_deep_by_prefix(value: &mut serde_yml::Value, prefix: &str) {
    if let Some(mapping) = value.as_mapping_mut() {
        let keys_to_remove: Vec<String> = mapping.keys()
            .filter_map(|k| k.as_str().map(String::from))
            .filter(|k| k.starts_with(prefix))
            .collect();
        for key in keys_to_remove {
            mapping.remove(&serde_yml::Value::String(key));
        }
        for (_, v) in mapping.iter_mut() {
            strip_deep_by_prefix(v, prefix);
        }
    } else if let Some(seq) = value.as_sequence_mut() {
        for item in seq {
            strip_deep_by_prefix(item, prefix);
        }
    }
}
```

Call this from `load_contract` instead of the direct `serde_yml::from_reader` → `OpenAPI` path. The YAML/JSON parsing path should be:
- Try `tolerant_openapi_yaml` first
- If that fails with a consumed-field error, propagate it
- If it succeeds, pass to the existing normalization pipeline

- [ ] **Step 4: Run the test**

Run: `cargo test phase2_d13_tolerates_malformed_tag_metadata`
Expected: PASS — diff succeeds without parse error

- [ ] **Step 5: Verify DigitalOcean spec now normalizes**

The DigitalOcean spec in the compatibility corpus should now pass. Verify by running:

```powershell
cargo run -p apiwatch-lock-size-report -- --manifest compat/specs.json --compat-dir .compat-cache --privacy-fixture testdata/openapi/privacy_sentinels.yaml --max-lock-bytes 5242880 --json-out docs/benchmarks/phase-1-lock-size-report.json --markdown-out docs/benchmarks/phase-1-lock-size-report.md --v4-json-out docs/benchmarks/phase-2-v4-lock-size-report.json --v4-markdown-out docs/benchmarks/phase-2-v4-lock-size-report.md
```

If DigitalOcean no longer fails, update `compat/specs.json` entry from `"status": "known_failing"` to `"status": "passing"` and remove `expected_error`.

- [ ] **Step 6: Full test suite**

Run: `cargo test --workspace`

- [ ] **Step 7: Clippy and formatting**

```powershell
cargo fmt --check; cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat: tolerate malformed metadata in OpenAPI documents (D-13)"
```

---

### Task 4: D-15 — External `$ref` Resolution (File Only)

**Files:**
- Modify: `src/openapi/mod.rs:633-800` (SchemaResolver — add external file loading)
- Modify: `src/cli.rs` (add `--ref-root` flag)
- Modify: `src/main.rs` (pass ref_root to normalizer)
- Create: `testdata/openapi/phase3_d15_api.yaml`
- Create: `testdata/openapi/phase3_d15_schemas.yaml`
- Create: `testdata/lock/v4_d15.lock` (golden, after code working)
- Modify: `tests/cli_lock.rs` (external ref lock test)
- Modify: `tests/cli_diff.rs` (external ref diff test)

**Interfaces:**
- Consumes: `(none — stands alone)`
- Produces: `load_external_ref_file(base: &Path, ref_path: &str) -> Result<Value>` — new function on SchemaResolver or as a module-level helper

#### Steps

- [ ] **Step 1: Write failing test**

Create `testdata/openapi/phase3_d15_schemas.yaml`:

```yaml
openapi: "3.0.3"
info:
  title: Shared Schemas
  version: "1.0.0"
paths: {}
components:
  schemas:
    User:
      type: object
      properties:
        id:
          type: integer
        name:
          type: string
```

Create `testdata/openapi/phase3_d15_api.yaml`:

```yaml
openapi: "3.0.3"
info:
  title: API
  version: "1.0.0"
paths:
  /users/{id}:
    get:
      operationId: getUser
      responses:
        "200":
          description: A user
          content:
            application/json:
              schema:
                $ref: "phase3_d15_schemas.yaml#/components/schemas/User"
```

Add regression test in `tests/cli_lock.rs`:

```rust
#[test]
fn phase2_d15_lock_resolves_external_file_refs() {
    let dir = tempfile::tempdir().expect("tempdir");
    let lock = dir.path().join("api.lock");
    Command::cargo_bin("apiwatch")
        .expect("binary")
        .args(&[
            "lock",
            "testdata/openapi/phase3_d15_api.yaml",
            "--name", "split-spec",
            "--output", lock.to_str().unwrap(),
            "--max-lock-bytes", "5242880",
        ])
        .assert()
        .success();
    let lock_bytes = std::fs::read_to_string(&lock).expect("read lock");
    assert!(lock_bytes.contains("User"), "lockfile should contain User schema");
    assert!(lock_bytes.contains("id"), "lockfile should contain id property");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test phase2_d15_lock_resolves_external_file_refs`
Expected: FAIL with schema resolution error (external ref unresolved)

- [ ] **Step 3: Implement external file ref resolution**

In `src/openapi/mod.rs`, modify `SchemaResolver` to accept an optional `ref_root: Option<PathBuf>`. Add a method to load an external file:

```rust
struct SchemaResolver {
    parameters: BTreeMap<String, Parameter>,
    request_bodies: BTreeMap<String, RequestBody>,
    responses: BTreeMap<String, Response>,
    schemas: BTreeMap<String, openapiv3::Schema>,
    path_items: BTreeMap<String, openapiv3::PathItem>,
    ref_root: Option<PathBuf>,  // NEW
    loaded_files: BTreeMap<PathBuf, openapiv3::OpenAPI>,  // NEW: cache
}

impl SchemaResolver {
    fn load_external_schema(&mut self, ref_path: &str, pointer: &str) -> Result<Schema> {
        let (file_path, fragment) = ref_path.split_once('#').unwrap_or((ref_path, ""));
        let base = self.ref_root.as_deref().unwrap_or(/* source spec parent dir */);
        let full_path = base.join(file_path);

        // Reject path traversal
        let canonical = full_path.canonicalize()
            .context("external ref file not found")?;
        let canonical_base = base.canonicalize().unwrap_or(base.to_owned());
        if !canonical.starts_with(&canonical_base) {
            return Err(anyhow!("external ref escapes the resolution root"));
        }

        // Reject remote URLs
        if ref_path.starts_with("https://") || ref_path.starts_with("http://") {
            return Err(anyhow!("remote references are not yet supported; use --ref-root with pre-downloaded files"));
        }

        // Load and cache the external file
        let external: openapiv3::OpenAPI = self.loaded_files
            .entry(canonical.clone())
            .or_insert_with(|| {
                let bytes = std::fs::read_to_string(&canonical).expect("read external file");
                serde_yml::from_str(&bytes).expect("parse external file")
            })
            .clone();

        // Resolve the pointer within the external file
        // ... lookup in external.components.schemas ...
    }
}
```

Modify the `resolve` method to detect external refs: if `reference` starts with `./` or `../` or contains a non-`#` path, route to `load_external_schema`. If it starts with `http://` or `https://`, return the unsupported error.

- [ ] **Step 4: Add `--ref-root` CLI flag**

In `src/cli.rs`, add to `LockArgs` and `DiffArgs`:

```rust
#[arg(long, value_hint = ValueHint::DirPath)]
pub ref_root: Option<PathBuf>,
```

In `src/main.rs`, pass `ref_root` to `load_contract` / `normalize` calls.

The normalizer function signature changes to:

```rust
fn normalize(document: OpenAPI, ref_root: Option<&Path>) -> Result<ApiContract>
```

And `SchemaResolver::new` takes `ref_root` and the source spec's directory for building relative paths.

- [ ] **Step 5: Path traversal rejection test**

Add a test that verifies `../` escapes are rejected:

```rust
#[test]
fn phase2_d15_rejects_path_traversal() {
    // Create a ref that tries to escape testdata/
    // Expected: error with "escapes the resolution root"
}
```

- [ ] **Step 6: Run the test**

Run: `cargo test phase2_d15`
Expected: PASS for resolution test, PASS for rejection test

- [ ] **Step 7: Full test suite**

Run: `cargo test --workspace`

- [ ] **Step 8: Clippy and formatting**

```powershell
cargo fmt --check; cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat: resolve external file $ref targets with traversal protection (D-15)"
```

---

### Task 5: D-12 — OpenAPI 3.1 Support

**Files:**
- Modify: `src/openapi/mod.rs:102-118` (remove version rejection, add 3.1→3.0 compat)
- Modify: `src/openapi/mod.rs:23-58` (load_contract — route 3.1 docs through compat layer)
- Create: `testdata/openapi/phase3_d12_31_nullable_old.yaml`
- Create: `testdata/openapi/phase3_d12_31_nullable_new.yaml`
- Create: `testdata/lock/v4_31.lock` (golden)
- Modify: `tests/cli_lock.rs` (3.1 lock test)
- Modify: `tests/cli_diff.rs` (3.1 nullable type diff test)
- Modify: `tests/cli_verify.rs` (3.1 verify test)

**Interfaces:**
- Consumes: D-13 tolerant parsing, D-14 cycle-breaking, D-15 external refs (all foundations in place)
- Produces: `normalize_openapi31(value: &Value) -> Result<OpenAPI>` — converts 3.1 YAML value to 3.0-compatible OpenAPI struct

#### Steps

- [ ] **Step 1: Write failing tests**

Create `testdata/openapi/phase3_d12_31_nullable_old.yaml`:

```yaml
openapi: "3.1.0"
info:
  title: 3.1 Nullable Test
  version: "1.0.0"
paths:
  /items:
    get:
      operationId: listItems
      responses:
        "200":
          description: OK
          content:
            application/json:
              schema:
                type: object
                properties:
                  id:
                    type: integer
                  label:
                    type: ["string", "null"]
```

Create `testdata/openapi/phase3_d12_31_nullable_new.yaml`:

```yaml
openapi: "3.1.0"
info:
  title: 3.1 Nullable Test
  version: "2.0.0"
paths:
  /items:
    get:
      operationId: listItems
      responses:
        "200":
          description: OK
          content:
            application/json:
              schema:
                type: object
                properties:
                  id:
                    type: integer
                  label:
                    type: ["string", "null"]
                  count:
                    type: integer
```

Add regression tests in `tests/cli_diff.rs`:

```rust
#[test]
fn phase2_d12_openapi_31_rejected_before_fix_negative() {
    Command::cargo_bin("apiwatch")
        .expect("binary")
        .args(&["diff",
            "testdata/openapi/phase3_d12_31_nullable_old.yaml",
            "testdata/openapi/phase3_d12_31_nullable_old.yaml",
        ])
        .assert()
        .failure();
}

#[test]
fn phase2_d12_31_nullable_type_diff_detects_property_addition() {
    Command::cargo_bin("apiwatch")
        .expect("binary")
        .args(&["diff",
            "testdata/openapi/phase3_d12_31_nullable_old.yaml",
            "testdata/openapi/phase3_d12_31_nullable_new.yaml",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("count").and(predicate::str::contains("added")));
}

#[test]
fn phase2_d12_31_lock_creates_deterministic_v4_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let lock = dir.path().join("api.lock");
    Command::cargo_bin("apiwatch")
        .expect("binary")
        .args(&[
            "lock",
            "testdata/openapi/phase3_d12_31_nullable_old.yaml",
            "--name", "31-test",
            "--output", lock.to_str().unwrap(),
            "--max-lock-bytes", "5242880",
        ])
        .assert()
        .success();
    let lock_bytes = std::fs::read_to_string(&lock).expect("read lock");
    assert!(lock_bytes.contains("version: 4"));
}
```

- [ ] **Step 2: Run negative test to verify it FAILS as expected**

Run: `cargo test phase2_d12_openapi_31_rejected_before_fix_negative`
Expected: PASS (currently rejected — this is the negative that should fail after the fix)

Wait — this is a NEGATIVE test. Initially the diff SHOULD fail (rejection). After the fix, the rejection test should be removed and the positive tests should pass.

Let me restructure:

Run: `cargo test phase2_d12_31_nullable_type_diff`
Expected: FAIL with "OpenAPI 3.1 is not yet supported"

- [ ] **Step 3: Remove version guard and add 3.1→3.0 compat layer**

In `src/openapi/mod.rs`, modify `validate_openapi_version` (line 102-118):

```rust
fn validate_openapi_version(version: Option<&str>) -> Result<()> {
    let Some(version) = version else {
        return Ok(());
    };
    if version == "3.0" || version.starts_with("3.0.") {
        return Ok(());
    }
    if version == "3.1" || version.starts_with("3.1.") {
        return Ok(());  // Now supported
    }
    Err(anyhow!(
        "unsupported OpenAPI version {version}; expected OpenAPI 3.0 or 3.1"
    ))
}
```

Add a `normalize_openapi31` function that converts 3.1-specific constructs to 3.0 equivalents:

```rust
fn normalize_openapi31_to_30(value: &mut serde_yml::Value) -> Result<()> {
    // 1. Convert type: ["string", "null"] → type: "string", nullable: true
    normalize_nullable_types(value);

    // 2. Keep exclusiveMinimum/exclusiveMaximum as numbers (already supported)
    // 3. Convert prefixItems → items (linearize)
    normalize_prefix_items(value);

    // 4. Convert $defs → components/schemas
    normalize_defs_to_components(value);

    // 5. Convert bool schemas
    normalize_bool_schemas(value);

    Ok(())
}

fn normalize_nullable_types(value: &mut serde_yml::Value) {
    if let Some(mapping) = value.as_mapping_mut() {
        if let Some(type_val) = mapping.get_mut("type") {
            if let Some(type_arr) = type_val.as_sequence() {
                let mut types: Vec<String> = type_arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                if let Some(null_idx) = types.iter().position(|t| t == "null") {
                    types.remove(null_idx);
                    if types.len() == 1 {
                        mapping.insert(
                            "type".into(),
                            serde_yml::Value::String(types.remove(0)),
                        );
                        mapping.insert(
                            "nullable".into(),
                            serde_yml::Value::Bool(true),
                        );
                    }
                }
            }
        }
        // Recurse into nested schemas
        for key in &["properties", "items", "additionalProperties"] {
            if let Some(child) = mapping.get_mut(*key) {
                normalize_nullable_types(child);
            }
        }
        // Recurse into allOf/oneOf/anyOf
        for key in &["allOf", "oneOf", "anyOf"] {
            if let Some(branches) = mapping.get_mut(*key) {
                if let Some(seq) = branches.as_sequence_mut() {
                    for item in seq {
                        normalize_nullable_types(item);
                    }
                }
            }
        }
    }
}
```

Similar recursive tree-walk functions for `normalize_prefix_items`, `normalize_defs_to_components`, `normalize_bool_schemas`.

In `load_contract`, detect OpenAPI version early and apply the 3.1 compat transform:

```rust
fn load_contract_input(input: &ContractInput, ref_root: Option<&Path>) -> Result<ApiContract> {
    let bytes = /* read bytes */;
    let mut value: serde_yml::Value = serde_yml::from_slice(&bytes)?;

    // Detect and normalize 3.1
    let is_31 = value.get("openapi")
        .and_then(|v| v.as_str())
        .map(|v| v.starts_with("3.1"))
        .unwrap_or(false);

    if is_31 {
        normalize_openapi31_to_30(&mut value)?;
    }

    // Continue with tolerant parsing (handles 3.0 + normalized 3.1)
    let cleaned = serde_yml::to_string(&value)?;
    let document: OpenAPI = serde_yml::from_str(&cleaned)?;
    ensure_openapi_3(&document)?;
    normalize(document, ref_root)
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test phase2_d12_31`
Expected: all PASS — lock creates v4 file, diff detects property addition, same-spec diff produces 0 changes

- [ ] **Step 5: Additional 3.1 feature tests**

Add tests for:
- `exclusiveMinimum` as number in 3.1
- `prefixItems` normalization to items
- `$defs` folding to components
- Bool schemas (`true` → any type, `false` → nothing)
- Webhooks normalization (stored as pseudo-operations in contract)

- [ ] **Step 6: Full test suite**

Run: `cargo test --workspace`

- [ ] **Step 7: Clippy and formatting**

```powershell
cargo fmt --check; cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat: support OpenAPI 3.1 with nullable types, prefixItems, and $defs (D-12)"
```

---

### Task 6: `.apiwatch.yaml` Configuration

**Files:**
- Create: `src/config.rs` (config parser, discovery, rule engine)
- Modify: `src/cli.rs` (add `--config` flag)
- Modify: `src/diff/mod.rs` (apply ignore rules and severity overrides to change list)
- Modify: `src/main.rs` (load config, apply to verify flow)
- Modify: `src/lib.rs` (add `pub mod config`)
- Create: `testdata/config/basic.yaml` (test config fixture)
- Create: `testdata/config/empty.yaml` (empty config fixture)
- Modify: `tests/cli_diff.rs` (config ignore/severity/fail_on tests)
- Modify: `tests/cli_verify.rs` (config-filtered verify tests)

**Interfaces:**
- Consumes: diff engine change types, severity enum
- Produces:
  - `Config { ignore: Vec<IgnoreRule>, severity: Vec<SeverityOverride>, fail_on: FailOnThresholds }`
  - `Config::discover(lock_path: &Path) -> Result<Config>`
  - `apply_config(changes: Vec<Change>, config: &Config) -> Vec<Change>`

#### Steps

- [ ] **Step 1: Write failing test**

Create `testdata/config/basic.yaml`:

```yaml
ignore:
  - rule: "parameter-removed"
    path: "/deprecated/*"
severity:
  - change: "endpoint-added"
    severity: "warning"
fail_on:
  breaking: 0
  warning: 10
```

Build a test spec with a deprecated endpoint and an added endpoint, verify config alters output:

```rust
#[test]
fn phase3_config_ignore_rule_filters_changes() {
    // Lock a spec, modify it, verify with config that ignores the change
    // Expected: change count reduced from N to N-ignored
}

#[test]
fn phase3_config_severity_override_promotes_warning() {
    // Expected: endpoint-added appears as Warning instead of NonBreaking
}
```

For this task, since config is a new module with no existing patterns, the tests should use a simpler structure:

Create `testdata/openapi/phase3_config_base.yaml`:

```yaml
openapi: "3.0.3"
info:
  title: Config Test
  version: "1.0.0"
paths:
  /deprecated/old:
    get:
      operationId: oldEndpoint
      parameters:
        - name: removed_param
          in: query
          schema:
            type: string
      responses:
        "200":
          description: OK
  /stable:
    get:
      operationId: stableEndpoint
      responses:
        "200":
          description: OK
```

Create `testdata/openapi/phase3_config_changed.yaml` — removes `/deprecated/old` `removed_param`, adds `/new` endpoint:

```yaml
openapi: "3.0.3"
info:
  title: Config Test
  version: "2.0.0"
paths:
  /deprecated/old:
    get:
      operationId: oldEndpoint
      parameters: []
      responses:
        "200":
          description: OK
  /stable:
    get:
      operationId: stableEndpoint
      responses:
        "200":
          description: OK
  /new:
    get:
      operationId: newEndpoint
      responses:
        "200":
          description: OK
```

Add test:

```rust
#[test]
fn phase3_config_ignore_parameter_removed_on_deprecated_path() {
    // Without config: diff reports parameter-removed as Breaking + endpoint-added as NonBreaking
    // With config: parameter-removed on /deprecated/* is ignored -> only endpoint-added remains
}
```

- [ ] **Step 2: Run test to verify it fails**

```powershell
cargo test phase3_config
```
Expected: FAIL (config module doesn't exist)

- [ ] **Step 3: Create config module**

Create `src/config.rs`:

```rust
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use anyhow::{anyhow, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub ignore: Vec<IgnoreRule>,
    #[serde(default)]
    pub severity: Vec<SeverityOverride>,
    #[serde(default)]
    pub fail_on: FailOnThresholds,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IgnoreRule {
    pub rule: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub method: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeverityOverride {
    pub change: String,
    pub severity: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FailOnThresholds {
    #[serde(default = "default_breaking")]
    pub breaking: usize,
    #[serde(default = "default_max")]
    pub warning: usize,
}

fn default_breaking() -> usize { 0 }
fn default_max() -> usize { usize::MAX }

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read_to_string(path)
            .context("failed to read config file")?;
        let config: Config = serde_yml::from_str(&bytes)
            .context("failed to parse config file")?;
        Ok(config)
    }

    pub fn discover(lock_path: &Path) -> Result<Self> {
        let mut dir = lock_path.parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();
        loop {
            let candidate = dir.join(".apiwatch.yaml");
            if candidate.exists() {
                return Self::load(&candidate);
            }
            if !dir.pop() {
                break;
            }
        }
        Ok(Config::default())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ignore: vec![],
            severity: vec![],
            fail_on: FailOnThresholds {
                breaking: 0,
                warning: usize::MAX,
            },
        }
    }
}
```

Add `pub mod config;` to `src/lib.rs` at line 21.

- [ ] **Step 4: Implement change filtering**

In `src/diff/mod.rs` or a new `src/config.rs` function, apply ignore rules after diff:

```rust
pub fn apply_config(changes: &mut Vec<Change>, config: &Config) {
    // 1. Apply ignore rules
    changes.retain(|change| !is_ignored(change, &config.ignore));

    // 2. Apply severity overrides
    for change in changes.iter_mut() {
        if let Some(severity) = find_override(&change.message, &config.severity) {
            // Override severity
        }
    }
}

fn is_ignored(change: &Change, rules: &[IgnoreRule]) -> bool {
    rules.iter().any(|rule| {
        let rule_matches = change.message.contains(&rule.rule)
            || matches_category(change, &rule.rule);  // map change to rule category
        let path_matches = match &rule.path {
            Some(pattern) => glob_matches(pattern, &change.operation.path),
            None => true,
        };
        rule_matches && path_matches
    })
}
```

Since no glob library is available, implement a simple glob match: `*` matches any sequence, `{param}` matches any single path segment.

```rust
fn glob_matches(pattern: &str, path: &str) -> bool {
    let pattern_segments: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    let path_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if pattern_segments.len() != path_segments.len() && !pattern.contains("**") {
        return false;
    }
    for (pat, seg) in pattern_segments.iter().zip(path_segments.iter()) {
        if *pat == "*" { continue; }
        if *pat == "**" { return true; }
        if pat.starts_with('{') && pat.ends_with('}') { continue; }  // template param
        if pat != seg { return false; }
    }
    pattern_segments.len() == path_segments.len()
}
```

- [ ] **Step 5: Run the tests**

Run: `cargo test phase3_config`
Expected: PASS

- [ ] **Step 6: Wire config into verify flow**

In `src/main.rs`, load config before the verify call:

```rust
Command::Verify { ... } => {
    let config_path = config_flag
        .as_deref()
        .or_else(|| Some(&lock_path));
    let config = config::Config::discover(config_path.unwrap_or(Path::new(".")))
        .unwrap_or_default();

    // ... existing verify logic ...

    let mut changes = diff::diff_contracts(locked, &current);
    config::apply_config(&mut changes, &config);

    // Apply fail_on thresholds
    let breaking_count = changes.iter().filter(|c| c.severity == diff::Severity::Breaking).count();
    let warning_count = changes.iter().filter(|c| c.severity == diff::Severity::Warning).count();
    if breaking_count > config.fail_on.breaking || warning_count > config.fail_on.warning {
        // exit with appropriate code
    }
}
```

Add `--config` flag to CLI in `src/cli.rs`:

```rust
#[arg(long, value_hint = ValueHint::FilePath)]
pub config: Option<PathBuf>,
```

- [ ] **Step 7: Full test suite**

Run: `cargo test --workspace`

- [ ] **Step 8: Clippy and formatting**

```powershell
cargo fmt --check; cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat: add .apiwatch.yaml configuration with ignore, severity, and fail_on"
```

---

### Task 7: Remote Authentication Headers

**Files:**
- Modify: `src/config.rs` (remote.headers section)
- Modify: `src/cli.rs` (add `--header` flag)
- Modify: `src/remote.rs` (inject auth headers into reqwest client)
- Modify: `src/main.rs` (resolve env vars, pass headers to remote fetch)
- Modify: `tests/cli_verify.rs` (auth header test — uses mock or env var)

**Interfaces:**
- Consumes: Config struct, remote fetch functions
- Produces:
  - `ResolvedHeaders = BTreeMap<String, String>` (header name → resolved value)
  - `resolve_headers(config: &Config, extra: &[String]) -> Result<ResolvedHeaders>`

#### Steps

- [ ] **Step 1: Write failing test**

Since we can't easily test against a real authenticated API in offline tests, write a unit test for env-var resolution:

Create a test function in `tests/cli_verify.rs`:

```rust
#[test]
fn phase3_headers_rejects_raw_value_in_config() {
    // Create temp .apiwatch.yaml with raw header value
    // Expected: config parse error
}
```

And a test for env-var resolution:

```rust
#[test]
fn phase3_headers_resolves_env_vars() {
    std::env::set_var("TEST_API_KEY", "secret123");
    // Verify that ${TEST_API_KEY} resolves correctly
    // Expected: resolved = "secret123"
    std::env::remove_var("TEST_API_KEY");
}
```

Add a test in `tests/cli_verify.rs` for the verify-with-headers flow (using a mock or env):

```rust
#[test]
fn phase3_verify_with_env_var_auth_header() {
    // Set env var, run verify against a known spec with headers in config
    // Verify the fetch succeeds (exit 0 or exit 1 for drift, not exit 2 for fetch failure)
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test phase3_headers`
Expected: FAIL (no header support)

- [ ] **Step 3: Add remote headers to config model**

In `src/config.rs`, add to the `Config` struct:

```rust
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    // ... existing fields ...
    #[serde(default)]
    pub remote: RemoteConfig,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct RemoteConfig {
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
}
```

Add header resolution:

```rust
pub fn resolve_headers(
    config_headers: &BTreeMap<String, String>,
    cli_headers: &[String],
) -> Result<BTreeMap<String, String>> {
    let mut resolved = BTreeMap::new();

    for (name, value_template) in config_headers {
        let value = resolve_env(value_template)?;
        resolved.insert(name.clone(), value);
    }

    for raw in cli_headers {
        let (name, value_template) = raw.split_once(':')
            .ok_or_else(|| anyhow!("invalid header format: {raw} (expected Name: ${ENV_VAR})"))?;
        let value = resolve_env(value_template.trim())?;
        resolved.insert(name.trim().to_owned(), value);
    }

    Ok(resolved)
}

fn resolve_env(template: &str) -> Result<String> {
    let template = template.trim();
    if !template.starts_with("${") || !template.ends_with('}') {
        return Err(anyhow!("header value must be an env-var reference like ${{NAME}}, got: {template}"));
    }
    let var_name = &template[2..template.len()-1];
    std::env::var(var_name)
        .context(format!("environment variable {var_name} is not set for header value"))
}
```

- [ ] **Step 4: Add `--header` CLI flag**

In `src/cli.rs`, add to the Verify command:

```rust
#[arg(long = "header", value_name = "NAME:${ENV_VAR}", value_hint = ValueHint::Other)]
pub header: Vec<String>,
```

- [ ] **Step 5: Wire headers into remote fetch**

In `src/remote.rs`, modify the fetch function to accept headers:

```rust
pub fn fetch_spec(url: &str, headers: Option<&BTreeMap<String, String>>) -> Result<Vec<u8>> {
    let mut request = reqwest::blocking::Client::new().get(url);
    if let Some(headers) = headers {
        for (name, value) in headers {
            request = request.header(name.as_str(), value.as_str());
        }
    }
    let response = request.send().context("failed to fetch remote spec")?;
    // ... rest of existing fetch logic ...
}
```

In `src/main.rs`, resolve headers before remote fetch:

```rust
let resolved_headers = if !config.remote.headers.is_empty() || !header_args.is_empty() {
    Some(config::resolve_headers(&config.remote.headers, &header_args)?)
} else {
    None
};
```

Pass `resolved_headers.as_ref()` to the remote fetch call.

**Critical:** The header values must never appear in lockfiles, diagnostics, or logs. The `remote.rs` code already reads response bytes into memory — add explicit zeroing of the header variables after use (or drop them before any logging boundary):

```rust
// After fetch completes, drop headers before any logging
drop(resolved_headers);
```

- [ ] **Step 6: Run the tests**

Run: `cargo test phase3_headers`
Expected: PASS

- [ ] **Step 7: Full test suite**

Run: `cargo test --workspace`

- [ ] **Step 8: Clippy and formatting**

```powershell
cargo fmt --check; cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat: support env-var-based remote authentication headers"
```

---

### Task 8: Global Compatibility Corpus Expansion

**Files:**
- Modify: `compat/specs.json` (add 5 new entries, update Stripe/DigitalOcean)
- Modify: `tests/compat.rs` (update expected corpus count)

**Interfaces:**
- Consumes: lock-size-report manifest format
- Produces: updated `compat/specs.json` with 10 entries

#### Steps

- [ ] **Step 1: Update DigitalOcean status**

Since D-13 now tolerates malformed tag descriptions, update DigitalOcean:

In `compat/specs.json`, change `digitalocean` from `"known_failing"` to `"passing"` and remove `expected_error`. Add `phase1_measurement` populated from the next lock-size-report run.

- [ ] **Step 2: Update Stripe status**

Since D-14 now handles cycle-breaking references, update Stripe:

In `compat/specs.json`, change `stripe` from `"known_failing"` to `"passing"` and remove `expected_error`. Add `phase1_measurement` populated from the next lock-size-report run.

- [ ] **Step 3: Add 5 new corpus entries**

After locating commit-pinned URLs, add entries like:

```json
{
  "name": "fhir-r4",
  "file": "fhir-r4.json",
  "url": "<commit-pinned raw URL from github.com/HL7/fhir>",
  "sha256": "<computed after first fetch>",
  "max_bytes": 52428800,
  "status": "passing"
},
{
  "name": "deutsche-bahn",
  "file": "deutsche-bahn.yaml",
  "url": "<commit-pinned raw URL from developer.deutschebahn.com>",
  "sha256": "<computed after first fetch>",
  "max_bytes": 52428800,
  "status": "passing"
},
{
  "name": "mercado-libre",
  "file": "mercado-libre.yaml",
  "url": "<commit-pinned raw URL from github.com/mercadolibre>",
  "sha256": "<computed after first fetch>",
  "max_bytes": 52428800,
  "status": "passing"
},
{
  "name": "japan-digital-agency",
  "file": "japan-digital-agency.yaml",
  "url": "<commit-pinned raw URL from github.com/digital-go-jp>",
  "sha256": "<computed after first fetch>",
  "max_bytes": 52428800,
  "status": "passing"
},
{
  "name": "paystack",
  "file": "paystack.yaml",
  "url": "<commit-pinned raw URL from github.com/PaystackHQ>",
  "sha256": "<computed after first fetch>",
  "max_bytes": 52428800,
  "status": "passing"
}
```

Each entry needs a real, commit-pinned raw GitHub URL for an OpenAPI specification. After the first download, the SHA-256 and measurements will be populated by the lock-size-report tool. Some entries may become `known_failing` if they exercise edge cases not yet handled — document with `expected_error`.

- [ ] **Step 4: Fetch and measure all new specs**

Run the lock-size-report to download and measure the expanded corpus:

```powershell
cargo run -p apiwatch-lock-size-report -- --manifest compat/specs.json --compat-dir .compat-cache --privacy-fixture testdata/openapi/privacy_sentinels.yaml --max-lock-bytes 5242880
```

After the run:
- Verify SHA-256 hashes match
- Verify phase1_measurements are populated
- Verify any `known_failing` entries have `expected_error` populated
- Update `compat/specs.json` with computed hashes and measurements

- [ ] **Step 5: Verify all production v4 payloads fit the 5 MB ceiling**

Run the lock-size-report with `--check`:

```powershell
cargo run -p apiwatch-lock-size-report -- --manifest compat/specs.json --compat-dir .compat-cache --privacy-fixture testdata/openapi/privacy_sentinels.yaml --max-lock-bytes 5242880 --json-out docs/benchmarks/phase-1-lock-size-report.json --markdown-out docs/benchmarks/phase-1-lock-size-report.md --v4-json-out docs/benchmarks/phase-2-v4-lock-size-report.json --v4-markdown-out docs/benchmarks/phase-2-v4-lock-size-report.md --check
```

Expected: passes for all passing specs. If any v4 payload exceeds 5 MB, the spec remains `known_failing` with `expected_error` explaining the ceiling breach.

- [ ] **Step 6: Update compat test count**

In `tests/compat.rs`, update any hardcoded spec count assertions:

If the compat.rs test asserts a specific number of specs, update from 5 to 10 (or 10 minus known_failing count).

- [ ] **Step 7: Regenerate benchmark files**

```powershell
Remove-Item docs/benchmarks/phase-1-lock-size-report.json
Remove-Item docs/benchmarks/phase-1-lock-size-report.md
cargo run -p apiwatch-lock-size-report -- --manifest compat/specs.json --compat-dir .compat-cache --privacy-fixture testdata/openapi/privacy_sentinels.yaml --max-lock-bytes 5242880 --json-out docs/benchmarks/phase-1-lock-size-report.json --markdown-out docs/benchmarks/phase-1-lock-size-report.md --v4-json-out docs/benchmarks/phase-2-v4-lock-size-report.json --v4-markdown-out docs/benchmarks/phase-2-v4-lock-size-report.md
```

- [ ] **Step 8: Full test suite**

Run: `cargo test --workspace`
Expected: all tests pass (with updated compat expectations)

Run: Python tests:

```powershell
python scripts/release_smoke.py
```

Expected: 4/4 passing

- [ ] **Step 9: Clippy and formatting**

```powershell
cargo fmt --check; cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 10: Commit**

```bash
git add -A
git commit -m "feat: expand global compatibility corpus to 10 specs with Stripe and DigitalOcean passing"
```

---

## Phase 3 Exit Verification

After all 8 tasks are complete:

1. `cargo test --workspace` — all tests pass
2. `cargo fmt --check` — zero formatting errors
3. `cargo clippy --workspace --all-targets -- -D warnings` — zero warnings
4. `python scripts/release_smoke.py` — 4/4 passing
5. `cargo run -p apiwatch-lock-size-report -- ... --check` — report checks pass
6. All production v4 lock payloads for the expanded corpus stay within 5 MB
7. Stripe and DigitalOcean are `passing` in the compatibility corpus
8. OpenAPI 3.1 nullable-type fixture diffs correctly through the v4 engine
9. Split multi-file spec resolves `./schemas.yaml#/User` safely
10. `.apiwatch.yaml` ignore rules and severity overrides produce expected filtered output
