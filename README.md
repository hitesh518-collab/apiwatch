# apiwatch

API lockfiles for external services.

`apiwatch` is a CLI-first open-source tool for locking, diffing, and verifying the APIs your code depends on.

The mental model:

```text
package-lock.json : packages
api.lock          : external APIs
```

## Status

`apiwatch` is in early development. The first milestone is semantic diffing for local OpenAPI 3.x files, starting with endpoint, authentication, parameter, status-code, request-schema, and response-schema changes.

## CLI

```bash
apiwatch diff old.openapi.yaml new.openapi.yaml
```

Planned future commands:

```bash
apiwatch lock --config apiwatch.yaml
apiwatch verify
```

## MVP Scope

- Parse local OpenAPI 3.x YAML and JSON files.
- Normalize API operations into an internal contract model.
- Detect high-confidence endpoint, authentication, parameter, status-code, request-schema, and response-schema changes.
- Print CI-friendly output.

## Non-Goals For The MVP

- Dashboard
- User accounts
- Cloud backend
- Static code scanning
- Runtime monitoring
- AI features

## License

Apache-2.0
