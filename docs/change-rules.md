# Change Rules

`apiwatch` classifies semantic API changes as breaking, warning, or non-breaking.

## Implementation Status

The catalog below is the implemented semantic contract for `apiwatch diff` and
current v4 declared Verify. Both call the same comparison engine.

- Version 3 declared Verify uses the older normalized contract and reports
  `coverage: partial` with `phase2_relock_required`.
- Version 1 and version 2 declared locks contain routes only and report
  `coverage: routes` with `route_only_lock`.
- Re-lock from the original OpenAPI source to obtain full v4 coverage.

## Approved Phase 2 Matrices

### Request bodies and media types

| Change | Classification |
|---|---|
| Add a required request body | Breaking |
| Add an optional request body | Non-breaking |
| Remove a request body | Breaking |
| Request body optional → required | Breaking |
| Request body required → optional | Non-breaking |
| Remove a canonical request media type | Breaking |
| Add a canonical request media type | Non-breaking |
| Add or remove a canonical response media type for an existing status | Breaking |

Media types are compared after MIME canonicalization. Schemas under media
types present on both sides continue through the schema matrix.

### Field requiredness

| Schema use | Change | Classification |
|---|---|---|
| Request | Optional → required | Breaking |
| Request | Required → optional | Non-breaking |
| Response | Required → optional | Breaking |
| Response | Optional → required | Non-breaking |

### Enum values and composition alternatives

| Schema use | Set change | Classification |
|---|---|---|
| Request | Value or `oneOf`/`anyOf` branch added | Non-breaking |
| Request | Value or `oneOf`/`anyOf` branch removed | Breaking |
| Response | Value or `oneOf`/`anyOf` branch added | Breaking |
| Response | Value or `oneOf`/`anyOf` branch removed | Non-breaking |

String, integer, number, and boolean enum values use the same directional
policy. Duplicate normalized values do not produce duplicate findings.

### `additionalProperties`

The normalized policies are `forbidden`, `any`, `schema`, and `unknown`.
`schema` → `schema` recursively compares the value schemas. If either side is
`unknown`, no policy finding is emitted.

| Policy direction | Request schema | Response schema |
|---|---|---|
| Narrowing: `any` → `schema`/`forbidden`, or `schema` → `forbidden` | Breaking | Non-breaking |
| Widening: the reverse directions | Non-breaking | Breaking |

### Effective servers

| Change | Classification |
|---|---|
| Effective server template removed | Breaking |
| Effective server template added | Non-breaking |

Operation servers override path servers, which override root servers.
Identity retains scheme, authority, port, path structure, query-key identity,
and variable positions while redacting literal query values and rejecting
credentials. Server variables are compared by placeholder position, not
source label.

### Composition

`allOf` is normalized as a semantic intersection. Object constraints,
requiredness, enums, items, and dictionary policies are merged; incompatible
intersections are input errors. `oneOf` and `anyOf` branches are canonical
sets: reordering is unchanged, exact branches match first, and one
unambiguous same-shape replacement is recursively compared. Metadata-only
`allOf` branches are unconstrained identities, including around a composed
schema.

## Breaking

- Endpoint removed.
- HTTP method removed.
- Authentication requirement added.
- Authentication scheme type changed.
- Authentication scope added.
- Required parameter added.
- Parameter removed.
- Parameter type changed.
- Parameter became required.
- Required request field added.
- Request field removed.
- Request field type changed.
- Request field became required.
- Request field became non-nullable.
- Request enum value removed.
- Response field removed.
- Response field type changed.
- Response field became nullable.
- Response enum value added.
- Successful status code removed.
- Request media type removed.
- Response media type added or removed.

## Warning

- Numeric type widened or narrowed.
- Format changed.
- Response field became optional.
- New error status code added.
- Ambiguous supported OpenAPI shape.

## Non-Breaking

- Endpoint added.
- Authentication requirement removed.
- Authentication scope removed.
- Successful status code added.
- Non-success status code removed.
- Optional parameter added.
- Parameter became optional.
- Optional request field added.
- Request field became optional.
- Request field became nullable.
- Request enum value added.
- Optional response field added.
- Response field became non-nullable.
- Response enum value removed.

## Philosophy

Rules should be high-confidence and explainable. False positives reduce trust, so uncertain cases should be warnings before they become breaking changes.

OpenAPI 3.0 and 3.1 are supported declared-contract targets, including nullable
type arrays in 3.1 documents. The compatibility corpus still records separate
unsupported-input limitations such as Swagger 2.0 and selected path-level
references.
Invalid input, unsupported OpenAPI versions, unsupported `$ref` locations,
circular schema/parameter/response/request body/security scheme/path item
references, and parse failures are input errors rather than semantic warnings.
The CLI exits with code `2` for those cases.

Local `#/components/schemas/...` references are resolved for normalized schemas.
Local `#/components/parameters/...` references are resolved for normalized parameters.
Local `#/components/responses/...` references are resolved for normalized responses.
Local `#/components/requestBodies/...` references are resolved for normalized request bodies.
Local `#/components/securitySchemes/...` references are resolved for normalized authentication schemes.
Local `#/paths/...` references are resolved for normalized path items.
Array item schemas are first-class and diffed under the `items` path, for
example `items.name`. `oneOf` and `anyOf` findings use canonical branch paths
such as `oneOf[0]`; `allOf` is intersected before comparison.

See [ROADMAP.md](../ROADMAP.md) for the correctness sequence and phase exit
criteria.
