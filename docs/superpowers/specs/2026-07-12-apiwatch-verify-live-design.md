# APIWatch Verify Live Design

## Goal

Extend `apiwatch verify` so one named v1 `api.lock` entry can be checked against an OpenAPI document served over HTTP or HTTPS, while preserving the existing local-file behavior and exit contract.

## Command

```text
apiwatch verify <OPENAPI_OR_URL> --name <NAME> --lock <PATH>
```

- `<OPENAPI_OR_URL>` is either a local YAML/JSON file path or an `http://` or `https://` URL.
- `--name <NAME>` selects one non-empty v1 lockfile entry.
- `--lock <PATH>` remains a local v1 `api.lock` YAML file.

Values with an `http` or `https` scheme are fetched remotely. Every other value is handled as a local file path, preserving the current command behavior. Other URL schemes are invalid input.

## Architecture

Keep lockfile selection, operation comparison, rendered output, and exit handling unchanged. Add a narrow remote-source module that:

1. Parses and validates the URL scheme.
2. Fetches the document with a blocking Rustls-backed HTTP client.
3. Uses a ten-second request timeout, permits at most five redirects, and rejects a response body larger than 10 MiB.
4. Requires a successful HTTP status and returns only document text plus its inferred format.

Refactor the OpenAPI loader so local files and fetched text share one parse-and-normalize path. JSON is selected for a JSON response content type or a final URL path ending in `.json`; YAML is the fallback. Existing raw path-key validation and OpenAPI normalization apply identically to both sources.

## Behavior And Errors

Successful matches keep printing `Verified <NAME>` and exit `0`. Deterministic operation drift remains unchanged and exits `1`.

The following are input errors and exit `2` with no successful verification output:

- malformed or unsupported URL schemes;
- connection, timeout, redirect, or response-body-size failures;
- non-success HTTP statuses;
- malformed or unsupported remote OpenAPI documents; and
- existing lockfile, name, and local-file errors.

Errors are concise and must not print remote response bodies or raw control characters. The command sends no authentication, custom headers, or user configuration.

## Testing

Use a local `TcpListener` integration-test server to prove:

- HTTP-fetched matching operations exit `0` with the standard success line;
- fetched drift exits `1` with existing deterministic output;
- a non-success status exits `2` with empty stdout;
- malformed and unsupported URLs exit `2` with empty stdout; and
- JSON response handling reaches the same parser/validation pipeline.

Retain existing local Verify coverage. Run formatting, Clippy with denied warnings, the complete test suite, and direct local-server CLI smoke checks.

## Scope

This slice supports HTTP and HTTPS URLs only. It excludes auth, custom headers, config files, lockfile mutation, schema/auth comparison beyond the v1 operation set, multiple lock entries, caching, GitHub Actions integration, and remote discovery.
