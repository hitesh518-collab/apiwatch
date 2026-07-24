# api.lock

`api.lock` is a repository-level lockfile for external API contracts.

The implemented formats are intentionally small. Declared entries in versions
1 and 2 store normalized operation routes, not complete schemas, parameters,
authentication, content types, or response contracts.

## Format Status

| Version | Status | Declared entries | Observed entries |
|---|---|---|---|
| 1 | Readable legacy format | Route-only | Not supported |
| 2 | Current development format | Route-only with provenance | Value-free shapes |
| 3 | Approved direction; not designed or implemented | Complete normalized contracts | First-class observed contracts |

The exact version 3 schema, canonical digest representation, endpoint-scoping
CLI, and migration command remain
[Phase 1](../ROADMAP.md#phase-1--make-verify-meaningful) design decisions.
No version 3 example in this document should be inferred from the goals below.

## Version 1

```yaml
version: 1
apis:
  users:
    source: openapi
    operations:
      - method: GET
        path: /users
      - method: POST
        path: /users
```

## Fields

- `version`: lockfile format version. The initial format uses `1`.
- `apis`: map of API names to locked API metadata.
- `apis.<name>.source`: source kind used to produce the lock. The initial command writes `openapi`.
- `apis.<name>.operations`: deterministic list of normalized operations.
- `method`: uppercase HTTP method.
- `path`: normalized OpenAPI path template.

## Version 2

Version 2 keeps declared entries readable while adding explicit provenance for
declared and observed contracts:

```yaml
version: 2
apis:
  users:
    provenance: declared
    source: openapi
    operations:
      - method: GET
        path: /users
  portfolio:
    provenance: observed
    shape:
      kind: object
      observations: 1
      properties:
        live_price:
          observations: 1
          shape:
            kind: number
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

- `provenance: declared` retains the OpenAPI `source` and `operations` fields.
- `provenance: observed` stores a value-free JSON shape. Supported shape kinds
  are `null`, `boolean`, `number`, `string`, `object`, `map`, `array`,
  `union`, and `unknown`.
- Object-property `observations` determine requiredness across merged
  recordings. Array item shapes use `unknown` until a non-empty array is
  observed. Union variants are deterministic.
- Version-1 declared locks remain readable. Adding an observed entry upgrades
  the rendered lock to version 2.

### Observed Maps

`apiwatch record` can explicitly annotate dynamic-key objects with repeatable
`--map-at <JSONPATH>` options. The accepted JSONPath subset is `$` and named
property segments only, for example `$.by_broker` or `$.state.by_region`. A
segment begins with an ASCII letter or underscore and may continue with ASCII
letters, digits, or underscores. Empty segments, bracket notation, arrays,
wildcards, filters, scripts, and every other JSONPath form are rejected.

An annotation converts the selected object into `kind: map`. The node stores a
single merged `values` shape and retains neither dynamic keys nor captured
values. Empty maps use `unknown` values. During `record --merge`, map values
merge monotonically with later ordinary JSON objects; a normal object becomes
a map only through an explicit annotation.

Verify is directional: a locked map accepts an actual object with any keys,
including no keys, but every actual value must match the locked `values` shape.
An actual scalar, array, or null at a locked map path is incompatible. Map
diagnostics use the annotated path plus a stable `<map-value>` segment in place
of each dynamic key, along with shape names only. This redacted notation is
used consistently in text, JSON, SARIF messages, and SARIF fingerprints.

## Planned Version 3

The breaking version 3 direction is approved in the
[product pivot design](superpowers/specs/2026-07-24-apiwatch-product-pivot-design.md).
Implementation requires a separate format design and explicit approval.

Version 3 must:

- store enough normalized declared contract data for Verify to call the same
  comparison engine as `diff`;
- keep declared and observed provenance explicit and first-class;
- serialize deterministically with stable ordering;
- remain reviewable in Git;
- tolerate unknown fields where forward compatibility permits;
- exclude captured values, examples, defaults, credentials, headers, and
  other source data that is unnecessary for comparison;
- target a 5 MB default ceiling per upstream API committed to Git;
- support explicit endpoint scoping for larger APIs.

The size ceiling is a design target, not an implemented file-size limit.
Phase 1 must prototype representative small and large specifications before
choosing the final serialization and scoping interfaces.

### Phase 1 Prototype Results

The completed lock-size prototype recommends `deduplicated_yaml`. It is the
only tested full-contract representation that remains below the 5,242,880-byte
ceiling for every currently normalizable public corpus entry: GitHub measures
2,327,580 bytes, Asana 806,691 bytes, and Box 485,332 bytes. Expanded YAML and
canonical JSON exceed the ceiling on the GitHub contract. Privacy sentinels
remain absent from all three candidate representations.

The reproducible evidence is available as a
[human-readable report](benchmarks/phase-1-lock-size-report.md) and
[machine-readable report](benchmarks/phase-1-lock-size-report.json).
This result selects the representation to take into the exact schema design;
it does not implement or approve the final version 3 schema.

### Migration Policy

Versions 1 and 2 remain readable during migration. A route-only declared entry
cannot be upgraded into a complete contract from the lockfile alone because
the required schema, parameter, authentication, content-type, and response
data was never stored.

Users must therefore re-lock from the original OpenAPI source to obtain a
complete version 3 declared entry. Migration tooling may preserve names and
other available metadata, but it must warn clearly and must not invent missing
contract data. The command syntax is deferred to the Phase 1 design.

## Privacy

The lockfile avoids secrets, sensitive raw payloads, examples, headers, raw
OpenAPI fragments, and captured JSON values. Complete declared contracts may
add normalized schema metadata or canonical hashes while preserving this
boundary.

See [ROADMAP.md](../ROADMAP.md) for the implementation order and exit
criteria.
