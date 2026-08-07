# HAR to Lock Demo

Record API structure from browser traffic — export a HAR from DevTools and
commit the evidence.

## What this demo proves

- You can lock multiple API endpoints from a single HAR capture
- No OpenAPI document, no code harness, no credentials needed
- `apiwatch coverage` reports which fields are observed and which need more
  samples
- The lock is value-free — no customer data, product names, or prices stored

## Run it

```bash
# 1. Record structure from a HAR file
apiwatch record --from-har traffic.har --output api.lock

# 2. See what was captured
apiwatch coverage --lock api.lock

# 3. Inspect the lockfile
cat api.lock
```

## The fixture

`traffic.har` contains two endpoints from a synthetic checkout API:

- `GET /v1/products` — returns product data (`id`, `name`, `price`,
  `in_stock`)
- `GET /v1/orders` — returns order data (`order_id`, `total`, `status`,
  nested `items`)

## How to create your own HAR

1. Open your browser's DevTools (F12)
2. Go to the Network tab
3. Interact with the third-party API your app depends on
4. Right-click a request → "Save all as HAR with content"
5. Run `apiwatch record --from-har capture.har --output api.lock`
6. Commit `api.lock` to your repository

## Limitations

- HAR evidence represents the traffic you captured. Endpoints or response
  variants you never visited are not recorded.
- Values in the HAR are not stored in the lock. Only structure (types,
  nesting, field names) is preserved.
