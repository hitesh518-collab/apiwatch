# Observed JSON Drift Demo

Catch a breaking change in a third-party JSON API — no OpenAPI document required.

## What this demo proves

- You can lock the structure of an API response without an OpenAPI spec
- The lockfile stores types and paths, never captured values
- `apiwatch verify` exits 0 for a matching response and 1 for a breaking change
- You can update the lock intentionally when the API evolves compatibly

## Run it

```bash
# 1. Record the expected structure from a sample response
apiwatch record --from-json baseline.json --name payments --output api.lock

# 2. Inspect the lock — it contains only structure, no values
cat api.lock

# 3. Verify the same response passes
apiwatch verify baseline.json --name payments --lock api.lock
# Verified payments (observed) — exit 0

# 4. Verify a breaking response fails
apiwatch verify changed.json --name payments --lock api.lock
# BREAKING $.amount: expected number, found string — exit 1
```

## The breaking change

`baseline.json` defines a payment response with `amount: 42.50` (number).
`changed.json` returns `amount: "42.50"` (string).

APIWatch detects the type drift and reports exactly which field changed, from
what to what.

## Privacy

`api.lock` stores `kind: number` — it never captures `42.50`, `pay_123`,
`USD`, or any payment data. Grep the lock for any value from the samples
and you will find only type names.

## Limitations

- One sample is enough for a demo; in production record several responses
  to distinguish required fields from optional ones.
- An observed contract represents sampled structure, not every possible
  response variant.
