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

`diff` compares two normalized contracts. Current v1 and v2 declared locks
store only normalized operation routes, so current declared Verify detects
route drift rather than complete semantic drift.

Phase 1 will design lockfile v3 and store enough normalized contract data for
declared Verify to deserialize the lock and call the same `diff_contracts`
comparison path as `diff`. This is an approved direction, not an implemented
format. The exact v3 representation remains a separate design decision.

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
