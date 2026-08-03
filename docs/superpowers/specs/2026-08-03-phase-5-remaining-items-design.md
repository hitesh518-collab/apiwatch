# Phase 5 — Remaining Items: Design

**Target:** v0.12.0
**Date:** 2026-08-03
**Status:** approved

## Overview

Five remaining Phase 5 features completing the frictionless recording and CI
adoption story. Each is an independent subsystem with its own CLI surface,
designed to compose with the existing lock-and-verify pipeline.

---

## 1. Live URL Recording

### Goal

Record the observed shape of a JSON response fetched from a live URL, using
the same infer/record pipeline as `--from-json` and `--from-har`.

### CLI

```
apiwatch record --from-url https://api.example.com/users --output api.lock
  --method GET|POST|PUT|DELETE|PATCH (default GET)
  --path-identity "GET /api/users" (optional grouping prefix)
  --header "Authorization:${MY_TOKEN}" (optional auth, same as verify)
  --name my-api (optional key override)
  --map-at, --merge, --required-threshold (existing flags)
```

### Behavior

1. Fetch the URL with `reqwest::blocking`. Default method is GET, overridden
   by `--method`. All standard HTTP methods accepted.
2. Reject non-JSON responses as a hard error (user explicitly chose this URL,
   unlike HAR where non-JSON is silently skipped).
3. Parse the JSON body via `serde_json`.
4. Auto-derive entry key: `{METHOD} {path}` from the URL (e.g.
   `GET /users`). `--name` overrides. `--path-identity` groups entries under
   a prefix, same as HAR.
5. Feed to existing pipeline: `observed::infer()` →
   `lockfile::record_observed()` → `lockfile::render()` → write.

### Authentication

`--header NAME:${ENV_VAR}` reuses `config::resolve_headers()` from the verify
command. The `reqwest` client already supports custom headers. Same
10 MiB body limit and no-proxy policy as verify.

### Mutual Exclusivity

`--from-url`, `--from-json`, and `--from-har` are mutually exclusive. Exactly
one must be present. Enforced in `main.rs`.

### Error Handling

| Condition | Result |
|-----------|--------|
| Non-2xx status | Hard error: "server returned {status}" |
| Non-JSON content-type | Hard error: "response is not JSON ({content-type})" |
| Invalid JSON body | Hard error: "failed to parse JSON response" |
| Network error | Hard error: "failed to fetch {url}: {error}" |
| Body exceeds 10 MiB | Hard error: truncated |

### Files Touched

| File | Change |
|------|--------|
| `src/cli.rs` | Add `--from-url` (PathBuf, optional), `--method` (String, optional) to `Command::Record` |
| `src/main.rs` | Add `--from-url` branch to `Command::Record` handler |
| No new modules needed | `reqwest` already available |

---

## 2. Multi-Entry Verify

### Goal

Verify all observed entries in a lock against a live API in one command.

### CLI

```
apiwatch verify --all --lock api.lock --source-url https://api.example.com
  --header "Authorization:${MY_TOKEN}" (optional, repeated)
  --format text|json|sarif (default text)
```

### Behavior

1. Load the lockfile, collect all observed entries (skip declared).
2. For each observed entry keyed `{METHOD} {path}`:
   - Construct URL: `{source-url}{path}` (e.g. `GET /users` →
     `https://api.example.com/users`)
   - Fetch with `reqwest` using the entry's method
   - Parse JSON response
   - Call `observed::verify_with_tiers()` using the entry's `shape` and
     `threshold`
3. Report per-entry with threshold, timestamps, and tiered output.
4. Exit code 1 if ANY entry has breaking changes; 0 otherwise.

### Reporting

Each entry's output follows the existing observed verify format (text, JSON,
or SARIF). Entries are reported sequentially with a divider between them.

Text output:
```
=== GET /users (observed, threshold 0.50) ===
Verified GET /users
  first seen: 2026-08-03T10:00:00Z
  last seen:  2026-08-03T10:05:00Z

=== GET /orders (observed, threshold 0.50) ===
Verified GET /orders
  first seen: 2026-08-03T10:00:00Z
  last seen:  2026-08-03T10:05:00Z
```

JSON output: array of per-entry reports.
SARIF output: runs array with per-entry results.

### Authentication

`--header` reuses the same `config::resolve_headers()` path as single-entry
verify. Headers apply to all entries.

### Edge Cases

- No observed entries in lock → error: "no observed entries to verify"
- Network error for one entry → report the error, continue with remaining
- Non-JSON response → report as error for that entry, continue

### Files Touched

| File | Change |
|------|--------|
| `src/cli.rs` | Add `--all` (bool), `--source-url` (String) to `Command::Verify`. `openapi` positional becomes optional when `--all` is set |
| `src/main.rs` | Add `--all` branch to `Command::Verify` handler |
| `src/output/mod.rs` | Add multi-entry aggregation for text/JSON/SARIF |

---

## 3. `apiwatch init`

### Goal

Scaffold a new lockfile and CI workflow so users can start using APIWatch in
a new project with one command.

### CLI

```
apiwatch init --output api.lock
```

### Behavior

1. Check if `api.lock` exists. If yes, prompt: "api.lock already exists.
   Overwrite? [y/N]". Exit 2 if user declines or output is not a TTY.
2. Write empty v4 lockfile:

```yaml
version: 4
apis: {}
```

3. Check if `.github/workflows/apiwatch.yml` exists. If yes, skip with a
   message (don't overwrite CI config).
4. Write `.github/workflows/apiwatch.yml`:

```yaml
name: apiwatch
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
jobs:
  verify:
    uses: hitesh518-collab/apiwatch/.github/workflows/action.yml@main
```

5. Print summary:
```
Created api.lock (version 4)
Created .github/workflows/apiwatch.yml

Next steps:
  1. Record an observed contract: apiwatch record --from-har capture.har --output api.lock
  2. Or lock a declared contract: apiwatch lock --openapi spec.yaml --name my-api --output api.lock
  3. Verify in CI: git add api.lock .github/workflows/ && git commit -m "add apiwatch"
```

### Edge Cases

- `.github/workflows/` directory does not exist → create it
- Output file is not named `api.lock` → still create the workflow (it
  references the default name, user can edit)
- Non-GitHub CI → workflow file is GitHub-specific but harmless if ignored

### Files Touched

| File | Change |
|------|--------|
| `src/cli.rs` | Add `Init` variant to `Command` |
| `src/main.rs` | Add `Init` handler |
| No new modules | |

---

## 4. Coverage Command

### Goal

Show the user what endpoints and fields are covered in an observed lock,
distinguishing hardened from lenient fields.

### CLI

```
apiwatch coverage --lock api.lock
  --name GET /users (optional, filter to one entry)
```

### Behavior

1. Load the lockfile.
2. For each observed entry (or the named one), traverse the shape tree.
3. Report per-property: path, observation count, hardening status
   (hardened/lenient), and the reason if lenient (below observation floor,
   below threshold ratio).

### Text Output

```
GET /users (observed, threshold 0.50, 10 total observations)
  first seen: 2026-08-03T10:00:00Z
  last seen:  2026-08-03T10:05:00Z

  Fields:
    $.id               number    10/10 observations  hardened
    $.name             string    10/10 observations  hardened
    $.email            string     8/10 observations  hardened (0.80 >= 0.50)
    $.metadata         object     2/10 observations  lenient (below floor: 2 < 3)
    $.metadata.region  string     1/2  observations  lenient (0.50 < 0.50 threshold)
    $.tags[]           string     3/10 observations  lenient (below floor: 3 < 3)
    $.tags[] items     unknown    no item evidence   lenient (empty array)

GET /orders (observed, threshold 0.50, 5 total observations)
  ...
```

### Key Logic

- Hardened: `is_hardened(parent_obs, property_obs, threshold)` returns true
- Lenient: `is_hardened()` returns false → show the reason (floor or ratio)
- Empty containers (empty array/object) always shown as lenient
- Map values shown as `<map-value>` segment

### Edge Cases

- Lock has only declared entries with no observed → "no observed entries in
  lock"
- `--name` specifies a declared entry → "entry is declared, not observed"

### Files Touched

| File | Change |
|------|--------|
| `src/cli.rs` | Add `Coverage` variant to `Command` with `--lock` and `--name` |
| `src/main.rs` | Add `Coverage` handler that walks shapes and prints hardening status |
| No new modules | Shape traversal functions already exist in `observed::tiered_report` |

---

## 5. Onboarding & Examples

### Goal

Give new users a quickstart path from zero to value-free lock in CI.

### Changes

1. **README quickstart section** — replace or update the usage section with:
   - Step 1: Record from HAR (`apiwatch record --from-har capture.har --output api.lock`)
   - Step 2: Verify against live API (`apiwatch verify --all --lock api.lock --source-url ...`)
   - Step 3: Set up CI (`apiwatch init --output api.lock`)
   - Short note about keying: auto-derived from method+path, `--name` to override

2. **Sample HAR fixture** — add `testdata/har/example-quickstart.har` with
   2-3 realistic-looking JSON API responses (e.g., a users endpoint and an
   orders endpoint) so newcomers can try the tool without capturing their own
   traffic first.

3. **Update known limitations table** — remove Phase 5 entries (D-19
   observed inputs) and update the status.

### Files Touched

| File | Change |
|------|--------|
| `README.md` | Add quickstart section, update known limitations |
| `testdata/har/example-quickstart.har` | New sample HAR for onboarding |

---

## Global Constraints

- MSRV 1.88
- No new dependencies (reqwest, serde_json, url already available)
- All existing 336 tests must remain green
- No lockfile version bump
- Deterministic output
- No scalar values, credentials, or dynamic map keys in serialized output

## Exit Criterion

A user can record from a live URL, verify all observed entries against a
live API in one command, scaffold a new project with init, inspect coverage,
and follow the quickstart to get from zero to CI in 3 steps.
