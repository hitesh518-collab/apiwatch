# Declared OpenAPI Drift Demo

Lock a provider-style OpenAPI document and catch semantic changes.

## What this demo proves

- You can lock a normalized OpenAPI contract into a deterministic lockfile
- `apiwatch diff` reports semantic changes between two specs
- `apiwatch verify` checks a newer spec against the recorded contract
- Breaking changes include field removal, type changes, and required-field
  additions

## Run it

```bash
# 1. Lock the baseline spec
apiwatch lock baseline.openapi.yaml --name widgets --output api.lock

# 2. Verify the baseline — passes
apiwatch verify baseline.openapi.yaml --name widgets --lock api.lock
# Verified widgets — exit 0

# 3. Diff against a changed spec
apiwatch diff baseline.openapi.yaml changed.openapi.yaml
# Breaking: items.description removed
# Breaking: items.price type changed from number to string
# — exit 1

# 4. Or verify the changed spec against the lock
apiwatch verify changed.openapi.yaml --name widgets --lock api.lock
# — exit 1
```

## The breaking changes

`baseline.openapi.yaml` defines a Widget schema with `id`, `name`, `price`
(number), and `description`.

`changed.openapi.yaml`:
- changes `price` from `number` to `string`
- removes `description`
- adds `category` as a new optional field

APIWatch classifies the price type change and field removal as breaking.
The new optional field is non-breaking — a consumer can safely ignore it.

## Lockfile contents

`api.lock` is a version 4 declared lock containing the normalized contract.
It stores schema kinds, requiredness, types, and structural metadata. It
excludes examples, defaults, descriptions, and raw OpenAPI fragments.

See [docs/change-rules.md](../../docs/change-rules.md) for the complete
semantic rule catalog governing declared-contract comparison.
