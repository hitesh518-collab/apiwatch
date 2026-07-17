# APIWatch Observed Map Annotations Design

## Goal

Add explicit dynamic-key map annotations to the local JSON observed-contract
workflow. Teams can mark a JSON object as a map when its keys are data rather
than contract fields, allowing key churn while still verifying every map value
against a deterministic structural shape.

## Scope

- Repeatable `--map-at <JSONPATH>` on `apiwatch record --from-json`.
- A value-free `map` observed shape node with deterministic merge and Verify
  behavior.
- A strict JSONPath subset: root `$` and named property segments such as
  `$.by_broker` and `$.state.by_region`.
- Fixtures, unit tests, CLI tests, documentation, and release validation.

## Deferred Scope

- Automatic map inference or suggestions.
- Bracket notation, arrays, wildcards, recursive descent, filters, scripts,
  and all other JSONPath expressions.
- Declared/observed/unverified coverage reporting and `--fail-on-unverified`.
- HAR and live recording, consumer projections, enum inference, runtime
  monitoring, source discovery, dashboards, cloud services, accounts, and AI.

Automatic inference is deliberately excluded: a map changes compatibility
meaning and must never be inferred silently.

## CLI Contract

`Record` gains a repeatable option:

```text
apiwatch record --from-json <BODY> --name <NAME> --output <PATH> \
  [--merge] [--map-at <JSONPATH>]...
```

For example:

```text
apiwatch record --from-json portfolio.json --name portfolio --output api.lock \
  --map-at $.by_broker --map-at $.state.by_region
```

Annotations apply to the incoming shape before recording. When `--merge` is
used, the same annotations also transform the existing observed shape before
the two shapes merge. Therefore an existing ordinary object can become a map
only through an explicit Record command, and later merges into a recorded map
do not need the annotation repeated.

The parser accepts only `$` followed by zero or more `.` plus property-name
segments. A property name starts with an ASCII letter or underscore and then
contains ASCII letters, digits, or underscores. Duplicate paths, missing
paths, non-object targets, empty segments, bracket notation, wildcards,
filters, scripts, and malformed paths are input errors with exit code `2`.

## Shape And Lockfile Model

Observed `Shape` gains a v2-serializable map variant:

```yaml
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

An annotated non-empty object creates `Map { values }` by merging every object
property shape into one value shape. An empty object creates
`Map { values: Unknown }`. The node stores no keys, input values, examples,
tokens, headers, or body fragments.

This is a backward-compatible v2 addition. Existing v1 declared locks and v2
observed locks without a `map` node continue to parse and behave exactly as
before. No lockfile version bump is required.

## Merge And Verification Semantics

Merging is monotonic:

- `Map` with `Map` merges their value shapes.
- A recorded `Map` with an incoming ordinary object merges every incoming
  object-property shape into the map value shape; incoming empty objects do
  not narrow it.
- An ordinary object becomes a map only through `--map-at`.
- A map combined with a non-object shape forms an existing deterministic union
  rather than discarding either accepted shape.

Observed Verify is directional from the lock to the supplied local JSON:

- A locked map accepts an actual object regardless of which keys are present
  or absent, including an empty object.
- Every actual object-property shape must be compatible with the locked map
  value shape.
- An actual scalar, array, or null where a map is locked is breaking.
- Existing object field, array item, union, unknown, and privacy behavior
  stays unchanged.

Map diagnostics use the annotated JSONPath plus the stable redacted
`<map-value>` segment and type names only. For example:

```text
BREAKING $.by_broker.<map-value>: expected number, found string
```

## Architecture

- `src/observed/mod.rs` owns `Shape::Map`, strict path parsing, annotation,
  recursive map conversion, map merge, and map compatibility comparison.
- `src/cli.rs` adds the repeatable `map_at: Vec<String>` Record argument.
- `src/main.rs` forwards the annotations with the incoming shape.
- `src/lockfile/mod.rs` applies annotations to a new observed entry or an
  existing observed entry before its normal recording/merge behavior.
- `src/output/mod.rs` needs no format-specific changes because observed change
  paths and shape names flow through its existing text, JSON, and SARIF
  serializers.

## Testing And Documentation

Tests are written before implementation and cover:

- Parsing valid root and property paths plus rejecting every unsupported form.
- Annotating non-empty and empty objects; duplicate, missing, and non-object
  targets return sanitized errors.
- Merge of later ordinary objects into recorded maps.
- Verify pass for changed or removed keys; failures for a dynamic value type
  drift and map-to-scalar drift.
- Deterministic v2 rendering and confirmation that fixture secrets and source
  values are absent from lockfiles and diagnostics.
- All existing declared and observed CLI JSON/SARIF tests remain green.

README and the lockfile specification gain explicit map examples and state
that auto-inference and coverage reporting remain deferred. The Unreleased
changelog records map annotations.

The release gate remains:

```text
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
git diff --check
```
