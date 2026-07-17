# api.lock

`api.lock` is a repository-level lockfile for external API contracts.

The first lockfile version is intentionally small and stores normalized operation metadata produced by the single-API `apiwatch lock` command.

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

## Privacy

The lockfile avoids secrets, sensitive raw payloads, examples, headers, raw
OpenAPI fragments, and captured JSON values. Future versions may add schema
metadata or hashes while keeping sensitive input out of the file.
