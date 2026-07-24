# APIWatch Lockfile v3 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement deterministic, privacy-safe lockfile v3 storage and make declared Verify compare complete locked contracts through `diff_contracts`.

**Architecture:** Add a focused `lockfile::v3` module for strict wire types, canonical hashing, schema interning, validation, and reconstruction. Keep legacy v1/v2 decoding in `lockfile::mod`, add safe create/update services around a unified in-memory lock, and route full declared Verify through the existing diff engine and renderers.

**Tech Stack:** Rust 2021, Rust 1.86 MSRV, `serde`, `serde_yaml`, `serde_json`, `sha2`, `tempfile`, `clap`, `anyhow`, `assert_cmd`, `predicates`.

## Global Constraints

- Work on `codex/phase-1-lockfile-v3`; do not create a worktree.
- Use test-driven development: observe every new behavior fail before implementing it.
- Lockfile v3 remains one deterministic YAML file with per-API schema tables.
- The default per-declared-contract limit is exactly `5_242_880` bytes and every override is positive.
- Schema and contract digests use `sha256:<64 lowercase hexadecimal characters>`.
- Canonical digest bytes use compact UTF-8 JSON, fixed struct field order, recursively sorted maps, and no trailing newline.
- `contract_bytes` counts standalone deterministic YAML bytes for `contract`, including its trailing newline.
- Plain `lock` never overwrites; `lock --update` is required for an existing output.
- Failed create, update, migration, scope, size, or integrity operations preserve existing bytes.
- v1 and v2 remain readable and are never silently promoted by Verify.
- Source descriptions, examples, defaults, extensions, credentials, headers, raw fragments, and captured values never enter declared v3 payloads.
- Selected-operation disappearance during Verify is a breaking finding, not an input error.
- Full declared Verify uses `diff_contracts`; do not build a parallel semantic comparison model.
- Do not change observed shape semantics, OpenAPI support, external-reference behavior, or unrelated diff rules.
- Keep `.compat-cache/`, `target/`, and `implementation-log/` untracked.

---

## File Structure

### New production files

- `src/lockfile/v3/mod.rs`: v3 wire types, strict serde model, declared-entry build/render/load orchestration, size/digest validation, and conversion to `ApiContract`.
- `src/lockfile/v3/canonical.rs`: recursively canonical extension values, SHA-256 IDs, schema and contract digest inputs.
- `src/lockfile/v3/schema.rs`: schema interning, reachability validation, digest verification, and recursive schema reconstruction.
- `src/lockfile/atomic.rs`: same-directory temporary-file create and replacement.

### Existing production files

- `src/lockfile/mod.rs`: legacy readers, unified lock state, migration/update policy, scoped/full/observed Verify targets.
- `src/lock_size.rs`: reuse exact selector parsing and scoping primitives; expose a non-strict current-contract scoper for Verify.
- `src/cli.rs`: add `--update`, `--include-operation`, and `--max-lock-bytes`.
- `src/main.rs`: dispatch safe v3 create/update and full declared Verify.
- `src/output/mod.rs`: version-2 declared Verify JSON, shared diff findings, SARIF legacy notifications.
- `Cargo.toml` and `Cargo.lock`: add runtime `tempfile`.

### Tests and fixtures

- `tests/cli_lock.rs`: replace v1 creation expectations with safe v3 creation/update behavior.
- `tests/cli_verify.rs`: full-contract Verify, scope, exit semantics, legacy limitations, and corruption.
- `scripts/release_smoke.py`: keep reusable Action/release behavior aligned.
- `testdata/openapi/v3_d16_old.yaml`: locked side of D-16.
- `testdata/openapi/v3_d16_new.yaml`: changed side producing four findings.
- `testdata/openapi/v3_scoped.yaml`: multi-operation scope fixture.
- `testdata/openapi/v3_scoped_added_unrelated.yaml`: scoped Verify fixture with an unrelated addition.
- `testdata/openapi/v3_scoped_without_users.yaml`: scoped Verify fixture with the selected endpoint removed.
- `testdata/lock/v3_users.lock`: committed deterministic golden lock.
- `testdata/lock/v2_declared_observed.lock`: migration-preservation fixture.
- `testdata/lock/v2_multiple_declared.lock`: ambiguous migration fixture.

### Documentation and status

- `docs/lockfile-spec.md`: replace planned-v3 language with exact implemented format and examples.
- `README.md`: document full declared Verify, lock/update/scoping, limits, and legacy warnings.
- `ROADMAP.md`: mark Phase 1 ordered items complete only after D-16 passes.
- `CHANGELOG.md`: record v3, migration, scope, and Verify behavior.
- `implementation-log/2026-07-25-lockfile-v3.md`: ignored task record.

---

### Task 1: Introduce Strict v3 Wire Types and Canonical Digests

**Files:**
- Create: `src/lockfile/v3/mod.rs`
- Create: `src/lockfile/v3/canonical.rs`
- Create: `src/lockfile/v3/schema.rs`
- Modify: `src/lockfile/mod.rs`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`

**Interfaces:**
- Consumes: `ApiContract`, `Schema`, and the prototype `sha256_id` policy.
- Produces:
  - `v3::Scope`
  - `v3::V3Lock`
  - `v3::V3Api`
  - `v3::DeclaredEntry`
  - `v3::Contract`
  - `canonical::schema_id(&WireSchema) -> Result<String>`
  - `canonical::contract_digest(&Scope, &Contract, &Extensions) -> Result<String>`
  - `schema::intern_contract(&ApiContract) -> Result<Contract>`

- [ ] **Step 1: Add the module shell and failing canonicalization tests**

Add `mod v3;` to `src/lockfile/mod.rs`. Create these exact core types in
`src/lockfile/v3/mod.rs`; all semantic wire structs use
`#[serde(deny_unknown_fields)]`:

```rust
mod canonical;
mod schema;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const DEFAULT_MAX_LOCK_BYTES: u64 = 5_242_880;

pub type Extensions = BTreeMap<String, serde_json::Value>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct V3Lock {
    pub version: u8,
    pub apis: BTreeMap<String, V3Api>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "provenance", rename_all = "snake_case")]
pub enum V3Api {
    Declared(DeclaredEntry),
    Observed(ObservedEntry),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedEntry {
    pub shape: crate::observed::Shape,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Scope {
    All(AllScope),
    Operations(OperationScope),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AllScope {
    #[serde(rename = "all")]
    All,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationScope {
    pub operations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredEntry {
    pub source: String,
    pub scope: Scope,
    pub max_lock_bytes: u64,
    pub contract_bytes: u64,
    pub contract_digest: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: Extensions,
    pub contract: Contract,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    pub operations: BTreeMap<String, WireOperation>,
    pub schemas: BTreeMap<String, WireSchema>,
}
```

Add the remaining wire structs exactly as the design specifies:
`WireOperation`, `WireAuth`, `WireParameter`, `WireSchema`, and
`WireProperty`. Use required fields rather than serde defaults for semantic
data. Derive `Default` for `Contract`, add `Scope::all()` returning
`Scope::All(AllScope::All)`, and add `WireSchema::unknown()` returning an
unknown, non-nullable schema with null format and empty enum/property
collections.

In `canonical.rs`, first add failing tests:

```rust
#[test]
fn schema_ids_are_domain_separated_and_stable() {
    let schema = WireSchema::unknown();
    let first = schema_id(&schema).unwrap();
    let second = schema_id(&schema).unwrap();
    assert_eq!(first, second);
    assert!(first.starts_with("sha256:"));
    assert_eq!(first.len(), 71);
}

#[test]
fn extension_object_order_does_not_change_contract_digest() {
    let left = extension_fixture([("x-b", 2), ("x-a", 1)]);
    let right = extension_fixture([("x-a", 1), ("x-b", 2)]);
    assert_eq!(
        contract_digest(&Scope::all(), &Contract::default(), &left).unwrap(),
        contract_digest(&Scope::all(), &Contract::default(), &right).unwrap()
    );
}

#[test]
fn invalid_extension_keys_are_rejected() {
    let extensions = BTreeMap::from([("vendor".into(), serde_json::json!(true))]);
    assert!(validate_extensions(&extensions)
        .unwrap_err()
        .to_string()
        .contains("extension key must start with x-"));
}
```

- [ ] **Step 2: Run the focused tests and observe RED**

Run:

```powershell
cargo test lockfile::v3::canonical::tests
```

Expected: compilation fails because canonical helpers and constructors do not
exist.

- [ ] **Step 3: Implement exact canonical helpers**

In `canonical.rs`, implement:

```rust
use std::collections::BTreeMap;

use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{Contract, Extensions, Scope, WireSchema};

#[derive(Serialize)]
struct SchemaDigestInput<'a> {
    domain: &'static str,
    schema: &'a WireSchema,
}

#[derive(Serialize)]
struct ContractDigestInput<'a> {
    domain: &'static str,
    scope: &'a Scope,
    contract: &'a Contract,
    extensions: &'a Extensions,
}

pub fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

pub fn schema_id(schema: &WireSchema) -> Result<String> {
    let bytes = serde_json::to_vec(&SchemaDigestInput {
        domain: "apiwatch.schema.v3",
        schema,
    })
    .context("failed to canonicalize v3 schema")?;
    Ok(sha256(&bytes))
}

pub fn contract_digest(
    scope: &Scope,
    contract: &Contract,
    extensions: &Extensions,
) -> Result<String> {
    validate_extensions(extensions)?;
    let extensions = canonical_extensions(extensions);
    let bytes = serde_json::to_vec(&ContractDigestInput {
        domain: "apiwatch.declared-contract.v3",
        scope,
        contract,
        extensions: &extensions,
    })
    .context("failed to canonicalize v3 contract")?;
    Ok(sha256(&bytes))
}

pub fn validate_digest(value: &str) -> Result<()> {
    let hex = value
        .strip_prefix("sha256:")
        .ok_or_else(|| anyhow!("digest must start with sha256:"))?;
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(anyhow!(
            "digest must contain 64 lowercase hexadecimal characters"
        ));
    }
    Ok(())
}
```

Implement `canonical_extensions` recursively. Rebuild objects from keys sorted
into `BTreeMap<String, Value>` before converting back to `Value`. Reject
non-`x-` direct keys in `validate_extensions`.

- [ ] **Step 4: Add failing schema-interning tests**

In `schema.rs`:

```rust
#[test]
fn repeated_schemas_are_interned_once() {
    let contract = fixture_contract_with_repeated_schema();
    let wire = intern_contract(&contract).unwrap();
    assert_eq!(wire.schemas.len(), 1);
    assert_eq!(
        wire.operations["GET /users"].responses["200"]["application/json"],
        wire.operations["GET /accounts"].responses["200"]["application/json"]
    );
}

#[test]
fn tampered_schema_id_is_rejected() {
    let contract = fixture_contract_with_repeated_schema();
    let mut wire = intern_contract(&contract).unwrap();
    let schema = wire.schemas.pop_first().unwrap().1;
    wire.schemas.insert(format!("sha256:{}", "0".repeat(64)), schema);
    assert!(validate_schema_table(&wire)
        .unwrap_err()
        .to_string()
        .contains("schema digest mismatch"));
}

#[test]
fn orphan_schema_is_rejected() {
    let mut wire = Contract::default();
    let schema = WireSchema::unknown();
    wire.schemas.insert(schema_id(&schema).unwrap(), schema);
    assert!(validate_schema_table(&wire)
        .unwrap_err()
        .to_string()
        .contains("orphan schema"));
}
```

- [ ] **Step 5: Run schema tests and observe RED**

Run:

```powershell
cargo test lockfile::v3::schema::tests
```

Expected: compilation fails because interning and validation functions do not
exist.

- [ ] **Step 6: Implement interning and strict table validation**

Move the proven recursive interning algorithm from `src/lock_size.rs` into
`schema.rs`, adapting the wire form to remove redundant auth and parameter
names. Keep the prototype code intact for benchmark reproducibility.

Expose:

```rust
pub fn intern_contract(contract: &ApiContract) -> Result<Contract>;
pub fn validate_schema_table(contract: &Contract) -> Result<()>;
pub fn expand_contract(contract: &Contract) -> Result<ApiContract>;
```

`validate_schema_table` must:

1. validate every key with `canonical::validate_digest`;
2. collect roots from every operation;
3. walk child references with `visiting` and `reachable` sets;
4. reject missing references and cycles;
5. recompute every reachable schema ID;
6. reject every unvisited table key as orphaned.

`expand_contract` parses operation keys and parameter keys strictly, restores
redundant `AuthRequirement.name` and `Parameter.name`, and recursively expands
schema references into `Schema`.

- [ ] **Step 7: Run Task 1 gates**

Run:

```powershell
cargo fmt --all
cargo test lockfile::v3
cargo test --workspace
```

Expected: v3 unit tests and all existing workspace tests pass.

- [ ] **Step 8: Commit Task 1**

```powershell
git add Cargo.toml Cargo.lock src/lockfile/mod.rs src/lockfile/v3
git commit -m "feat: define content-addressed lockfile v3"
```

---

### Task 2: Build, Render, Load, and Validate Complete v3 Entries

**Files:**
- Modify: `src/lockfile/v3/mod.rs`
- Modify: `src/lockfile/v3/canonical.rs`
- Modify: `src/lockfile/v3/schema.rs`
- Create: `testdata/lock/v3_users.lock`
- Modify: `testdata/openapi/privacy_sentinels.yaml`

**Interfaces:**
- Consumes: Task 1 wire types and schema helpers.
- Produces:
  - `v3::build_declared(contract, scope, max_lock_bytes, extensions) -> Result<DeclaredEntry>`
  - `v3::validate_declared(name, entry) -> Result<ApiContract>`
  - `v3::render(lock) -> Result<String>`
  - `v3::load(contents) -> Result<V3Lock>`
  - `v3::contract_yaml(contract) -> Result<Vec<u8>>`

- [ ] **Step 1: Write failing deterministic build/round-trip tests**

Add to `v3/mod.rs`:

```rust
#[test]
fn builds_and_round_trips_a_deterministic_declared_entry() {
    let source =
        crate::openapi::load_contract(Path::new("testdata/openapi/verify_matching.yaml")).unwrap();
    let entry = build_declared(
        &source,
        Scope::all(),
        DEFAULT_MAX_LOCK_BYTES,
        BTreeMap::new(),
    )
    .unwrap();
    let lock = V3Lock::single_declared("users", entry);
    let first = render(&lock).unwrap();
    let second = render(&load(&first).unwrap()).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        validate_declared("users", lock.declared("users").unwrap()).unwrap(),
        source
    );
}

#[test]
fn contract_byte_count_and_digest_are_revalidated() {
    let mut entry = declared_fixture();
    entry.contract_bytes += 1;
    assert!(validate_declared("users", &entry)
        .unwrap_err()
        .to_string()
        .contains("contract byte count mismatch"));

    let mut entry = declared_fixture();
    entry.contract_digest = format!("sha256:{}", "0".repeat(64));
    assert!(validate_declared("users", &entry)
        .unwrap_err()
        .to_string()
        .contains("contract digest mismatch"));
}

#[test]
fn payload_at_limit_succeeds_and_one_byte_under_limit_fails() {
    let contract = fixture_contract();
    let baseline =
        build_declared(&contract, Scope::all(), u64::MAX, BTreeMap::new()).unwrap();
    assert!(build_declared(
        &contract,
        Scope::all(),
        baseline.contract_bytes,
        BTreeMap::new()
    )
    .is_ok());
    assert!(build_declared(
        &contract,
        Scope::all(),
        baseline.contract_bytes - 1,
        BTreeMap::new()
    )
    .unwrap_err()
    .to_string()
    .contains("exceeds"));
}
```

- [ ] **Step 2: Run focused tests and observe RED**

Run:

```powershell
cargo test lockfile::v3::tests
```

Expected: compilation fails on missing build/load/render interfaces.

- [ ] **Step 3: Implement build and validation**

Implement:

```rust
pub fn contract_yaml(contract: &Contract) -> Result<Vec<u8>> {
    let mut bytes = serde_yaml::to_string(contract)
        .context("failed to serialize v3 declared contract")?
        .into_bytes();
    if !bytes.ends_with(b"\n") {
        bytes.push(b'\n');
    }
    Ok(bytes)
}

pub fn build_declared(
    contract: &ApiContract,
    scope: Scope,
    max_lock_bytes: u64,
    extensions: Extensions,
) -> Result<DeclaredEntry> {
    if max_lock_bytes == 0 {
        return Err(anyhow!("max-lock-bytes must be positive"));
    }
    validate_scope(&scope)?;
    canonical::validate_extensions(&extensions)?;
    let contract = schema::intern_contract(contract)?;
    let contract_bytes =
        u64::try_from(contract_yaml(&contract)?.len()).context("contract size overflow")?;
    if contract_bytes > max_lock_bytes {
        return Err(anyhow!(
            "declared contract is {contract_bytes} bytes and exceeds {max_lock_bytes}"
        ));
    }
    let contract_digest = canonical::contract_digest(&scope, &contract, &extensions)?;
    Ok(DeclaredEntry {
        source: "openapi".into(),
        scope,
        max_lock_bytes,
        contract_bytes,
        contract_digest,
        extensions,
        contract,
    })
}
```

`validate_declared` validates name, source, positive limit, scope, extensions,
schema graph, exact byte count, and contract digest before expanding the
contract.

`load` parses with contextual `failed to parse api.lock v3 YAML`, requires
`version == 3`, validates all API names, and validates every declared entry.
`render` validates first and always emits one final newline.

- [ ] **Step 4: Add failing privacy and strict-field tests**

```rust
#[test]
fn production_v3_writer_excludes_privacy_sentinels() {
    let contract =
        crate::openapi::load_contract(Path::new("testdata/openapi/privacy_sentinels.yaml")).unwrap();
    let lock = V3Lock::single_declared(
        "private",
        build_declared(
            &contract,
            Scope::all(),
            DEFAULT_MAX_LOCK_BYTES,
            BTreeMap::new(),
        )
        .unwrap(),
    );
    let rendered = render(&lock).unwrap();
    for sentinel in crate::lock_size::PRIVACY_SENTINELS {
        assert!(!rendered.contains(sentinel), "leaked {sentinel}");
    }
}

#[test]
fn unknown_semantic_fields_are_rejected() {
    let text = golden_text().replace(
        "source: openapi",
        "source: openapi\n    unexpected: true",
    );
    assert!(load(&text)
        .unwrap_err()
        .to_string()
        .contains("unknown field"));
}
```

- [ ] **Step 5: Run tests, generate the golden, and verify byte stability**

Run focused tests, capture the deterministic rendered YAML from the golden
assertion, add those exact bytes to `testdata/lock/v3_users.lock` with
`apply_patch`, and retain:

```rust
#[test]
fn rendered_users_lock_matches_golden() {
    let contract =
        crate::openapi::load_contract(Path::new("testdata/openapi/verify_matching.yaml")).unwrap();
    let lock = V3Lock::single_declared(
        "users",
        build_declared(
            &contract,
            Scope::all(),
            DEFAULT_MAX_LOCK_BYTES,
            BTreeMap::new(),
        )
        .unwrap(),
    );
    assert_eq!(
        render(&lock).unwrap(),
        fs::read_to_string("testdata/lock/v3_users.lock").unwrap()
    );
}
```

Run:

```powershell
cargo fmt --all
cargo test lockfile::v3
cargo test --workspace
git diff --check
```

Expected: all pass and the golden is stable.

- [ ] **Step 6: Commit Task 2**

```powershell
git add src/lockfile/v3 testdata/lock/v3_users.lock testdata/openapi/privacy_sentinels.yaml
git commit -m "feat: validate and render complete v3 contracts"
```

---

### Task 3: Unify Lock State and Implement Strict Legacy Migration

**Files:**
- Modify: `src/lockfile/mod.rs`
- Modify: `src/lockfile/v3/mod.rs`
- Create: `testdata/lock/v2_declared_observed.lock`
- Create: `testdata/lock/v2_multiple_declared.lock`

**Interfaces:**
- Consumes: Task 2 validated `V3Lock` and `DeclaredEntry`.
- Produces:
  - `lockfile::new_v3(name, entry) -> Result<ApiLock>`
  - `lockfile::replace_declared(lock, name, entry) -> Result<ApiLock>`
  - `lockfile::render(&ApiLock) -> Result<String>` supporting v1/v2/v3
  - `lockfile::load(&Path) -> Result<ApiLock>` supporting v1/v2/v3

- [ ] **Step 1: Refactor `ApiLock` behind version-neutral maps**

Replace the current dual-use `apis` field with:

```rust
pub struct ApiLock {
    version: u8,
    legacy_declared: BTreeMap<String, LockedApi>,
    declared: BTreeMap<String, v3::DeclaredEntry>,
    observed: BTreeMap<String, Shape>,
}
```

Update existing v1/v2 loaders, renderers, record functions, and unit fixtures
mechanically. Run:

```powershell
cargo test lockfile::tests
cargo test --workspace
```

Expected: all pre-v3 behavior stays green before migration behavior is added.

- [ ] **Step 2: Add failing migration-policy tests**

```rust
#[test]
fn replaces_the_sole_legacy_declared_entry_and_preserves_observed() {
    let lock = load(Path::new("testdata/lock/v2_declared_observed.lock")).unwrap();
    let entry = v3_declared_fixture();
    let migrated = replace_declared(lock, "users", entry.clone()).unwrap();
    let rendered = render(&migrated).unwrap();
    assert!(rendered.starts_with("version: 3\n"));
    assert!(rendered.contains("provenance: declared"));
    assert!(rendered.contains("provenance: observed"));
    assert_eq!(migrated.declared["users"], entry);
}

#[test]
fn refuses_partial_migration_of_multiple_legacy_entries() {
    let lock = load(Path::new("testdata/lock/v2_multiple_declared.lock")).unwrap();
    let error = replace_declared(lock, "users", v3_declared_fixture()).unwrap_err();
    assert!(error.to_string().contains("requires original sources"));
    assert!(error.to_string().contains("payments"));
}

#[test]
fn refuses_to_replace_an_observed_name() {
    let lock = load(Path::new("testdata/lock/v2_declared_observed.lock")).unwrap();
    assert!(replace_declared(lock, "portfolio", v3_declared_fixture())
        .unwrap_err()
        .to_string()
        .contains("is observed"));
}
```

- [ ] **Step 3: Run focused migration tests and observe RED**

Run:

```powershell
cargo test lockfile::tests::replaces_the_sole_legacy
cargo test lockfile::tests::refuses_partial_migration
cargo test lockfile::tests::refuses_to_replace_an_observed
```

Expected: compilation fails because replacement is not implemented.

- [ ] **Step 4: Implement version-aware replacement**

Implement `replace_declared` with these exact branches:

```rust
pub fn replace_declared(
    mut lock: ApiLock,
    name: &str,
    entry: v3::DeclaredEntry,
) -> Result<ApiLock> {
    let name = normalized_name(name)?.to_owned();
    if lock.observed.contains_key(&name) {
        return Err(anyhow!("api {name} is observed and cannot be replaced as declared"));
    }
    if lock.version < 3 {
        if !lock.legacy_declared.contains_key(&name) {
            return Err(anyhow!("legacy declared api {name} not found"));
        }
        let remaining: Vec<_> = lock
            .legacy_declared
            .keys()
            .filter(|candidate| candidate.as_str() != name)
            .cloned()
            .collect();
        if !remaining.is_empty() {
            return Err(anyhow!(
                "cannot migrate api.lock to v3; original sources are required for: {}",
                remaining.join(", ")
            ));
        }
        lock.legacy_declared.clear();
    }
    lock.version = 3;
    lock.declared.insert(name, entry);
    Ok(lock)
}
```

For an existing v3 lock, replacement may add a new declared name. For v1/v2,
the target must exist and be the sole legacy declared entry.

- [ ] **Step 5: Add v3 load/render integration**

Dispatch `load` version `3` through `v3::load`, then translate validated
variants into `ApiLock`. Dispatch `render` version `3` by building `V3Lock`
from `declared` and `observed`.

Ensure `record_observed` preserves version 3 when modifying a v3 file and still
upgrades v1 to v2 when no v3 declared entry exists.

- [ ] **Step 6: Run Task 3 gates**

```powershell
cargo fmt --all
cargo test lockfile::tests
cargo test --workspace
git diff --check
```

Expected: migration tests and all legacy/observed tests pass.

- [ ] **Step 7: Commit Task 3**

```powershell
git add src/lockfile testdata/lock/v2_declared_observed.lock testdata/lock/v2_multiple_declared.lock
git commit -m "feat: migrate legacy locks safely to v3"
```

---

### Task 4: Add Safe v3 Lock Creation, Update, Scope, and Size CLI

**Files:**
- Create: `src/lockfile/atomic.rs`
- Modify: `src/lockfile/mod.rs`
- Modify: `src/lock_size.rs`
- Modify: `src/cli.rs`
- Modify: `src/main.rs`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `tests/cli_lock.rs`
- Create: `testdata/openapi/v3_scoped.yaml`

**Interfaces:**
- Consumes: `v3::build_declared`, `new_v3`, `replace_declared`, selector parser.
- Produces:
  - `atomic::write_new(path, bytes) -> Result<()>`
  - `atomic::replace(path, bytes) -> Result<()>`
  - CLI `--update`, repeatable `--include-operation`, and
    `--max-lock-bytes`.

- [ ] **Step 1: Add runtime `tempfile` and failing atomic-write tests**

Move/add:

```toml
tempfile = "3"
```

under root `[dependencies]`.

Create `atomic.rs` tests:

```rust
#[test]
fn write_new_refuses_to_replace_existing_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("api.lock");
    fs::write(&path, "preserve").unwrap();
    assert!(write_new(&path, b"replacement").is_err());
    assert_eq!(fs::read_to_string(path).unwrap(), "preserve");
}

#[test]
fn replace_atomically_updates_existing_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("api.lock");
    fs::write(&path, "old").unwrap();
    replace(&path, b"new").unwrap();
    assert_eq!(fs::read_to_string(path).unwrap(), "new");
}
```

- [ ] **Step 2: Run atomic tests and observe RED**

```powershell
cargo test lockfile::atomic::tests
```

Expected: compilation fails because the module and functions do not exist.

- [ ] **Step 3: Implement same-directory writes**

```rust
fn temporary(path: &Path) -> Result<tempfile::NamedTempFile> {
    let parent = path
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create lockfile directory {}", parent.display()))?;
    tempfile::NamedTempFile::new_in(parent)
        .context("failed to create temporary lockfile")
}

fn fill(mut file: tempfile::NamedTempFile, bytes: &[u8]) -> Result<tempfile::NamedTempFile> {
    file.write_all(bytes)
        .and_then(|_| file.flush())
        .and_then(|_| file.as_file().sync_all())
        .context("failed to write temporary lockfile")?;
    Ok(file)
}

pub fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    fill(temporary(path)?, bytes)?
        .persist_noclobber(path)
        .map_err(|error| anyhow!("failed to create lockfile: {}", error.error))?;
    Ok(())
}

pub fn replace(path: &Path, bytes: &[u8]) -> Result<()> {
    fill(temporary(path)?, bytes)?
        .persist(path)
        .map_err(|error| anyhow!("failed to replace lockfile: {}", error.error))?;
    Ok(())
}
```

- [ ] **Step 4: Add failing CLI tests**

Update/add:

```rust
#[test]
fn lock_creates_a_deterministic_v3_file() {
    let output = temporary_output();
    apiwatch()
        .args([
            "lock",
            "testdata/openapi/verify_matching.yaml",
            "--name",
            "users",
            "--output",
            path(&output),
        ])
        .assert()
        .success();
    assert_eq!(
        fs::read_to_string(output).unwrap(),
        fs::read_to_string("testdata/lock/v3_users.lock").unwrap()
    );
}

#[test]
fn lock_requires_update_and_preserves_existing_bytes() {
    let output = existing_output("preserve");
    apiwatch().args(lock_args(&output)).assert().code(2);
    assert_eq!(fs::read_to_string(output).unwrap(), "preserve");
}

#[test]
fn lock_update_preserves_observed_entries() {
    let output = copy_fixture("testdata/lock/v2_declared_observed.lock");
    apiwatch()
        .args(lock_args(&output))
        .arg("--update")
        .assert()
        .success();
    let rendered = fs::read_to_string(output).unwrap();
    assert!(rendered.starts_with("version: 3\n"));
    assert!(rendered.contains("provenance: observed"));
}

#[test]
fn lock_stores_exact_scope_and_rejects_missing_selectors() {
    let output = temporary_output();
    apiwatch()
        .args([
            "lock",
            "testdata/openapi/v3_scoped.yaml",
            "--name",
            "scoped",
            "--output",
            path(&output),
            "--include-operation",
            "GET /users",
        ])
        .assert()
        .success();
    assert!(fs::read_to_string(&output)
        .unwrap()
        .contains("operations:\n      - GET /users"));

    let missing = temporary_output();
    apiwatch()
        .args(lock_args(&missing))
        .args(["--include-operation", "DELETE /missing"])
        .assert()
        .code(2);
    assert!(!missing.exists());
}
```

Add a size-failure test using a very small `--max-lock-bytes 1`, asserting no
output is created or modified.

- [ ] **Step 5: Run CLI tests and observe RED**

```powershell
cargo test --test cli_lock
```

Expected: new tests fail because CLI options and v3 creation do not exist.

- [ ] **Step 6: Add exact CLI fields**

Change `Command::Lock` to include:

```rust
#[arg(long)]
update: bool,
#[arg(long = "include-operation")]
include_operations: Vec<String>,
#[arg(long, default_value_t = crate::lockfile::DEFAULT_MAX_LOCK_BYTES)]
max_lock_bytes: u64,
```

Expose `pub const DEFAULT_MAX_LOCK_BYTES: u64 = v3::DEFAULT_MAX_LOCK_BYTES;`
from `src/lockfile/mod.rs`.

- [ ] **Step 7: Implement create/update dispatch**

In `main.rs`:

```rust
let contract = openapi::load_contract(&openapi)?;
let scoped = lock_size::scope_contract(&contract, &include_operations)?;
let scope = lockfile::scope_from_selectors(&include_operations)?;
let entry = lockfile::build_v3_declared(&scoped, scope, max_lock_bytes)?;

let rendered = if update {
    if !output.exists() {
        anyhow::bail!("--update requires an existing lockfile");
    }
    let existing = lockfile::load(&output)?;
    let updated = lockfile::replace_declared(existing, &name, entry)?;
    lockfile::render(&updated)?
} else {
    let created = lockfile::new_v3(&name, entry)?;
    lockfile::render(&created)?
};

if update {
    lockfile::atomic_replace(&output, rendered.as_bytes())?;
} else {
    lockfile::atomic_write_new(&output, rendered.as_bytes())?;
}
```

Do not pre-check existence for plain create; `persist_noclobber` is the final
race-safe authority. Map create/update filesystem errors to exit `2`.

- [ ] **Step 8: Run Task 4 gates**

```powershell
cargo fmt --all
cargo test --test cli_lock
cargo test lockfile::
cargo test --workspace
git diff --check
```

Expected: safe create/update, scope, size, migration, and all regressions pass.

- [ ] **Step 9: Commit Task 4**

```powershell
git add Cargo.toml Cargo.lock src/cli.rs src/main.rs src/lock_size.rs src/lockfile tests/cli_lock.rs testdata/openapi/v3_scoped.yaml
git commit -m "feat: write and update v3 locks atomically"
```

---

### Task 5: Route Full Declared Verify Through `diff_contracts`

**Files:**
- Modify: `src/lockfile/mod.rs`
- Modify: `src/lock_size.rs`
- Modify: `src/main.rs`
- Modify: `tests/cli_verify.rs`
- Create: `testdata/openapi/v3_d16_old.yaml`
- Create: `testdata/openapi/v3_d16_new.yaml`
- Create: `testdata/openapi/v3_scoped_added_unrelated.yaml`
- Create: `testdata/openapi/v3_scoped_without_users.yaml`

**Interfaces:**
- Consumes: validated v3 contracts and stored `Scope`.
- Produces:
  - `VerifyTargetKind::FullDeclared { contract, scope }`
  - `scope_current_for_verify(current, scope) -> ApiContract`
  - one `Vec<diff::Change>` for full declared Verify.

- [ ] **Step 1: Replace Verify target internals with explicit variants**

Introduce:

```rust
pub enum VerifyTargetKind {
    LegacyDeclared {
        operations: BTreeSet<LockedOperation>,
    },
    FullDeclared {
        contract: ApiContract,
        scope: v3::Scope,
    },
    Observed {
        shape: Shape,
    },
}

pub struct VerifyTarget {
    name: String,
    kind: VerifyTargetKind,
}
```

Add `kind(&self) -> &VerifyTargetKind`. Adapt current tests without changing
legacy comparison behavior yet.

- [ ] **Step 2: Add failing scoped/full Verify tests**

```rust
#[test]
fn verify_v3_reports_schema_and_auth_changes_through_diff_messages() {
    let lock = lock_from("testdata/openapi/v3_d16_old.yaml", "d16");
    apiwatch()
        .args([
            "verify",
            "testdata/openapi/v3_d16_new.yaml",
            "--name",
            "d16",
            "--lock",
            path(&lock),
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("authentication"))
        .stdout(predicate::str::contains("parameter"))
        .stdout(predicate::str::contains("successful response"));
}

#[test]
fn verify_v3_scope_ignores_unselected_additions() {
    let lock = scoped_lock("GET /users");
    apiwatch()
        .args(verify_args("testdata/openapi/v3_scoped_added_unrelated.yaml", &lock))
        .assert()
        .success()
        .stdout("Verified scoped\n");
}

#[test]
fn verify_v3_selected_operation_removal_is_breaking() {
    let lock = scoped_lock("GET /users");
    apiwatch()
        .args(verify_args("testdata/openapi/v3_scoped_without_users.yaml", &lock))
        .assert()
        .code(1)
        .stdout(predicate::str::contains("endpoint removed"));
}

#[test]
fn verify_v3_warning_only_change_exits_zero() {
    let lock = lock_from("testdata/openapi/status_error_added_old.yaml", "users");
    apiwatch()
        .args(verify_args("testdata/openapi/status_error_added_new.yaml", &lock))
        .assert()
        .success()
        .stdout(predicate::str::contains("Warnings"));
}
```

- [ ] **Step 3: Run focused tests and observe RED**

```powershell
cargo test --test cli_verify verify_v3
```

Expected: tests fail because v3 target selection and shared diff dispatch are
not implemented.

- [ ] **Step 4: Implement non-strict Verify scoping**

Add:

```rust
pub fn scope_current_for_verify(
    current: &ApiContract,
    selectors: &[String],
) -> Result<ApiContract> {
    if selectors.is_empty() {
        return Ok(current.clone());
    }
    let keys = selectors
        .iter()
        .map(|value| parse_operation_selector(value))
        .collect::<Result<BTreeSet<_>>>()?;
    Ok(ApiContract {
        operations: current
            .operations
            .iter()
            .filter(|(key, _)| keys.contains(*key))
            .map(|(key, operation)| (key.clone(), operation.clone()))
            .collect(),
    })
}
```

Unlike lock-time `scope_contract`, this function does not reject absent
selected operations.

- [ ] **Step 5: Implement full Verify dispatch**

In `main.rs`, match `VerifyTargetKind`:

```rust
VerifyTargetKind::FullDeclared { contract: locked, scope } => {
    let current = openapi::load_contract_input(&openapi)?;
    let current = lockfile::scope_current_for_verify(&current, scope)?;
    let changes = diff::diff_contracts(locked, &current);
    render_declared_verify(&lock_path, target.name(), format, &changes, None)?;
    Ok(if changes
        .iter()
        .any(|change| change.severity == Severity::Breaking)
    {
        1
    } else {
        0
    })
}
```

Leave observed behavior unchanged. Keep legacy route comparison until Task 6
maps it into shared findings.

- [ ] **Step 6: Build exact D-16 fixtures and assertion**

The old fixture has one operation with no auth, a required query parameter
`account_id: string`, and successful responses `200` and `204`. The new
fixture:

- adds bearer auth;
- renames `account_id` to `account: integer`;
- removes `204`.

Assert the full JSON findings in order:

```rust
assert_eq!(summary.breaking, 4);
assert_eq!(
    messages,
    vec![
        "authentication bearerAuth (bearer) added",
        "query parameter account_id removed",
        "query parameter account added as required",
        "response status 204 removed",
    ]
);
```

The fixture represents the approved rename/retype as removal of required
`account_id: string` plus addition of required `account: integer`. Do not
change the diff engine or expected messages for this task.

- [ ] **Step 7: Run Task 5 gates**

```powershell
cargo fmt --all
cargo test --test cli_verify verify_v3
cargo test --test cli_verify
cargo test --workspace
git diff --check
```

Expected: D-16 has four breaking findings; warning-only full Verify exits `0`;
all regressions pass.

- [ ] **Step 8: Commit Task 5**

```powershell
git add src/lockfile/mod.rs src/lock_size.rs src/main.rs tests/cli_verify.rs testdata/openapi/v3_d16_old.yaml testdata/openapi/v3_d16_new.yaml testdata/openapi/v3_scoped*
git commit -m "feat: verify complete v3 contracts through diff"
```

---

### Task 6: Unify Declared Verify Output and Add Legacy Limitations

**Files:**
- Modify: `src/lockfile/mod.rs`
- Modify: `src/main.rs`
- Modify: `src/output/mod.rs`
- Modify: `tests/cli_verify.rs`

**Interfaces:**
- Consumes: `Vec<Change>` from full Verify and legacy route comparison.
- Produces:
  - `output::render_declared_verify_text(name, changes) -> String`
  - `output::render_declared_verify_json(name, coverage, limitation, changes) -> Result<String>`
  - `output::render_declared_verify_sarif(path, name, limitation, changes) -> Result<String>`
  - `Coverage::{Full, Routes}`
  - `Limitation::RouteOnlyLock`

- [ ] **Step 1: Map legacy route changes into `Change`**

Change legacy comparison to return `Vec<Change>`:

```rust
for operation in target.difference(&current) {
    changes.push(Change {
        severity: Severity::Breaking,
        operation: operation.to_operation_key()?,
        message: "endpoint removed".into(),
    });
}
for operation in current.difference(target) {
    changes.push(Change {
        severity: Severity::Warning,
        operation: operation.to_operation_key()?,
        message: "endpoint added outside route-only lock".into(),
    });
}
```

Run existing legacy tests and update their expected text only after observing
the intentional failure.

- [ ] **Step 2: Add failing JSON output tests**

```rust
#[test]
fn verify_v3_json_uses_full_coverage_and_diff_findings() {
    let output = run_v3_verify_json();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["version"], 2);
    assert_eq!(json["coverage"], "full");
    assert_eq!(json["limitations"], serde_json::json!([]));
    assert!(json["summary"]["breaking"].as_u64().unwrap() > 0);
    assert!(json["changes"][0]["message"].is_string());
}

#[test]
fn legacy_json_reports_route_only_limitation_without_stderr_noise() {
    let output = run_legacy_verify_json();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["coverage"], "routes");
    assert_eq!(json["limitations"][0]["code"], "route_only_lock");
    assert!(output.stderr.is_empty());
}
```

- [ ] **Step 3: Run JSON tests and observe RED**

```powershell
cargo test --test cli_verify verify_v3_json
cargo test --test cli_verify legacy_json_reports
```

Expected: old version-1 route JSON lacks coverage/limitations and full v3 JSON
is not implemented.

- [ ] **Step 4: Implement version-2 declared Verify JSON**

Define:

```rust
#[derive(Clone, Copy)]
pub enum Coverage {
    Full,
    Routes,
}

#[derive(Serialize)]
struct DeclaredVerifyJson<'a> {
    version: u8,
    command: &'static str,
    name: &'a str,
    provenance: &'static str,
    coverage: &'static str,
    limitations: Vec<LimitationJson>,
    summary: DiffSummary,
    changes: Vec<DiffJsonChange<'a>>,
}
```

Factor `summarize_changes` and `json_changes` out of Diff JSON so Diff and full
Verify use the same severity/message mapping.

Route coverage returns one limitation:

```rust
LimitationJson {
    code: "route_only_lock",
    message: "api.lock v1/v2 declared entry is route-only; full contract changes are not verified",
}
```

- [ ] **Step 5: Add failing text and SARIF limitation tests**

```rust
#[test]
fn legacy_text_warns_on_stderr() {
    run_legacy_verify_text()
        .assert()
        .stderr(predicate::str::contains(
            "api.lock v1/v2 declared entry is route-only",
        ));
}

#[test]
fn legacy_sarif_uses_tool_execution_notification() {
    let output = run_legacy_verify_sarif();
    assert!(output.stderr.is_empty());
    let sarif: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        sarif["runs"][0]["invocations"][0]["toolExecutionNotifications"][0]["descriptor"]["id"],
        "apiwatch/route-only-lock"
    );
}
```

- [ ] **Step 6: Run text/SARIF tests and observe RED**

```powershell
cargo test --test cli_verify legacy_text_warns
cargo test --test cli_verify legacy_sarif_uses
```

Expected: warnings are missing.

- [ ] **Step 7: Implement text and SARIF limitations**

Text prints the exact approved warning to stderr only for legacy text mode.
Full text uses `render_changes`, except an empty result prints
`Verified <name>\n`.

Extend `SarifRun`:

```rust
#[serde(skip_serializing_if = "Vec::is_empty")]
invocations: Vec<SarifInvocation>,
```

Add serializable invocation/notification/descriptor structs with SARIF field
renames. Full Verify uses no notification. Legacy Verify supplies one
warning-level notification and otherwise uses diff rules/results.

- [ ] **Step 8: Run Task 6 gates**

```powershell
cargo fmt --all
cargo test output::tests
cargo test --test cli_verify
cargo test --workspace
git diff --check
```

Expected: full and legacy text/JSON/SARIF tests pass; observed output remains
byte-compatible.

- [ ] **Step 9: Commit Task 6**

```powershell
git add src/lockfile/mod.rs src/main.rs src/output/mod.rs tests/cli_verify.rs
git commit -m "feat: align declared verify output with diff"
```

---

### Task 7: Complete Documentation, Action, CI, and Roadmap Acceptance

**Files:**
- Modify: `docs/lockfile-spec.md`
- Modify: `README.md`
- Modify: `ROADMAP.md`
- Modify: `CHANGELOG.md`
- Modify: `.github/workflows/ci.yml`
- Modify: `scripts/release_smoke.py`
- Create/update: `implementation-log/2026-07-25-lockfile-v3.md`

**Interfaces:**
- Consumes: complete user-facing v3 behavior.
- Produces: accurate documentation, CI enforcement, D-16 exit evidence, and
  ignored implementation record.

- [ ] **Step 1: Add release-smoke assertions before docs**

Extend `scripts/release_smoke.py` to:

1. create a v3 lock from a copied fixture;
2. verify an unchanged source successfully;
3. verify D-16 and assert exit `1`;
4. parse JSON and assert `coverage == "full"` and four breaking findings;
5. confirm a legacy Verify includes the route-only limitation.

Run:

```powershell
python scripts/release_smoke.py
```

Expected: RED until all command/output details match production behavior; then
PASS after correcting only integration defects.

- [ ] **Step 2: Update the lockfile specification**

Replace “Planned Version 3” with an implemented v3 section that copies the
approved exact fields and includes:

```yaml
version: 3
apis:
  users:
    provenance: declared
    source: openapi
    scope: all
    max_lock_bytes: 5242880
    contract_bytes: 18432
    contract_digest: sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
    contract:
      operations: {}
      schemas: {}
```

Document digest validation, payload byte definition, strict fields,
extensions, scoping, create/update behavior, migration refusal, and legacy
warnings. Keep v1/v2 examples and label them readable route-only formats.

- [ ] **Step 3: Update README and changelog**

Add executable examples:

```powershell
apiwatch lock openapi.yaml --name users --output api.lock
apiwatch lock openapi.yaml --name users --output api.lock --update
apiwatch lock openapi.yaml --name users --output api.lock `
  --include-operation "GET /users/{id}" `
  --max-lock-bytes 5242880
apiwatch verify current.yaml --name users --lock api.lock
```

State that full v3 declared Verify uses the diff engine, warning-only changes
exit `0`, legacy locks are route-only, and migration requires original
sources. Add an Unreleased changelog entry.

- [ ] **Step 4: Update roadmap status only after acceptance passes**

Mark Phase 1 ordered items 2–9 completed. Update the Phase 1 exit criterion
with a link to the D-16 fixture/test. Do not alter later phase ordering.

- [ ] **Step 5: Strengthen CI smoke**

Keep the existing workspace and report checks. Add one explicit command after
Rust tests:

```yaml
- run: cargo test --test cli_verify verify_v3_d16_reports_four_breaking_findings
```

If this test already runs in `cargo test --workspace`, the explicit line is
still retained as the roadmap acceptance signal.

- [ ] **Step 6: Run documentation and Action smoke**

```powershell
python scripts/release_smoke.py
rg -n "version: 3|--update|--include-operation|max-lock-bytes|route-only|diff_contracts" README.md docs/lockfile-spec.md ROADMAP.md CHANGELOG.md
```

Expected: smoke passes and every user-facing policy is documented.

- [ ] **Step 7: Write the ignored implementation log**

Create `implementation-log/2026-07-25-lockfile-v3.md` containing:

- approved schema and migration decisions;
- files/areas touched;
- red/green cycles for each task;
- exact D-16 findings;
- size/privacy/integrity evidence;
- verification results;
- known parser limitations;
- next roadmap task.

Confirm it is ignored:

```powershell
git check-ignore -v implementation-log/2026-07-25-lockfile-v3.md
```

- [ ] **Step 8: Run Task 7 gates and commit tracked files**

```powershell
cargo fmt --all -- --check
cargo test --workspace
python scripts/release_smoke.py
git diff --check
git add .github/workflows/ci.yml README.md ROADMAP.md CHANGELOG.md docs/lockfile-spec.md scripts/release_smoke.py
git commit -m "docs: complete Phase 1 lockfile v3"
```

Do not force-add the implementation log.

---

### Task 8: Full Verification, Review, and Branch Integration Gate

**Files:**
- Review all files changed by Tasks 1–7.
- Update: `implementation-log/2026-07-25-lockfile-v3.md`

**Interfaces:**
- Consumes: the complete branch.
- Produces: fresh evidence suitable for merge and push.

- [ ] **Step 1: Run formatting and strict stable Clippy**

```powershell
cargo fmt --all -- --check
$stableRustc = rustup which --toolchain stable rustc
$env:RUSTC = $stableRustc
$env:CARGO_TARGET_DIR = 'target\lockfile-v3-clippy-stable'
rustup run stable cargo clippy --workspace --all-targets --all-features -- -D warnings
Remove-Item Env:RUSTC
Remove-Item Env:CARGO_TARGET_DIR
```

Expected: both commands exit `0` with no warnings.

- [ ] **Step 2: Run workspace and MSRV gates**

```powershell
$env:CARGO_TARGET_DIR = 'target\lockfile-v3-tests'
cargo test --workspace
Remove-Item Env:CARGO_TARGET_DIR
$msrvRustc = rustup which --toolchain 1.86.0 rustc
$env:RUSTC = $msrvRustc
$env:CARGO_TARGET_DIR = 'target\lockfile-v3-msrv'
rustup run 1.86.0 cargo check --workspace --locked
Remove-Item Env:RUSTC
Remove-Item Env:CARGO_TARGET_DIR
```

Expected: all active workspace tests pass; compatibility tests remain ignored
in the standard run; Rust 1.86 check passes.

- [ ] **Step 3: Run Python, corpus, report, and release gates**

```powershell
python -m unittest discover -s scripts/tests -p "test_*.py"
python scripts/fetch_compat_specs.py
cargo test --test compat -- --ignored --nocapture
cargo run -p apiwatch-lock-size-report -- --manifest compat/specs.json --compat-dir .compat-cache --privacy-fixture testdata/openapi/privacy_sentinels.yaml --max-lock-bytes 5242880 --json-out docs/benchmarks/phase-1-lock-size-report.json --markdown-out docs/benchmarks/phase-1-lock-size-report.md --check
python scripts/release_smoke.py
```

Expected: Python tests pass; five corpus expectations pass; committed reports
are unchanged; release smoke passes.

- [ ] **Step 4: Run targeted acceptance and preservation checks**

```powershell
cargo test --test cli_lock
cargo test --test cli_verify verify_v3_d16_reports_four_breaking_findings -- --exact
cargo test --test cli_verify legacy
cargo test production_v3_writer_excludes_privacy_sentinels -- --exact
```

Expected: safe writes, D-16, legacy limitations, and privacy all pass.

- [ ] **Step 5: Audit branch scope and whitespace**

```powershell
git diff --check main...HEAD
git status --short
git log --oneline main..HEAD
git diff --stat main...HEAD
```

Expected: tracked tree clean; only the approved v3 design, plan,
implementation, fixtures, tests, CI, and documentation are present.

- [ ] **Step 6: Request code review and resolve findings**

Use `superpowers:requesting-code-review`. Review specifically:

- strict serde boundaries and untrusted-input diagnostics;
- digest domain separation and recursive map canonicalization;
- schema reachability/collision handling;
- create/update race safety and failure preservation;
- migration refusal rules;
- scoped-removal semantics;
- full Verify severity/exit behavior;
- JSON/SARIF schema correctness;
- privacy exclusions.

If review finds a defect, use systematic debugging and TDD, rerun the affected
focused test, then rerun Steps 1–5.

- [ ] **Step 7: Update the implementation log with final evidence**

Record exact test counts, Clippy/MSRV results, compatibility results, branch,
and final HEAD. Keep the file ignored.

- [ ] **Step 8: Finish the branch**

Use `superpowers:finishing-a-development-branch`. The standing integration
choice is:

1. merge `codex/phase-1-lockfile-v3` into `main`;
2. rerun `cargo test --workspace` on merged `main`;
3. push `main` to `origin`;
4. delete the merged local feature branch.

Do not merge or push if any verification or review gate is failing.
