# Change Rules

`apiwatch` classifies semantic API changes as breaking, warning, or non-breaking.

## Breaking

- Endpoint removed.
- HTTP method removed.
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
- Optional request field added.
- Request field became optional.
- Request field became nullable.
- Request enum value added.
- Optional response field added.
- Response field became non-nullable.
- Response enum value removed.
- Optional request parameter added.

## Philosophy

Rules should be high-confidence and explainable. False positives reduce trust, so uncertain cases should be warnings before they become breaking changes.

Invalid input, unsupported OpenAPI versions, unsupported `$ref` locations, and parse failures are input errors rather than semantic warnings. The CLI exits with code `2` for those cases.
