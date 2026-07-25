# api.lock

`api.lock` is a repository-level lockfile for external API contracts.

Versions 1 and 2 store normalized operation routes. Version 3 stores a
complete normalized declared contract suitable for semantic verification.

## Format Status

| Version | Status | Declared entries | Observed entries |
|---|---|---|---|
| 1 | Readable legacy format | Route-only | Not supported |
| 2 | Readable legacy format | Route-only with provenance | Value-free shapes |
| 3 | Current format | Complete normalized contracts | Value-free shapes |

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

## Version 3

Version 3 stores complete normalized declared contracts in deterministic YAML:

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

`contract.operations` contains normalized operation, authentication,
parameter, request-body, and response data. `contract.schemas` is a
content-addressed table. Schema IDs are lowercase `sha256:` digests over a
domain-separated canonical representation. Repeated schemas share one table
entry; every referenced schema must exist, every stored schema must be
reachable, and its key must match its content.

`contract_digest` is a domain-separated SHA-256 digest over `scope`,
`contract`, and any `x-*` extensions. It excludes metadata, the measured byte
count, and the digest itself. Loading revalidates schema IDs, reachability,
`contract_bytes`, and `contract_digest`; tampering is rejected.

`contract_bytes` is the exact byte length of the standalone canonical YAML
serialization of `contract`, including its final newline. `max_lock_bytes`
defaults to 5,242,880 bytes and is enforced before any destination file is
created or replaced.

Semantic fields are strict: missing, malformed, or unknown semantic fields
are rejected. Optional forward-compatible metadata must use a direct `x-*`
key. Extension objects are recursively canonicalized so map insertion order
does not affect the digest.

### Scope

`scope: all` locks the full normalized contract. A scoped entry instead stores
an exact, sorted list:

```yaml
scope:
  operations:
    - GET /users/{id}
```

Create scoped locks with repeatable `--include-operation "METHOD /path"`
arguments. A selector absent at lock time is an input error. During Verify,
an absent selected operation is a breaking endpoint removal; unrelated
operations outside the stored scope are ignored.

### Create, Update, and Migration

Plain `apiwatch lock` creates a new file and refuses to overwrite existing
bytes. `--update` requires an existing lock and atomically replaces the named
declared entry after all parsing, size, integrity, and migration checks pass.

An existing v1 or v2 file can migrate only when the updated name is its sole
legacy declared entry. Observed entries are preserved. If other legacy
declared entries exist, migration is refused and lists the APIs whose
original OpenAPI sources are required. An observed name cannot be replaced as
declared.

Version 3 declared Verify reconstructs the locked contract and calls the same
semantic `diff_contracts` path as `apiwatch diff`. Breaking findings exit `1`;
warning-only and non-breaking findings exit `0`. Text, version-2 Verify JSON,
and SARIF share the same severities and messages.

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
This evidence selected the content-addressed YAML representation implemented
by version 3.

### Migration Policy

Versions 1 and 2 remain readable during migration. A route-only declared entry
cannot be upgraded into a complete contract from the lockfile alone because
the required schema, parameter, authentication, content-type, and response
data was never stored.

Users must re-lock from the original OpenAPI source to obtain a complete
version 3 declared entry. Legacy Verify remains available, reports
`coverage: routes` plus a `route_only_lock` limitation in JSON, emits a warning
for text, and records a SARIF tool execution notification. It never invents
missing contract data.

## Privacy

The lockfile avoids secrets, sensitive raw payloads, examples, headers, raw
OpenAPI fragments, and captured JSON values. Complete declared contracts may
add normalized schema metadata or canonical hashes while preserving this
boundary.

See [ROADMAP.md](../ROADMAP.md) for the implementation order and exit
criteria.
