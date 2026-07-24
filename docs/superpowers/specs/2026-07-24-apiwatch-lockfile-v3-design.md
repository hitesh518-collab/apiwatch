# APIWatch Lockfile v3 Design

**Date:** 2026-07-24

**Status:** Approved design; implementation pending

**Roadmap:** Phase 1, ordered scope items 2–9

## Purpose

Lockfile v3 makes declared API verification meaningful. It stores a complete,
normalized, privacy-safe contract so `apiwatch verify` can compare the locked
contract with the current contract through the same `diff_contracts` engine as
`apiwatch diff`.

The design productionizes the Phase 1 lock-size prototype. That prototype
measured expanded YAML, canonical JSON, and schema-deduplicated YAML on pinned
public contracts. Schema-deduplicated YAML was the only candidate below the
5,242,880-byte ceiling for every currently normalizable corpus entry.

## Goals

- Store the complete normalized declared contract represented by
  `ApiContract`.
- Keep declared and observed provenance explicit.
- Deduplicate repeated schemas within each declared API entry.
- Remain deterministic, reviewable, and self-contained in one YAML file.
- Validate schema content IDs and whole-contract integrity.
- Enforce a configurable 5,242,880-byte default limit per declared API.
- Support exact endpoint scoping for APIs that exceed the limit.
- Keep v1 and v2 readable with an explicit route-only limitation.
- Require deliberate, atomic migration from legacy declared entries.
- Keep raw source values and non-semantic OpenAPI content out of the lock.

## Non-Goals

- Repairing unrelated comparison semantics.
- Changing observed-contract inference, merging, or map behavior.
- Supporting OpenAPI 3.1.
- Resolving external or multi-file OpenAPI references.
- Orchestrating migration from multiple source documents in one command.
- Adding traffic capture, HAR ingestion, or proxy features.

## Chosen Architecture

Version 3 remains a single `api.lock` YAML document. Each declared API owns a
content-addressed contract payload and schema table. Observed entries retain
their existing value-free shape representation.

This per-API organization is preferred over a global schema store because it:

- keeps the size limit aligned with one upstream API;
- lets one entry update without changing another entry;
- contains schema references within one reviewable boundary;
- avoids cross-API garbage collection and integrity coupling.

A sidecar-file design is rejected because it would introduce path security,
multi-file atomicity, packaging, and review problems.

## File Structure

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
      operations:
        "GET /users/{id}":
          auth:
            oauth:
              kind: oauth2
              scopes:
                - users:read
          parameters:
            "path:id":
              required: true
              schema: sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
          request_body: null
          responses:
            "200":
              application/json: sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
      schemas:
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa":
          kind: string
          nullable: false
          format: uuid
          enum_values: []
          properties: {}
        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb":
          kind: object
          nullable: false
          format: null
          enum_values: []
          properties:
            id:
              required: true
              schema: sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
```

An observed entry remains:

```yaml
version: 3
apis:
  portfolio:
    provenance: observed
    shape:
      kind: object
      observations: 1
      properties: {}
```

Declared and observed entries can coexist in one v3 file.

## Top-Level Fields

### `version`

Required integer `3`. Other versions use their version-specific reader.

### `apis`

Required map keyed by normalized non-empty API name. Names are sorted
lexicographically when rendered. A name maps to exactly one provenance
variant.

Unknown top-level fields are rejected.

## Declared Entry

### `provenance`

Required literal `declared`.

### `source`

Required literal `openapi`. The current release has no other declared source
normalizer.

### `scope`

Required and encoded in one of two forms:

```yaml
scope: all
```

or:

```yaml
scope:
  operations:
    - GET /users
    - POST /users
```

Selectors are exact, unique, and sorted by normalized method and path. Methods
are uppercase supported HTTP methods. Paths begin with `/`, contain no control
characters, and otherwise preserve the normalized OpenAPI path exactly.

The scoped contract payload contains only selected operations. During locking,
every requested selector must exist. During Verify, a selected operation that
is absent from the current contract remains absent from the scoped current
contract so `diff_contracts` reports its removal as a breaking finding.
Unselected current operations are ignored.

### `max_lock_bytes`

Required positive integer. It records the limit used when the entry was last
written. The default is `5_242_880` bytes. A stored value above the current
default remains readable because it records historical policy.

### `contract_bytes`

Required non-negative integer equal to the UTF-8 byte length of the standalone
deterministic YAML encoding of `contract`, including its final newline.

The per-upstream budget includes only `operations` and `schemas`. It excludes
the API name, provenance, source, scope, extensions, integrity metadata,
observed entries, outer YAML indentation, and other APIs.

### `contract_digest`

Required string in this exact form:

```text
sha256:<64 lowercase hexadecimal characters>
```

It is recomputed when loading and must match the canonical digest input
defined below.

### `extensions`

Optional map. Every direct key must begin with `x-`. Values may contain only
JSON-compatible null, boolean, number, string, array, and string-keyed map
values. YAML tags and non-string map keys are rejected.

Extensions are preserved and integrity-protected but do not affect comparison
semantics. APIWatch never copies OpenAPI source extensions into this map
automatically.

### `contract`

Required declared-contract payload containing `operations` and `schemas`.

Unknown declared-entry and contract fields are rejected.

## Contract Payload

### Operations

`operations` is a sorted map keyed by exact `METHOD /path`. Each value contains:

- `auth`: sorted map keyed by normalized security-scheme name;
- `parameters`: sorted map keyed by `location:name`;
- `request_body`: null or a sorted content-type-to-schema-ID map;
- `responses`: sorted status-to-content-type-to-schema-ID maps.

Auth values contain:

- `kind`: `apiKey`, `basic`, `bearer`, `oauth2`, `openIdConnect`, `http`, or
  `unknown`;
- `scopes`: sorted unique strings.

The auth map key reconstructs `AuthRequirement.name`; no duplicate `name`
field is serialized.

Parameter locations are `path`, `query`, `header`, or `cookie`. Parameter
values contain:

- `required`: boolean;
- `schema`: schema ID.

The parameter map key reconstructs both location and name; no duplicate
`name` field is serialized. Parsing splits at the first `:`; the location and
name must both be non-empty, while later `:` characters remain part of the
name.

Status and media-type strings preserve the normalized `ApiContract` values.
All strings reject control characters. Schema IDs must exist in the same
declared entry. Every operation field is required, including empty `auth` and
`parameters` maps and a null `request_body`, so omission cannot change meaning.

### Schema Table

`schemas` is a sorted map keyed by schema ID. A schema node contains:

- `kind`;
- `nullable`;
- `format`;
- `enum_values`;
- `properties`.

Kinds are `object`, `array`, `oneOf`, `allOf`, `anyOf`, `string`, `integer`,
`number`, `boolean`, and `unknown`.

`properties` is a sorted map. Each property contains `required` and a child
schema ID. The normalized model's synthetic `items`, `oneOf[n]`, `allOf[n]`,
and `anyOf[n]` property names are preserved.

Every schema must be reachable from an operation through parameters, request
bodies, responses, or another schema. Missing references and orphan schema
nodes are invalid.

## Canonicalization and Integrity

Canonical bytes use compact UTF-8 JSON generated from fixed-order structs,
recursively sorted string-keyed maps, and the normalized array ordering.
Extension objects are recursively copied into sorted maps before hashing.
Canonical bytes contain no insignificant whitespace and end without a newline.

### Schema IDs

For each schema, APIWatch canonicalizes:

```json
{"domain":"apiwatch.schema.v3","schema":{...}}
```

Child schemas are interned first, so the parent contains only validated child
IDs. The ID is `sha256:` plus the lowercase SHA-256 digest of those canonical
bytes.

If two different canonical byte sequences produce one ID, loading or writing
fails with a digest-collision error. The error never prints source contents.

### Contract Digest

APIWatch canonicalizes:

```json
{
  "domain": "apiwatch.declared-contract.v3",
  "scope": "...",
  "contract": {"operations": {}, "schemas": {}},
  "extensions": {}
}
```

The actual encoding is compact and fixed-order. Missing `extensions` is
canonicalized as an empty map. The digest excludes `contract_digest`,
`contract_bytes`, `max_lock_bytes`, `provenance`, `source`, and the API name.

The digest protects all comparison-relevant data, the stored scope, and
explicit extensions without becoming self-referential.

### Load Validation Order

A v3 reader validates in this order:

1. YAML syntax, version, variants, and strict field sets.
2. API names, scalar values, operation keys, parameter keys, selectors, and
   extension keys.
3. Schema reference existence and reachability.
4. Each schema ID and collision invariant.
5. Deterministic `contract` byte count.
6. Whole-contract digest.
7. Internal reconstruction into `ApiContract`.

Any failure rejects the entire lockfile with exit code `2`. Diagnostics name
the entry and violated invariant but do not echo untrusted control characters,
credentials, raw bodies, or source values.

## Privacy Boundary

Declared v3 payloads contain only normalized comparison data. They exclude:

- OpenAPI descriptions and summaries;
- examples and defaults;
- source extensions;
- server URLs;
- credentials and authorization values;
- request or response payload examples;
- raw OpenAPI fragments;
- captured JSON values.

The existing privacy sentinel fixture is run through the production v3 writer
for all relevant tests. Explicit lockfile `extensions` are user-authored
metadata and are never populated from the source document.

## Lock Command

The production interface is:

```text
apiwatch lock <OPENAPI> --name <NAME> --output <PATH>
  [--update]
  [--include-operation "METHOD /path"]...
  [--max-lock-bytes <BYTES>]
```

### New File

Without `--update`, `lock`:

1. requires that the output path does not exist;
2. loads and normalizes the OpenAPI source;
3. validates and applies selectors;
4. builds the per-entry schema table;
5. computes size and integrity metadata;
6. renders a one-entry v3 file;
7. writes it atomically.

An existing output is an input error instructing the user to pass `--update`.

### Update Existing v3

With `--update`, `lock`:

1. requires that the output exists;
2. loads and fully validates it;
3. builds the replacement declared entry independently;
4. rejects a name currently used by an observed entry;
5. replaces or adds only the named declared entry;
6. preserves every other validated entry;
7. renders and atomically replaces the complete file.

### Legacy Migration

For v1 or v2:

- the named API must be the sole legacy declared entry;
- its original OpenAPI source must be supplied to `lock --update`;
- all observed entries are preserved;
- the complete file is rendered as v3;
- route-only data is never promoted or invented.

If another legacy declared entry exists, migration fails before mutation and
lists only the API names that still require original sources. Multi-source
migration orchestration is deferred.

### Atomicity

All parsing, normalization, scoping, measurement, digesting, validation, and
rendering completes in memory before filesystem mutation. The writer creates
a temporary file in the destination directory, writes and flushes it, and
uses a platform-appropriate atomic replacement. A failure leaves the previous
file byte-for-byte unchanged and removes the temporary file where possible.

## Verify Command

For a v3 declared entry:

1. load and validate the locked contract;
2. load and normalize the current local or remote OpenAPI input;
3. apply the stored scope to the current contract;
4. call `diff_contracts(locked, current)`;
5. render the returned `Change` values without translation to a second
   finding model.

Text messages, severity, ordering, JSON fields, SARIF rules, and fingerprints
derive from the same `Change` collection as `diff`.

Exit codes are:

- `0`: no breaking finding, including warning-only and non-breaking changes;
- `1`: one or more breaking findings;
- `2`: invalid input, invalid lock integrity, unsupported data, or operational
  failure.

## Output Contracts

### Text

No changes prints `Verified <name>`. Findings use the existing diff text
renderer.

A v1/v2 declared Verify writes this limitation to stderr before normal output:

```text
warning: api.lock v1/v2 declared entry is route-only; schema, parameter, authentication, content-type, and response changes are not verified
```

### JSON

Full-contract declared Verify uses a version-2 Verify result:

```json
{
  "version": 2,
  "command": "verify",
  "name": "users",
  "provenance": "declared",
  "coverage": "full",
  "limitations": [],
  "summary": {
    "breaking": 0,
    "warning": 0,
    "non_breaking": 0
  },
  "changes": []
}
```

Each change uses the same `severity`, `method`, `path`, and `message` fields as
Diff JSON.

Legacy declared Verify uses `coverage: "routes"` and a structured limitation:

```json
{
  "code": "route_only_lock",
  "message": "api.lock v1/v2 declared entry is route-only; full contract changes are not verified"
}
```

It maps removed routes to breaking findings and added routes to warning
findings. Existing version-1 JSON remains documented as the pre-v3 schema.

Observed Verify retains its current versioned output contract.

### SARIF

Full-contract Verify uses the same diff rule IDs, levels, messages, and rule
metadata. Fingerprints include the Verify command and API name so findings
remain stable and distinct across entries.

Legacy route-only Verify adds a warning-level
`toolExecutionNotification` with ID `apiwatch/route-only-lock`. SARIF output
remains valid JSON with no warning text written to stdout.

## Error Handling

All user-controlled names and paths are contextualized without printing
control characters. Integrity diagnostics identify the entry and invariant,
not the raw canonical bytes.

Representative exit-2 errors include:

- output exists without `--update`;
- `--update` output is missing;
- `--max-lock-bytes` is zero;
- malformed or duplicate operation selector;
- requested lock-time selector is absent;
- payload exceeds the configured limit;
- unsupported lock version or provenance;
- ambiguous legacy migration;
- declared/observed name collision;
- missing or orphaned schema;
- invalid schema or contract digest;
- invalid recorded byte count;
- unknown semantic field.

## Testing Strategy

### Wire and Integrity Tests

- deterministic schema IDs with domain separation;
- deterministic contract digest;
- forced digest collision rejection;
- missing-reference and orphan rejection;
- malformed operation and parameter key rejection;
- strict unknown-field rejection;
- valid and invalid `x-...` extensions;
- exact byte-count validation;
- golden YAML load/render round trip.

### Lock CLI Tests

- new v3 file creation;
- existing-output refusal without `--update`;
- deterministic re-lock;
- v3 replacement and addition while preserving other entries;
- observed-name collision rejection;
- exact selector parsing and stored scope;
- configured limit equality succeeds and one byte over fails;
- every failure preserves the destination bytes;
- privacy fixture emits none of its sentinels.

### Migration Tests

- v1 sole declared entry migrates;
- v2 sole declared plus observed entries migrates and preserves shapes;
- multiple legacy declared entries are rejected without mutation;
- missing original target is rejected;
- no legacy route-only data is converted into a complete contract.

### Verify Tests

- v3 no-change success;
- stored scope ignores unrelated endpoints;
- selected endpoint removal is a breaking finding, not an input error;
- warning-only changes exit `0`;
- breaking changes exit `1`;
- corrupt lock data exits `2`;
- local and remote inputs use the same comparison flow;
- text, JSON, and SARIF derive from identical `Change` values;
- v1/v2 warnings are present in all output formats.

The D-16 acceptance fixture must produce four correctly classified findings:
authentication addition, parameter removal, parameter addition/retype, and
successful-response removal.

### Regression Gates

- `cargo fmt --all -- --check`;
- strict workspace Clippy;
- full workspace tests;
- Rust 1.86 workspace check;
- Python tests;
- all pinned compatibility expectations;
- deterministic lock-size report `--check`;
- release smoke;
- privacy sentinel scan.

## Implementation Sequence

1. Introduce public-in-crate v3 wire types, canonicalization, and validators.
2. Add v3 rendering, loading, and internal `ApiContract` reconstruction.
3. Add CLI options, new-file safety, size enforcement, and atomic updates.
4. Implement strict legacy migration while preserving observed entries.
5. Route v3 declared Verify through `diff_contracts`.
6. Unify full Verify output with diff findings and add legacy warnings.
7. Add D-16 acceptance coverage, documentation, Action updates, and CI gates.

Each step is committed independently after its focused and workspace tests
pass.

## Completion Criteria

This work is complete when:

- `lock` writes deterministic, privacy-safe v3 files;
- configured size and scope policies are enforced;
- v3 integrity corruption is rejected;
- v1/v2 remain readable with explicit limitations;
- migration never invents missing contract data;
- declared Verify calls `diff_contracts` on the locked and current contracts;
- D-16 produces four correctly classified findings;
- text, JSON, SARIF, and the reusable Action agree;
- all regression gates pass.
