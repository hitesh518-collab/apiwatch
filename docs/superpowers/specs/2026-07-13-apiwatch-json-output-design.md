# APIWatch JSON Output Design

## Goal

Add deterministic JSON output to both `apiwatch diff` and `apiwatch verify` while preserving their current text output and exit-code behavior. The JSON result becomes the machine-readable contract that a later SARIF feature can consume.

## Scope

This slice adds `--format text|json` to `diff` and `verify` only. `text` remains the default. `lock` remains text-only.

The slice includes:

- A shared CLI output-format value enum.
- Versioned JSON envelopes for Diff and Verify results.
- Deterministic serialization and targeted CLI tests.
- README and changelog documentation.

It excludes SARIF, action-output plumbing, a JSON error envelope, custom output files, and changes to exit codes.

## CLI Interface

```text
apiwatch diff <OLD> <NEW> [--format text|json]
apiwatch verify <OPENAPI_OR_URL> --name <NAME> --lock <PATH> [--format text|json]
```

`--format text` is the default, retaining the present stdout exactly. Clap rejects unsupported format values before command execution with exit code `2`.

## JSON Contract

Every successful Diff or Verify execution with `--format json` writes exactly one JSON object to stdout followed by a newline. Field names, array order, and summary key order are deterministic. Operational and validation failures remain human-readable stderr output with exit code `2`; they do not emit a partial JSON document.

### Diff

```json
{
  "version": 1,
  "command": "diff",
  "summary": {
    "breaking": 1,
    "warning": 0,
    "non_breaking": 0
  },
  "changes": [
    {
      "severity": "breaking",
      "method": "GET",
      "path": "/users",
      "message": "endpoint removed"
    }
  ]
}
```

- `version` is the JSON schema version and starts at integer `1`.
- `command` is the literal string `diff`.
- `summary` always contains `breaking`, `warning`, and `non_breaking` non-negative integer counts, in that order.
- `changes` preserves the existing deterministic `diff_contracts` order.
- Each change contains `severity` (`breaking`, `warning`, or `non_breaking`), uppercase `method`, normalized `path`, and the existing human-readable `message`.
- A no-change result has all summary counts set to `0` and an empty `changes` array; it exits `0`.
- A result containing one or more breaking changes exits `1`, as it does today. Warning-only and non-breaking-only results exit `0`.

### Verify

```json
{
  "version": 1,
  "command": "verify",
  "name": "users",
  "summary": {
    "removed": 1,
    "added": 0
  },
  "changes": [
    {
      "kind": "removed",
      "method": "GET",
      "path": "/users"
    }
  ]
}
```

- `version` is the JSON schema version and starts at integer `1`.
- `command` is the literal string `verify`.
- `name` is the validated named `api.lock` entry being checked.
- `summary` always contains `removed` and `added` non-negative integer counts, in that order.
- `changes` preserves the lockfile comparison order: removed operations first, then added operations; each group remains ordered by HTTP method and normalized path.
- Each change contains `kind` (`removed` or `added`), uppercase `method`, and normalized `path`.
- A matching contract returns an empty `changes` array, zero summary counts, and exit `0`.
- Any drift returns the structured changes and exit `1`.

## Architecture

`cli.rs` defines a single `OutputFormat` value enum used by the Diff and Verify command variants. `main.rs` selects text or JSON rendering after loading and comparison; parsing, contract normalization, comparison, severity classification, and exit-code decisions remain unchanged.

`output/mod.rs` owns JSON result types and serializers alongside the existing text renderers. The JSON types adapt existing `Change` and `VerifyChange` values rather than requiring broad `Serialize` derives across internal contract and diff modules. `serde_json` performs serialization, and serialization failure is propagated as an operational error.

The structure intentionally mirrors the two existing result domains. Diff reports semantic severity and messages; Verify reports lock drift direction. Both use the same versioned envelope conventions so SARIF can later map their normalized operation fields without parsing human text.

## Error Handling

- Existing command errors stay on stderr and return `2` for both formats.
- Invalid `--format` values are rejected by Clap before any OpenAPI or lockfile work.
- JSON serialization failures propagate through the existing top-level error handler, producing stderr output and exit `2`.
- No success result is mixed with an error document.

## Testing

Tests are added first in the existing `tests/cli_diff.rs` and `tests/cli_verify.rs` integration suites. They parse stdout as JSON and assert the public fields, exact summary values, deterministic change ordering, and preserved exit codes for:

- Diff: breaking drift, warning/non-breaking drift, and no changes.
- Verify: drift and a matching contract.
- Default text output remains byte-for-byte unchanged for representative Diff and Verify results.
- Invalid `--format` exits `2` before normal command processing.

The full Rust quality gate remains `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, and `git diff --check`.

## Documentation

README gains a concise JSON-output example for Diff and Verify, documents the schema-versioned result contract, and states that text remains the default. The Unreleased changelog records JSON output for Diff and Verify.
