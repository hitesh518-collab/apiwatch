# Phase 5 — HAR Import: Design

**Target:** v0.12.0
**Date:** 2026-08-03
**Status:** approved

## Goal

Let users adopt observed contracts from real traffic by importing HAR capture
files. This is the highest-priority Phase 5 feature: a user captures network
traffic in their browser's DevTools or a proxy, exports a HAR file, and runs a
single command to produce a value-free lockfile.

## Scope

- HAR file import as a new source for `apiwatch record`
- Entry grouping by user-provided path identity patterns
- Auto-filtering: JSON-only, configurable status codes, explicit skip reporting
- Honest exclusion of binary, base64-encoded, empty, and malformed response bodies
- Reuse all existing shape inference, merging, threshold, map-at, and tiered
  reporting infrastructure

### Excluded

- Live URL recording (separate Phase 5 item)
- Multi-entry Verify (separate Phase 5 item)
- `apiwatch init`, coverage commands, or onboarding examples (later Phase 5 items)
- HAR request body recording (Phase 5 is response-shape focused per ROADMAP)
- HAR cookie, header, or timing extraction (out of scope)

## Architecture

### New Module: `src/har.rs`

A dedicated module with no dependency on `observed`, `lockfile`, or `output`.
It parses HAR JSON, filters entries, and produces grouped recording data that
`main.rs` feeds into the existing shape pipeline.

```
  capture.har
      |
  src/har.rs (parsing, filtering, grouping)
      |
  Vec<HarRecording> per group
      |
  main.rs -> observed::infer() -> observed::merge() -> lockfile::record_observed()
```

### Core Types

```rust
#[derive(Debug, Deserialize)]
struct Har {
    log: HarLog,
}

#[derive(Debug, Deserialize)]
struct HarLog {
    entries: Vec<HarEntry>,
}

#[derive(Debug, Deserialize)]
struct HarEntry {
    request: HarRequest,
    response: HarResponse,
}

#[derive(Debug, Deserialize)]
struct HarRequest {
    method: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct HarResponse {
    status: u16,
    content: HarContent,
}

#[derive(Debug, Deserialize)]
struct HarContent {
    #[serde(default)]
    #[serde(rename = "mimeType")]
    mime_type: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    encoding: Option<String>,
}

/// A successfully filtered and parsed recording ready for shape inference.
#[derive(Debug)]
pub(crate) struct HarRecording {
    pub method: String,
    pub path: String,
    pub body: serde_json::Value,
}

/// Reason a HAR entry was skipped.
#[derive(Debug)]
pub(crate) enum HarSkipReason {
    NonJsonContentType(String),
    NonMatchingStatus { status: u16, path: String },
    EmptyBody,
    JsonParseError(String),
    Base64Encoded,
}

/// Grouped recordings keyed by method+path identity string.
pub(crate) type HarRecordings = BTreeMap<String, Vec<HarRecording>>;
```

### Public API

```rust
/// Load a HAR file from disk and return grouped recordings.
///
/// `path_identities` are METHOD + path prefix patterns (e.g. "GET /api/users").
/// When non-empty, entries are grouped under matching identities rather than
/// their raw request paths.
///
/// `status_filter`: when non-empty, only these HTTP status codes are recorded.
/// When empty, all 2xx responses are recorded.
///
/// Returns (recordings, skips) where recordings are grouped recordings and
/// skips are entries that were filtered out with reasons.
pub(crate) fn load_har(
    path: &Path,
    path_identities: &[String],
    status_filter: &[u16],
) -> Result<(HarRecordings, Vec<(String, HarSkipReason)>)>
```

### Parsing and Validation

1. Read the file, deserialize as `Har` via serde_json
2. Reject if `log.entries` is absent or empty
3. For each entry, validate `request.method` is present, `request.url` is parseable by the `url` crate
4. Reject duplicate `--path-identity` values at parse time (clap-level or early validation)

### Path Identity Matching

When the user provides `--path-identity "GET /api/users"`:

1. Split on the first space: `method = "GET"`, `path_prefix = "/api/users"`
2. Method matching is case-insensitive; stored uppercased
3. Path matching is prefix-based: an entry's URL path must start with `path_prefix`
4. If an identity has no matching entries, it is a hard error
5. An entry can match at most one identity (first match wins)
6. Entries that match no identity when identities are provided are skipped

When `--path-identity` is absent, each unique `METHOD /raw-path` becomes its own key.
The raw path is extracted from the URL via the `url` crate's `.path()` method.

### Response Filtering Pipeline

For each HAR entry, apply in order:

1. **Status filter**: If `status_filter` is non-empty, skip entries where
   `response.status` is not in the filter. If empty, skip non-2xx responses.
2. **Encoding check**: If `content.encoding == Some("base64")`, skip with
   `Base64Encoded` reason.
3. **Content-type check**: `mime_type` must contain `application/json`
   (case-insensitive prefix match on the media type, ignoring charset parameters).
   Skip with `NonJsonContentType` reason if not.
4. **Body presence**: `text` must be non-empty. Skip with `EmptyBody` if empty.
5. **JSON parse**: Parse `text` via `serde_json::from_str`. Skip with
   `JsonParseError` if parse fails.

### Entry Grouping

Within each group (keyed by effective path identity):

1. The first `HarRecording` is `infer()`'ed into a `Shape`
2. Each subsequent recording in the group is `merge()`'ed into it
3. The result is passed to `lockfile::record_observed()` under the group key
4. When `--merge` is present, the group merges into the existing lock entry
   (using the existing `--merge` behavior in `record_observed`)

## CLI Surface

### New Flags on `Command::Record`

```
apiwatch record
  --from-har <PATH>           HAR file to import (mutually exclusive with --from-json)
  --path-identity <METHOD /path>  Repeatable. Group entries under this key
  --status <CODE>             Repeatable. Only record these HTTP status codes
```

### `--name` Interaction

`--name` overrides path-based keying: all matching entries go under one named
entry. Useful for simple single-endpoint HAR files. When both `--name` and
`--path-identity` are present, `--name` wins.

### Mutual Exclusivity

`--from-har` and `--from-json` are in a clap argument group. Only one may be
present. `--name` is required with `--from-json` but optional with `--from-har`.

### Defaults

| Flag | Default | Behavior |
|------|---------|----------|
| `--status` (absent) | 2xx only | All 200-299 responses are recorded; 4xx/5xx skipped |
| `--path-identity` (absent) | Raw method+path | Each unique path becomes its own entry |
| `--required-threshold` | 0.5 | Same as existing `record` |

## Output

### Text Reporting

```
Recorded 3 endpoints:
  GET /api/users: 5 samples merged
  GET /api/orders: 4 samples merged
  GET /api/products: 3 samples merged

Skipped 8 responses:
  - GET /api/health (status 503): non-matching status
  - POST /api/upload (image/png): non-JSON content type
  - GET /api/binary (status 200): base64 encoded
  - GET /api/empty (status 200): empty body
  - GET /api/broken (status 200): JSON parse error
  ...
```

Only skipped entries are shown if there are skips. A clean run prints only the
recorded summary and the usual "Wrote <output>" line.

### JSON Output

The existing `record` command does not produce JSON output (that is a verify
feature). No JSON output change for record.

### SARIF Output

The existing `record` command does not produce SARIF output. No SARIF output
change for record.

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success (at least one entry recorded) |
| 1 | N/A (record has no "breaking" concept) |
| 2 | Input error: HAR not found, invalid JSON, no matching entries, duplicate identity |

## Backward Compatibility

- `--from-json` path is unchanged
- All existing `record` tests must remain green
- Observed lock entries produced by HAR import are identical in structure to
  those produced by repeated `--from-json --merge` calls
- Lockfile version behavior unchanged (min v2 for observed entries)

## Constraints

- No new dependencies: HAR is parsed with `serde_json` (already present)
- MSRV 1.88
- All existing tests must pass
- No lockfile version bump
- Deterministic output: same HAR + same flags = byte-identical lock
- No scalar values, credentials, or dynamic map keys in serialized output

## Files Touched

| File | Change |
|------|--------|
| `src/har.rs` | New module: HAR types, parsing, filtering, grouping |
| `src/main.rs` | Wire `--from-har` path, call `har::load_har`, iterate groups |
| `src/cli.rs` | Add `--from-har`, `--path-identity`, `--status` flags to `Command::Record` |
| `src/lib.rs` | Declare `pub(crate) mod har` |
| `tests/cli_record.rs` | Integration tests for HAR import, grouping, filtering, skip reporting |

## Test Plan

### Unit Tests in `src/har.rs`

1. Load valid HAR with single entry → returns one group
2. Load valid HAR with multiple entries → groups by method+path
3. Path identity grouping → multiple requests map to one key
4. Path identity with no matches → error
5. Duplicate path identity → error
6. Status filter → only matching status codes recorded
7. Non-JSON content type → skipped with NonJsonContentType
8. Base64 encoding → skipped with Base64Encoded
9. Empty body → skipped with EmptyBody
10. Invalid JSON body → skipped with JsonParseError
11. Non-2xx skipped when no --status filter → skipped
12. Missing log.entries → error
13. Valid HAR with all entries skipped → error (nothing to record)

### Integration Tests in `tests/cli_record.rs`

1. `record --from-har` with single endpoint → produces valid v2+ lock
2. `record --from-har` with multiple endpoints → multiple observed entries
3. `record --from-har --path-identity` → entries grouped under identity key
4. `record --from-har --merge` → merges into existing lock entry
5. `record --from-har --status 200 201` → only matching statuses recorded
6. `record --from-har` with mixed content types → only JSON recorded, skips reported
7. `record --from-har` file not found → exit 2 with error
8. `record --from-har` with `--name` → single named entry
9. `record --from-har` + `--map-at` → map annotations applied
10. `record --from-har` produces byte-identical lock for same input
11. Existing `record --from-json` tests pass unchanged

## Exit Criterion

A user can export a HAR file from browser DevTools for an undocumented API,
run `apiwatch record --from-har capture.har --output api.lock`, and get a
value-free lockfile with correctly grouped entries, JSON-only response
filtering, and honest skip reporting.
