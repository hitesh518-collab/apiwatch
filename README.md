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
apiwatch lock openapi.yaml --name users --output api.lock
apiwatch verify openapi.yaml --name users --lock api.lock
```

`apiwatch verify` compares uppercase HTTP method and normalized path pairs in a local OpenAPI file with one named `api.lock` entry. It exits `0` when they match, `1` when operations have drifted, and `2` for invalid input or lockfile data.

## MVP Scope

- Parse local OpenAPI 3.x YAML and JSON files.
- Normalize API operations into an internal contract model.
- Detect high-confidence endpoint, authentication, parameter, status-code, request-schema, and response-schema changes.
- Resolve local component schema, parameter, response, request body, security scheme, and path item references used by normalized contracts.
- Diff composed schemas using `oneOf`, `allOf`, and `anyOf` branch paths.
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

## Changelog

See [CHANGELOG.md](CHANGELOG.md).
