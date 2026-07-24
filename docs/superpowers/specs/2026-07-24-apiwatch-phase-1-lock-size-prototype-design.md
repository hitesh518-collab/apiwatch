# APIWatch Phase 1 Lock-Size Prototype Design

**Date:** 2026-07-24

**Status:** Approved design

## Purpose

Phase 1 makes declared `verify` compare complete normalized API contracts
instead of route sets. Before approving the breaking v3 lock representation,
APIWatch will measure realistic Git footprint, determinism, and privacy using
the same normalized `ApiContract` model used by `diff`.

This first Phase 1 slice produces evidence. It does not change `api.lock`,
`lock`, or `verify` behavior.

## Approved Product Policies

The following decisions are inputs to the later v3 format design:

- A declared API entry has a default hard ceiling of exactly 5,242,880 bytes.
- `lock` will accept an explicit positive `--max-lock-bytes <BYTES>` override.
- Exceeding the selected ceiling will exit 2 before writing and will preserve
  any existing output file.
- Endpoint scoping will use repeatable exact selectors:
  `--include-operation "METHOD PATH"`.
- Unknown, malformed, and duplicate operation selectors will fail before
  writing.
- Migration will use `lock --update`: replace the named API, preserve unrelated
  declared and observed entries, and retain explicit route-only warnings for
  legacy declared entries that have not been re-locked.
- Complete v3 declared entries will store a mandatory digest formatted as
  `sha256:<64 lowercase hexadecimal characters>`.
- The digest will cover the deterministic canonical normalized-contract
  representation, not the original OpenAPI source.
- A missing or mismatched v3 digest will make lock loading fail with exit 2.

These policies are documented by the prototype but are not implemented until
the measured v3 representation receives separate approval.

## Architecture

### Shared library boundary

Create `src/lib.rs` and move module ownership from the binary crate to the
library crate. The existing `apiwatch` binary will import the shared modules
from the library. Existing CLI behavior and output bytes must remain unchanged.

The library exposes a narrow, documentation-hidden analysis interface for the
repository tool. This interface is pre-v1 and carries no stability promise.
The production CLI and the prototype must use the same OpenAPI normalizer and
`ApiContract`; the tool must not reimplement parsing or normalization.

### Measurement module

Create `src/lock_size.rs` with four responsibilities:

1. convert one `ApiContract` into deterministic candidate data;
2. encode expanded YAML, canonical JSON, and deduplicated YAML;
3. parse and apply exact operation selectors;
4. return structured measurements and privacy results.

The module must not read files, download specifications, or write reports.
Those orchestration concerns belong to the tool.

### Repository tool

Create a separate Cargo package at `tools/lock-size-report/`. Keeping it outside
the root package's binary targets prevents `cargo install apiwatch` from
installing an internal analysis executable.

Invocation:

```bash
cargo run --manifest-path tools/lock-size-report/Cargo.toml -- \
  --manifest compat/specs.json \
  --compat-dir .compat-cache \
  --max-lock-bytes 5242880 \
  --json-out docs/benchmarks/phase-1-lock-size-report.json \
  --markdown-out docs/benchmarks/phase-1-lock-size-report.md
```

Optional scoping uses repeatable arguments:

```text
--include-operation "GET /users/{id}"
--include-operation "POST /users"
```

The tool is offline-only. `scripts/fetch_compat_specs.py` remains the only
corpus acquisition step.

## Candidate Representations

Every candidate consumes the same complete normalized `ApiContract`. The
prototype representation includes only comparison-relevant fields already
present in that model:

- operation method and path;
- authentication name, kind, and scopes;
- parameter name, location, requiredness, and schema;
- request content types and schemas;
- response status codes, content types, and schemas;
- schema kind, nullable flag, format, enum values, properties, and property
  requiredness.

No candidate includes raw OpenAPI fragments, descriptions, examples, defaults,
extensions, headers, credentials, servers, or source payload values.

### Expanded YAML

Expanded YAML embeds every schema at each use site. Maps are emitted in sorted
order and sequences preserve normalized deterministic order. It maximizes Git
readability but may repeat large schemas.

### Canonical JSON

Canonical JSON represents the same expanded data without insignificant
whitespace. Object keys are ordered deterministically and output ends with one
newline. It provides a compact baseline but is less reviewable than YAML.

### Deduplicated YAML

Deduplicated YAML stores operations separately from a schema table. Each schema
is converted to canonical JSON and identified by the SHA-256 of those bytes.
Use sites reference `sha256:<digest>`.

Schema-table keys are sorted lexicographically. If one digest is produced for
different canonical schema bytes, encoding fails rather than merging them.
Current recursive schemas remain unsupported, so the prototype only interns
acyclic normalized schema trees.

## Corpus and Data Flow

`compat/specs.json` is the sole corpus manifest. The existing cache contains:

- GitHub REST, expected to normalize successfully;
- Asana, expected to normalize successfully;
- Box, expected to normalize successfully;
- Stripe, expected to reproduce the pinned recursive-schema error;
- DigitalOcean, expected to reproduce the pinned strict-metadata error.

For each entry, the tool:

1. reads the cached file under `--compat-dir`;
2. enforces its manifest byte limit;
3. verifies its pinned SHA-256;
4. invokes the existing APIWatch normalizer;
5. verifies success or expected failure against the manifest;
6. applies any exact operation selectors;
7. encodes every candidate;
8. measures uncompressed UTF-8 bytes;
9. compares each size with `--max-lock-bytes`;
10. adds the result to deterministic JSON and Markdown reports.

Expected failures are report rows, not missing data. An unexpected success,
unexpected failure, or changed error message makes the run fail.

## Exact Operation Selection

A selector is one uppercase or lowercase supported HTTP method, one ASCII
space, and one exact normalized OpenAPI path beginning with `/`. Methods are
normalized to uppercase. Paths are compared byte-for-byte with normalized
`OperationKey.path`.

The selector set is deterministic and rejects:

- unsupported methods;
- missing or additional separator whitespace;
- empty or non-slash paths;
- control characters;
- duplicate normalized selectors;
- selectors absent from the contract.

When selectors are supplied, every candidate and operation count uses only
the selected operations. The standard committed corpus report uses the full
contracts. Scoping exists so the tool can demonstrate the smallest valid
subset if no full representation fits.

## Size Policy and Selection Rule

Sizes are uncompressed UTF-8 bytes, because the committed lockfile's actual Git
footprint is the design constraint. The default ceiling is exactly 5,242,880
bytes.

The report recommends the v3 representation using this rule:

1. Recommend expanded YAML only if every successfully normalized full contract
   uses no more than 4,194,304 bytes, leaving at least 20 percent headroom.
2. Otherwise recommend deduplicated YAML if every successfully normalized full
   contract uses no more than 5,242,880 bytes.
3. Recommend canonical JSON only if neither YAML representation fits and JSON
   does.
4. If no candidate fits, recommend exact operation scoping for the oversized
   API and report the smallest measured scoped example.

The recommendation is data, not automatic format approval. Production v3
implementation requires a second design review.

## Privacy Verification

Create `testdata/openapi/privacy_sentinels.yaml` containing comparison-relevant
contract structure plus distinct sentinel strings in:

- examples;
- defaults;
- descriptions;
- vendor extensions;
- credential-like fields and security-scheme descriptions.

The fixture must normalize successfully. Every candidate's rendered bytes are
scanned for every sentinel. Any match fails the tool and tests.

The report also records that candidate encoders consume `ApiContract`, not the
raw OpenAPI document. Property names that happen to be `example`, `default`, or
similar are legitimate contract structure and are not rejected; privacy is
proven with sentinel values and the normalized model boundary, not ambiguous
key-name filtering.

## Reports

Create:

- `docs/benchmarks/phase-1-lock-size-report.json`;
- `docs/benchmarks/phase-1-lock-size-report.md`.

Both reports contain:

- report schema version;
- APIWatch package version;
- configured ceiling;
- each corpus name, source commit embedded in its URL, and pinned SHA-256;
- raw source bytes;
- normalization status;
- operation count for successful specifications;
- byte size and ceiling status for every candidate;
- expected parser errors;
- privacy-sentinel result;
- representation recommendation and rule explanation.

Reports contain no timestamp, absolute path, hostname, username, cache path, or
platform-specific separator. Two runs over identical inputs must be
byte-identical.

Reports are written through sibling temporary files and atomically replace
their destinations only after the complete run succeeds. A failure leaves
existing reports untouched.

## Error and Exit Semantics

- Exit 0: every pin, expectation, encoding, privacy check, and report write
  succeeded. Oversized candidates are valid recorded measurements.
- Exit 1: an expected compatibility result changed, encoding was
  nondeterministic, privacy leaked, digest collision was detected, or a
  committed report differs in check mode.
- Exit 2: invocation or local input is invalid, including missing files,
  hash mismatch, invalid selectors, or invalid output paths.

Diagnostics identify the corpus name and failure class but never include raw
source content or privacy sentinel values.

## Verification

### Library extraction

- Existing CLI integration tests must pass without snapshot or expected-output
  changes.
- Root `cargo test`, formatting, and Clippy remain the quality gate.

### Unit tests

- deterministic ordering for all encoders;
- canonical JSON byte stability;
- stable schema SHA-256 identifiers;
- identical schemas intern once;
- forced digest collision protection;
- exact selector parsing, normalization, duplication, and missing-operation
  errors;
- exact byte accounting;
- representation recommendation boundaries at 4,194,304 and 5,242,880 bytes.

### Integration tests

- a small local OpenAPI fixture produces deterministic reports;
- the privacy fixture emits no sentinel values;
- a failed run preserves existing report files;
- two runs produce byte-identical Markdown and JSON.

### Real corpus and CI

The existing compatibility CI job will:

1. test the Python corpus fetcher;
2. fetch or verify the pinned cache;
3. run the existing five compatibility tests;
4. run the lock-size tool in check mode;
5. fail if regenerated reports differ from committed reports.

The standard root test suite remains offline. Real-corpus tests stay opt-in.

## Scope Boundary

This slice does not:

- write or load version 3 locks;
- change `apiwatch lock`, `verify`, `diff`, or `record` behavior;
- implement the 5 MB production write guard;
- implement production endpoint scoping;
- implement `lock --update`;
- fix recursive schemas or strict irrelevant-metadata parsing;
- repair Phase 2 comparison semantics;
- publish v0.8.0.

## Exit Criterion

The slice is complete when:

- the permanent tool and tests pass;
- the pinned report is deterministic and enforced in CI;
- successful corpus entries have measured candidate sizes;
- known corpus failures are reproduced exactly;
- the privacy fixture passes every candidate;
- the report applies the approved selection rule and recommends one concrete
  v3 representation without an unresolved size or privacy question.

The next step is a separate v3 schema design using the committed measurements.
