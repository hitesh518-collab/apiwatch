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
apiwatch verify https://api.example.com/openapi.yaml --name users --lock api.lock
```

```bash
apiwatch diff old.openapi.yaml new.openapi.yaml --format json
apiwatch verify openapi.yaml --name users --lock api.lock --format json
```

`apiwatch verify <OPENAPI_OR_URL> --name <NAME> --lock <PATH>` compares uppercase HTTP method and normalized path pairs in one OpenAPI contract with a named `api.lock` entry. It accepts local YAML or JSON files and HTTP/HTTPS URLs. It exits `0` for a match, `1` for drift, and `2` for invalid local or remote input.

Remote verification uses a 10-second timeout and a 10 MiB response limit. Authentication, custom headers, and configuration files are not included.

## JSON Output

`apiwatch diff` and `apiwatch verify` support `--format text|json`; text is the default. JSON output is a versioned, deterministic result document written to stdout. Diff reports `breaking`, `warning`, and `non_breaking` summary counts with operation messages; Verify reports the named lock entry and `removed`/`added` operation drift. Exit codes remain `0` for a clean result, `1` for detected breaking changes or Verify drift, and `2` for operational or validation errors.

## GitHub Action

Use the reusable action from an Ubuntu workflow after checking out the consumer repository:

```yaml
steps:
  - uses: actions/checkout@v4
  - uses: hitesh518-collab/apiwatch@<commit-sha>
    with:
      openapi: https://api.example.com/openapi.yaml
      name: users
      lock: api.lock
```

The `openapi` and `name` inputs are required. `lock` defaults to `api.lock`, and `working-directory` defaults to `.`.

Pin the action to a commit SHA or release tag. The action builds `apiwatch` from source with Cargo, propagates Verify's `0`/`1`/`2` exit codes, and supports the `working-directory` input. It does not provide caching, action outputs, authentication, custom headers, or configuration files.

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
