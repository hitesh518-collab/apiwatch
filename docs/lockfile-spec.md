# api.lock Draft

`api.lock` is planned as a repository-level lockfile for external API contracts.

This format is intentionally unstable during early development.

Possible shape:

```yaml
version: 1
apis:
  github:
    source: openapi
    base_url: https://api.github.com
    endpoints:
      - method: GET
        path: /repos/{owner}/{repo}
        response_schema_hash: sha256:example
```

The lockfile should avoid secrets and sensitive raw payloads. It should store normalized contract metadata and hashes.
