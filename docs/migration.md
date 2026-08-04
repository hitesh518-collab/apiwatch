# APIWatch Lockfile Migration Guide

## Version Quick Reference

| Version | Features | Breakage Risk | Upgrade Action |
|---------|----------|---------------|----------------|
| v1 | Route-only declared entries | None | Automatic: re-serialize writes v2 |
| v2 | Observed entries, per-entry shapes | None | Run `apiwatch lock` to upgrade |
| v3 | Phase 1 payload reduction, partial coverage | None | Re-lock from original OpenAPI source |
| v4 (current) | Full contract payload, observed contracts | None | Already current |
| v4/v5 (planned, v2.0.0) | Content-addressed observed entries | TBD | Re-record from original HAR/JSON/URL source |

## Compatibility Guarantee

The v4 lockfile format (`version: 4` in `api.lock`) is frozen as of APIWatch
v1.0.0. Future format changes will use a new version number — v4 will never be
silently changed.

v2 and v3 lockfiles remain readable via the `legacy-lock-format` Cargo feature
(on by default). APIWatch will always be able to read them; writing always
produces v4.

## Migrating from v2

v2 locks contain route-only declared entries and optional observed shapes.

```
apiwatch lock --openapi path/to/spec.yaml --name my-api --output api.lock --update
```

This loads the existing v2 lock, replaces the named entry with a full v4
declared entry, and writes v4 output. Observed entries are preserved.

## Migrating from v3

v3 locks contain Phase 1 contract payloads with reduced scope. For full
contract coverage:

```
apiwatch lock --openapi path/to/spec.yaml --name my-api --output api.lock --update
```

v3 locks work correctly for diff and verify, but declared verify provides only
partial coverage. Re-locking from the original OpenAPI source enables full
coverage.

## New Project Setup

```
apiwatch init --output api.lock
apiwatch lock --openapi spec.yaml --name my-api --output api.lock
apiwatch record --from-har capture.har --output api.lock
git add api.lock .github/workflows/
git commit -m "add apiwatch contract checking"
```

## Troubleshooting

### "api.lock version N requires the legacy-lock-format feature"

Your build excludes legacy format support. Enable the feature:

```toml
[dependencies]
apiwatch = { version = "1", features = ["legacy-lock-format"] }
```

### "warning: api.lock v3 lacks Phase 2 contract fields"

This warning appears during v3 declared verify. Re-lock from the original
OpenAPI source to upgrade to v4 and enable full coverage.
