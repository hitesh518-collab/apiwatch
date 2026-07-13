# APIWatch SARIF Output Design

## Goal

Add deterministic SARIF 2.1.0 output for both `apiwatch diff` and `apiwatch verify`, then allow the reusable GitHub Action to upload Verify findings to GitHub Code Scanning without changing Verify's exit behavior.

## Scope

This slice extends the existing `--format text|json` interface to `--format text|json|sarif` on `diff` and `verify`. Text remains the default and `lock` remains text-only.

The slice includes:

- Versioned SARIF 2.1.0 output for Diff and Verify findings.
- Stable rule IDs, result levels, repository-file locations, and partial fingerprints.
- An opt-in `sarif-file` GitHub Action input that uploads Verify SARIF.
- CLI coverage, action documentation, and full release validation.

It excludes SARIF support for `lock`, JSON/SARIF error envelopes, arbitrary action outputs, custom HTTP behavior, and a workflow that uploads this repository's fixture findings during CI.

## CLI Interface

```text
apiwatch diff <OLD> <NEW> [--format text|json|sarif]
apiwatch verify <OPENAPI_OR_URL> --name <NAME> --lock <PATH> [--format text|json|sarif]
```

`text` remains the default. Invalid format values are rejected by Clap with exit code `2` before OpenAPI or lockfile processing begins.

## SARIF Document Contract

`--format sarif` writes exactly one compact JSON document followed by a newline. The top-level document is SARIF `2.1.0` and contains:

```json
{
  "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
  "version": "2.1.0",
  "runs": [
    {
      "tool": {
        "driver": {
          "name": "apiwatch",
          "semanticVersion": "0.1.0",
          "rules": []
        }
      },
      "results": []
    }
  ]
}
```

- `semanticVersion` comes from `env!("CARGO_PKG_VERSION")`.
- The `runs` array has exactly one run.
- The tool driver includes all five APIWatch rule descriptors in the fixed order below, even when the result list is empty.
- Results preserve the existing Diff and Verify ordering.
- Operational and validation errors remain human-readable stderr output with exit code `2`; they emit no partial SARIF document.

### Stable Rules and Levels

| Rule ID | Applies To | Result Level | Rule Meaning |
| --- | --- | --- | --- |
| `apiwatch/diff-breaking` | Diff breaking changes | `error` | A contract change is classified as breaking. |
| `apiwatch/diff-warning` | Diff warnings | `warning` | A contract change needs review but is not classified as breaking. |
| `apiwatch/diff-non-breaking` | Diff non-breaking changes | `note` | A contract change is classified as non-breaking. |
| `apiwatch/verify-removed` | Verify removed operations | `error` | A locked operation is missing from the current contract. |
| `apiwatch/verify-added` | Verify added operations | `warning` | The current contract exposes an operation absent from the lock entry. |

Every rule includes a stable `id`, short `name`, `shortDescription.text`, `help.text`, `defaultConfiguration.level`, `properties.precision: "high"`, and `properties.problem.severity` matching the result level (`error`, `warning`, or `recommendation` for notes).

### Diff Results

Each Diff change becomes one SARIF result with:

- `ruleId` selected from its existing `Severity`.
- `level` selected from its existing `Severity` (`error`, `warning`, or `note`).
- `message.text` equal to the current `Change.message`.
- One `locations` entry whose `physicalLocation.artifactLocation.uri` is the `new` OpenAPI input path.
- `partialFingerprints.apiwatch/v1` equal to `diff:<rule-id>:<method>:<path>:<message>`.

The Diff command accepts local OpenAPI paths, so the supplied `new` path is always available as an artifact location. A change that removed an operation still points to the new contract file at file level; APIWatch does not have source-line positions.

### Verify Results

Each Verify change becomes one SARIF result with:

- `ruleId` `apiwatch/verify-removed` or `apiwatch/verify-added` selected from its existing `VerifyChangeKind`.
- `level` `error` for removed and `warning` for added.
- `message.text` in the form `locked operation removed: METHOD /path` or `unlocked operation added: METHOD /path`.
- One `locations` entry whose `physicalLocation.artifactLocation.uri` is the `--lock` path.
- `partialFingerprints.apiwatch/v1` equal to `verify:<name>:<rule-id>:<method>:<path>`.

Verify uses the local lockfile as the repository artifact even when its OpenAPI input is remote. This gives GitHub Code Scanning a stable local location and keeps drift alerts tied to the dependency contract the repository owns.

### Exit Behavior

- Diff with one or more breaking changes emits SARIF and exits `1`.
- Diff with warning-only, non-breaking-only, or no changes emits SARIF and exits `0`.
- Verify drift emits SARIF and exits `1`.
- Verify match emits an empty-results SARIF document and exits `0`.
- Input, parsing, loading, and serialization errors retain stderr and exit `2` without SARIF output.

## Architecture

`OutputFormat` gains a `Sarif` value. `main.rs` keeps loading, comparison, and exit decisions unchanged while passing the relevant artifact path to SARIF rendering: Diff passes `new`; Verify passes `lock` and the selected API name.

`output/mod.rs` owns private Serde serializer structures for a single SARIF document and helper functions that map existing `Severity` and `VerifyChangeKind` values to rule IDs, levels, messages, and fingerprints. Internal contract, diff, and lockfile types do not gain broad serialization derives.

The existing JSON renderers and default text renderers remain unchanged. SARIF renderers are separate functions so each public format has one clear serializer boundary.

## GitHub Action

The root composite action gains one optional input:

```yaml
sarif-file:
  description: Relative SARIF output path within working-directory; enables Code Scanning upload when set.
  required: false
  default: ""
```

When `sarif-file` is empty, the action keeps its exact current behavior: it runs text-mode Verify and returns its direct exit code.

When `sarif-file` is set:

1. The action validates that the path is relative and does not contain a `..` segment.
2. It creates the output file's parent directories under `working-directory`.
3. It runs Verify with `--format sarif`, redirects stdout to `sarif-file`, captures its exit code, and stops immediately for exit `2` without uploading.
4. For exit `0` or `1`, it uploads `${{ inputs.working-directory }}/${{ inputs.sarif-file }}` with `github/codeql-action/upload-sarif@v4` using category `apiwatch-${{ inputs.name }}`.
5. After upload succeeds, it exits with the captured Verify code, preserving `0` for a match and `1` for drift.

The composite action does not expose action outputs. Consumer workflows that set `sarif-file` must grant:

```yaml
permissions:
  contents: read
  security-events: write
```

An upload failure fails the action rather than hiding a missing code-scanning report. Repository CI continues to invoke the action without `sarif-file`, avoiding fixture-derived Code Scanning alerts in this repository.

## Testing

Tests are written first in the existing Diff and Verify CLI integration suites. They parse SARIF stdout as JSON and assert:

- Top-level SARIF schema/version and one run.
- Tool identity/version, stable fixed rule order, result rule IDs, levels, messages, locations, and fingerprints.
- Diff breaking, warning-only, and no-change behavior.
- Verify removed/added drift and matching behavior.
- Deterministic result ordering and unchanged text/JSON behavior.
- Invalid `--format` still exits `2` with no stdout document.

Action metadata is reviewed for the disabled-by-default path, safe `sarif-file` validation, captured Verify exits, upload step conditions, consumer-relative upload path, and final exit restoration. Existing `action-smoke` remains text-mode and must continue to pass.

The full release gate is `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, `git diff --check`, and completed GitHub `rust` plus `action-smoke` jobs.

## Documentation

README documents the `sarif` format for Diff and Verify, SARIF's Code Scanning purpose, and the action workflow example with `security-events: write` plus `sarif-file`. The changelog records SARIF output and opt-in GitHub Code Scanning upload support.
