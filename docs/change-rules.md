# Change Rules

`apiwatch` classifies semantic API changes as breaking, warning, or non-breaking.

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

Invalid input, unsupported OpenAPI versions, unsupported `$ref` locations, circular schema/parameter/response/request body/security scheme/path item references, and parse failures are input errors rather than semantic warnings. The CLI exits with code `2` for those cases.

Local `#/components/schemas/...` references are resolved for normalized schemas.
Local `#/components/parameters/...` references are resolved for normalized parameters.
Local `#/components/responses/...` references are resolved for normalized responses.
Local `#/components/requestBodies/...` references are resolved for normalized request bodies.
Local `#/components/securitySchemes/...` references are resolved for normalized authentication schemes.
Local `#/paths/...` references are resolved for normalized path items.
Composed schemas using `oneOf`, `allOf`, and `anyOf` are diffed by branch index paths such as `oneOf[0]`.
