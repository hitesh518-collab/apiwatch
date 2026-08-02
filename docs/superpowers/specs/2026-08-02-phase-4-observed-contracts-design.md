# Phase 4 — Trustworthy Observed Contracts: Design

**Target:** v0.11.0
**Date:** 2026-08-02
**Status:** approved

## Goal

Make the confidence and boundaries of inferred response shapes explicit enough for
reliable CI use. Users must be able to distinguish verified structure from
insufficient evidence, repeated input must produce byte-identical locks, and
locks and diagnostics must contain no captured scalar values, credentials, or
dynamic map keys.

## Approach

Entry-metadata wrapper + shape threshold logic. A new `ObservedEntry` struct
carries lockfile-level metadata (threshold, timestamps) while `Shape` itself
gains minimal changes. Threshold logic (same flag for both D-17 null hardening
and D-18 requiredness) lives in merge and compare, not in new Shape variants.

## 1. Lockfile Model

### ObservedEntry

```rust
pub struct ObservedEntry {
    pub shape: Shape,
    pub threshold: f64,        // 0.0..=1.0, default 0.5
    pub first_seen: String,    // ISO 8601, set at initial record
    pub last_seen: String,     // ISO 8601, updated on --merge
}
```

- `ApiLock.observed` changes from `BTreeMap<String, Shape>` to `BTreeMap<String, ObservedEntry>`
- All call sites that read/write observed entries are updated
- Wire format: each observed entry serializes `threshold`, `first_seen`, `last_seen`, and `shape`
- Backward compat: existing v2/v3/v4 locks without these fields default `threshold = 1.0` (current binary-required behavior), `first_seen = ""`, `last_seen = ""`
- No lockfile version bump — extends v4 observed section with optional fields

### Threshold on `record`

```
apiwatch record --from-json body.json --name my-api --output api.lock --required-threshold 0.8
```

- Default: `0.5`
- Range: `0.0..=1.0`
- Immutable after first record for a given entry (subsequent `--merge` calls ignore the flag or error if it differs)
- Reject threshold changes on `--merge` to prevent silent drift

## 2. D-17 — Null Hardening

A field observed exclusively as `null` may be underdetermined (especially with
few samples). The hardening rule combines a floor and a ratio gate.

### Hardening Rule

A property is **hardened** (shape strictly enforced at verify) only when:

1. **Floor**: parent object has ≥ 3 total observations
2. **Ratio**: `property_observations / parent_observations >= threshold`

A property that fails either check is **lenient** — its expected shape is treated
as `Shape::Unknown` for verify purposes (accepts any actual shape without
signaling drift).

### Example

| Samples | Parent obs | Property obs | Ratio (thresh=0.5) | Floor ≥3? | Result |
|---------|-----------|-------------|---------------------|-----------|--------|
| `{x: null}` x1 | 1 | 1 | 1.0 | No | Lenient |
| `{x: null}` x3 | 3 | 3 | 1.0 | Yes | Hardened as null |
| `{x: null}` x5 | 5 | 5 | 1.0 | Yes | Hardened as null |
| `{x: null}` x2, `{x: "hello"}` x8 | 10 | 10 (union) | 1.0 | Yes | Hardened as Union{Null, String} |

When a property is lenient at verify, `compare_at` treats the expected shape
as if it were `Shape::Unknown` — the comparison short-circuits with no change
reported. When hardened and shape is `Shape::Null`, any non-null actual value
produces an `IncompatibleShape` change.

### Implementation Notes

- The null-hardening check happens in `compare_at`, not in `merge`. Merge
  continues to produce the tightest shape from available evidence; leniency is
  a verify-time softening.
- Hardening state is computed per-property per-verify, not persisted in the
  lock. This keeps the lockfile a faithful record of observations.

## 3. D-18 — Requiredness Threshold

Replaces the current binary requiredness check.

### Current Rule

```rust
// property is required if it appeared in every observation
None if expected_property.observations == *observations => {
    // MissingRequiredField change
}
```

### New Rule

A property is **required** when:

1. **Floor**: parent object has ≥ 3 total observations
2. **Ratio**: `property_observations / parent_observations >= threshold`

If either fails, the property is **optional** — its absence at verify does not
produce a `MissingRequiredField` change.

### Example (threshold 0.8)

| Parent obs | Property obs | Ratio | Floor ≥3? | Result |
|-----------|-------------|-------|-----------|--------|
| 100 | 100 | 1.0 | Yes | Required |
| 100 | 80 | 0.8 | Yes | Required |
| 100 | 79 | 0.79 | Yes | Optional |
| 2 | 2 | 1.0 | No | Optional |
| 10 | 1 | 0.1 | Yes | Optional |

### CLI

```
apiwatch record --required-threshold 0.8 ...
```

Threshold stored in `ObservedEntry.threshold`. Verify reads it from the
lockfile and applies it to both D-17 and D-18 checks.

## 4. Empty Container Evolution

Empty arrays (`Array { items: Unknown }`) and empty objects (`Object { properties: {} }`) remain lenient at verify time — they accept any populated actual without signaling drift.

In tiered reporting, empty containers are classified as **insufficiently observed** regardless of observation count, because they provide zero structural evidence about their contents.

Current merge behavior is unchanged: merging a populated sample into an empty container narrows the shape (e.g., `Unknown` items become a concrete type).

## 5. Confidence Metadata

### Entry-Level Timestamps (`ObservedEntry`)

| Field | Set when | Updated when |
|-------|----------|-------------|
| `first_seen` | `record` (new entry, `--merge` absent) | Never |
| `last_seen` | `record` (new entry) | `record --merge` |
| `threshold` | `record --required-threshold` | Never (immutable after first record) |

### Per-Shape Observation Counts (existing, no changes)

- `Shape::Object.observations`: total times this object shape was observed
- `ObservedProperty.observations`: total times this property was present

### Wire Format

```yaml
observed:
  my-api:
    threshold: 0.50
    first_seen: "2026-08-02T10:00:00Z"
    last_seen: "2026-08-02T10:05:00Z"
    shape:
      kind: object
      observations: 10
      properties:
        id:
          observations: 10
          shape:
            kind: number
```

### Output Exposure

Confidence metadata (threshold, timestamps, observation counts) is included in
the verify output header for text, JSON, and SARIF formats alongside the entry
name and provenance.

## 6. Tiered Reporting

Observed verify output gains three categories beyond breaking changes:

| Tier | Condition | Output category |
|------|-----------|----------------|
| **Verified** | Property is hardened (≥ floor, ≥ ratio) and shape matches | (no output) |
| **Insufficiently observed** | Property is lenient (< floor or < ratio), or empty container | Non-breaking informational |
| **Unverified** | Field present in actual JSON but absent in lock | Non-breaking informational |

Breaking changes (type mismatch, missing hardened required fields) continue to
report as today. The three tiers add informational categories on top.

### Text Output

```
Verified my-api (observed, threshold 0.50)
  first seen: 2026-08-02T10:00:00Z
  last seen:  2026-08-02T10:05:00Z

Insufficiently observed:
  - $.metadata (object, seen 2/50 times, threshold 0.50)
  - $.items[] (empty array, no item evidence)

Unverified:
  - $.new_field (string, not in lock)
```

### JSON Extensions

`ObservedVerifyJson` gains `insufficiently_observed` and `unverified` arrays.
Each entry includes `path`, optional `kind`, and `detail`.

### SARIF Extensions

New rule IDs:
- `apiwatch/verify-observed-insufficient` (level: `warning`)
- `apiwatch/verify-observed-unverified` (level: `note`)

Existing breaking-change rules are unchanged.

## 7. Map-At Semantics Preservation

No changes to `--map-at`. Existing invariants confirmed by tests:

- Map annotations are never silently inferred from observed structure
- Threshold/leniency logic does not affect map-annotated paths
- Duplicate, overlapping, invalid, and non-object target paths are rejected
- Map values remain key-free in serialized output

Phase 4 regression requirement: all existing map-at tests must remain green.

## 8. Privacy Threat Model

New document: `docs/privacy-threat-model.md`.

Describes:
- **What APIWatch captures**: structural shapes, property names (user's API contract), observation counts, timestamps. Never scalar values, credentials, or dynamic map keys.
- **What APIWatch does not capture**: response body values, authentication tokens, API keys, PII within JSON bodies.
- **Attack surface**: shape-structure side channels (e.g., deeply nested property names encoding data), lockfile size as observation-count oracle.
- **Mitigations**: property names originate from the user's JSON keys, not user secrets; shapes are value-free by construction; `map-at` explicitly strips dynamic-key names.
- **Residual risks**: timing side channels during recording, differential observation counts leaking access patterns.
- **Recommendations**: run recording in CI with sanitized test data, not production traffic with PII.

## 9. Property Tests

New tests covering Phase 4 invariants. Added as a `#[cfg(test)] mod property_tests` in `src/observed/mod.rs` or as integration tests in `tests/`.

### Test Cases

1. **Round-trip determinism**: `infer(sample) → serialize → deserialize → infer` produces identical shape
2. **Merge idempotence**: `merge(A, A)` produces the same shape as `A`
3. **Compare reflexivity**: `compare(A, A)` is always empty
4. **Order invariance**: object key order in input JSON does not change inferred shape
5. **Value absence**: no scalar value, credential-like string, or dynamic map key appears in any serialized shape or diagnostic output
6. **Threshold edge cases**: threshold 0.0 (all optional), threshold 1.0 (all required, current behavior), threshold 0.5 with various observation distributions
7. **Floor boundary**: parent with 2 vs 3 observations, property with varying ratios
8. **Null hardening**: single-sample null vs multi-sample null vs null-in-union

### Approach

Hand-written table-driven tests with explicit JSON fixtures (preferred over
`proptest` to keep test determinism and avoid adding a dev-dependency). Each
invariant test uses small, focused sample sets.

## Files Touched

| File | Changes |
|------|---------|
| `src/observed/mod.rs` | `ObservedEntry` struct, D-17 leniency in `compare_at`, D-18 threshold math, tiered reporting data structures, property tests |
| `src/lockfile/mod.rs` | `ApiLock.observed` type change, `record_observed` signature, `select_verify_target`, wire format, backward compat |
| `src/lockfile/v3/mod.rs` | Observed storage format |
| `src/lockfile/v4/mod.rs` | Observed storage format |
| `src/cli.rs` | `--required-threshold` on `Record` subcommand |
| `src/main.rs` | Updated `Command::Record` and `Command::Verify` (observed path) to use `ObservedEntry` |
| `src/output/mod.rs` | Tiered reporting for text/JSON/SARIF, new SARIF rules, confidence metadata in headers |
| `docs/privacy-threat-model.md` | New document |
| `tests/` | Integration tests for tiered output, threshold behavior |

## Exit Criterion

Users can distinguish verified structure from insufficient evidence, repeated
input produces byte-identical locks, and locks and diagnostics contain no
captured scalar values, credentials, or dynamic map keys.

## Excluded

- HAR or live capture (Phase 5)
- Proxy operation (post-v1)
- Enum inference without separate privacy review
- `proptest` dependency (use hand-written table-driven property tests)
