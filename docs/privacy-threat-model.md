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
