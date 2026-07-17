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

## Observed JSON Contracts

When an OpenAPI specification is absent or incomplete, record the shape of a
local JSON response, then verify future local JSON responses against it:

```bash
apiwatch record --from-json body.json --name portfolio --output api.lock
apiwatch record --from-json updated.json --name portfolio --output api.lock --merge
apiwatch verify body.json --name portfolio --lock api.lock
```

APIWatch records JSON structure, never captured values. `record` is an
explicit learning command that updates a lock; `verify` only checks it. This
release accepts local JSON files for observed contracts. Map annotations,
coverage reporting, HAR imports, and live recording are deferred.

```bash
apiwatch diff old.openapi.yaml new.openapi.yaml --format json
apiwatch verify openapi.yaml --name users --lock api.lock --format json
```

`apiwatch verify <INPUT> --name <NAME> --lock <PATH>` selects OpenAPI or observed JSON verification from the named lock entry's provenance. Declared OpenAPI entries accept local YAML/JSON files and HTTP/HTTPS URLs; observed entries accept local JSON only. It exits `0` for a match, `1` for drift, and `2` for invalid input.

Remote verification uses a 10-second timeout and a 10 MiB response limit. Authentication, custom headers, and configuration files are not included.

## JSON Output

```bash
apiwatch diff old.openapi.yaml new.openapi.yaml --format json
apiwatch verify openapi.yaml --name users --lock api.lock --format json
apiwatch diff old.openapi.yaml new.openapi.yaml --format sarif
apiwatch verify openapi.yaml --name users --lock api.lock --format sarif
```

`apiwatch diff` and `apiwatch verify` support `--format text|json|sarif`; text is the default. JSON output is a versioned, deterministic result document written to stdout. Diff reports `breaking`, `warning`, and `non_breaking` summary counts with operation messages; Verify reports the named lock entry and `removed`/`added` operation drift. SARIF 2.1.0 output is intended for GitHub Code Scanning and preserves the same exit codes: `0` for a clean result, `1` for detected breaking changes or Verify drift, and `2` for operational or validation errors.

## Homebrew

The repository includes a source-building Homebrew formula for the current v0.6.0 tagged release. Clone this repository, then install the local formula:

```bash
git clone https://github.com/hitesh518-collab/apiwatch.git
cd apiwatch
brew install --build-from-source ./Formula/apiwatch.rb
```

This first formula is not a Homebrew tap, so `brew install apiwatch` is not available. Each apiwatch release updates the formula's pinned source URL and SHA-256 checksum.

## Scoop

The repository includes a Scoop manifest for source-building the current v0.6.0 tagged release on Windows. Clone this repository, then install the local manifest:

```powershell
git clone https://github.com/hitesh518-collab/apiwatch.git
cd apiwatch
scoop install ./Scoop/apiwatch.json
```

Scoop installs the Rust dependency automatically. Rust source builds on Windows also require Microsoft C++ Build Tools and a Windows SDK. This first manifest is not in a Scoop bucket, so `scoop install apiwatch` is not available. Each apiwatch release updates the manifest's pinned source URL and SHA-256 checksum after its tag is published.

## GitHub Action

Use the reusable action from an Ubuntu workflow after checking out the consumer repository:

```yaml
permissions:
  contents: read
  security-events: write

steps:
  - uses: actions/checkout@v4
  - uses: hitesh518-collab/apiwatch@<commit-sha>
    with:
      openapi: https://api.example.com/openapi.yaml
      name: users
      lock: api.lock
      sarif-file: apiwatch.sarif
```

The `openapi` and `name` inputs are required. `lock` defaults to `api.lock`, and `working-directory` defaults to `.`. `sarif-file` is relative to `working-directory`; when set, it enables Code Scanning upload and requires `security-events: write`. A Verify drift report uploads before the action returns exit `1`.

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
