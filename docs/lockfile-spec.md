# api.lock

`api.lock` is a repository-level lockfile for external API contracts.

The first lockfile version is intentionally small and stores normalized operation metadata from one or more APIs.

## Version 1

```yaml
version: 1
apis:
  users:
    source: openapi
    operations:
      - method: GET
        path: /users
      - method: POST
        path: /users
```

## Fields

- `version`: lockfile format version. The initial format uses `1`.
- `apis`: map of API names to locked API metadata.
- `apis.<name>.source`: source kind used to produce the lock. The initial command writes `openapi`.
- `apis.<name>.operations`: deterministic list of normalized operations.
- `method`: uppercase HTTP method.
- `path`: normalized OpenAPI path template.

## Privacy

The lockfile avoids secrets, sensitive raw payloads, examples, headers, and raw OpenAPI fragments. Future versions may add schema metadata or hashes while keeping sensitive input out of the file.
