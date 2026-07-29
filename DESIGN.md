# apiwatch Design

APIWatch is a Rust CLI built around deterministic contract normalization,
locking, comparison, and CI-friendly reporting.

## Contract Paths

### Declared Contracts

```text
OpenAPI 3.0 document
        ↓
normalized ApiContract
        ↓
diff / lock / Verify
        ↓
text, JSON, or SARIF
```

`diff` compares two normalized contracts. The normalized `ApiContract`
boundary owns semantic operation identity, effective server templates,
authentication wire identity, parameters, request/response media, schema
requiredness and nullability, formats, enums, composition branches,
first-class array items, and `additionalProperties` policy. Input-specific
OpenAPI labels, branch order, examples, defaults, and literal server values do
not cross that boundary.

Current v4 declared locks encode that complete normalized model in a
content-addressed, value-free wire contract. Declared Verify reconstructs an
`ApiContract` and calls the same `diff_contracts` comparison path as `diff`.
Version 3 reconstructs its older model but reports partial Phase 2 coverage;
versions 1 and 2 remain route-only.

### Observed Contracts

```text
explicit JSON samples
        ↓
value-free observed shape
        ↓
monotonic merge / lock / Verify
        ↓
text, JSON, or SARIF
```

`record` is the only learning operation. It may create or widen an observed
entry. `verify` is read-only and reports directional shape drift. Explicit
`--map-at` annotations distinguish dynamic-key maps from fixed API objects;
APIWatch does not silently infer that semantic change.

Observed locks and diagnostics retain paths, shape kinds, provenance, and
observation metadata only. They exclude captured scalar values, credentials,
and dynamic map keys.

## Stable Boundaries

- Normalized contracts isolate input parsing from comparison.
- The comparison engine consumes only normalized contracts; it does not parse
  OpenAPI or inspect lockfile wire structures.
- Lockfile v4 owns deterministic interning, wire validation, digest
  revalidation, size enforcement, and reconstruction back to `ApiContract`.
- Lock entries carry explicit declared or observed provenance.
- Text, versioned JSON, and SARIF formatters present comparison results.
- Exit code `0` means clean, `1` means drift or breaking change, and `2` means
  invalid input or operational failure.
- Ordering and serialization are deterministic.
- Verify never mutates or widens a lock.

## Design Records

- [Original OpenAPI-first design](docs/superpowers/specs/2026-07-08-apiwatch-design.md)
- [Approved product pivot](docs/superpowers/specs/2026-07-24-apiwatch-product-pivot-design.md)
- [Authoritative roadmap](ROADMAP.md)
- [Lockfile specification](docs/lockfile-spec.md)
- [Semantic change rules](docs/change-rules.md)
