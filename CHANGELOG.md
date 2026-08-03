# Changelog

## v0.10.0 - 2026-08-03

## v0.9.0 - 2026-08-02

### Added

- Content-addressed `api.lock` version 4 entries with the complete Phase 2
  normalized comparison model: request bodies, canonical media types, response
  requiredness, schema formats, `additionalProperties`, effective servers,
  positional path identity, authentication wire identity, semantic composition,
  first-class array items, and directional enums.
- Content-addressed `api.lock` version 3 entries containing complete
  normalized declared contracts with strict integrity validation.
- Atomic Lock creation and `--update`, repeatable `--include-operation`
  scoping, and an enforced 5,242,880-byte default contract ceiling.
- Full declared Verify through the shared `diff_contracts` engine, including
  severity-aligned text, version-2 JSON, and SARIF output.
- Deterministic combined report generation and `--check` validation.
- Reproducible production-v4 payload-size evidence for the commit-pinned
  GitHub, Asana, and Box compatibility corpus.
- Eleven audited comparison defects fixed with regression fixtures (D-01
  through D-11).

### Changed

- `diff` and v4 declared Verify now share the completed Phase 2 comparison
  rules and deterministic findings.
- Versions 1 and 2 remain readable but explicitly report route-only coverage.
  Added endpoints are warnings; removals remain breaking.
- Version 3 declared Verify reports partial Phase 2 coverage and instructs
  users to re-lock from the original OpenAPI source.
- Legacy migration preserves observed entries and refuses partial migration
  when other declared APIs require their original OpenAPI sources.

### Security

- Versions 3 and 4 exclude examples, defaults, credentials, source extensions,
  and captured values; schema and contract digests, reachability, and exact
  payload bytes are revalidated on load.

## v0.7.0 - 2026-07-24

### Added

- Versioned observed JSON contracts with local shape recording, monotonic
  merging, and read-only verification.
- Explicit repeatable `--map-at` annotations for value-free dynamic-key maps.
- Matching observed Verify output in text, versioned JSON, and SARIF 2.1.0.
- A commit-pinned, hash-verified compatibility suite for five public OpenAPI
  specifications.
- A declared and CI-checked minimum supported Rust version of 1.86.

### Changed

- OpenAPI 3.1 documents now fail with an explicit unsupported-version message
  instead of entering the OpenAPI 3.0 parser.
- Documentation now distinguishes route-only declared Verify from full
  semantic verification and lists all audited limitations.

### Security

- Observed locks and diagnostics retain structure only and redact dynamic map
  keys consistently across text, JSON, SARIF, and fingerprints.

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
