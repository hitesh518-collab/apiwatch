# APIWatch Phase 2 Trustworthy Comparison Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate the eleven audited comparison-engine defects, introduce deterministic lockfile v4 coverage for the corrected semantics, and keep direct `diff` and declared `verify` on the same comparison path.

**Architecture:** Extend the normalized `ApiContract` with explicit semantic identities and add a separate `src/lockfile/v4/` wire implementation with v4 digest domains. Implement each D-01 through D-11 defect as a regression-first slice in roadmap order; v3 reconstruction uses explicit unknown sentinels for data it never stored so it remains readable without inventing coverage.

**Tech Stack:** Rust 2021, Rust 1.86 MSRV, `openapiv3` 2.x, Serde/YAML/JSON, SHA-256, Clap, assert_cmd, Python unittest, GitHub Actions.

## Global Constraints

- Keep Rust `rust-version = "1.86"` and verify with the Rust 1.86 toolchain.
- Keep the default declared-entry payload ceiling at exactly `5_242_880` bytes.
- Preserve correct existing text, JSON, SARIF, rule-ID, fingerprint, and ordering behavior.
- Every D-01 through D-11 fix starts with a failing regression fixture and focused failing test.
- Direct `diff` and v4 declared `verify` must both call `diff_contracts`.
- Versions 1 and 2 remain route-only; v3 remains readable with `partial` coverage; v4 reports `full` coverage.
- Never persist descriptions, examples, schema defaults, server-variable defaults, URL user-info, literal server query values, credentials, or observed scalar values.
- Keep lock creation and update atomic; every failure preserves existing destination bytes.
- Use deterministic `BTreeMap`/`BTreeSet` ordering and v4-specific schema and contract digest domains.
- Do not add configuration files, severity overrides, OpenAPI 3.1, external references, observed capture, or new protocols.
- Do not push, tag, publish, or repin package metadata during plan execution.

## File and Responsibility Map

- `src/contract/mod.rs`: normalized semantic model and legacy-unknown sentinels.
- `src/openapi/mod.rs`: extract effective OpenAPI 3.0 semantics into the model.
- `src/openapi/identity.rs`: canonical media type, server/auth URL, and path-template identities.
- `src/diff/mod.rs`: all D-01 through D-11 comparison and severity rules.
- `src/lockfile/mod.rs`: version dispatch, shared scope, migration, and Verify target coverage.
- `src/lockfile/v4/mod.rs`: v4 wire types, entry validation, payload limits, and rendering.
- `src/lockfile/v4/schema.rs`: model-to-wire interning and wire-to-model expansion.
- `src/lockfile/v4/canonical.rs`: v4 schema IDs, contract digests, and extension validation.
- `src/main.rs`: v4 Lock and coverage-aware Verify dispatch.
- `src/output/mod.rs`: v3 partial-coverage JSON and SARIF limitation reporting.
- `src/lock_size.rs`: operation scoping across canonical path identities.
- `tests/cli_diff.rs`: public diff behavior for every audit defect.
- `tests/cli_lock.rs`: v4 creation, update, migration, size, privacy, and atomicity.
- `tests/cli_verify.rs`: v3 partial coverage and v4 diff/Verify parity.
- `tests/compat.rs`: pinned-corpus self-diff and v4 size gates.
- `testdata/openapi/phase2_d*_old.yaml`, `phase2_d*_new.yaml`: one audit fixture pair per defect.
- `testdata/lock/v4_users.lock`, `v4_private.lock`: deterministic v4 golden locks.
- `tools/lock-size-report/`: production-v4 payload measurement and deterministic Phase 2 report.
- `docs/benchmarks/phase-2-v4-lock-size-report.{json,md}`: committed v4 size evidence.
- `README.md`, `ROADMAP.md`, `CHANGELOG.md`, `DESIGN.md`, `docs/change-rules.md`, `docs/lockfile-spec.md`: public behavior and migration documentation.
- `.github/workflows/ci.yml`, `scripts/release_smoke.py`: phase acceptance gates.

---

### Task 1: Establish the v4 Lock and Coverage Foundation

**Files:**
- Create: `src/lockfile/v4/mod.rs`
- Create: `src/lockfile/v4/schema.rs`
- Create: `src/lockfile/v4/canonical.rs`
- Create: `testdata/lock/v4_users.lock`
- Create: `testdata/lock/v4_private.lock`
- Modify: `src/lockfile/mod.rs`
- Modify: `src/main.rs`
- Modify: `src/output/mod.rs`
- Modify: `tests/cli_lock.rs`
- Modify: `tests/cli_verify.rs`

**Interfaces:**
- Produces: `lockfile::Scope`, shared by v3 and v4.
- Produces: `DeclaredCoverage::{PartialV3, FullV4}`.
- Produces: `VerifyTargetKind::Declared { contract, scope, coverage }`.
- Produces: `build_v4_declared(&ApiContract, Scope, u64) -> Result<v4::DeclaredEntry>`.
- Produces: `new_v4(&str, v4::DeclaredEntry) -> Result<ApiLock>`.
- Produces: `replace_declared_v4(ApiLock, &str, v4::DeclaredEntry) -> Result<ApiLock>`.
- Produces: `Coverage::Partial` and `Limitation::Phase2RelockRequired`.

- [ ] **Step 1: Add failing v4 create, round-trip, migration, and coverage tests**

Add tests with these assertions:

```rust
#[test]
fn lock_writes_version_four() {
    let output = temp_lock_path("v4-create");
    Command::cargo_bin("apiwatch")
        .unwrap()
        .args([
            "lock",
            "testdata/openapi/verify_matching.yaml",
            "--name",
            "users",
            "--output",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    assert!(fs::read_to_string(&output).unwrap().starts_with("version: 4\n"));
    fs::remove_file(output).ok();
}

#[test]
fn verify_v3_json_reports_partial_phase_two_coverage() {
    let output = verify_command("testdata/openapi/verify_matching.yaml", "users")
        .args(["--lock", "testdata/lock/v3_users.lock", "--format", "json"])
        .output()
        .unwrap();
    let rendered: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(rendered["coverage"], "partial");
    assert_eq!(
        rendered["limitations"][0]["code"],
        "phase2_relock_required"
    );
}
```

Also assert that updating a sole v3 declared entry writes v4 and preserves an
observed entry, while a v3 file with another declared entry is rejected and
preserved byte-for-byte.

- [ ] **Step 2: Run the focused tests and record the expected red state**

Run:

```powershell
cargo test --test cli_lock lock_writes_version_four -- --exact
cargo test --test cli_verify verify_v3_json_reports_partial_phase_two_coverage -- --exact
```

Expected: the first test finds `version: 3`; the second finds `coverage:
"full"`.

- [ ] **Step 3: Move the shared scope type out of v3**

Define in `src/lockfile/mod.rs`:

```rust
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
    operations: Vec<String>,
}
```

Update v3 to import `super::Scope`; preserve its serialized bytes and existing
scope validation tests.

- [ ] **Step 4: Add the v4 module with distinct integrity domains**

Port the v3 entry, contract, interning, validation, render, and load structure
into `src/lockfile/v4/`. Change the wire version and domains exactly:

```rust
const SCHEMA_DOMAIN: &str = "apiwatch.schema.v4";
const CONTRACT_DOMAIN: &str = "apiwatch.declared-contract.v4";

pub(super) struct V4Lock {
    version: u8,
    apis: BTreeMap<String, V4Api>,
}
```

Keep `#[serde(deny_unknown_fields)]`, schema reachability, collision rejection,
`contract_bytes`, `contract_digest`, extension validation, control-character
validation, and the final-newline payload measurement.

- [ ] **Step 5: Add version-aware in-memory dispatch**

Use separate maps so old entries are never silently re-encoded:

```rust
pub struct ApiLock {
    version: u8,
    legacy_declared: BTreeMap<String, LockedApi>,
    declared_v3: BTreeMap<String, v3::DeclaredEntry>,
    declared_v4: BTreeMap<String, v4::DeclaredEntry>,
    observed: BTreeMap<String, Shape>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclaredCoverage {
    PartialV3,
    FullV4,
}

pub enum VerifyTargetKind {
    LegacyDeclared { operations: BTreeSet<LockedOperation> },
    Declared {
        contract: ApiContract,
        scope: Scope,
        coverage: DeclaredCoverage,
    },
    Observed { shape: Shape },
}
```

Make `load` dispatch versions 1, 2, 3, and 4. Make `render` preserve the loaded
version except when a deliberate declared update migrates the whole lock.

- [ ] **Step 6: Implement deliberate v4 creation and migration**

Add `new_v4`, `build_v4_declared`, and `replace_declared_v4`. Migration accepts
the named old declared entry only when no other v1/v2/v3 declared entry would
remain. Preserve observed entries and validate the complete v4 lock before
returning it.

Use this refusal text:

```rust
return Err(anyhow!(
    "cannot migrate api.lock to v4; migration requires original sources for: {}",
    remaining.join(", ")
));
```

- [ ] **Step 7: Route Lock and Verify through v4**

In `src/main.rs`, replace `build_v3_declared`/`new_v3` with the v4 functions.
For declared Verify, map coverage as follows:

```rust
let (coverage, limitation) = match coverage {
    lockfile::DeclaredCoverage::PartialV3 => (
        output::Coverage::Partial,
        Some(output::Limitation::Phase2RelockRequired),
    ),
    lockfile::DeclaredCoverage::FullV4 => (output::Coverage::Full, None),
};
```

Text mode writes the v3 limitation to stderr before rendering changes. JSON
and SARIF use the existing structured limitation channels.

- [ ] **Step 8: Add the partial-coverage renderer contract**

Extend `src/output/mod.rs`:

```rust
pub enum Coverage {
    Full,
    Partial,
    Routes,
}

pub enum Limitation {
    RouteOnlyLock,
    Phase2RelockRequired,
}
```

Use code `phase2_relock_required`, SARIF notification ID
`apiwatch/phase2-relock-required`, and message:

```text
api.lock v3 lacks Phase 2 contract fields; re-lock from the original OpenAPI source for full coverage
```

- [ ] **Step 9: Generate and assert deterministic v4 golden locks**

Generate `v4_users.lock` and `v4_private.lock` through the production Lock
command, normalize fixture checkout line endings only in test comparisons, and
assert production rendering itself ends in LF.

- [ ] **Step 10: Run foundation verification**

Run:

```powershell
cargo fmt --all -- --check
cargo test lockfile --lib
cargo test --test cli_lock
cargo test --test cli_verify
```

Expected: all commands pass; v1/v2 routes, v3 partial, and v4 full coverage are
distinct.

- [ ] **Step 11: Commit the foundation**

```powershell
git add src/lockfile src/main.rs src/output/mod.rs tests/cli_lock.rs tests/cli_verify.rs testdata/lock/v4_users.lock testdata/lock/v4_private.lock
git commit -m "feat: establish lockfile v4 foundation"
```

---

### Task 2: Fix D-01 Request-Body Presence and Requiredness

**Files:**
- Create: `testdata/openapi/phase2_d01_request_body_old.yaml`
- Create: `testdata/openapi/phase2_d01_request_body_new.yaml`
- Modify: `src/contract/mod.rs`
- Modify: `src/openapi/mod.rs`
- Modify: `src/diff/mod.rs`
- Modify: `src/lockfile/v4/mod.rs`
- Modify: `src/lockfile/v4/schema.rs`
- Modify: `src/lockfile/v3/schema.rs`
- Modify: `tests/cli_diff.rs`
- Modify: `tests/cli_verify.rs`

**Interfaces:**
- Produces: `RequestBody { required: Option<bool>, content }`.
- `Some(bool)` means known OpenAPI/v4 requiredness; `None` means absent from a
  reconstructed v3 lock.
- Produces: `diff_request_bodies(&mut Vec<Change>, &OperationKey,
  Option<&RequestBody>, Option<&RequestBody>)`.

- [ ] **Step 1: Create the D-01 fixture pair**

Use three POST operations in one pair:

```yaml
openapi: 3.0.3
info: { title: D-01, version: '1' }
paths:
  /required-added:
    post:
      responses: { '204': { description: ok } }
  /optional-added:
    post:
      responses: { '204': { description: ok } }
  /requiredness:
    post:
      requestBody:
        required: false
        content:
          application/json:
            schema: { type: object }
      responses: { '204': { description: ok } }
```

The new fixture adds a required body to `/required-added`, an optional body to
`/optional-added`, and changes `/requiredness` to `required: true`. Keep the
same response maps.

- [ ] **Step 2: Add failing direct-diff and v4-Verify tests**

Assert these exact forward messages and severities:

```rust
[
    ("request body added as required", Severity::Breaking),
    ("request body added as optional", Severity::NonBreaking),
    (
        "request body changed from optional to required",
        Severity::Breaking,
    ),
]
```

Reverse the fixture arguments and assert body removal is breaking and
required-to-optional is non-breaking. Lock the old fixture into a temporary v4
file and assert Verify JSON contains the same ordered `changes` array as Diff
JSON.

- [ ] **Step 3: Run the D-01 tests and confirm they fail**

Run:

```powershell
cargo test --test cli_diff phase2_d01 -- --nocapture
cargo test --test cli_verify phase2_d01 -- --nocapture
```

Expected: no body-presence or requiredness findings are emitted.

- [ ] **Step 4: Normalize and serialize known requiredness**

Change the model and OpenAPI normalization:

```rust
pub struct RequestBody {
    pub required: Option<bool>,
    pub content: BTreeMap<String, Schema>,
}
```

OpenAPI normalization sets `Some(request_body.required)`. v4 wire uses a
non-optional `required: bool` and expands to `Some(required)`. v3 expands to
`None`, so v3 never invents requiredness.

- [ ] **Step 5: Implement D-01 comparison**

Handle `(None, Some)`, `(Some, None)`, and known requiredness changes before
schema comparison. Skip only the requiredness sub-check when either stored
value is `None`; body presence is known in v3 and remains comparable.

- [ ] **Step 6: Run focused and regression tests**

Run:

```powershell
cargo test --test cli_diff phase2_d01
cargo test --test cli_verify phase2_d01
cargo test --test cli_verify verify_v3
cargo test lockfile --lib
```

Expected: all pass.

- [ ] **Step 7: Commit D-01**

```powershell
git add src/contract/mod.rs src/openapi/mod.rs src/diff/mod.rs src/lockfile/v3/schema.rs src/lockfile/v4 tests/cli_diff.rs tests/cli_verify.rs testdata/openapi/phase2_d01_request_body_old.yaml testdata/openapi/phase2_d01_request_body_new.yaml
git commit -m "fix: compare request body presence"
```

---

### Task 3: Fix D-02 Canonical Media-Type Sets

**Files:**
- Create: `src/openapi/identity.rs`
- Create: `testdata/openapi/phase2_d02_content_type_old.yaml`
- Create: `testdata/openapi/phase2_d02_content_type_new.yaml`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `src/openapi/mod.rs`
- Modify: `src/diff/mod.rs`
- Modify: `src/lockfile/v3/schema.rs`
- Modify: `src/lockfile/v4/schema.rs`
- Modify: `src/lockfile/v4/mod.rs`
- Modify: `tests/cli_diff.rs`
- Modify: `tests/cli_verify.rs`

**Interfaces:**
- Produces: `openapi::identity::canonical_media_type(&str) -> Result<String>`.
- Consumes: `RequestBody` and `Response` content maps.

- [ ] **Step 1: Add the MIME dependency and D-02 fixtures**

Add:

```toml
mime = "0.3"
```

The fixture pair contains `/request` with request `application/json` changing
to `application/xml`, and `/response` status 200 with `application/json`
changing to both `application/json` and `application/problem+json`. Add
`/case-only` where `Application/JSON; Charset=UTF-8` changes only in
case/parameter order.

- [ ] **Step 2: Add failing media-set tests**

Assert:

```text
POST /request: request content type application/json removed
POST /request: request content type application/xml added
GET /response: response 200 content type application/problem+json added
```

Use breaking/non-breaking/breaking respectively. Assert no `/case-only`
finding. Reverse the fixtures and assert request XML removal and response
problem media removal are breaking.

- [ ] **Step 3: Run the focused tests and confirm the false negatives**

Run:

```powershell
cargo test --test cli_diff phase2_d02 -- --nocapture
```

Expected: media additions and removals are absent.

- [ ] **Step 4: Implement canonical media parsing**

In `src/openapi/identity.rs`, parse `mime::Mime`, lowercase type/subtype and
parameter names, sort parameters by `(name, value)`, and render:

```rust
pub(crate) fn canonical_media_type(value: &str) -> Result<String> {
    let parsed: mime::Mime = value
        .parse()
        .context("invalid media type")?;
    let mut parameters = parsed
        .params()
        .map(|(name, value)| (name.as_str().to_ascii_lowercase(), value.as_str().to_string()))
        .collect::<Vec<_>>();
    parameters.sort();
    let mut canonical = format!(
        "{}/{}",
        parsed.type_().as_str().to_ascii_lowercase(),
        parsed.subtype().as_str().to_ascii_lowercase()
    );
    for (name, value) in parameters {
        canonical.push_str(&format!(";{name}={value}"));
    }
    Ok(canonical)
}
```

Normalize direct OpenAPI maps and canonicalize v3 expansion. Require v4 wire
keys to already equal their canonical form.

- [ ] **Step 5: Compare request and response media sets**

Add explicit old-only and new-only loops around the existing matching-schema
loops. Use the approved severities and exact messages from Step 2.

- [ ] **Step 6: Run D-02 and compatibility-sensitive tests**

Run:

```powershell
cargo test --test cli_diff phase2_d02
cargo test --test cli_verify phase2_d02
cargo test --test cli_diff diff_resolves_component_request_body_refs_for_request_diff
cargo test --test cli_diff diff_resolves_component_response_refs_for_response_diff
```

Expected: all pass and referenced media behavior remains unchanged.

- [ ] **Step 7: Commit D-02**

```powershell
git add Cargo.toml Cargo.lock src/openapi src/diff/mod.rs src/lockfile/v3/schema.rs src/lockfile/v4 tests/cli_diff.rs tests/cli_verify.rs testdata/openapi/phase2_d02_content_type_old.yaml testdata/openapi/phase2_d02_content_type_new.yaml
git commit -m "fix: compare canonical content types"
```

---

### Task 4: Fix D-03 Response Requiredness Symmetry

**Files:**
- Create: `testdata/openapi/phase2_d03_response_required_old.yaml`
- Create: `testdata/openapi/phase2_d03_response_required_new.yaml`
- Modify: `src/diff/mod.rs`
- Modify: `tests/cli_diff.rs`
- Modify: `tests/cli_verify.rs`

**Interfaces:**
- Consumes: existing `Property.required`.
- Produces: response-aware `diff_requiredness`.

- [ ] **Step 1: Add a fixture with both response directions**

The old response object requires `id` but not `name`; the new response object
requires `name` but not `id`. Both properties remain present with unchanged
string schemas.

- [ ] **Step 2: Add failing symmetry tests**

Assert:

```text
GET /users: response 200 application/json field id changed from required to optional
GET /users: response 200 application/json field name changed from optional to required
```

The first is breaking and the second non-breaking. Assert identical v4 Verify
JSON findings.

- [ ] **Step 3: Run the D-03 tests and confirm no findings**

Run:

```powershell
cargo test --test cli_diff phase2_d03 -- --nocapture
```

Expected: response requiredness is ignored.

- [ ] **Step 4: Make requiredness severity usage-aware**

Replace the request-only early return:

```rust
let severity = match (usage, old_required, new_required) {
    (SchemaUsage::Request, false, true) => Severity::Breaking,
    (SchemaUsage::Request, true, false) => Severity::NonBreaking,
    (SchemaUsage::Response, true, false) => Severity::Breaking,
    (SchemaUsage::Response, false, true) => Severity::NonBreaking,
    (_, _, _) => return,
};
```

Keep the existing message template.

- [ ] **Step 5: Run focused and existing requiredness tests**

Run:

```powershell
cargo test --test cli_diff phase2_d03
cargo test --test cli_diff required
cargo test --test cli_verify phase2_d03
```

Expected: all pass.

- [ ] **Step 6: Commit D-03**

```powershell
git add src/diff/mod.rs tests/cli_diff.rs tests/cli_verify.rs testdata/openapi/phase2_d03_response_required_old.yaml testdata/openapi/phase2_d03_response_required_new.yaml
git commit -m "fix: compare response requiredness"
```

---

### Task 5: Fix D-05 Schema Format Comparison

**Files:**
- Create: `testdata/openapi/phase2_d05_format_old.yaml`
- Create: `testdata/openapi/phase2_d05_format_new.yaml`
- Modify: `src/diff/mod.rs`
- Modify: `tests/cli_diff.rs`
- Modify: `tests/cli_verify.rs`

**Interfaces:**
- Consumes: existing `Schema.format: Option<String>`.
- Produces: warning-only format findings.

- [ ] **Step 1: Add format fixtures and failing tests**

Use unchanged string, integer, and response properties with transitions
`None -> uuid`, `int32 -> int64`, and `date -> date-time`. Assert warnings:

```text
POST /users: request application/json field id format changed from none to uuid
POST /users: request application/json field count format changed from int32 to int64
GET /events: response 200 application/json field created_at format changed from date to date-time
```

- [ ] **Step 2: Run the D-05 tests and confirm no warnings**

Run:

```powershell
cargo test --test cli_diff phase2_d05 -- --nocapture
```

Expected: all format-only changes are missed.

- [ ] **Step 3: Add exact format comparison**

After kind and nullable checks in `diff_schema`, add:

```rust
if old.format != new.format {
    changes.push(Change {
        severity: Severity::Warning,
        operation: operation.clone(),
        message: format!(
            "{context} {} format changed from {} to {}",
            schema_target(path),
            format_name(old.format.as_deref()),
            format_name(new.format.as_deref())
        ),
    });
}
```

`format_name(None)` returns `"none"`.

- [ ] **Step 4: Run format, JSON, and SARIF tests**

Run:

```powershell
cargo test --test cli_diff phase2_d05
cargo test --test cli_verify phase2_d05
cargo test --test cli_diff diff_json
cargo test --test cli_diff diff_sarif
```

Expected: warnings exit 0 and use unchanged JSON/SARIF schemas.

- [ ] **Step 5: Commit D-05**

```powershell
git add src/diff/mod.rs tests/cli_diff.rs tests/cli_verify.rs testdata/openapi/phase2_d05_format_old.yaml testdata/openapi/phase2_d05_format_new.yaml
git commit -m "fix: warn on schema format changes"
```

---

### Task 6: Fix D-04 `additionalProperties` Semantics

**Files:**
- Create: `testdata/openapi/phase2_d04_additional_properties_old.yaml`
- Create: `testdata/openapi/phase2_d04_additional_properties_new.yaml`
- Modify: `src/contract/mod.rs`
- Modify: `src/openapi/mod.rs`
- Modify: `src/diff/mod.rs`
- Modify: `src/lock_size.rs`
- Modify: `src/lockfile/mod.rs`
- Modify: `src/lockfile/v3/mod.rs`
- Modify: `src/lockfile/v4/mod.rs`
- Modify: `src/lockfile/v4/schema.rs`
- Modify: `src/lockfile/v3/schema.rs`
- Modify: `tests/cli_diff.rs`
- Modify: `tests/cli_verify.rs`

**Interfaces:**
- Produces: `AdditionalProperties::{Unknown, Forbidden, Any, Schema(Box<Schema>)}`.
- `Unknown` is accepted only while reconstructing v3 and is rejected by v4
  interning.

- [ ] **Step 1: Add the D-04 directional fixture**

Use four unchanged object schemas:

```yaml
/request-narrowed:  request additionalProperties true -> false
/request-broadened: request additionalProperties false -> true
/response-broadened: response additionalProperties false -> true
/response-narrowed: response additionalProperties true -> false
```

Add a fifth request map whose `additionalProperties` schema changes from
`type: string` to `type: integer`.

- [ ] **Step 2: Add failing matrix and nested-schema tests**

Assert breaking, non-breaking, breaking, non-breaking for the four paths.
Assert:

```text
POST /typed-map: request application/json additionalProperties type changed from string to integer
```

is breaking. Add v4 round-trip and tamper tests for schema-valued policies.

- [ ] **Step 3: Run the D-04 tests and confirm false negatives**

Run:

```powershell
cargo test --test cli_diff phase2_d04 -- --nocapture
cargo test lockfile::v4 --lib additional_properties -- --nocapture
```

Expected: no policy findings and no v4 wire field.

- [ ] **Step 4: Normalize explicit policy**

Add:

```rust
pub enum AdditionalProperties {
    Unknown,
    Forbidden,
    Any,
    Schema(Box<Schema>),
}
```

Object schemas normalize omitted or `Any(true)` to `Any`, `Any(false)` to
`Forbidden`, and `Schema` recursively. Non-object schemas use `Forbidden`.
v3 expansion uses `Unknown`; v4 rejects `Unknown` before hashing or writing.
Update every `Schema` constructor in `src/lock_size.rs`,
`src/lockfile/mod.rs`, and `src/lockfile/v3/mod.rs` with the correct policy.

- [ ] **Step 5: Add v4 wire references and reachability**

Use:

```rust
#[serde(tag = "kind", rename_all = "snake_case")]
enum WireAdditionalProperties {
    Forbidden,
    Any,
    Schema { schema: String },
}
```

Include schema-valued references in root traversal, digest validation, and
orphan checks.

- [ ] **Step 6: Implement usage-direction comparison**

Skip comparison if either side is `Unknown`. Classify policy narrowing as
breaking for requests and non-breaking for responses; classify broadening in
the inverse direction. Recurse into two schema-valued policies with path
`additionalProperties`.

- [ ] **Step 7: Run D-04 and v3 compatibility tests**

Run:

```powershell
cargo test --test cli_diff phase2_d04
cargo test --test cli_verify phase2_d04
cargo test --test cli_verify verify_v3
cargo test lockfile --lib
```

Expected: v4 is complete and v3 skips only the missing policy.

- [ ] **Step 8: Commit D-04**

```powershell
git add src/contract/mod.rs src/openapi/mod.rs src/diff/mod.rs src/lock_size.rs src/lockfile/mod.rs src/lockfile/v3 src/lockfile/v4 tests/cli_diff.rs tests/cli_verify.rs testdata/openapi/phase2_d04_additional_properties_old.yaml testdata/openapi/phase2_d04_additional_properties_new.yaml
git commit -m "fix: compare additional properties policies"
```

---

### Task 7: Fix D-06 Effective Server Changes

**Files:**
- Create: `testdata/openapi/phase2_d06_servers_old.yaml`
- Create: `testdata/openapi/phase2_d06_servers_new.yaml`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `src/contract/mod.rs`
- Modify: `src/openapi/identity.rs`
- Modify: `src/openapi/mod.rs`
- Modify: `src/diff/mod.rs`
- Modify: `src/lock_size.rs`
- Modify: `src/lockfile/mod.rs`
- Modify: `src/lockfile/v3/mod.rs`
- Modify: `src/lockfile/v4/mod.rs`
- Modify: `src/lockfile/v4/schema.rs`
- Modify: `src/lockfile/v3/schema.rs`
- Modify: `tests/cli_diff.rs`
- Modify: `tests/cli_verify.rs`

**Interfaces:**
- Produces: `ServerTemplate(String)`.
- Produces: `Operation.servers: Option<BTreeSet<ServerTemplate>>`.
- `None` means unavailable from v3; direct OpenAPI and v4 always use `Some`.
- Produces: `canonical_server_template(&str) -> Result<ServerTemplate>`.

- [ ] **Step 1: Add URL parsing and the D-06 fixture**

Add direct dependency:

```toml
url = "2"
```

The fixture covers root, path, and operation precedence, one removed server,
one added server, declaration reordering, a relative default server, and query
values that change while query keys remain the same.

- [ ] **Step 2: Add failing precedence, privacy, and classification tests**

Assert:

```text
GET /removed: server https://api.example.com/v1 removed
GET /added: server https://backup.example.com/v1 added
```

Use breaking then non-breaking. Assert no reorder or literal-query-value
finding. Add an input-error test for `https://user:secret@example.com/v1` and
assert `secret` is absent from stderr and lock bytes.

- [ ] **Step 3: Run D-06 tests and confirm servers are ignored**

Run:

```powershell
cargo test --test cli_diff phase2_d06 -- --nocapture
```

Expected: server changes produce no findings.

- [ ] **Step 4: Implement privacy-safe template identity**

In `identity.rs`, replace `{variable}` tokens with deterministic temporary
tokens before URL parsing, support absolute and relative URLs, reject nonempty
username/password, restore placeholders, sort query keys, and render every
literal query value as `{redacted}`. Exclude fragments, descriptions,
variable defaults, and variable descriptions.

Define the stored identity as:

```rust
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ServerTemplate(pub String);
```

- [ ] **Step 5: Normalize OpenAPI precedence**

Extend `OperationNormalizeContext` with root servers. Pass path-item servers
to `insert_operation`. Select operation servers when nonempty, otherwise path
servers, otherwise root servers, otherwise `"/"`. Normalize into a sorted set.

- [ ] **Step 6: Store known/unknown servers**

Add `servers: Option<BTreeSet<ServerTemplate>>` to `Operation`. v4 wire stores
a required sorted list and expands to `Some`; v3 expands to `None`. v4
validation rejects duplicates, noncanonical strings, and missing server data.
Update every `Operation` constructor in `src/lock_size.rs`,
`src/lockfile/mod.rs`, and `src/lockfile/v3/mod.rs`.

- [ ] **Step 7: Compare effective server sets**

When both sides are `Some`, emit old-only breaking removals and new-only
non-breaking additions. Skip only this sub-check when either side is `None`.

- [ ] **Step 8: Run focused, privacy, and remote tests**

Run:

```powershell
cargo test --test cli_diff phase2_d06
cargo test --test cli_verify phase2_d06
cargo test --test cli_lock privacy
cargo test --test cli_verify remote
```

Expected: all pass.

- [ ] **Step 9: Commit D-06**

```powershell
git add Cargo.toml Cargo.lock src/contract/mod.rs src/openapi src/diff/mod.rs src/lock_size.rs src/lockfile/mod.rs src/lockfile/v3 src/lockfile/v4 tests/cli_diff.rs tests/cli_verify.rs testdata/openapi/phase2_d06_servers_old.yaml testdata/openapi/phase2_d06_servers_new.yaml
git commit -m "fix: compare effective server templates"
```

---

### Task 8: Fix D-09 Composition Identity and `allOf` Merging

**Files:**
- Create: `testdata/openapi/phase2_d09_composition_old.yaml`
- Create: `testdata/openapi/phase2_d09_composition_new.yaml`
- Modify: `src/contract/mod.rs`
- Modify: `src/openapi/mod.rs`
- Modify: `src/diff/mod.rs`
- Modify: `src/lock_size.rs`
- Modify: `src/lockfile/mod.rs`
- Modify: `src/lockfile/v3/mod.rs`
- Modify: `src/lockfile/v4/mod.rs`
- Modify: `src/lockfile/v4/schema.rs`
- Modify: `src/lockfile/v3/schema.rs`
- Modify: `tests/cli_diff.rs`
- Modify: `tests/cli_verify.rs`

**Interfaces:**
- Produces: `Schema.branches: Vec<Schema>` for `OneOf` and `AnyOf`.
- Produces: `merge_all_of(Vec<Schema>) -> Result<Schema>`.
- Produces: infallible `Schema::structural_key() -> String` using a
  deterministic field encoder and SHA-256.

- [ ] **Step 1: Create the composition audit fixture**

Use operations for:

- `allOf` with the same object branches reordered;
- `oneOf` with string/integer branches reordered;
- `anyOf` with two object branches reordered;
- a request `oneOf` branch removal;
- a response `anyOf` branch addition;
- an `allOf` required-property change that must still report a focused field
  finding.

- [ ] **Step 2: Add failing zero-noise and directional tests**

Assert no changes for the three reorder operations. Assert request branch
removal and response branch addition are breaking. Assert the merged `allOf`
property finding uses its property path rather than `allOf[0]`.

- [ ] **Step 3: Run D-09 tests and confirm index-based false positives**

Run:

```powershell
cargo test --test cli_diff phase2_d09 -- --nocapture
```

Expected: reordered branches produce type/property findings.

- [ ] **Step 4: Add first-class canonical branches**

Add `branches: Vec<Schema>` to `Schema`. Canonicalize nested schemas first,
sort branches by SHA-256 of canonical JSON, and deduplicate identical keys.
Reject branches on non-composed kinds during v4 validation.

- [ ] **Step 5: Implement supported `allOf` intersection**

Implement `merge_all_of` with these exact rules:

- `Unknown` adopts the other kind; different known kinds are an input error.
- Nullability is logical AND.
- One missing format adopts the present format; two unequal formats are an
  input error.
- An empty enum means unconstrained; two constrained enums intersect, and an
  empty intersection is an input error.
- Properties union; duplicate properties recursively intersect and required
  is logical OR.
- The current synthetic array-item property and schema-valued additional
  properties recursively intersect.
- Forbidden is narrower than schema-constrained, which is narrower than Any.

- [ ] **Step 6: Preserve focused matching within branch sets**

Remove exact structural matches first. Pair remaining branches only when each
side has one branch with the same shape key (kind plus property/item topology);
recurse through paired branches. Classify unpaired additions/removals using
the approved request/response matrix. Use canonical new-branch order for
display paths such as `oneOf[0]`.

- [ ] **Step 7: Update v4 and v3 reconstruction**

v4 stores branch schema IDs as sorted lists and includes them in reachability.
v3 expansion converts synthetic `oneOf[n]`/`anyOf[n]` properties into branch
vectors before comparison. `allOf[n]` v3 properties are merged through the
same intersection function.
Update all `Schema` constructors in `src/lock_size.rs`,
`src/lockfile/mod.rs`, and `src/lockfile/v3/mod.rs` with an empty branch list
for non-composed schemas.

- [ ] **Step 8: Run composition and integrity tests**

Run:

```powershell
cargo test --test cli_diff phase2_d09
cargo test --test cli_diff composition
cargo test --test cli_verify phase2_d09
cargo test lockfile::v4 --lib
```

Expected: reorder noise disappears and real branch/property changes remain.

- [ ] **Step 9: Commit D-09**

```powershell
git add src/contract/mod.rs src/openapi/mod.rs src/diff/mod.rs src/lock_size.rs src/lockfile/mod.rs src/lockfile/v3 src/lockfile/v4 tests/cli_diff.rs tests/cli_verify.rs testdata/openapi/phase2_d09_composition_old.yaml testdata/openapi/phase2_d09_composition_new.yaml
git commit -m "fix: compare composed schemas semantically"
```

---

### Task 9: Fix D-07 Positional Path-Template Identity

**Files:**
- Create: `testdata/openapi/phase2_d07_path_template_old.yaml`
- Create: `testdata/openapi/phase2_d07_path_template_new.yaml`
- Modify: `src/contract/mod.rs`
- Modify: `src/openapi/identity.rs`
- Modify: `src/openapi/mod.rs`
- Modify: `src/diff/mod.rs`
- Modify: `src/lock_size.rs`
- Modify: `src/lockfile/mod.rs`
- Modify: `src/lockfile/v3/schema.rs`
- Modify: `src/lockfile/v4/mod.rs`
- Modify: `src/lockfile/v4/schema.rs`
- Modify: `src/output/mod.rs`
- Modify: `tests/cli_diff.rs`
- Modify: `tests/cli_lock.rs`
- Modify: `tests/cli_verify.rs`

**Interfaces:**
- Produces: `OperationIdentity { method, path }` for map/scoping identity.
- Retains: `OperationKey { method, path }` for diagnostics.
- Produces: an `Operation.key: OperationKey` field containing the diagnostic
  method and display path.
- Produces: `canonical_path_template(&str) -> Result<(String, Vec<String>)>`.

- [ ] **Step 1: Add placeholder-rename and collision fixtures**

The old fixture defines `GET /users/{userId}/orders/{orderId}` with matching
path parameters. The new fixture renames them to `{id}` and `{order}` without
changing schemas. Add a raw YAML collision test containing both `/users/{id}`
and `/users/{name}` in one document.

- [ ] **Step 2: Add failing diff, scope, and collision tests**

Assert renamed placeholders produce no endpoint or parameter findings. Create
a scoped v4 lock with the old selector and Verify against the new document.
Assert the collision exits 2 with:

```text
ambiguous operation identity GET /users/{0}
```

- [ ] **Step 3: Run D-07 tests and confirm endpoint churn**

Run:

```powershell
cargo test --test cli_diff phase2_d07 -- --nocapture
cargo test --test cli_verify phase2_d07 -- --nocapture
```

Expected: endpoint removed/added and path-parameter churn appear.

- [ ] **Step 4: Split identity from diagnostic keys**

Change the contract map:

```rust
pub struct ApiContract {
    pub operations: BTreeMap<OperationIdentity, Operation>,
}

pub struct OperationIdentity {
    pub method: HttpMethod,
    pub path: String,
}

pub struct Operation {
    pub key: OperationKey,
    pub auth: BTreeMap<String, AuthRequirement>,
    pub parameters: BTreeMap<ParameterKey, Parameter>,
    pub request_body: Option<RequestBody>,
    pub responses: BTreeMap<String, Response>,
    pub servers: Option<BTreeSet<ServerTemplate>>,
}
```

Update `Change.operation` to receive `operation.key.clone()` so public output
continues using display paths.

- [ ] **Step 5: Canonicalize placeholders and path parameter identity**

Parse each `{name}` segment, reject empty, repeated, unclosed, or unbound
placeholders, and render `{0}`, `{1}` by occurrence. Change path parameter
keys to canonical slot strings while retaining the source name in
`Parameter.name` for messages.

- [ ] **Step 6: Make scoping identity-aware**

Normalize CLI selectors to `OperationIdentity`. v4 scope stores canonical
selectors; v3 scope selectors canonicalize on load. Missing scoped operations
use the locked operation display key for the removal diagnostic.

- [ ] **Step 7: Update v4 wire keys**

Key v4 operations by canonical method/path and add required `display_path`.
Validate that `display_path` canonicalizes back to the wire key. v3 expansion
derives both from its stored operation key.

- [ ] **Step 8: Run path, scope, output, and D-16 tests**

Run:

```powershell
cargo test --test cli_diff phase2_d07
cargo test --test cli_lock scope
cargo test --test cli_verify phase2_d07
cargo test --test cli_verify verify_v3_d16_reports_four_breaking_findings
```

Expected: all pass and existing messages retain display paths.

- [ ] **Step 9: Commit D-07**

```powershell
git add src/contract/mod.rs src/openapi src/diff/mod.rs src/lock_size.rs src/lockfile src/output/mod.rs tests/cli_diff.rs tests/cli_lock.rs tests/cli_verify.rs testdata/openapi/phase2_d07_path_template_old.yaml testdata/openapi/phase2_d07_path_template_new.yaml
git commit -m "fix: normalize path template identity"
```

---

### Task 10: Fix D-08 Semantic Authentication Identity

**Files:**
- Create: `testdata/openapi/phase2_d08_auth_identity_old.yaml`
- Create: `testdata/openapi/phase2_d08_auth_identity_new.yaml`
- Modify: `src/contract/mod.rs`
- Modify: `src/openapi/identity.rs`
- Modify: `src/openapi/mod.rs`
- Modify: `src/diff/mod.rs`
- Modify: `src/lockfile/mod.rs`
- Modify: `src/lockfile/v3/mod.rs`
- Modify: `src/lockfile/v4/mod.rs`
- Modify: `src/lockfile/v4/schema.rs`
- Modify: `src/lockfile/v3/schema.rs`
- Modify: `tests/cli_diff.rs`
- Modify: `tests/cli_verify.rs`

**Interfaces:**
- Produces: `AuthRequirement.identity: Option<AuthIdentity>`.
- `None` means v3 lacks wire identity and uses component-label matching.
- Produces: `AuthIdentity::{ApiKey, Http, OAuth2, OpenIdConnect, Unknown}`.

- [ ] **Step 1: Add equivalent-rename and real-change fixtures**

Use one operation whose `bearerAuth` component is renamed `accessToken` with
the same HTTP bearer scheme. Use another whose unchanged `apiKeyAuth` label
changes transmitted header from `X-API-Key` to `X-Client-Key`. Keep
requirements and scopes otherwise stable.

- [ ] **Step 2: Add failing identity tests**

Assert no finding for the bearer component rename. Assert the API-key wire
change uses:

```text
GET /keyed: authentication apiKeyAuth changed identity
```

with breaking severity. Add duplicate-semantic-identity input that exits 2
without echoing control characters.

- [ ] **Step 3: Run D-08 tests and confirm rename churn**

Run:

```powershell
cargo test --test cli_diff phase2_d08 -- --nocapture
```

Expected: bearer removal/addition are reported and API-key header identity is
missed.

- [ ] **Step 4: Define exact semantic identities**

Add:

```rust
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum AuthIdentity {
    ApiKey { location: ParameterLocation, name: String },
    Http { scheme: String },
    OAuth2 { flows: BTreeSet<OAuthFlowIdentity> },
    OpenIdConnect { discovery: ServerTemplate },
    Unknown { kind: AuthSchemeKind },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct OAuthFlowIdentity {
    pub kind: OAuthFlowKind,
    pub authorization: Option<ServerTemplate>,
    pub token: Option<ServerTemplate>,
    pub refresh: Option<ServerTemplate>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum OAuthFlowKind {
    Implicit,
    Password,
    ClientCredentials,
    AuthorizationCode,
}
```

OAuth flow identity stores flow kind plus sanitized authorization, token, and
refresh endpoint templates. Do not store scope descriptions or bearer-format
hints.

- [ ] **Step 5: Normalize resolved security schemes**

Make the security-scheme resolver return the complete identity plus display
kind. Reject two differently labeled schemes that collapse to one identity
with different requirements on the same operation.

- [ ] **Step 6: Match requirements deterministically**

Comparison order:

1. Match exact known semantic identities across labels and compare scopes.
2. Match remaining same-label requirements and emit the identity/type-change
   diagnostic.
3. Emit existing removals for unmatched old requirements.
4. Emit existing additions for unmatched new requirements.
5. For `identity: None` from v3, retain the old label-plus-kind behavior.

- [ ] **Step 7: Store and validate v4 auth identities**

Add tagged v4 wire identities, canonical sorted OAuth flows, safe endpoint
validation, and required identity presence. v3 expands with `identity: None`.
Update `AuthRequirement` constructors in `src/lockfile/mod.rs` and
`src/lockfile/v3/mod.rs`.

- [ ] **Step 8: Run auth, privacy, and v3 tests**

Run:

```powershell
cargo test --test cli_diff phase2_d08
cargo test --test cli_diff authentication
cargo test --test cli_verify phase2_d08
cargo test --test cli_verify verify_v3
cargo test --test cli_lock privacy
```

Expected: all pass.

- [ ] **Step 9: Commit D-08**

```powershell
git add src/contract/mod.rs src/openapi src/diff/mod.rs src/lockfile/mod.rs src/lockfile/v3 src/lockfile/v4 tests/cli_diff.rs tests/cli_verify.rs testdata/openapi/phase2_d08_auth_identity_old.yaml testdata/openapi/phase2_d08_auth_identity_new.yaml
git commit -m "fix: match authentication by semantic identity"
```

---

### Task 11: Fix D-10 First-Class Array Items

**Files:**
- Create: `testdata/openapi/phase2_d10_array_items_old.yaml`
- Create: `testdata/openapi/phase2_d10_array_items_new.yaml`
- Modify: `src/contract/mod.rs`
- Modify: `src/openapi/mod.rs`
- Modify: `src/diff/mod.rs`
- Modify: `src/lockfile/mod.rs`
- Modify: `src/lockfile/v3/mod.rs`
- Modify: `src/lockfile/v4/mod.rs`
- Modify: `src/lockfile/v4/schema.rs`
- Modify: `src/lockfile/v3/schema.rs`
- Modify: `src/lock_size.rs`
- Modify: `tests/cli_diff.rs`
- Modify: `tests/cli_verify.rs`

**Interfaces:**
- Produces: `Schema.items: Option<Box<Schema>>`.
- Removes synthetic `properties["items"]` encoding for arrays.

- [ ] **Step 1: Add nested array audit fixtures**

Use a response array of objects whose item field `name` is removed, a request
array whose required item field `email` is added, and a nested array whose
inner item format changes.

- [ ] **Step 2: Add failing first-class model and output tests**

Assert the normalized array has `items.is_some()` and no property named
`items`. Preserve exact messages:

```text
GET /users: response 200 application/json field items.name removed
POST /users: request application/json field items.email added as required
```

Assert the nested format warning uses `items.items`.

- [ ] **Step 3: Run D-10 tests and confirm synthetic storage**

Run:

```powershell
cargo test --test cli_diff phase2_d10 -- --nocapture
cargo test openapi --lib array -- --nocapture
```

Expected: the public shallow cases may pass, but the model assertion and
nested format coverage fail.

- [ ] **Step 4: Normalize arrays into `Schema.items`**

Add `items` to `Schema`, set it from `ArrayType.items`, and stop inserting the
synthetic property. Initialize non-array schemas with `None`. Update every
`Schema` constructor in `src/lock_size.rs`, `src/lockfile/mod.rs`, and
`src/lockfile/v3/mod.rs`. Update `merge_all_of` to intersect first-class item
schemas instead of the removed synthetic property.

- [ ] **Step 5: Recurse through items in comparison**

When both items exist, call `diff_schema` with `field_path(path, "items")`.
Classify item addition/removal using request/response schema direction and
retain the `items` display vocabulary.

- [ ] **Step 6: Store item references in v4 and reconstruct v3**

Add optional v4 item schema ID and include it in interning, reachability, and
tamper validation. During v3 expansion, move the synthetic array
`properties["items"]` schema into `items`.

- [ ] **Step 7: Update prototype encoders and run array regressions**

Teach `src/lock_size.rs` candidate encoders about first-class items so the
historical report remains deterministic for the new normalized model.

Run:

```powershell
cargo test --test cli_diff phase2_d10
cargo test --test cli_diff array
cargo test --test cli_verify phase2_d10
cargo test -p apiwatch-lock-size-report
```

Expected: all pass.

- [ ] **Step 8: Commit D-10**

```powershell
git add src/contract/mod.rs src/openapi/mod.rs src/diff/mod.rs src/lockfile/mod.rs src/lockfile/v3 src/lockfile/v4 src/lock_size.rs tests/cli_diff.rs tests/cli_verify.rs testdata/openapi/phase2_d10_array_items_old.yaml testdata/openapi/phase2_d10_array_items_new.yaml
git commit -m "refactor: model array items directly"
```

---

### Task 12: Lock D-11 Enum Severity Policy

**Files:**
- Create: `testdata/openapi/phase2_d11_enum_policy_old.yaml`
- Create: `testdata/openapi/phase2_d11_enum_policy_new.yaml`
- Modify: `src/openapi/mod.rs`
- Modify: `src/diff/mod.rs`
- Modify: `tests/cli_diff.rs`
- Modify: `tests/cli_verify.rs`

**Interfaces:**
- Consumes: `SchemaUsage::{Request, Response}`.
- Produces: explicit four-cell enum severity table.

- [ ] **Step 1: Add one fixture pair covering all four directions**

Use request properties `request_added` and `request_removed`, plus response
properties `response_added` and `response_removed`. Change enum sets so one
value is added and one removed in each usage. Repeat the newly added response
value twice in the source enum to prove normalization emits only one finding.

- [ ] **Step 2: Add the failing acceptance matrix**

Assert:

```rust
[
    ("request enum value added", Severity::NonBreaking),
    ("request enum value removed", Severity::Breaking),
    ("response enum value added", Severity::Breaking),
    ("response enum value removed", Severity::NonBreaking),
]
```

Assert v4 Verify JSON has identical severities and messages.

- [ ] **Step 3: Run D-11 tests and confirm the duplicate diagnostic**

Run:

```powershell
cargo test --test cli_diff phase2_d11 -- --nocapture
cargo test --test cli_verify phase2_d11 -- --nocapture
```

Expected: the directional severities agree, but the duplicated response enum
value emits the same breaking finding twice.

- [ ] **Step 4: Make the policy table explicit in code**

Sort and deduplicate normalized enum values in `src/openapi/mod.rs`. Replace
separate add/remove helpers with:

```rust
#[derive(Clone, Copy)]
enum SetChange {
    Added,
    Removed,
}

fn enum_change_severity(
    usage: SchemaUsage,
    direction: SetChange,
) -> Severity {
    match (usage, direction) {
        (SchemaUsage::Request, SetChange::Added) => Severity::NonBreaking,
        (SchemaUsage::Request, SetChange::Removed) => Severity::Breaking,
        (SchemaUsage::Response, SetChange::Added) => Severity::Breaking,
        (SchemaUsage::Response, SetChange::Removed) => Severity::NonBreaking,
    }
}
```

Use `SetChange::{Added, Removed}` for composition-set classification too, so
the directional vocabulary has one definition.

- [ ] **Step 5: Run enum and composition regressions**

Run:

```powershell
cargo test --test cli_diff phase2_d11
cargo test --test cli_verify phase2_d11
cargo test --test cli_diff enum
cargo test --test cli_diff phase2_d09
```

Expected: all pass.

- [ ] **Step 6: Commit D-11**

```powershell
git add src/openapi/mod.rs src/diff/mod.rs tests/cli_diff.rs tests/cli_verify.rs testdata/openapi/phase2_d11_enum_policy_old.yaml testdata/openapi/phase2_d11_enum_policy_new.yaml
git commit -m "test: stabilize enum severity policy"
```

---

### Task 13: Complete v4 Size Evidence, Documentation, and Phase Gates

**Files:**
- Create: `docs/benchmarks/phase-2-v4-lock-size-report.json`
- Create: `docs/benchmarks/phase-2-v4-lock-size-report.md`
- Modify: `tools/lock-size-report/src/main.rs`
- Modify: `tools/lock-size-report/src/report.rs`
- Modify: `tools/lock-size-report/tests/cli.rs`
- Modify: `tests/compat.rs`
- Modify: `scripts/release_smoke.py`
- Modify: `.github/workflows/ci.yml`
- Modify: `README.md`
- Modify: `ROADMAP.md`
- Modify: `CHANGELOG.md`
- Modify: `DESIGN.md`
- Modify: `docs/change-rules.md`
- Modify: `docs/lockfile-spec.md`
- Modify: `implementation-log/2026-07-26-phase-2-orientation.md`

**Interfaces:**
- Produces: `v4_contract_bytes` per passing compatibility corpus entry.
- Produces: deterministic Phase 2 JSON/Markdown reports.
- Consumes: production v4 contract payload encoder and `DEFAULT_MAX_LOCK_BYTES`.

- [ ] **Step 1: Add failing production-v4 measurement tests**

Extend report rows:

```rust
pub struct CorpusResult {
    pub name: String,
    pub source_commit: String,
    pub sha256: String,
    pub source_bytes: u64,
    pub normalization_status: String,
    pub operation_count: Option<usize>,
    pub measurements: Option<ContractMeasurement>,
    pub v4_contract_bytes: Option<u64>,
    pub expected_error: Option<String>,
}
```

Assert the simple report includes a positive `v4_contract_bytes`, the value
fits 5,242,880 bytes, and privacy sentinels are absent from production v4
payload bytes.

- [ ] **Step 2: Run report tests and confirm the field is absent**

Run:

```powershell
cargo test -p apiwatch-lock-size-report
```

Expected: the new JSON and Markdown assertions fail.

- [ ] **Step 3: Expose a testable production v4 payload measurement**

Add a hidden library function:

```rust
#[doc(hidden)]
pub fn measure_v4_contract_payload(contract: &ApiContract) -> Result<u64> {
    v4::measure_contract_payload(contract)
}
```

Use the same interning and `contract_yaml` function as Lock, not a duplicate
encoder.

- [ ] **Step 4: Generate separate Phase 2 report outputs**

Add optional CLI flags `--v4-json-out` and `--v4-markdown-out`. Preserve the
Phase 1 report bytes under the existing flags. Render the Phase 2 report with
source identity, operation count, v4 payload bytes, ceiling result, and known
normalization failures.

- [ ] **Step 5: Add pinned-corpus v4 size gates**

For GitHub, Asana, and Box, build production v4 contracts and assert payload
bytes are at most `DEFAULT_MAX_LOCK_BYTES`. Preserve the Stripe recursive
schema and DigitalOcean metadata expected failures.

- [ ] **Step 6: Extend release smoke and CI**

Release smoke must:

- create a v4 lock;
- Verify a matching contract in text, JSON, and SARIF;
- verify one Phase 2 breaking fixture exits 1;
- prove v3 JSON reports partial coverage;
- prove failed v4 update preserves bytes.

CI runs the Phase 2 report with `--check` after fetching the pinned corpus and
replaces the D-16-only acceptance command with:

```yaml
- run: cargo test --test cli_verify phase2_
```

- [ ] **Step 7: Update public documentation**

Make these exact status changes:

- README: v4 is current; v3 is partial and requires re-locking.
- Roadmap: mark each D-01 through D-11 item completed only after its regression
  passes; mark the Phase 2 exit criterion met after the full gate.
- Change rules: document the approved request/response media, enum,
  requiredness, `additionalProperties`, server, and composition matrices.
- Lockfile spec: add the v4 wire fields, digest domains, coverage table,
  migration refusal, privacy exclusions, and scope identity.
- Changelog: add an unreleased v0.9.0 Phase 2 section without changing package
  version or publishing metadata.
- DESIGN: update the normalized contract and comparison-engine boundaries.

- [ ] **Step 8: Run the complete local verification matrix**

Run:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo +1.86.0 check --workspace --locked
python -m unittest discover -s scripts/tests -p "test_*.py"
python scripts/release_smoke.py
git diff --check
```

With the pinned corpus available, also run:

```powershell
python scripts/fetch_compat_specs.py
cargo test --test compat -- --ignored --nocapture
cargo run -p apiwatch-lock-size-report -- --manifest compat/specs.json --compat-dir .compat-cache --privacy-fixture testdata/openapi/privacy_sentinels.yaml --json-out docs/benchmarks/phase-1-lock-size-report.json --markdown-out docs/benchmarks/phase-1-lock-size-report.md --v4-json-out docs/benchmarks/phase-2-v4-lock-size-report.json --v4-markdown-out docs/benchmarks/phase-2-v4-lock-size-report.md --check
```

Expected: every command passes.

- [ ] **Step 9: Update the ignored implementation log**

Record the goal, semantic decisions, D-01 through D-11 commits, v4 migration
behavior, files/areas touched, red/green evidence, full verification results,
remaining blockers, and the next release action. Do not include complete
command transcripts or secret-bearing data.

- [ ] **Step 10: Commit Phase 2 integration**

```powershell
git add .github/workflows/ci.yml README.md ROADMAP.md CHANGELOG.md DESIGN.md docs/change-rules.md docs/lockfile-spec.md docs/benchmarks/phase-2-v4-lock-size-report.json docs/benchmarks/phase-2-v4-lock-size-report.md scripts/release_smoke.py tests/compat.rs tools/lock-size-report
git commit -m "docs: complete Phase 2 comparison engine"
```

- [ ] **Step 11: Verify the exact committed state**

Run:

```powershell
git status --short
cargo test --workspace
git log -13 --oneline
```

Expected: tracked working tree clean, workspace tests pass, and the Phase 2
commit series is visible. Stop before push, tag, publication, or package
repinning.
