# Change Rules

`apiwatch` classifies semantic API changes as breaking, warning, or non-breaking.

## Implementation Status

The catalog below is the intended semantic contract, not a claim that every
rule is implemented correctly in the current release.

- `apiwatch diff` implements many rules but has confirmed false-negative and
  false-positive gaps.
- Declared version 1 and version 2 locks contain routes only, so current
  declared Verify compares added and removed operations rather than applying
  this full catalog.
- [Roadmap Phase 1](../ROADMAP.md#phase-1--make-verify-meaningful) will make
  declared Verify use the same comparison engine as `diff`.
- [Roadmap Phase 2](../ROADMAP.md#phase-2--make-the-comparison-engine-trustworthy)
  will resolve the audited comparison gaps with regression fixtures.

Until those exit criteria pass, callers should consult the
[README known limitations](../README.md#known-limitations) before treating a
clean result as proof that every listed change class was checked.

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
- Content type changed.

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

OpenAPI 3.0 is the current declared-contract target. OpenAPI 3.1 support is
planned in [Roadmap Phase 3](../ROADMAP.md#phase-3--real-world-compatibility).
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
Array item schemas are diffed under the synthetic `items` path, for example `items.name`.
Composed schemas using `oneOf`, `allOf`, and `anyOf` are diffed by branch index paths such as `oneOf[0]`.

See [ROADMAP.md](../ROADMAP.md) for the correctness sequence and phase exit
criteria.
