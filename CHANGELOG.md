# Changelog

## Unreleased

### Added

- Versioned observed JSON contracts with local shape recording, monotonic merging, and verification.
- Add explicit repeatable `--map-at` annotations for dynamic-key observed JSON maps.
- A repository-owned Homebrew formula for source-building the tagged apiwatch release.
- A repository-owned Scoop manifest for source-building the tagged apiwatch release on Windows.

## v0.6.0 - 2026-07-16

### Added

- SARIF 2.1.0 output for `apiwatch diff` and `apiwatch verify`, plus opt-in GitHub Code Scanning upload from the reusable action.
- Deterministic, versioned JSON output for `apiwatch diff` and `apiwatch verify` via `--format json`.
- `apiwatch lock <OPENAPI> --name <NAME> --output <PATH>` writes a deterministic v1 `api.lock` file with normalized operation metadata.
- `apiwatch verify <OPENAPI> --name <NAME> --lock <PATH>` compares a local OpenAPI contract to one named v1 `api.lock` entry and exits `1` for deterministic operation drift.
- `apiwatch verify <OPENAPI_OR_URL> --name <NAME> --lock <PATH>` supports HTTP/HTTPS OpenAPI URLs for live verification; remote fetch failures exit `2`.
- Invalid `verify` input and lockfile data errors exit `2`.
- Reusable `apiwatch verify` composite GitHub Action that builds from source and propagates Verify exit codes.

## v0.1.0

Initial semantic OpenAPI diff milestone.

### Added

- `apiwatch diff <OLD> <NEW>` for local OpenAPI 3.x YAML and JSON files.
- Endpoint, authentication, parameter, status-code, request-schema, and response-schema diffing.
- Breaking, warning, and non-breaking change classification with deterministic CLI output.
- Local `$ref` resolution for schemas, parameters, responses, request bodies, security schemes, and path items.
- Recursive schema diffing for nested objects, arrays, and `oneOf`/`allOf`/`anyOf` branches.
- Input-error handling for unsupported OpenAPI versions, malformed YAML/JSON, unsupported references, and circular references.

### Verification

- Rust formatting, Clippy with warnings denied, and the full test suite are part of release verification.
