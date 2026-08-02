# Phase 4 — Trustworthy Observed Contracts: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make observed contract confidence explicit — add observation threshold, null hardening, confidence metadata, tiered reporting, privacy threat model, and property tests.

**Architecture:** An `ObservedEntry` struct wraps lockfile-level metadata (threshold, timestamps) around existing `Shape`. Threshold logic in `compare_at` softens null-only and low-observation fields at verify time. Tiered reporting extends existing text/JSON/SARIF output with "insufficiently observed" and "unverified" categories.

**Tech Stack:** Rust 1.86+, serde/serde_yml, existing `Shape`/`observed` module

## Global Constraints

- MSRV 1.86
- No new dependencies
- 304 existing tests must remain green
- Backward compat: v2/v3/v4 locks without `threshold`/`first_seen`/`last_seen` default to threshold=1.0, empty timestamps
- No lockfile version bump — extends v3/v4 observed section with optional fields
- No scalar values, credentials, or dynamic map keys in serialized output
- Deterministic serialization; byte-identical lock output for identical input

---

### Task 1: Define `ObservedEntry` and tiered reporting types

**Files:**
- Modify: `src/observed/mod.rs`

**Interfaces:**
- Produces: `pub struct ObservedEntry`, `pub struct ObservedVerifyReport`, `pub struct TieredEntry`
- Produces: `pub fn is_hardened(parent_observations: u64, property_observations: u64, threshold: f64) -> bool`

**Description:** Add the new public types that Phase 4 depends on. No behavior changes yet — just types and a helper function.

- [ ] **Step 1: Add `ObservedEntry` struct to `src/observed/mod.rs`**

After the existing `Shape` enum (around line 30), add:

```rust
pub const DEFAULT_REQUIRED_THRESHOLD: f64 = 0.5;
pub const MINIMUM_OBSERVATION_FLOOR: u64 = 3;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservedEntry {
    pub shape: Shape,
    #[serde(default = "default_threshold")]
    pub threshold: f64,
    #[serde(default)]
    pub first_seen: String,
    #[serde(default)]
    pub last_seen: String,
}

fn default_threshold() -> f64 {
    DEFAULT_REQUIRED_THRESHOLD
}
```

- [ ] **Step 2: Add `is_hardened` helper function**

After `shape_name` function (around line 244), add:

```rust
pub fn is_hardened(parent_observations: u64, property_observations: u64, threshold: f64) -> bool {
    if parent_observations < MINIMUM_OBSERVATION_FLOOR {
        return false;
    }
    if parent_observations == 0 {
        return false;
    }
    let ratio = property_observations as f64 / parent_observations as f64;
    ratio >= threshold
}
```

- [ ] **Step 3: Add tiered reporting types**

After `ObservedChange` (around line 50), add:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TieredEntry {
    pub tier: TieredKind,
    pub path: String,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TieredKind {
    InsufficientlyObserved,
    Unverified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedVerifyReport {
    pub changes: Vec<ObservedChange>,
    pub tiered: Vec<TieredEntry>,
}
```

- [ ] **Step 4: Add `tiered_kind` helper for empty containers**

```rust
pub fn tiered_report(shape: &Shape, path: &str, parent_observations: u64, threshold: f64) -> Vec<TieredEntry> {
    let mut entries = Vec::new();
    collect_tiered(shape, path, parent_observations, threshold, &mut entries);
    entries
}

fn collect_tiered(shape: &Shape, path: &str, parent_observations: u64, threshold: f64, entries: &mut Vec<TieredEntry>) {
    match shape {
        Shape::Object { observations, properties } => {
            if properties.is_empty() {
                entries.push(TieredEntry {
                    tier: TieredKind::InsufficientlyObserved,
                    path: path.to_string(),
                    detail: format!("empty object, seen {observations} time(s)"),
                });
                return;
            }
            for (name, property) in properties {
                let property_path = format!("{path}.{name}");
                if !is_hardened(*observations, property.observations, threshold) {
                    entries.push(TieredEntry {
                        tier: TieredKind::InsufficientlyObserved,
                        path: property_path.clone(),
                        detail: format!(
                            "seen {}/{} time(s), threshold {:.2}",
                            property.observations, observations, threshold
                        ),
                    });
                }
                collect_tiered(&property.shape, &property_path, *observations, threshold, entries);
            }
        }
        Shape::Array { items } if matches!(items.as_ref(), Shape::Unknown) => {
            entries.push(TieredEntry {
                tier: TieredKind::InsufficientlyObserved,
                path: format!("{path}[]"),
                detail: "empty array, no item evidence".to_string(),
            });
        }
        Shape::Array { items } => {
            collect_tiered(items, &format!("{path}[]"), parent_observations, threshold, entries);
        }
        Shape::Map { values } => {
            collect_tiered(values, &format!("{path}.<map-value>"), parent_observations, threshold, entries);
        }
        Shape::Union { variants } => {
            for variant in variants {
                collect_tiered(variant, path, parent_observations, threshold, entries);
            }
        }
        _ => {}
    }
}
```

- [ ] **Step 5: Run tests to confirm no regressions**

```powershell
cargo test
```

Expected: 304 tests pass (no new tests yet, no behavior changes — just new types).

- [ ] **Step 6: Commit**

```bash
git add src/observed/mod.rs
git commit -m "feat: add ObservedEntry, tiered reporting types, and is_hardened helper"
```

---

### Task 2: Wire `ObservedEntry` through lockfile store

**Files:**
- Modify: `src/lockfile/mod.rs`
- Modify: `src/lockfile/v3/mod.rs`
- Modify: `src/lockfile/v4/mod.rs`

**Interfaces:**
- Consumes: `observed::ObservedEntry`, `observed::DEFAULT_REQUIRED_THRESHOLD`
- Produces: Updated `ApiLock.observed` type, updated v3/v4 `V3Api::Observed`, `V4Api::Observed`, `from_parts`, `into_parts`, `render`, `load`

**Description:** Change all observed storage from `Shape` to `ObservedEntry` throughout the lockfile system. Mechanical type migration with backward compatibility for existing locks.

- [ ] **Step 1: Update v3 `ObservedEntry` wire struct in `src/lockfile/v3/mod.rs`**

Replace the existing `ObservedEntry` struct (lines 86-90):

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ObservedEntry {
    shape: crate::observed::Shape,
    #[serde(default = "default_threshold")]
    threshold: f64,
    #[serde(default)]
    first_seen: String,
    #[serde(default)]
    last_seen: String,
}

fn default_threshold() -> f64 {
    1.0  // backward-compat: old locks without threshold default to 1.0 (binary required)
}
```

- [ ] **Step 2: Update v3 `from_parts` and `into_parts`**

Change `from_parts` parameter type (line 43) from `BTreeMap<String, crate::observed::Shape>` to `BTreeMap<String, crate::observed::ObservedEntry>`:

```rust
pub(super) fn from_parts(
    declared: BTreeMap<String, DeclaredEntry>,
    observed: BTreeMap<String, crate::observed::ObservedEntry>,
) -> Self {
    let mut apis = declared
        .into_iter()
        .map(|(name, entry)| (name, V3Api::Declared(entry)))
        .collect::<BTreeMap<_, _>>();
    apis.extend(
        observed
            .into_iter()
            .map(|(name, entry)| (name, V3Api::Observed(ObservedEntry {
                shape: entry.shape,
                threshold: entry.threshold,
                first_seen: entry.first_seen,
                last_seen: entry.last_seen,
            }))),
    );
    Self { version: 3, apis }
}
```

Change `into_parts` return type (line 62) from `BTreeMap<String, crate::observed::Shape>` to `BTreeMap<String, crate::observed::ObservedEntry>`:

```rust
pub(super) fn into_parts(
    self,
) -> (
    BTreeMap<String, DeclaredEntry>,
    BTreeMap<String, crate::observed::ObservedEntry>,
) {
    let mut declared = BTreeMap::new();
    let mut observed = BTreeMap::new();
    for (name, api) in self.apis {
        match api {
            V3Api::Declared(entry) => {
                declared.insert(name, entry);
            }
            V3Api::Observed(entry) => {
                observed.insert(name, crate::observed::ObservedEntry {
                    shape: entry.shape,
                    threshold: entry.threshold,
                    first_seen: entry.first_seen,
                    last_seen: entry.last_seen,
                });
            }
        }
    }
    (declared, observed)
}
```

- [ ] **Step 3: Update v3 raw shape validation to accept new fields**

In `validate_raw_observed_shapes` (line 299), the `allowed` keys list for `"object"` already checks `&["kind", "observations", "properties"]`. These are Shape-internal keys — the new fields (`threshold`, `first_seen`, `last_seen`) live alongside `shape` at the API entry level, not inside the Shape. The existing `reject_unknown_mapping_keys` for the API entry level checks `&["observations", "shape"]` for properties — but the API entry level is validated by `#[serde(deny_unknown_fields)]` on `V3Api`. With the new fields on `ObservedEntry`, the deserialization will accept them. No raw validation changes needed for v3 since fields are at the entry level.

But wait — there's a raw validation pass that rejects unknown keys before serde deserializes. Let me check: `validate_raw_observed_shapes` only validates the inner `shape` value, not the surrounding API entry keys. The API entry structure (`V3Api::Observed(ObservedEntry)`) is validated by serde's `deny_unknown_fields`. With `#[serde(default)]` on the new fields, existing locks without them will deserialize fine. No raw validation changes needed.

Actually wait — the raw validation iterates APIs and checks `"provenance" == "observed"` then validates the inner shape. It doesn't check fields at the ObservedEntry level. The `#[serde(deny_unknown_fields)]` on `ObservedEntry` would reject old locks that lack the new fields... UNLESS the new fields have `#[serde(default)]`. Yes, they do. So old locks deserialize with defaults. Good.

No raw validation changes needed. Step 3 is a no-op for v3.

- [ ] **Step 3: Update v4 `ObservedEntry` wire struct**

Same as v3. In `src/lockfile/v4/mod.rs`, replace lines 66-70:

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ObservedEntry {
    shape: crate::observed::Shape,
    #[serde(default = "default_threshold")]
    threshold: f64,
    #[serde(default)]
    first_seen: String,
    #[serde(default)]
    last_seen: String,
}

fn default_threshold() -> f64 {
    1.0
}
```

- [ ] **Step 4: Update v4 `from_parts` and `into_parts`**

```rust
// from_parts — change parameter type
pub(super) fn from_parts(
    declared: BTreeMap<String, DeclaredEntry>,
    observed: BTreeMap<String, crate::observed::ObservedEntry>,
) -> Self {
    let mut apis = declared
        .into_iter()
        .map(|(name, entry)| (name, V4Api::Declared(entry)))
        .collect::<BTreeMap<_, _>>();
    apis.extend(
        observed
            .into_iter()
            .map(|(name, entry)| (name, V4Api::Observed(ObservedEntry {
                shape: entry.shape,
                threshold: entry.threshold,
                first_seen: entry.first_seen,
                last_seen: entry.last_seen,
            }))),
    );
    Self { version: 4, apis }
}

// into_parts — change return type
pub(super) fn into_parts(
    self,
) -> (
    BTreeMap<String, DeclaredEntry>,
    BTreeMap<String, crate::observed::ObservedEntry>,
) {
    let mut declared = BTreeMap::new();
    let mut observed = BTreeMap::new();
    for (name, api) in self.apis {
        match api {
            V4Api::Declared(entry) => {
                declared.insert(name, entry);
            }
            V4Api::Observed(entry) => {
                observed.insert(name, crate::observed::ObservedEntry {
                    shape: entry.shape,
                    threshold: entry.threshold,
                    first_seen: entry.first_seen,
                    last_seen: entry.last_seen,
                });
            }
        }
    }
    (declared, observed)
}
```

- [ ] **Step 5: Update `ApiLock` in `src/lockfile/mod.rs`**

Change the `observed` field (line 71) from `BTreeMap<String, Shape>` to `BTreeMap<String, ObservedEntry>`:

```rust
pub struct ApiLock {
    version: u8,
    #[serde(rename = "apis")]
    legacy_declared: BTreeMap<String, LockedApi>,
    #[serde(skip)]
    declared_v3: BTreeMap<String, v3::DeclaredEntry>,
    #[serde(skip)]
    declared_v4: BTreeMap<String, v4::DeclaredEntry>,
    #[serde(skip)]
    observed: BTreeMap<String, observed::ObservedEntry>,
}
```

Update the import: add `use crate::observed::{self, ObservedEntry, Shape};` at the top, or adjust existing imports.

- [ ] **Step 6: Update `from_contract` in lockfile/mod.rs (line 191)**

The `observed: BTreeMap::new()` stays the same — type infers correctly.

- [ ] **Step 7: Update `render` in lockfile/mod.rs — v2 path (line 249)**

Change the iteration from `&lock.observed` to destructure `ObservedEntry`:

```rust
for (name, entry) in &lock.observed {
    apis.insert(
        name,
        V2RenderedApi::Observed {
            provenance: "observed",
            shape: &entry.shape,
        },
    );
}
```

- [ ] **Step 8: Update `load` in lockfile/mod.rs — v3/v4 paths**

The v3 load path (line 273): change `observed` variable type:

```rust
3 => {
    let (declared, observed) = v3::load(&contents)?.into_parts();
    Ok(ApiLock {
        version: 3,
        legacy_declared: BTreeMap::new(),
        declared_v3: declared,
        declared_v4: BTreeMap::new(),
        observed,
    })
}
```

The v4 load path (line 283): same pattern:

```rust
4 => {
    let (declared_v4, observed) = v4::load(&contents)?.into_parts();
    Ok(ApiLock {
        version: 4,
        legacy_declared: BTreeMap::new(),
        declared_v3: BTreeMap::new(),
        declared_v4,
        observed,
    })
}
```

- [ ] **Step 9: Update `load_v2` in lockfile/mod.rs (line 510)**

Change v2 observed creation to wrap Shape in ObservedEntry:

```rust
"observed" => {
    let shape = api
        .shape
        .ok_or_else(|| anyhow!("observed api {name} is missing shape"))?;
    observed.insert(name, ObservedEntry {
        shape,
        threshold: 1.0,
        first_seen: String::new(),
        last_seen: String::new(),
    });
}
```

Update `V2LockedApi` struct (line 86): change `shape: Option<Shape>` — no change needed, V2LockedApi uses raw Shape from YAML, conversion happens in load_v2.

- [ ] **Step 10: Update `record_observed` in lockfile/mod.rs (line 456)**

Change the function to work with `ObservedEntry`:

```rust
pub fn record_observed(
    lock: &mut ApiLock,
    name: &str,
    incoming: Shape,
    merge_existing: bool,
    map_paths: &[String],
    threshold: f64,
) -> Result<()> {
    let name = normalized_name(name)?;
    if lock.legacy_declared.contains_key(name)
        || lock.declared_v3.contains_key(name)
        || lock.declared_v4.contains_key(name)
    {
        return Err(anyhow!(
            "api {name} is declared and cannot be recorded as observed"
        ));
    }

    let mut incoming = incoming;
    apply_map_annotations(&mut incoming, map_paths)?;

    let now = chrono_now();

    match lock.observed.get(name) {
        Some(existing) if merge_existing => {
            let mut shape = existing.shape.clone();
            apply_map_annotations(&mut shape, map_paths)?;
            merge_shapes(&mut shape, &incoming);
            lock.observed.insert(name.to_string(), ObservedEntry {
                shape,
                threshold: existing.threshold,
                first_seen: existing.first_seen.clone(),
                last_seen: now,
            });
        }
        Some(_) => return Err(anyhow!("api {name} already exists; use --merge")),
        None if merge_existing => return Err(anyhow!("observed api {name} was not found")),
        None => {
            lock.observed.insert(name.to_string(), ObservedEntry {
                shape: incoming,
                threshold,
                first_seen: now.clone(),
                last_seen: now,
            });
        }
    }

    if lock.version < 3 {
        lock.version = 2;
    }
    Ok(())
}

fn chrono_now() -> String {
    // Use std::time::SystemTime for no-dependency ISO 8601
    use std::time::SystemTime;
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    // Format: YYYY-MM-DDTHH:MM:SSZ
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;
    // Simple Gregorian date calculation (good enough for logging timestamps)
    let (year, month, day) = civil_from_days(days_since_epoch as i64 + 719468);
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    // Algorithm from Howard Hinnant
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
```

- [ ] **Step 11: Update `select_verify_target` in lockfile/mod.rs (line 548)**

Change the observed match arm:

```rust
if let Some(entry) = lock.observed.get(name) {
    return Ok(VerifyTarget {
        name: name.to_string(),
        kind: VerifyTargetKind::Observed {
            shape: entry.shape.clone(),
        },
    });
}
```

- [ ] **Step 12: Update `new_v3` in lockfile/mod.rs (line 296)**

Change the `observed` argument to `BTreeMap::new()` since type infers correctly.

- [ ] **Step 13: Update `new_v4` in lockfile/mod.rs (line 339)**

Same — `observed: BTreeMap::new()` infers correctly.

- [ ] **Step 14: Update `replace_declared` in lockfile/mod.rs**

Line 377 checks `lock.observed.contains_key(&name)` — this still works with the new type. No change needed.

Line 408: `v3::render` call passes `lock.observed.clone()` — needs to pass the right type. Wait, `v3::render` takes `&v3::V3Lock` which is constructed via `from_parts`. The `from_parts` now takes `BTreeMap<String, observed::ObservedEntry>`. The existing code is:

```rust
v3::render(&v3::V3Lock::from_parts(
    lock.declared_v3.clone(),
    lock.observed.clone(),
))?;
```

This now infers correctly since `lock.observed` is `BTreeMap<String, ObservedEntry>`. Good.

Line 449 (`replace_declared_v4`): same pattern — infers correctly.

- [ ] **Step 15: Run tests**

```powershell
cargo test
```

Expected: compile errors at call sites that still pass `BTreeMap<String, Shape>` — fix them, then all 304 tests pass.

- [ ] **Step 16: Commit**

```bash
git add src/lockfile/mod.rs src/lockfile/v3/mod.rs src/lockfile/v4/mod.rs
git commit -m "feat: wire ObservedEntry through lockfile storage with backward compat"
```

---

### Task 3: CLI flag and main.rs wiring

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `observed::DEFAULT_REQUIRED_THRESHOLD`, `lockfile::record_observed` (updated signature)
- Produces: `--required-threshold` CLI flag, updated `Command::Record` and `Command::Verify` (observed path)

- [ ] **Step 1: Add `--required-threshold` to `Command::Record` in `src/cli.rs`**

After the `#[arg(long = "map-at")]` line (line 74), add:

```rust
/// Observation ratio (0.0-1.0) required before a field hardens.
#[arg(long = "required-threshold", default_value_t = crate::observed::DEFAULT_REQUIRED_THRESHOLD)]
required_threshold: f64,
```

The `Record` variant becomes:

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
    #[arg(long = "map-at")]
    map_at: Vec<String>,
    #[arg(long = "required-threshold", default_value_t = crate::observed::DEFAULT_REQUIRED_THRESHOLD)]
    required_threshold: f64,
},
```

- [ ] **Step 2: Validate threshold range in cli.rs (or main.rs at parse time)**

In `main.rs`, before calling `record_observed`, validate:

```rust
if required_threshold < 0.0 || required_threshold > 1.0 {
    anyhow::bail!("--required-threshold must be between 0.0 and 1.0");
}
```

- [ ] **Step 3: Update `Command::Record` handler in `src/main.rs` (line 83)**

Update the destructuring and call:

```rust
Command::Record {
    from_json,
    name,
    output,
    merge,
    map_at,
    required_threshold,
} => {
    if required_threshold < 0.0 || required_threshold > 1.0 {
        anyhow::bail!("--required-threshold must be between 0.0 and 1.0");
    }
    let shape = observed::load_shape(&from_json)?;
    let mut lock = lockfile::load_or_create_for_record(&output)?;
    lockfile::record_observed(&mut lock, &name, shape, merge, &map_at, required_threshold)?;
    let rendered = lockfile::render(&lock)?;
    fs::write(&output, rendered)
        .with_context(|| format!("failed to write lockfile {}", output.display()))?;
    println!("Wrote {}", output.display());
    Ok(0)
}
```

- [ ] **Step 4: Update observed verify path in main.rs (line 124)**

The `VerifyTargetKind::Observed { shape: expected }` already extracts the `Shape` from the `ObservedEntry` via `select_verify_target`. No changes needed here — the shape extraction happens in `select_verify_target`.

- [ ] **Step 5: Reject threshold changes on `--merge`**

In `record_observed`, add a check before merging:

```rust
Some(existing) if merge_existing => {
    if (threshold - existing.threshold).abs() > f64::EPSILON {
        return Err(anyhow!(
            "api {name} threshold is {:.2}; --required-threshold cannot change on --merge",
            existing.threshold
        ));
    }
    // ... rest of merge logic
}
```

- [ ] **Step 6: Run tests**

```powershell
cargo test
```

Expected: compile and pass.

- [ ] **Step 7: Commit**

```bash
git add src/cli.rs src/main.rs
git commit -m "feat: add --required-threshold flag and wire through record/verify"
```

---

### Task 4: D-17/D-18 threshold logic in `compare_at`

**Files:**
- Modify: `src/observed/mod.rs`

**Interfaces:**
- Consumes: `is_hardened()`, `MINIMUM_OBSERVATION_FLOOR`
- Modifies: `compare_at()`, `compare()` return type

**Description:** Replace binary requiredness with threshold-aware check. Implement null leniency (D-17) and observation-count-aware requiredness (D-18).

- [ ] **Step 1: Add a test for D-17 null hardness with threshold**

Add to the `#[cfg(test)] mod tests` block in `src/observed/mod.rs`:

```rust
#[test]
fn null_only_field_is_lenient_below_floor() {
    let expected = infer(&json!({"x": null}));
    // 1 observation, below floor of 3 — field should be lenient
    let changes = compare(
        &expected,
        &infer(&json!({"x": "hello"})),
    );
    // Without threshold logic this fails — with threshold it should pass
    assert!(changes.is_empty(), "null-only with 1 obs should be lenient");
}

#[test]
fn null_only_field_is_hardened_above_floor() {
    let mut expected = infer(&json!({"x": null}));
    merge(&mut expected, &infer(&json!({"x": null})));
    merge(&mut expected, &infer(&json!({"x": null})));
    // 3 observations, >= floor — null should be hardened
    let changes = compare(
        &expected,
        &infer(&json!({"x": "hello"})),
    );
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].kind, ObservedChangeKind::IncompatibleShape);
}

#[test]
fn requiredness_uses_threshold() {
    let mut expected = infer(&json!({"a": 1, "b": 2}));
    merge(&mut expected, &infer(&json!({"a": 1, "b": 2})));
    merge(&mut expected, &infer(&json!({"a": 1})));
    // "a" observed 3/3 times (required at any threshold)
    // "b" observed 2/3 times (optional at threshold 0.8, required at 0.5)
    let changes = compare(
        &expected,
        &infer(&json!({"a": 3})),
    );
    // "b" has 2/3 observations — at default threshold 0.5, 0.66 >= 0.5 so it IS required
    // But we're testing without threshold — current binary logic says 2 != 3 so optional
    // Wait — the current test infrastructure doesn't pass threshold. Let's test with the new API.
    let report = compare_with_threshold(
        &expected,
        &infer(&json!({"a": 3})),
        0.5,
    );
    assert!(report.changes.is_empty(), "b should be optional at default threshold? No: 2/3=0.66 >= 0.5 so required");
    // Actually: b has 2 obs, parent has 3 obs. Ratio = 0.66 >= 0.5, floor 3 >= 3. So b IS required.
    // Missing b at verify should produce MissingRequiredField.
    // Let me fix this test...
}
```

Wait — let me re-examine. The current code checks `property.observations == *observations`. With 3 parent and 2 property, 2 != 3, so the property is NOT required. My new threshold logic with 0.5: 2/3 = 0.66 >= 0.5, so it IS required. That's a behavior change.

But the user chose default 0.5. So 2/3 >= 0.5 → required. With floor 3, the parent (3 obs) passes the floor. So "b" (2/3 obs) becomes required at threshold 0.5.

This means the test needs to use a scenario where ratio < threshold. Let me fix:

```rust
#[test]
fn requiredness_uses_threshold() {
    let mut expected = infer(&json!({"a": 1, "b": 2}));
    // merge 7 more times to get 8 total obs
    for _ in 0..7 {
        merge(&mut expected, &infer(&json!({"a": 1})));
    }
    // "a" observed 8/8, "b" observed 1/8
    // At threshold 0.5: b ratio = 1/8 = 0.125 < 0.5 → optional
    let changes = compare(
        &expected,
        &infer(&json!({"a": 3})),
    );
    // With threshold 0.5: b is optional (0.125 < 0.5), so no MissingRequiredField
    assert!(changes.is_empty());
}
```

Actually, this test is tricky because we're still using the OLD `compare` function that doesn't take a threshold. Let me think about the API change...

I need to modify `compare` to take a threshold, or create a new function. Given the backward compat story, I should keep `compare` but add a threshold parameter. Actually, the spec says threshold is per-entry, stored in the lock. At verify time, the threshold is passed from the entry. So `compare` needs to accept threshold.

But `compare` is also used by existing tests that don't know about thresholds. Options:
1. Add a default threshold parameter to `compare` — breaks existing callers
2. Add `compare_with_threshold` as the new function, keep old `compare` as a thin wrapper with default threshold 1.0
3. Change `compare` signature: `compare(expected, actual, threshold)` — update all callers

Option 3 is cleanest. Default threshold should be 1.0 for backward compat in tests (binary requiredness). Let me go with renamed function.

Actually, let me keep `compare` with the old signature for backward compat in existing tests, and add `compare_with_threshold` for the new behavior. The main.rs will call `compare_with_threshold`. Existing tests call `compare` with the old behavior.

Hmm, but that's ugly. Let me think...

Actually the cleanest: change `compare` signature to `pub fn compare(expected: &Shape, actual: &Shape, threshold: f64) -> Vec<ObservedChange>`. Update all internal callers. The tests currently call `compare(&a, &b)` — I'll update them to `compare(&a, &b, 1.0)` to keep binary-required behavior.

But there are also tests that assert specific behaviors that depend on the threshold. I should update only the minimum needed.

Let me go with: change `compare` signature, add threshold parameter, update all test call sites to use 1.0 (preserving existing behavior).

OK let me rewrite the test steps properly.

- [ ] **Step 1: Write failing tests for D-17 and D-18**

Add to the `#[cfg(test)] mod tests` block:

```rust
#[test]
fn null_only_field_with_one_sample_is_lenient() {
    let expected = infer(&json!({"x": null}));
    // 1 observation below floor of 3, threshold 0.5
    let changes = compare(&expected, &infer(&json!({"x": "hello"})), 0.5);
    assert!(changes.is_empty(), "single-sample null must be lenient");
}

#[test]
fn null_only_field_with_three_samples_is_hardened() {
    let mut expected = infer(&json!({"x": null}));
    merge(&mut expected, &infer(&json!({"x": null})));
    merge(&mut expected, &infer(&json!({"x": null})));
    // 3 observations on parent, >= floor of 3, ratio 1.0 >= 0.5
    let changes = compare(&expected, &infer(&json!({"x": "hello"})), 0.5);
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].kind, ObservedChangeKind::IncompatibleShape);
}

#[test]
fn low_observation_field_is_optional_with_threshold() {
    let mut expected = infer(&json!({"a": 1, "b": 2}));
    // merge 7 more where b is absent: 1 b / 8 total
    for _ in 0..7 {
        merge(&mut expected, &infer(&json!({"a": 1})));
    }
    // 8 parent obs, b has 1 obs, ratio 0.125 < 0.5 → optional, floor 8 >= 3
    let changes = compare(&expected, &infer(&json!({"a": 3})), 0.5);
    assert!(changes.is_empty(), "b below threshold must be optional");
}

#[test]
fn threshold_one_zero_is_binary_required() {
    let mut expected = infer(&json!({"a": 1, "b": 2}));
    merge(&mut expected, &infer(&json!({"a": 1})));
    // 2 parent obs, "a" 2/2, "b" 1/2
    let changes = compare(&expected, &infer(&json!({"a": 3})), 1.0);
    // threshold 1.0: b ratio 0.5 < 1.0 → optional. No change.
    assert!(changes.is_empty());
    // But at threshold 0.0:
    let changes = compare(&expected, &infer(&json!({"a": 3})), 0.0);
    assert!(changes.is_empty(), "all optional at 0.0");
}
```

- [ ] **Step 2: Run tests — expect FAIL**

```powershell
cargo test null_only_field_with_one_sample_is_lenient null_only_field_with_three_samples_is_hardened low_observation_field_is_optional_with_threshold threshold_one_zero_is_binary_required
```

Expected: COMPILE ERROR (old `compare` takes 2 args, not 3) — this forces the signature change.

- [ ] **Step 3: Change `compare` signature to accept threshold**

In `src/observed/mod.rs`, change:

```rust
pub fn compare(expected: &Shape, actual: &Shape) -> Vec<ObservedChange> {
    let mut changes = Vec::new();
    compare_at(expected, actual, "$", &mut changes);
    // ...
}
```

To:

```rust
pub fn compare(expected: &Shape, actual: &Shape, threshold: f64) -> Vec<ObservedChange> {
    let mut changes = Vec::new();
    compare_at(expected, actual, "$", 0, threshold, &mut changes);
    changes.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.kind.cmp(&right.kind))
    });
    changes
}
```

Also update `compare_at` signature to pass through parent observations and threshold:

```rust
fn compare_at(expected: &Shape, actual: &Shape, path: &str, parent_observations: u64, threshold: f64, changes: &mut Vec<ObservedChange>) {
```

- [ ] **Step 4: Implement D-17 null hardening in compare_at**

In `compare_at`, before the `if matches!(expected, Shape::Unknown)` check, add null leniency:

```rust
fn compare_at(expected: &Shape, actual: &Shape, path: &str, parent_observations: u64, threshold: f64, changes: &mut Vec<ObservedChange>) {
    // D-17: lenient null — if expected is Null and property is not hardened, treat as Unknown
    if matches!(expected, Shape::Null) && !is_hardened(parent_observations, 0, threshold) {
        return; // underdetermined null: accept anything
    }

    if matches!(expected, Shape::Unknown) {
        return;
    }
    // ... rest of compare_at unchanged, but update recursive calls ...
```

Wait — `Shape::Null` doesn't have a property_observations field. The "observations" of a null field come from the parent ObservedProperty's count. But in `compare_at`, we only have the parent object's observations. Let me think...

Actually, looking at how `compare_at` works: when comparing objects, we iterate properties:

```rust
Shape::Object { observations, properties } => {
    for (name, expected_property) in properties {
        compare_at(
            &expected_property.shape,
            &actual_property.shape,
            &property_path,
            changes,
        );
    }
}
```

The property's observation count is `expected_property.observations`. But `compare_at` doesn't receive it. I need to thread it through.

Let me update `compare_at` to receive `property_observations` AND `parent_observations`:

```rust
fn compare_at(
    expected: &Shape, 
    actual: &Shape, 
    path: &str, 
    property_observations: u64,
    parent_observations: u64,
    threshold: f64, 
    changes: &mut Vec<ObservedChange>
)
```

For the top-level call (from `compare`), property_observations = 0 and parent_observations = 0 — the null leniency won't apply (both 0, hardening check fails → lenient). For properties within objects, the call passes the actual counts.

For the null check specifically: we need to know the property's observation count. When we're checking at `$.x` where x is Shape::Null, the call comes from the object comparator with `expected_property.observations`. So:

```rust
// In the object comparison loop:
compare_at(
    &expected_property.shape,
    &actual_property.shape,
    &property_path,
    expected_property.observations,  // property's own obs count
    *observations,                    // parent object's obs count  
    threshold,
    changes,
);
```

And the null check becomes:

```rust
if matches!(expected, Shape::Null) 
    && !is_hardened(parent_observations, property_observations, threshold) 
{
    return; // lenient null
}
```

But wait — Shape::Null currently has no observation count. The "how many times was this null seen" is on the ObservedProperty, not the Null shape. A ObservedProperty { observations: 5, shape: Null } means the field was null 5 out of N times. If the shape is just Null (not a Union), then property_observations == every time this property was present (and it was always null). So `is_hardened(parent_observations, property_observations, threshold)` gives the right answer.

But what about a Union that contains Null? Like `Union { Null, Number }`. In that case, the ObservedProperty has e.g. observations: 10, shape: Union { Null (10 obs?), Number (???) }. But the Null variant inside a Union doesn't carry its own count. Hmm, this is getting complex.

For D-17, the problem case is specifically: the property's shape is PURE `Shape::Null` (no union). That means the field was ALWAYS null. In that case, `property_observations` tells us how many times we saw it. And `is_hardened(parent_observations, property_observations, threshold)` checks if it was seen enough times relative to parent.

If the shape is a Union that contains Null, then some observations had null and some had other types. The Union comparison already handles this correctly — when expected is Union{Null, Number} and actual is "hello", the Null branch fails but Number branch passes.

OK so the null check should be: if the entire expected shape IS `Shape::Null`, and the property isn't hardened, treat as lenient. This is correct.

Let me refine the implementation:

```rust
fn compare_at(expected: &Shape, actual: &Shape, path: &str, parent_observations: u64, threshold: f64, changes: &mut Vec<ObservedChange>) {
    // D-17: null leniency
    if matches!(expected, Shape::Null) && !is_hardened(parent_observations, property_observations, threshold) {
        return;
    }
    // ...
}
```

Wait, I need `property_observations`. The top-level compare call doesn't have this. For the top level, I'll pass 0 — null checks at top level are irrelevant (you can't have a null-only top-level shape in a meaningful way).

Actually, let me simplify. The null hardening check only matters INSIDE objects (when comparing properties). At the top level and for array items, there's no property_observations to work with. So I'll add a separate parameter:

```rust
fn compare_at(
    expected: &Shape, 
    actual: &Shape, 
    path: &str, 
    property_obs: u64,
    parent_obs: u64, 
    threshold: f64, 
    changes: &mut Vec<ObservedChange>
)
```

For top-level compare(), pass (0, 0) — null leniency won't trigger. For object properties, pass the real counts. For array items, pass (0, 0) — null hardening in arrays uses the same logic but without per-property counts.

OK this is getting complex for the plan. Let me simplify the plan description and include the key implementation logic.

- [ ] **Step 4: Implement D-17/D-18 threshold logic**

Change `compare` to accept threshold:

```rust
pub fn compare(expected: &Shape, actual: &Shape, threshold: f64) -> Vec<ObservedChange> {
    let mut changes = Vec::new();
    compare_at(expected, actual, "$", 0, 0, threshold, &mut changes);
    changes.sort_by(|left, right| {
        left.path.cmp(&right.path).then_with(|| left.kind.cmp(&right.kind))
    });
    changes
}
```

Update `compare_at` signature and root entry:

```rust
fn compare_at(
    expected: &Shape,
    actual: &Shape,
    path: &str,
    property_obs: u64,
    parent_obs: u64,
    threshold: f64,
    changes: &mut Vec<ObservedChange>,
) {
    // D-17: null leniency — pure Null at property level, not hardened
    if matches!(expected, Shape::Null) && !is_hardened(parent_obs, property_obs, threshold) {
        return;
    }

    if matches!(expected, Shape::Unknown) {
        return;
    }
    // ... existing logic, update all recursive calls to pass property_obs, parent_obs, threshold
}
```

In the object comparison section, update recursive calls:

```rust
Shape::Object { observations, properties } => {
    // ...
    for (name, expected_property) in properties {
        let property_path = format!("{path}.{name}");
        match actual_properties.get(name) {
            Some(actual_property) => compare_at(
                &expected_property.shape,
                &actual_property.shape,
                &property_path,
                expected_property.observations,  // property obs
                *observations,                     // parent obs
                threshold,
                changes,
            ),
            None => {
                // D-18: threshold-aware requiredness
                if is_hardened(*observations, expected_property.observations, threshold) {
                    changes.push(ObservedChange {
                        kind: ObservedChangeKind::MissingRequiredField,
                        path: property_path,
                        expected: None,
                        actual: None,
                    });
                }
            }
        }
    }
}
```

Update all other recursive `compare_at` calls (arrays, maps, unions) to pass through the existing `property_obs`, `parent_obs`, `threshold` values. For array items, pass `(0, 0)` since arrays don't have per-item observation counts. For maps, same. For union variants, pass through the original counts.

- [ ] **Step 5: Update all existing test call sites**

Every `compare(&a, &b)` call changes to `compare(&a, &b, 1.0)` to preserve existing binary-required behavior in existing tests. There are calls in:
- `annotation_converts_an_object_to_a_value_free_map` (line 564-583) — `compare()` 3 times
- `merge_marks_late_fields_optional_and_sorts_a_scalar_union` (line 598-609) — `compare()` 2 times
- `empty_array_accepts_a_populated_array` (line 628)
- `reports_a_string_instead_of_a_locked_number` (line 634)

Add `1.0` as the third argument to all calls.

- [ ] **Step 6: Run tests — expect green**

```powershell
cargo test
```

All tests should pass — existing tests use threshold 1.0 (binary behavior), new D-17/D-18 tests verify threshold-based behavior.

- [ ] **Step 7: Commit**

```bash
git add src/observed/mod.rs
git commit -m "feat: implement D-17 null hardening and D-18 threshold-aware requiredness"
```

---

### Task 5: Integrate tiered reporting with compare and output

**Files:**
- Modify: `src/observed/mod.rs`
- Modify: `src/output/mod.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `ObservedVerifyReport`, `TieredEntry`, `tiered_report()`, `compare()` (updated)
- Produces: Updated `compare` to return report, updated output renderers

- [ ] **Step 1: Add `verify_with_tiers` function combining compare and tiered report**

In `src/observed/mod.rs`:

```rust
pub fn verify_with_tiers(expected: &Shape, actual: &Shape, threshold: f64) -> ObservedVerifyReport {
    let changes = compare(expected, actual, threshold);
    let tiered = tiered_report(expected, "$", 0, threshold);
    ObservedVerifyReport { changes, tiered }
}
```

Wait — `tiered_report` traverses the expected shape to find insufficiently-observed fields. But "unverified" fields come from the ACTUAL shape (fields present in actual but absent from expected). Let me add the unverified collection too.

Actually, looking at the tiered_report function from Task 1, it only walks the expected shape. For "unverified" (fields in actual but not in expected), I need a separate pass. Let me add `collect_unverified` that walks the actual shape against the expected shape:

```rust
pub fn verify_with_tiers(expected: &Shape, actual: &Shape, threshold: f64) -> ObservedVerifyReport {
    let changes = compare(expected, actual, threshold);
    let mut tiered = tiered_report(expected, "$", 0, threshold);
    collect_unverified(expected, actual, "$", &mut tiered);
    ObservedVerifyReport { changes, tiered }
}

fn collect_unverified(expected: &Shape, actual: &Shape, path: &str, entries: &mut Vec<TieredEntry>) {
    match (expected, actual) {
        (Shape::Object { properties: exp_props, .. }, Shape::Object { properties: act_props, .. }) => {
            for (name, _) in act_props.iter().filter(|(n, _)| !exp_props.contains_key(*n)) {
                entries.push(TieredEntry {
                    tier: TieredKind::Unverified,
                    path: format!("{path}.{name}"),
                    detail: format!("field not in lock"),
                });
            }
            for (name, exp_prop) in exp_props {
                if let Some(act_prop) = act_props.get(name) {
                    collect_unverified(&exp_prop.shape, &act_prop.shape, &format!("{path}.{name}"), entries);
                }
            }
        }
        (Shape::Array { items: exp_items }, Shape::Array { items: act_items }) => {
            collect_unverified(exp_items, act_items, &format!("{path}[]"), entries);
        }
        (Shape::Map { values: exp_vals }, Shape::Map { values: act_vals }) => {
            collect_unverified(exp_vals, act_vals, &format!("{path}.<map-value>"), entries);
        }
        (Shape::Union { variants: exp_vars }, _) => {
            for variant in exp_vars {
                collect_unverified(variant, actual, path, entries);
            }
        }
        (_, Shape::Union { variants: act_vars }) => {
            for variant in act_vars {
                collect_unverified(expected, variant, path, entries);
            }
        }
        _ => {}
    }
}
```

- [ ] **Step 2: Update main.rs observed verify path to use verify_with_tiers**

In `src/main.rs`, line 124-147, replace:

```rust
let changes = observed::compare(expected, &current);
```

With:

```rust
let threshold = // need to get threshold from the entry...
```

Wait — the threshold is on the ObservedEntry, but `select_verify_target` returns `VerifyTarget` which only carries `Shape`, not the full `ObservedEntry`. I need to either:
1. Add threshold to `VerifyTargetKind::Observed`
2. Or look it up again from the lock

Option 1 is cleaner. Update `VerifyTargetKind::Observed`:

In `src/lockfile/mod.rs`:

```rust
pub enum VerifyTargetKind {
    // ...
    Observed {
        shape: Shape,
        threshold: f64,
        first_seen: String,
        last_seen: String,
    },
}
```

Update `select_verify_target` to include metadata:

```rust
if let Some(entry) = lock.observed.get(name) {
    return Ok(VerifyTarget {
        name: name.to_string(),
        kind: VerifyTargetKind::Observed {
            shape: entry.shape.clone(),
            threshold: entry.threshold,
            first_seen: entry.first_seen.clone(),
            last_seen: entry.last_seen.clone(),
        },
    });
}
```

Update `VerifyTarget::observed_shape()` to handle the new variant:

```rust
pub fn observed_shape(&self) -> Option<&Shape> {
    match &self.kind {
        VerifyTargetKind::Observed { shape, .. } => Some(shape),
        _ => None,
    }
}
```

Then in `main.rs`:

```rust
lockfile::VerifyTargetKind::Observed { shape: expected, threshold, first_seen, last_seen } => {
    if openapi.starts_with("http://") || openapi.starts_with("https://") {
        anyhow::bail!("observed verification requires a local JSON file");
    }
    let current = observed::load_shape(std::path::Path::new(&openapi))?;
    let report = observed::verify_with_tiers(expected, &current, *threshold);
    let has_changes = !report.changes.is_empty();
    let has_tiered = !report.tiered.is_empty();
    let rendered = match format {
        OutputFormat::Text if !has_changes && !has_tiered => {
            format!("Verified {}\n  first seen: {first_seen}\n  last seen:  {last_seen}\n", target.name())
        }
        OutputFormat::Text => output::render_observed_verify_with_tiers(target.name(), threshold, first_seen, last_seen, &report),
        OutputFormat::Json => output::render_observed_verify_with_tiers_json(target.name(), threshold, first_seen, last_seen, &report)?,
        OutputFormat::Sarif => output::render_observed_verify_with_tiers_sarif(&lock_path, target.name(), &report)?,
    };
    print!("{rendered}");
    Ok(if has_changes { 1 } else { 0 })
}
```

- [ ] **Step 3: Implement text renderer in `src/output/mod.rs`**

```rust
pub fn render_observed_verify_with_tiers(
    name: &str,
    threshold: f64,
    first_seen: &str,
    last_seen: &str,
    report: &crate::observed::ObservedVerifyReport,
) -> String {
    let mut rendered = format!("Verified {name} (observed, threshold {threshold:.2})\n");
    if !first_seen.is_empty() {
        rendered.push_str(&format!("  first seen: {first_seen}\n"));
    }
    if !last_seen.is_empty() {
        rendered.push_str(&format!("  last seen:  {last_seen}\n"));
    }

    if !report.changes.is_empty() {
        rendered.push('\n');
        rendered.push_str(&render_observed_verify_changes(&report.changes));
    }

    let insufficient: Vec<_> = report.tiered.iter()
        .filter(|e| matches!(e.tier, crate::observed::TieredKind::InsufficientlyObserved))
        .collect();
    let unverified: Vec<_> = report.tiered.iter()
        .filter(|e| matches!(e.tier, crate::observed::TieredKind::Unverified))
        .collect();

    if !insufficient.is_empty() {
        rendered.push_str("\nInsufficiently observed:\n");
        for entry in insufficient {
            rendered.push_str(&format!("  - {}: {}\n", entry.path, entry.detail));
        }
    }

    if !unverified.is_empty() {
        rendered.push_str("\nUnverified:\n");
        for entry in unverified {
            rendered.push_str(&format!("  - {}: {}\n", entry.path, entry.detail));
        }
    }

    rendered
}
```

- [ ] **Step 4: Implement JSON renderer**

```rust
pub fn render_observed_verify_with_tiers_json(
    name: &str,
    threshold: f64,
    first_seen: &str,
    last_seen: &str,
    report: &crate::observed::ObservedVerifyReport,
) -> Result<String> {
    let changes: Vec<_> = report.changes.iter().map(|change| ObservedVerifyJsonChange {
        kind: match change.kind {
            crate::observed::ObservedChangeKind::MissingRequiredField => "missing_required_field",
            crate::observed::ObservedChangeKind::IncompatibleShape => "incompatible_shape",
        },
        path: &change.path,
        expected: change.expected.as_deref(),
        actual: change.actual.as_deref(),
    }).collect();

    #[derive(Serialize)]
    struct TieredJsonEntry<'a> {
        tier: &'static str,
        path: &'a str,
        detail: &'a str,
    }

    let tiered: Vec<_> = report.tiered.iter().map(|e| TieredJsonEntry {
        tier: match e.tier {
            crate::observed::TieredKind::InsufficientlyObserved => "insufficiently_observed",
            crate::observed::TieredKind::Unverified => "unverified",
        },
        path: &e.path,
        detail: &e.detail,
    }).collect();

    #[derive(Serialize)]
    struct FullReport<'a> {
        version: u8,
        command: &'static str,
        name: &'a str,
        provenance: &'static str,
        threshold: f64,
        first_seen: &'a str,
        last_seen: &'a str,
        summary: ObservedVerifySummary,
        changes: Vec<ObservedVerifyJsonChange<'a>>,
        tiered: Vec<TieredJsonEntry<'a>>,
    }

    let rendered = serde_json::to_string(&FullReport {
        version: 3,
        command: "verify",
        name,
        provenance: "observed",
        threshold,
        first_seen,
        last_seen,
        summary: ObservedVerifySummary { breaking: report.changes.len() },
        changes,
        tiered,
    }).context("failed to serialize observed verify JSON")?;

    Ok(format!("{rendered}\n"))
}
```

- [ ] **Step 5: Implement SARIF renderer**

```rust
pub fn render_observed_verify_with_tiers_sarif(
    artifact_path: &Path,
    name: &str,
    report: &crate::observed::ObservedVerifyReport,
) -> Result<String> {
    let artifact_uri = render_artifact_uri(artifact_path);
    let mut results: Vec<SarifResult> = report.changes.iter().map(|change| {
        // ... same as existing render_observed_verify_changes_sarif ...
        let (rule_id, message) = match change.kind {
            crate::observed::ObservedChangeKind::MissingRequiredField => (
                "apiwatch/verify-observed-missing-required-field",
                format!("required field missing: {}", change.path),
            ),
            crate::observed::ObservedChangeKind::IncompatibleShape => (
                "apiwatch/verify-observed-incompatible-shape",
                format!("incompatible shape at {}: expected {}, found {}",
                    change.path,
                    change.expected.as_deref().unwrap_or("unknown"),
                    change.actual.as_deref().unwrap_or("unknown"),
                ),
            ),
        };
        sarif_result(rule_id, "error", message, artifact_uri.clone(),
            format!("verify-observed:{name}:{rule_id}:{}:{}:{}",
                change.path,
                change.expected.as_deref().unwrap_or(""),
                change.actual.as_deref().unwrap_or(""),
            ))
    }).collect();

    for entry in &report.tiered {
        let (rule_id, level) = match entry.tier {
            crate::observed::TieredKind::InsufficientlyObserved => {
                ("apiwatch/verify-observed-insufficient", "warning")
            }
            crate::observed::TieredKind::Unverified => {
                ("apiwatch/verify-observed-unverified", "note")
            }
        };
        results.push(sarif_result(rule_id, level, entry.detail.clone(), artifact_uri.clone(),
            format!("verify-observed:{name}:{rule_id}:{}", entry.path)));
    }

    render_sarif_with_rules(tiered_observed_sarif_rules(), results, Vec::new())
}

fn tiered_observed_sarif_rules() -> Vec<SarifRule> {
    let mut rules = observed_sarif_rules();
    rules.push(sarif_rule(
        "apiwatch/verify-observed-insufficient",
        "Insufficiently observed structure",
        "An observed property has too few samples to be hardened.",
        "Record more samples to increase observation confidence.",
        "warning",
        "warning",
    ));
    rules.push(sarif_rule(
        "apiwatch/verify-observed-unverified",
        "Unverified structure",
        "A field is present in the actual JSON but absent from the lock.",
        "Update the lock entry if this field is expected.",
        "note",
        "recommendation",
    ));
    rules
}
```

- [ ] **Step 6: Run tests**

```powershell
cargo test
cargo clippy -- -D warnings
cargo fmt -- --check
```

Expected: all pass, no warnings.

- [ ] **Step 7: Commit**

```bash
git add src/observed/mod.rs src/output/mod.rs src/main.rs src/lockfile/mod.rs
git commit -m "feat: implement tiered reporting for observed verify (text/JSON/SARIF)"
```

---

### Task 6: Privacy threat model document

**Files:**
- Create: `docs/privacy-threat-model.md`

**Description:** Standalone document with no code dependencies.

- [ ] **Step 1: Write `docs/privacy-threat-model.md`**

Write the content at the end of this task.

- [ ] **Step 2: Commit**

```bash
git add docs/privacy-threat-model.md
git commit -m "docs: add privacy threat model for observed contracts"
```

**Document content:**

```markdown
# APIWatch Privacy Threat Model

## Scope

This document covers the privacy properties of APIWatch observed contracts.
Declared contracts (parsed from OpenAPI specifications) are out of scope —
they document intended API structure and contain no user data.

## What APIWatch Captures

| Captured | Not captured |
|----------|-------------|
| JSON property names (keys) | JSON property values |
| Shape kinds (null, boolean, number, string, object, array, map, union) | Scalar values (numbers, strings, booleans) |
| Observation counts (per object, per property) | Authentication tokens, API keys, PII |
| ISO 8601 timestamps (first/last seen per entry) | Request/response headers or bodies |
| Map-annotated paths (user-specified) | Dynamic map keys (stripped by `--map-at`) |

## Trust Boundary

```
User's JSON samples  ──→  APIWatch infer()  ──→  Value-free Shape  ──→  api.lock (on disk)
  (may contain PII)       (strips ALL values)      (only type structure)     (committed to git)
```

The critical boundary is `infer()`: it converts `serde_json::Value` (containing
potentially sensitive scalars) into `Shape` variants that retain only type
information and key names. No value crosses this boundary.

## Assets

1. **Shape structure in `api.lock`**: Committed to version control. Contains
   property names, type signatures, observation counts, and timestamps.
2. **Verify diagnostics (text/JSON/SARIF output)**: Printed to stdout or CI
   logs. Contains change descriptions with field paths but no values.
3. **JSON sample files on disk**: Read by `apiwatch record`. These files may
   contain PII. APIWatch reads them but does not copy values into the lock.

## Threat Actors

| Actor | Capability | Risk |
|-------|-----------|------|
| Internal developer | Reads `api.lock` from git | Low — only shape metadata |
| CI pipeline observer | Reads verify output logs | Low — field paths only |
| Malicious sample provider | Provides crafted JSON to `record` | Medium — see shape side channels |
| Repository attacker | Modifies `api.lock` on disk | Mitigated by digest validation |

## Attack Surface

### Shape-Structure Side Channels

A malicious sample provider could encode data in the SHAPE structure rather
than values:
- Deeply nested property names encoding data (e.g., `{"d41d8cd9": {"8f00b204": ...}}`)
- Observation count oracle: differential counts leaking access frequency

**Mitigations:**
- Property names originate from the user's API contract keys, not user secrets
- APIWatch records structural type information only; property name depth is
  bounded by the user's actual JSON structure
- Observation counts are aggregate totals, not per-user or per-request

### Dynamic Key Leakage

If `--map-at` is NOT used on dynamic-key objects (e.g., `{"user-123": {...}}`),
the dynamic keys would be captured as property names in the lock.

**Mitigation:** Users must explicitly annotate dynamic-key objects with
`--map-at`. APIWatch never silently infers map semantics.

### Lockfile Size Oracle

The lockfile size grows with observation counts and property cardinality. An
attacker with access to lockfile history could infer API complexity changes
over time.

**Residual risk:** Accepted. Observation counts are aggregate and intended
to be reviewable in git diffs.

### Timing Side Channels

`record` and `verify` runtime varies with sample size and shape complexity.
These are not signal-bearing in practice.

**Residual risk:** Accepted.

## What APIWatch Does NOT Do

- Does not record response body values, headers, or status codes (beyond shape)
- Does not capture authentication credentials from samples
- Does not transmit data off-machine (all operations are local file I/O)
- Does not infer dynamic map keys from observed structure

## Recommendations

1. **Use sanitized test data for recording**: Record from representative but
   non-production JSON samples. Avoid recording responses containing PII.
2. **Run recording in CI, not locally with production data**: Automate recording
   from integration test fixtures.
3. **Review `api.lock` diffs before merging**: Shape changes (new fields, type
   changes) are visible in git diffs. Review them like any other code change.
4. **Use `--map-at` for dynamic keys**: Any object whose keys are not fixed
   API field names should be annotated as a map.

## Version History

| Date | Version | Changes |
|------|---------|---------|
| 2026-08-02 | 1.0 | Initial threat model for Phase 4 observed contracts |
```

- [ ] **Step 3: Commit** (repeated from above)

---

### Task 7: Property tests

**Files:**
- Modify: `src/observed/mod.rs` (add `#[cfg(test)] mod property_tests`)

**Description:** Table-driven property tests covering Phase 4 invariants.

- [ ] **Step 1: Add property test module**

Add at the bottom of `src/observed/mod.rs`, inside the existing `#[cfg(test)]` block or as a new sibling:

```rust
#[cfg(test)]
mod property_tests {
    use serde_json::json;
    use super::*;

    #[test]
    fn round_trip_determinism() {
        let sample = json!({"id": 1, "name": "test", "tags": ["a", "b"], "meta": {"count": 42}});
        let shape = infer(&sample);
        let rendered = serde_yml::to_string(&shape).expect("serialize");
        let deserialized: Shape = serde_yml::from_str(&rendered).expect("deserialize");
        assert_eq!(shape, deserialized);
        let re_inferred = infer(&sample);
        assert_eq!(shape, re_inferred);
    }

    #[test]
    fn merge_idempotence() {
        let sample = json!({"x": 1, "y": "hello", "z": null});
        let mut shape = infer(&sample);
        let shape_before = shape.clone();
        merge(&mut shape, &shape_before.clone());
        assert_eq!(shape, shape_before);
    }

    #[test]
    fn compare_reflexivity() {
        let sample = json!({"a": 1, "b": {"c": [1, 2, 3]}});
        let shape = infer(&sample);
        assert!(compare(&shape, &shape, 0.5).is_empty());
        assert!(compare(&shape, &shape, 1.0).is_empty());
        assert!(compare(&shape, &shape, 0.0).is_empty());
    }

    #[test]
    fn order_invariance() {
        let left = infer(&json!({"a": 1, "b": 2}));
        let right = infer(&json!({"b": 2, "a": 1}));
        assert_eq!(left, right);
    }

    #[test]
    fn value_absence_in_serialized_shape() {
        let shape = infer(&json!({
            "token": "sk-abc123secret",
            "password": "hunter2",
            "amount": 9999
        }));
        let rendered = serde_yml::to_string(&shape).expect("serialize");
        assert!(!rendered.contains("sk-abc123secret"));
        assert!(!rendered.contains("hunter2"));
        assert!(!rendered.contains("9999"));
        assert!(!rendered.contains("9999.0"));
        assert!(rendered.contains("token"));
        assert!(rendered.contains("password"));
        assert!(rendered.contains("amount"));
    }

    #[test]
    fn value_absence_in_diagnostics() {
        let expected = infer(&json!({"secret": "s3cr3t"}));
        let actual = infer(&json!({"secret": 42}));
        let changes = compare(&expected, &actual, 0.5);
        assert_eq!(changes.len(), 1);
        let serialized = format!("{:?}", changes);
        assert!(!serialized.contains("s3cr3t"));
        assert!(!serialized.contains("42"));
    }

    #[test]
    fn threshold_zero_all_optional() {
        let mut expected = infer(&json!({"x": 1, "y": 2}));
        merge(&mut expected, &infer(&json!({"x": 1})));
        // x: 2/2, y: 1/2. At threshold 0.0: all hardened (ratio always >= 0.0)
        // But floor check: parent has 2 < 3 → NOT hardened
        // So both are lenient.
        let changes = compare(&expected, &infer(&json!({})), 0.0);
        assert!(changes.is_empty());
    }

    #[test]
    fn threshold_one_all_required_with_floor() {
        let mut expected = infer(&json!({"x": 1, "y": 2}));
        merge(&mut expected, &infer(&json!({"x": 1, "y": 2})));
        merge(&mut expected, &infer(&json!({"x": 1, "y": 2})));
        // parent: 3 obs (meets floor), x: 3/3 (1.0 >= 1.0 → required), y: 3/3 (required)
        let changes = compare(&expected, &infer(&json!({"x": 1})), 1.0);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "$.y");
        assert_eq!(changes[0].kind, ObservedChangeKind::MissingRequiredField);
    }

    #[test]
    fn floor_boundary_two_vs_three_parent_observations() {
        let mut two = infer(&json!({"x": 1}));
        merge(&mut two, &infer(&json!({"x": 1})));
        // parent: 2 obs, below floor → x is lenient even at ratio 1.0
        assert!(compare(&two, &infer(&json!({"x": "different"})), 0.5).is_empty());

        let mut three = infer(&json!({"x": 1}));
        merge(&mut three, &infer(&json!({"x": 1})));
        merge(&mut three, &infer(&json!({"x": 1})));
        // parent: 3 obs, meets floor → x is hardened at ratio 1.0, threshold 0.5
        assert_eq!(compare(&three, &infer(&json!({"x": "different"})), 0.5).len(), 1);
    }

    #[test]
    fn empty_container_compare_is_lenient() {
        let empty_arr = infer(&json!({"items": []}));
        let populated = infer(&json!({"items": [1, 2, 3]}));
        assert!(compare(&empty_arr, &populated, 0.5).is_empty());

        let empty_obj = infer(&json!({"meta": {}}));
        let populated_obj = infer(&json!({"meta": {"key": "val"}}));
        assert!(compare(&empty_obj, &populated_obj, 0.5).is_empty());
    }

    #[test]
    fn null_in_union_is_not_affected_by_leniency() {
        let mut expected = infer(&json!({"x": null}));
        merge(&mut expected, &infer(&json!({"x": "hello"})));
        merge(&mut expected, &infer(&json!({"x": null})));
        // x is Union { Null, String } after 3 merges
        // verify with a number should fail (neither null nor string)
        // Actually wait: merge(null) + merge("hello") = Union{Null, String}
        // Then merge(null) — null merges into the existing Null variant
        // So: Union{Null, String}, both observed
        // Actual=number → compare against Null fails, compare against String fails → incompatible
        let changes = compare(&expected, &infer(&json!({"x": 42})), 0.5);
        assert!(!changes.is_empty());
    }
}
```

Wait — some of these tests need fixes. Let me re-examine:

`threshold_zero_all_optional`: At threshold 0.0, `ratio >= 0.0` is always true. With parent=2 < floor=3, nothing is hardened. So the test should assert NO missing-required-field changes. My test says `assert!(changes.is_empty())` — correct.

`threshold_one_all_required_with_floor`: parent=3 meets floor. x ratio=1.0 and y ratio=1.0, both >= 1.0 threshold. Both hardened. Missing y → change. Correct.

Actually wait — in `null_in_union_is_not_affected_by_leniency`: I'm testing that a Union containing Null is NOT auto-lenient. The null leniency only applies when the expected shape IS `Shape::Null` (pure null, not inside a union). A Union {Null, String} means the field can be null OR string — the union comparison handles this correctly. Verifying a number against Union{Null, String} should fail because number matches neither variant. Correct.

But there's a subtlety: after 3 merges with null+string+null, the merge would produce a shape with parent obs=3, and property obs for x = 3 (present in all observations). But the shape is Union. The union comparison doesn't use the null-hardening check at all — it iterates variants. So this test is correct.

However, there's a bug in my test reasoning: the second merge of null ("x": null) — the existing shape is Union{Null, String}, and merge_union_variant would find the Null variant and... merge Null with Null. Null merges with Null → same_kind returns true → return (no change). So the Union stays as Union{Null, String}. Fine.

- [ ] **Step 2: Run property tests**

```powershell
cargo test property_tests
```

Expected: all pass.

- [ ] **Step 3: Run full test suite**

```powershell
cargo test
cargo clippy -- -D warnings
cargo fmt -- --check
```

Expected: all 304+ tests pass, no warnings.

- [ ] **Step 4: Commit**

```bash
git add src/observed/mod.rs
git commit -m "test: add property tests for Phase 4 invariants"
```

---

## Final Verification

After all tasks complete:

```powershell
cargo test
cargo clippy -- -D warnings
cargo fmt -- --check
```

All tests must pass. Verify that:
1. `apiwatch record --required-threshold 0.8` writes threshold to lock
2. `apiwatch verify --lock api.lock --name test` reads threshold from lock
3. Old lockfiles (v2/v3/v4 without threshold) load with default threshold 1.0
4. Existing observed verify output still works (backward compat)
5. Tiered output sections appear in text/JSON/SARIF
6. No scalar values in serialized shapes or diagnostics
7. `--map-at` semantics unchanged (existing tests pass)
