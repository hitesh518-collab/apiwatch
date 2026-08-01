# Phase 3 Design: Real-World Compatibility

**Target:** v0.10.0

**Goal:** Make declared contracts work against the specifications and delivery
patterns users encounter outside controlled fixtures.

## Ordered Scope

### 1. D-14 — Cycle-Breaking References

**Problem:** Recursive `$ref` chains (e.g., a schema that references itself,
directly or transitively) cause infinite resolution loops. The current behavior
rejects them outright. The pinned Stripe spec is `known_failing` with "circular
schema reference detected."

**Design:** Named cycle references via deterministic cycle detection.

- Walk schemas from `components/schemas`. When a `$ref` target revisits a schema
  already on the current resolution stack, a cycle is detected.
- The first JSON Pointer path that reaches each cyclic node becomes its **cycle
  name**: `#/cycles/components/schemas/<Name>`.
- At the back-reference point, the lockfile v4 schema stores
  `Schema::CycleRef { path: String, target: String }` — a terminal leaf.
- The comparison engine treats `CycleRef` as opaque: any two `CycleRef` nodes
  pointing to the same target are considered equal. If the cycle-target schema
  shape changes, a diff is reported at the original (non-cycle) location — not
  duplicated at every cycle site.
- If a cycle path forms a different cycle group (e.g., A→B→A vs B→C→B), each
  group gets its own cycle name.
- The Stripe spec (`file`) is the canonical real-world regression fixture.
- Nested cycles (e.g., a schema that contains a child that cycles back) resolve
  the outermost cycle first; the inner schema is part of the known structure
  before the cycle boundary is drawn.

**Security:** Cycles produce finite lockfile payloads — no stack overflows and
no exponential blow-up.

### 2. D-13 — Malformed Metadata Tolerance

**Problem:** OpenAPI documents containing metadata the normalizer does not
consume (e.g., `tags[i].description` as a map instead of a string) cause
hard parse failures. The pinned DigitalOcean spec is `known_failing` with
`tags[0].description: invalid type: map, expected a string`.

**Design:** Consumer-driven tolerance — the normalization pipeline decides
which fields are consumed.

- During deserialization, wrap the `openapiv3` parser in a tolerant layer
  that skips validation errors on fields the normalizer does not consume.
- Consumed fields (must parse strictly):
  - All `info` except `summary` and `contact` sub-fields
  - All `paths` and their operations (verb, parameters, request body, responses, security)
  - All `components/schemas`, `components/parameters`, `components/responses`,
    `components/requestBodies`, `components/securitySchemes`, `components/pathItems`
  - All `security` blocks
  - `servers` entries and their variables
  - Tag `name` only
- Tolerated fields (parse errors ignored): tag descriptions, external docs,
  callbacks, examples, links, extensions, vendor-specific keys, `license` details,
  `contact` details, `info.summary`.
- If a consumed field fails to parse (e.g., a required property on a schema),
  that still produces an error — no silent data loss.
- The pinned DigitalOcean spec becomes `passing` instead of `known_failing`.

### 3. D-15 — External `$ref` Resolution (File Only)

**Problem:** The tool cannot resolve `$ref: "./schemas/users.yaml#/..."` —
only intra-document `$ref` resolution is supported. Multi-file specs are
common in production APIs.

**Design:** Relative file resolution with path-traversal protection.

- When a `$ref` points to an external path (`$ref: "./schemas/users.yaml#/..."`),
  resolve the file relative to the source spec's directory.
- Path traversal (`../`) escaping the source spec's parent directory is
  rejected with an explicit error.
- `--ref-root <DIR>` CLI flag provides an override base directory for external
  file resolution. When set, all file-relative refs resolve from `<DIR>`.
- Remote `$ref` targets (`https://...`) produce an explicit error:
  "remote references are not yet supported; use --ref-root with pre-downloaded
  files."
- The Box spec exercises internal-only multi-file resolution (if applicable).
- A new split-spec regression fixture:
  - `testdata/openapi/phase3_d15_api.yaml` references
  - `testdata/openapi/phase3_d15_schemas.yaml#/components/schemas/User`
  - Lock, diff, and verify all pass through the resolved schema.

**Security constraints:**

- No symlink traversal outside the resolution root.
- No network access from file refs.
- Each referenced file is opened read-only and its bytes validated as
  parseable YAML or JSON before resolution.

### 4. D-12 — OpenAPI 3.1

**Problem:** OpenAPI 3.1 documents are rejected with an unsupported-version
error. Users on newer API specs cannot use the tool.

**Design:** Normalize 3.1 to the existing v4 contract model — no new
comparison rules.

- Detect OpenAPI 3.1 via `openapi: "3.1.0"` (currently caught and rejected
  by the version guard).
- Parse with a 3.1-aware deserializer that normalizes into the v4 model:
  - `type: ["string", "null"]` → v4 schema with `type: String` and
    `nullable: true`
  - `type` as a single string (e.g., `"object"`) unchanged
  - `exclusiveMinimum`/`exclusiveMaximum` as numbers → v4 range constraints
    already support numeric exclusive bounds
  - `prefixItems` (2020-12 replacement for `items` + `additionalItems`) →
    linearized to the v4 array-items model
  - `$defs` (2020-12 alias for `components/schemas`) → folded into the
    same schema interning pool
  - `unevaluatedProperties`/`unevaluatedItems` → ignored (not consumed by
    the comparison engine; documented as a known gap)
- `webhooks` section → normalized as pseudo-operations in the contract model
  with a `x-webhook` marker. Comparison rules match path operations.
- `info.summary`, `license.identifier`, `jsonSchemaDialect` → parsed but not
  consumed (metadata tolerance applies).
- The existing "unsupported version" error becomes a successful normalization
  path for valid 3.1 documents.
- A `bool` schema value in 3.1 (`true` = any, `false` = nothing) →
  `Schema::Any` / `Schema::Nothing` in the v4 model.

**Exit criterion fixture:** A 3.1 spec with nullable types, `exclusiveMinimum`
as a number, and a `prefixItems` definition diffs correctly through the v4 engine.

### 5. D-23 — Replace `serde_yaml`

**Problem:** `serde_yaml` 0.9 is deprecated and unmaintained. It blocks
future upgrades and has known correctness issues.

**Design:** Swap to `serde_yml` — the maintained community fork.

- Replace `serde_yaml = "0.9"` → `serde_yml = "0.0.1"` (or latest) in
  `Cargo.toml`.
- Update import paths: `serde_yaml::from_reader` → `serde_yml::from_reader`,
  `serde_yaml::to_string` → `serde_yml::to_string`.
- Lockfile YAML encoding (deduplicated schema encoding) uses the same `serde`
  derives — no logic changes.
- Deterministic lockfile output produces byte-identical results before and
  after the swap for all 5 pinned corpus entries.
- A golden-file test asserts that a known v4 lock re-encoded through the new
  library matches the committed golden exactly.

### 6. `.apiwatch.yaml` Configuration

**Problem:** Users cannot ignore known-safe changes, adjust severity
classifications, or set verification thresholds without modifying the tool
or wrapping it in scripts.

**Design:** Repo-local YAML configuration file co-located with `api.lock`.

```yaml
# .apiwatch.yaml
ignore:
  - rule: "parameter-removed"
    path: "/deprecated/*"
  - rule: "endpoint-removed"
    path: "/beta/*"
severity:
  - change: "endpoint-added"
    severity: "warning"
fail_on:
  breaking: 0
  warning: 10
```

**Schema:**

- `ignore` — list of rules. Each has:
  - `rule` (required): the diff change category (kebab-case from the diff
    engine: `endpoint-added`, `endpoint-removed`, `parameter-added`,
    `parameter-removed`, `schema-changed`, `auth-added`, `auth-removed`,
    `response-removed`, `response-added`, `request-body-added`,
    `request-body-removed`, `content-type-added`, `content-type-removed`,
    `server-changed`, `enum-changed`, `format-changed`, `additional-properties-changed`)
  - `path` (optional): glob pattern matching the operation path (e.g.,
    `/deprecated/*`, `/users/{id}`). If absent, applies to all paths.
  - `method` (optional): HTTP method to scope the ignore rule. If absent,
    applies to all methods.
- `severity` — list of overrides. Each has:
  - `change` (required): same change categories as above
  - `severity` (required): `breaking`, `warning`, or `non-breaking`
- `fail_on` — threshold overrides:
  - `breaking` (default: 0): exit 1 if breaking change count > N
  - `warning` (default: unlimited): exit 1 if warning count > N

**Discovery:** When verifying, walk up from the `api.lock` directory.
The first `.apiwatch.yaml` found wins. No global/user config in this phase.

**Validation:** Unknown top-level keys produce an explicit error. Unknown
`rule` or `change` values produce an error. Glob syntax errors produce an
error with the offending pattern. No silent ignoring of misconfiguration.

**Security:** The config file is meant to be committed to the repository.
It contains no secrets. Path globs are validated but cannot access the
filesystem.

### 7. Remote Authentication Headers

**Problem:** Private API specs hosted behind authentication (e.g.,
API-key-gated developer portals) cannot be fetched for remote Verify.

**Design:** Env-var-only interpolation via config and CLI.

```yaml
# .apiwatch.yaml
remote:
  headers:
    X-API-Key: ${MY_API_KEY}
    Authorization: ${AUTH_TOKEN}
```

**Constraints:**

- Values must start with `${` and end with `}` enclosing an environment
  variable name — raw string values (e.g., `Authorization: Bearer abc123`)
  are rejected at parse time.
- CLI override: `--header "X-Custom: ${MY_VAR}"` for one-off or CI use.
- At fetch time, unresolved env vars produce an explicit
  `environment variable <NAME> is not set` error.
- Header names and env-var references are never written to lockfiles,
  diagnostics, SARIF output, or logs. The remote fetch path strips all
  header content before any logging boundary.
- The `reqwest` client adds headers immediately before the request and
  drops them after response parsing.

### 8. Global Compatibility Corpus Expansion

**Problem:** The current 5-spec corpus is all US-based companies, limiting
the representative coverage of global API patterns, non-ASCII metadata,
and varied API design conventions.

**Design:** Grow the corpus from 5 to 10 specs targeting diverse regions
and industries.

**New entries:**

| Spec | Region | Industry | Rationale |
|---|---|---|---|
| FHIR R4 (HL7) | Global | Healthcare | Widely adopted healthcare standard; exercises recursive resource schemas |
| Deutsche Bahn (StaDa) | Germany/EU | Public Transport | Government transport data; German-language metadata; moderate size |
| Mercado Libre | Latin America | E-commerce | Largest Latin American API; Spanish/Portuguese metadata; high operation count |
| Japan Digital Agency | Japan | Government | Japanese government API; non-ASCII metadata; CJK handling |
| Paystack | Africa | Fintech | Leading African payments API; webhook/signature patterns |

**Corpus management:**

- Each entry is pinned to a commit hash with SHA-256 verification, same as
  the existing entries.
- Normalization status tracked per entry (`passing` / `known_failing` with
  expected error).
- The existing 5 entries are unchanged.
- A corpus-wide `--check` regression in the compat job enforces that no
  previously passing entry silently breaks.

**Expected outcomes:**

- At least 1 new entry exercises D-14 (recursive schemas beyond Stripe's
  `file` type — FHIR is recursive across resources).
- At least 1 entry exercises D-13 (Japanese government metadata tends to
  use extensions and non-standard metadata fields).
- CJK property names and descriptions are round-tripped correctly.
- Lockfile sizes remain within the 5 MB ceiling for all normalizable entries.

## Engineering Quality Gates

These carry forward from Phase 2 and the roadmap:

1. Reproduce each defect (D-12 through D-15, D-23) before fixing.
2. Add regression fixtures before changing behavior.
3. Keep diff and declared Verify on one `diff_contracts` comparison path.
4. Preserve deterministic ordering and byte-stable lock output.
5. Keep Verify read-only.
6. Never retain observed values, credentials, or dynamic map keys.
7. Keep documentation accurate for the tagged release.
8. Do not start implementation until this spec is approved.

## Phase 3 Exit Criterion

The compatibility suite passes all normalizable specs (including previously
`known_failing` Stripe and DigitalOcean), an OpenAPI 3.1 nullable-type fixture
diffs correctly, a split multi-file specification resolves
`./schemas.yaml#/User` safely, `.apiwatch.yaml` ignore rules and severity
overrides produce the expected filtered output, and all production v4 lock
payloads for the expanded corpus stay within the 5 MB ceiling.

## Excluded From This Phase

- Remote `$ref` resolution (`https://` refs) — explicit error, deferred
- `unevaluatedProperties`/`unevaluatedItems` semantics — documented gap
- Global or user-level config files — repo-local only
- Observed contract confidence improvements — Phase 4
- HAR or live capture — Phase 5
- Binary distribution or crate publishing — Continuous Distribution Track
