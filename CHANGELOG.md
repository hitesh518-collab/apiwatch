# Changelog

## v1.0.3 - 2026-08-07

### Fixed

- Refreshed compatibility snapshots and lock-size reports after promoting
  DigitalOcean, Intercom, and Figma corpus entries to their current statuses.
- Synchronized the roadmap, compatibility documentation, and Action guidance
  with the shipped v1.0.2 implementation and the planned v2.0.0 observed format.

## [1.0.2] — 2026-08-04

### Fixed
- **D-28 (re-fix):** The v1.0.1 schema memoization cache only activated for
  top-level references (`visiting.len() <= 1`), so it never helped the
  deeply-nested, densely-shared schema graphs it was meant to fix — Stripe's
  spec still hung indefinitely. The cache now applies at every depth, and
  cache hits are additionally charged against a size-aware expansion budget
  (materializing a fully-inlined tree from a shared schema DAG can blow up
  combinatorially even without a true cycle). Stripe now fails fast (~9s)
  with a clear `schema expansion exceeded resolution budget` error instead of
  hanging forever and burning the CI runner.
- **Compat corpus drift:** The v1.0.1 D-33/D-34 validation fixes were correct
  but exposed genuine upstream defects in two specs the corpus manifest
  still labeled `"passing"`: `shopify.json` has parameter names containing
  literal embedded description text (e.g. `"ids\n  required"`), and
  `intercom.yaml` binds two `{job_identifier}` path placeholders to
  parameters declared `in: query` instead of `in: path`. Both now correctly
  fail `diff`/`lock`, and are reclassified `known_failing` in
  `compat/specs.json` with matching `tests/compat.rs` coverage — previously
  this was undetected because `assert_clean_self_diff` never exercised
  `lock`, only `diff`.
- **digitalocean.yaml reclassified `passing`:** D-13 (stripping path
  operations missing `responses`) already fixed this spec; the corpus
  manifest and test were never updated to match, so it was still asserted
  as a `known_failing` case with a now-unreachable error string.
- **Release pipeline:** The `x86_64-apple-darwin` build job was pinned to
  the `macos-13` hosted runner, which has no available capacity for this
  account and queues indefinitely, blocking every tagged release behind one
  job that never starts. Switched to `macos-latest` (already proven to work
  for the `aarch64-apple-darwin` target in the same matrix).

## [1.0.1] — 2026-08-03

### Fixed
- **D-32:** Restored `cargo fmt` and `cargo clippy` cleanliness on `main`.
- **D-25 + D-26:** `--required-threshold` is now persisted in v2 lockfiles and
  round-tripped correctly (was silently discarded). Fixed double-offset timestamp
  calculation that produced year-3996 dates.
- **D-27:** The text output header now says "Drift detected in" instead of
  "Verified" when observed verify finds breaking changes.
- **D-28:** Added schema memoization cache to prevent exponential re-expansion
  of densely-shared schema graphs (caused an indefinite hang on Stripe specs).
- **D-29:** External `$ref` targets containing only components (no top-level
  `openapi:` field) are now accepted as fragment files.
- **D-30:** Bare relative filenames (e.g. `main.yaml` without `./` prefix)
  are now resolved correctly relative to CWD.
- **D-13:** Path operations missing a `responses` field no longer reject an
  otherwise usable specification.
- **D-33:** Parameter names containing control characters are now rejected at
  OpenAPI ingestion time with a clear error message.
- **D-34:** Path template placeholder binding validation now only considers
  path parameters, preventing false mismatches from query parameters with
  matching names.

### Changed
- **D-31:** Compat corpus CI job is now enabled on `push`/`pull_request`
  (previously gated to `workflow_dispatch`). Stripe test has a 30-second
  wall-clock timeout.
- **D-35:** README updated to reflect v1.0.0 capabilities: corrected MSRV
  (1.88), removed stale v0.7.0/v0.6.0 release references, removed false
  "not included" claims about headers/config, documented `--ref-root`,
  `--header`, `--config`, `coverage`, and `--from-url`.
- **D-20-R:** README.md and ROADMAP.md version references updated to v1.0.0.

## [1.0.0] — 2026-08-03

### Added
- Frozen v4 lockfile format with SemVer guarantees
- Legacy lock format feature gate (`legacy-lock-format`)
- SemVer contract enforcement test (`cli_semver.rs`)
- Parser fuzzing with `cargo-fuzz` for OpenAPI, v4 roundtrip, and observed infer
- Performance budget regression gates (diff and lock on compat corpus)
- Expanded compatibility corpus (20 specs: 13 passing, 7 known-failing)
- Deterministic output snapshot hashes for lock and diff
- Migration documentation and version upgrade tests
- Post-release install verification smoke test
- Compat corpus documentation

### Changed
- v2/v3 lock loading gated behind `legacy-lock-format` feature (on by default)
- Public library interface marked as v1 stable

## Stability Guarantees (v1.0.0+)

- The v4 lockfile format (`version: 4` in `api.lock`) is frozen. Future format
  changes require a new version number — never a silent schema change.
- CLI subcommands, flags, exit codes, and JSON/SARIF output schemas are stable
  within a major release. Additions are allowed in minor/patch; removals
  and renames require a major bump.
- Text output is human-readable and not guaranteed stable for parsing.
- v2 and v3 lockfiles remain readable behind the `legacy-lock-format` Cargo
  feature (on by default).

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
