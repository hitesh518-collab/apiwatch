# Change Rules

`apiwatch` classifies semantic API changes as breaking, warning, or non-breaking.

## Breaking

- Endpoint removed.
- HTTP method removed.
- Required request field added.
- Response field removed.
- Response field type changed.
- Enum value removed.
- Successful status code removed.
- Content type changed.

## Warning

- Nullable changed.
- Numeric type widened or narrowed.
- Format changed.
- Response field became optional.
- New error status code added.
- Unsupported or ambiguous OpenAPI shape.

## Non-Breaking

- Endpoint added.
- Optional response field added.
- Optional request parameter added.

## Philosophy

Rules should be high-confidence and explainable. False positives reduce trust, so uncertain cases should be warnings before they become breaking changes.
