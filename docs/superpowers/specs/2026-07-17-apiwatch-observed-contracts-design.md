# APIWatch Observed Contracts Design

## Goal

Deliver the first compatibility-preserving observed-contract workflow in the
v0.6.5 line. APIWatch will record the structural shape of a local JSON body,
merge later observations without narrowing the recorded shape, and verify a
new local JSON body against that observed shape.

The feature protects consumers when an OpenAPI contract is absent, empty, or
incomplete. It is an explicit learning workflow, not runtime monitoring:
`record` may update a lock; `verify` only reads it.

## Scope

- Versioned `api.lock` support for declared and observed entries.
- `apiwatch record --from-json <BODY> --name <NAME> --output <PATH>`.
- Monotonic `record --merge` for an existing observed entry.
- Provenance-aware `apiwatch verify <INPUT> --name <NAME> --lock <PATH>`.
- Observed Verify text, JSON, and SARIF output.
- Fixture-driven privacy, compatibility, deterministic-rendering, and CLI
  coverage.

## Deferred Scope

- `--map-at` annotations and map inference assistance.
- Declared, observed, and unverified coverage reporting and
  `--fail-on-unverified`.
- HAR recording, live URL recording, consumer projections, enum inference,
  runtime monitoring, source discovery, dashboards, cloud services, accounts,
  and AI features.

Map and coverage work needs an explicit association between observations and
the OpenAPI operation universe. It follows this self-contained
record-merge-verify vertical slice rather than introducing an implicit or
unreliable association now.

## Lockfile Compatibility

Existing version-1 declared lockfiles remain readable without modification:

```yaml
version: 1
apis:
  users:
    source: openapi
    operations:
      - method: GET
        path: /users
```

Version 2 is written whenever an observed entry is added. It represents every
entry with explicit provenance:

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
      observations: 2
      properties:
        holdings:
          observations: 2
          shape:
            kind: array
            items:
              kind: object
              observations: 3
              properties:
                ticker:
                  observations: 3
                  shape:
                    kind: string
        live_price:
          observations: 2
          shape:
            kind: union
            variants:
              - kind: null
              - kind: number
```

The v1 loader converts declared entries to the internal declared representation
without changing its public behavior. When `record` adds an observed entry to
a v1 lock, it writes an equivalent deterministic v2 declared entry beside the
observed one. A declared `lock` command remains able to create its current v1
single-API file; no existing declared lock is rewritten merely because it is
loaded for verification.

Every v2 render sorts API names, object-property names, and union variants;
uses a final newline; and never serializes JSON values, examples, request
credentials, headers, or response bodies.

## Observed Shape Model

The observed module normalizes `serde_json::Value` into value-free shape nodes.
Supported kinds are `null`, `boolean`, `number`, `string`, `object`, `array`,
`union`, and `unknown`.

- An object records its total `observations` and a sorted property map. Each
  property records the number of parent-object observations in which it was
  present. A property is required exactly when that count equals the parent
  object count; it is otherwise optional.
- An array always contains an `items` shape. An empty array has
  `items.kind: unknown`, allowing a later populated observation without
  falsely reporting drift.
- An empty object is still an explicit `object` with zero properties. It has
  no inferred property constraints, so later properties can be learned
  safely.
- Differing shapes normalize to a sorted union. Union branches are merged by
  kind where possible, so `number` plus `null` renders as a stable
  `number | null`-equivalent union instead of retaining source order.
- JSON integers and decimals are both `number`; the model deliberately does
  not retain numeric values or ranges.

`unknown` accepts any concrete later shape. It is used only for deliberately
unobserved collection positions, not as a fallback for invalid JSON.

## Commands And File Updates

The CLI adds:

```text
apiwatch record --from-json <BODY> --name <NAME> --output <PATH> [--merge]
```

`record` loads only a local JSON file. It creates a v2 observed lock when the
output does not exist. When the output exists, it preserves named entries
other than the requested name.

- Without `--merge`, an existing entry with the requested name is an error;
  this prevents accidental replacement of a learned shape or declared entry.
- With `--merge`, the requested entry must already be observed. APIWatch
  merges the incoming shape into it and writes the widened result.
- A requested name that identifies a declared entry cannot be recorded or
  merged. Users must choose a distinct observed-entry name.

The existing Verify positional argument is renamed internally from `openapi`
to `input`, while its declared-OpenAPI CLI syntax remains unchanged:

```text
apiwatch verify <INPUT> --name <NAME> --lock <PATH> [--format text|json|sarif]
```

After the lock entry is selected, Verify dispatches by provenance:

- `declared` retains the local-or-HTTP(S) OpenAPI loading, operation-drift
  comparison, output, and exit behavior already shipped.
- `observed` reads a local JSON document and compares its shape. HTTP(S) input
  is rejected with exit code `2`; live observation is a later milestone.

An observed match prints `Verified <name>` and exits `0`. Observed drift emits
all deterministic findings and exits `1`. Invalid JSON, invalid lockfile
data, an unsupported provenance, a missing entry, or a wrong input type emits
only a sanitized stderr error and exits `2`.

## Merge And Verification Semantics

Shape merging is monotonic and deterministic:

- Existing and incoming object properties are unioned. Presence counts retain
  whether each field has appeared in every parent observation.
- Corresponding objects, arrays, and same-kind union branches merge
  recursively.
- Different scalar or container kinds form a union.
- A concrete shape replaces an `unknown` collection position; an incoming
  `unknown` never narrows an existing concrete shape.
- The merge never removes a field, union branch, or accepted item shape.

Observed Verify is directional from the lock to the supplied JSON body:

- A missing required object field is breaking.
- A present field or array item whose shape does not match an accepted locked
  shape is breaking.
- Extra object fields, absent optional fields, scalar-value changes, array
  length changes, and a concrete value at a locked `unknown` position are
  compatible.
- An observed `number | null` accepts either number or null. A locked number
  does not accept an actual null, and reports a type incompatibility.

Diagnostics use JSONPath-like paths beginning at `$` and type names only. For
example:

```text
BREAKING $.summary.current_value: required field missing
BREAKING $.live_price: expected number | null, found string
```

They must never contain an input value, even when the JSON field name or
surrounding content is sensitive.

## Output Contracts

Declared Verify output remains byte-compatible in text, JSON, and SARIF.

Observed Verify keeps the same exit-code contract but has its own result
shapes:

- Text uses one `BREAKING <path>: <message>` line per finding.
- JSON is a newline-terminated version-2 envelope with `command: "verify"`,
  the verified `name`, `provenance: "observed"`, a `breaking` summary count,
  and ordered changes. Each change contains a stable kind, path, and only the
  expected and actual type names needed for type incompatibilities.
- SARIF is a valid 2.1.0 document tied to the local lockfile artifact. It has
  observed-specific rules for missing required fields and incompatible shapes;
  fingerprints contain the name, rule, path, and type names only.

The existing GitHub Action remains operationally unchanged for declared
entries. Its normal Verify invocation also works for an observed local JSON
input because command dispatch happens in the CLI; documentation will clarify
the accepted input is selected by lock provenance.

## Architecture

- `src/observed/` owns JSON loading, shape inference, merge, compatibility
  comparison, and observed change types. It has no OpenAPI dependency.
- `src/lockfile/` owns v1/v2 parsing, deterministic v2 rendering, provenance
  selection, and conversion of v1 declared entries into the shared internal
  representation.
- `src/cli.rs` adds `Record` and gives Verify a provenance-neutral input name.
- `src/main.rs` routes Record and selects the declared or observed Verify
  path after target selection.
- `src/output/` adds serializers for observed change types without changing
  the declared renderers.

## Testing And Documentation

Tests are written before implementation and include:

- Unit coverage for inference, stable union ordering, optional-field counts,
  empty collections, monotonic merge, and directional compatibility.
- CLI fixtures covering record creation, v1-to-v2 migration, no-overwrite
  behavior, successful merge, declared-entry rejection, observed Verify pass,
  missing-field drift, type drift, JSON output, SARIF output, and exit codes.
- A byte-level privacy test proving that fixture secrets and all other source
  values do not appear in generated locks or observed Verify diagnostics.
- Repeated recording of unchanged input produces byte-identical lockfiles.
- Existing declared OpenAPI lock, Verify, JSON, SARIF, action-smoke, and
  remote-Verify tests remain green.

README, `docs/lockfile-spec.md`, `CHANGELOG.md`, and action input
documentation describe the new commands, v2 compatibility, structural
privacy guarantee, and the deferred map/coverage boundary.

The final quality gate is:

```text
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
git diff --check
```
