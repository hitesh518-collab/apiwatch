# api.lock

`api.lock` is a repository-level lockfile for external API contracts.

Versions 1 and 2 store normalized operation routes. Version 3 stores the
Phase 1 normalized declared contract. Current version 4 stores the complete
Phase 2 normalized contract suitable for full semantic verification.

## Format Status

| Version | Status | Declared entries | Observed entries |
|---|---|---|---|
| 1 | Readable legacy format | Route-only | Not supported |
| 2 | Readable legacy format | Route-only with provenance | Value-free shapes |
| 3 | Readable legacy format | Partial Phase 2 contracts | Value-free shapes |
| 4 | Current format | Complete Phase 2 contracts | Value-free shapes |

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
- Observed entries currently serialize as v2 (extended with `threshold`,
  `first_seen`, and `last_seen` metadata as of v1.0.3). A migration to a
  content-addressed observed format (v4/v5) is planned for APIWatch v2.0.0.
  Until then, v2 is the stable observed format and covered by the same
  legacy read support as declared v2/v3 entries.

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
and SARIF share the same severities and messages. Because v3 predates the
Phase 2 wire fields, Verify reports partial coverage and requires re-locking
for full current behavior.

## Version 4

Version 4 is the current declared-contract format. Its outer declared entry
retains the v3 fields:

- `source: openapi`;
- `scope`;
- `max_lock_bytes`;
- `contract_bytes`;
- `contract_digest`;
- optional direct `x-*` extensions;
- `contract.operations` and `contract.schemas`.

The v4 operation key is `METHOD <canonical-path-identity>`. Each operation
stores:

- `display_path`, retaining the normalized source placeholder labels for
  diagnostics;
- `auth`, keyed by source label and containing `kind`, semantic `identity`,
  and sorted unique `scopes`;
- `servers`, the sorted effective privacy-safe server templates;
- `parameters`, keyed by location and semantic name, with display `name`,
  `required`, and a schema ID;
- nullable `request_body`, with explicit boolean `required` and canonical
  media-type-to-schema mappings;
- `responses`, mapping status codes to canonical media-type-to-schema
  mappings.

Authentication identity is strict and tagged: API keys store location and
wire name; HTTP stores scheme; OAuth2 stores sorted flow kinds plus canonical
authorization, token, and refresh endpoint templates; OpenID Connect stores
the canonical discovery template; unresolved schemes retain only their
normalized kind. A stored kind that disagrees with its identity is rejected.

Each content-addressed v4 schema stores:

- `kind`, `nullable`, optional `format`, and sorted unique `enum_values`;
- `properties`, whose values contain `required` and a schema ID;
- optional first-class `items`;
- `additional_properties` as `forbidden`, `any`, or `schema` with a schema ID;
- sorted unique `branches` for `oneOf` and `anyOf`.

Unknown fields, unknown dictionary policy, orphaned schemas, incorrect schema
IDs, invalid references, duplicate normalized identities, and noncanonical
arrays are rejected.

### Digest domains and payload measurement

Schema IDs hash canonical JSON containing the literal domain
`apiwatch.schema.v4` and the complete wire schema. `contract_digest` hashes
canonical JSON containing `apiwatch.declared-contract.v4`, `scope`,
`contract`, and recursively canonicalized direct `x-*` extensions. Both are
rendered as `sha256:` plus 64 lowercase hexadecimal characters.

`contract_bytes` is the exact UTF-8 length of the standalone deterministic
YAML serialization of the production v4 `contract`, including its final
newline. Lock creation and the committed Phase 2 size report call this same
interning and serialization path. The default maximum is 5,242,880 bytes.

### Scope and operation identity

Path placeholder labels are display data, not endpoint identity. `/users/{id}`
and `/users/{userId}` both use `/users/{0}` as the canonical operation and
scope identity. Later slots use `{1}`, `{2}`, and so on. The stored
`display_path` preserves the source spelling used for findings.

`scope: all` covers every normalized operation. Scoped selectors are
validated against the source, normalized to uppercase method plus canonical
positional path identity, sorted, and deduplicated. Verify applies the same
identity normalization, so harmless placeholder renames do not become
endpoint removals and additions.

### Coverage and migration

| Lock version | Declared Verify coverage | Structured limitation | Migration |
|---|---|---|---|
| 1–2 | `routes` | `route_only_lock` | Re-lock from the original source |
| 3 | `partial` | `phase2_relock_required` | Re-lock from the original source |
| 4 | `full` | None | Current |

`lock --update` can migrate an older file only when the updated name is its
sole pre-v4 declared entry. Observed entries are preserved. If any other v1,
v2, or v3 declared entry remains, migration is refused and lists every API
whose original source is required. An observed entry cannot be replaced as
declared. Parsing, normalization, scope, size, integrity, or migration failure
leaves the destination bytes unchanged.

### Privacy exclusions

The v4 contract excludes examples, defaults, descriptions, source extensions,
raw OpenAPI fragments, raw server literals, server credentials, literal query
values, captured JSON values, headers, and response bodies. It retains only
normalized comparison data: operation/display identities, semantic
authentication identity, privacy-safe server templates, canonical media
types, requiredness, schema kinds/formats/enums, composition, array items, and
dictionary policy. The production privacy fixture is checked against the
exact v4 payload encoder.

### Phase 2 production-size evidence

Every currently normalizable pinned corpus entry fits the default production
v4 payload ceiling: GitHub is 2,569,165 bytes, Asana is 946,072 bytes, and Box
is 589,237 bytes. Stripe remains an expected recursive-schema failure and
DigitalOcean remains an expected malformed-metadata failure.

See the [human-readable Phase 2 report](benchmarks/phase-2-v4-lock-size-report.md)
and [machine-readable Phase 2 report](benchmarks/phase-2-v4-lock-size-report.json).

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
version 4 declared entry. Legacy Verify remains available: v1/v2 reports
`coverage: routes` plus `route_only_lock`; v3 reports `coverage: partial` plus
`phase2_relock_required`. Text emits a warning and SARIF records a tool
execution notification. Verify never invents missing contract data.

## Privacy

The lockfile avoids secrets, sensitive raw payloads, examples, defaults,
descriptions, headers, raw OpenAPI fragments, literal server query values,
credentials, and captured JSON values. Complete declared contracts retain
only normalized semantic metadata and canonical hashes.

See [ROADMAP.md](../ROADMAP.md) for the implementation order and exit
criteria.
