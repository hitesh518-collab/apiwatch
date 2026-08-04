# apiwatch

API lockfiles for external services.

`apiwatch` is a CLI-first open-source tool for locking, diffing, and verifying
the APIs your applications depend on but do not control.

```text
package-lock.json : packages
api.lock          : external APIs
```

`oasdiff` diffs specs you own. **APIWatch locks APIs you don't.**

APIWatch uses declared contracts when a provider publishes a usable OpenAPI
document. When a specification is absent, incomplete, or unreliable, it can
record a value-free observed response shape instead. Both paths aim to make
external API expectations reviewable in Git and enforceable in CI.

## Status

APIWatch v1.0.2 is the current stable release. It bundles declared and observed
contract verification, the complete Phase 2 comparison model, lockfile v4,
JSON and SARIF output, a reusable GitHub Action, and source-building Homebrew
and Scoop definitions.

Current v4 declared locks contain the complete Phase 2 normalized contract.
Declared `verify` uses the same semantic comparison engine as `diff`, covering
operations, schemas, parameters, authentication, servers, content types,
composition, and responses. Version 3 remains readable with partial coverage
and requires re-locking from the original OpenAPI source for full Phase 2
coverage. Versions 1 and 2 remain readable as route-only legacy formats.

See [docs/migration.md](docs/migration.md) for lockfile version upgrade instructions.

## CLI

```bash
apiwatch diff old.openapi.yaml new.openapi.yaml
apiwatch lock openapi.yaml --name users --output api.lock
apiwatch lock openapi.yaml --name users --output api.lock --update
apiwatch lock openapi.yaml --name users --output api.lock \
  --include-operation "GET /users/{id}" \
  --max-lock-bytes 5242880
apiwatch verify openapi.yaml --name users --lock api.lock
apiwatch verify https://api.example.com/openapi.yaml --name users --lock api.lock
```

The declared-contract path currently targets OpenAPI 3.0 YAML and JSON.
`apiwatch diff` normalizes two documents and reports semantic changes.
`apiwatch lock` creates a deterministic v4 full-contract entry and refuses to
overwrite an existing file. Use `--update` for deliberate atomic replacement.
Repeatable `--include-operation` options scope large APIs; the default
per-entry payload ceiling is 5,242,880 bytes.

`apiwatch verify` reconstructs a v4 declared contract and runs the same
`diff_contracts` path used by `diff`. Warning-only and non-breaking changes
exit `0`; breaking changes exit `1`. Updating a v1, v2, or v3 lock requires the
original OpenAPI source and is refused when other older declared entries would
be left partially migrated. Version 3 Verify reports partial Phase 2 coverage;
v1/v2 Verify remains route-only. Each limitation is reported in text, JSON,
and SARIF.

`apiwatch diff` and `apiwatch verify` accept `--header NAME:${ENV_VAR}` for
authenticated remote fetches and `--config .apiwatch.yaml` for
per-project configuration (ignore rules, severity overrides, fail thresholds).
Use `--ref-root <PATH>` when the spec is in a different directory than its
`$ref` targets.

`apiwatch coverage api.lock` reports endpoint and field coverage for observed
entries.

Remote verification uses a 10-second timeout and a 10 MiB response limit.
Custom headers can be supplied via `--header` or `.apiwatch.yaml` configuration.

## Observed JSON Contracts

When an OpenAPI specification is absent or incomplete, record the shape of a
local JSON response, then verify future local JSON responses against it:

```bash
apiwatch record --from-json body.json --name portfolio --output api.lock
apiwatch record --from-json updated.json --name portfolio --output api.lock --merge
apiwatch record --from-url https://api.example.com/data --name portfolio --output api.lock
apiwatch verify body.json --name portfolio --lock api.lock
```

APIWatch records JSON structure, never captured values. `record` is an
explicit learning command that updates a lock; `verify` only checks it.
Observed entries accept local JSON files, HAR captures, and live URL recording.

An observed contract represents the samples supplied to it. It does not prove
that every endpoint, response variant, conditional field, or error shape has
been observed. Confidence-aware requiredness and coverage reporting are
planned in [Roadmap Phase 4](ROADMAP.md#phase-4--trustworthy-observed-contracts).

## Quickstart

```bash
# 1. Record from browser traffic (export HAR from DevTools)
apiwatch record --from-har traffic.har --output api.lock
# Try it with our example: apiwatch record --from-har testdata/har/example-quickstart.har --output api.lock

# 2. Verify all observed entries against a live API
apiwatch verify --all --lock api.lock --source-url https://api.example.com

# 3. Scaffold CI
apiwatch init --output api.lock
git add api.lock .github/workflows/
git commit -m "add apiwatch contract evidence"
```

### Observed JSON Maps

When object keys are dynamic data rather than API fields, mark the object
explicitly with repeatable `--map-at` annotations:

```bash
apiwatch record --from-json portfolio.json --name portfolio --output api.lock --map-at $.by_broker --map-at $.state.by_region
```

Each annotation accepts only `$` or named property segments such as
`$.by_broker`. Map keys may be added, removed, or renamed without drift, while
every map value is still verified structurally.

APIWatch never infers maps automatically. An annotation is required because
choosing map semantics changes compatibility. Stored locks and Verify
diagnostics contain field names, JSON paths, and shape names only—never
dynamic map keys or captured scalar values. Bracket notation, arrays,
wildcards, filters, scripts, advanced JSONPath, and coverage reporting are not
currently supported.

When a dynamic map value is incompatible, diagnostics use the stable redacted
segment `<map-value>`—for example,
`$.by_broker.<map-value>.pnl_pct`. Text, JSON, SARIF messages, and SARIF
fingerprints therefore never expose the actual dynamic key.

## Output and Exit Codes

```bash
apiwatch diff old.openapi.yaml new.openapi.yaml --format json
apiwatch verify openapi.yaml --name users --lock api.lock --format json
apiwatch diff old.openapi.yaml new.openapi.yaml --format sarif
apiwatch verify openapi.yaml --name users --lock api.lock --format sarif
```

`apiwatch diff` and `apiwatch verify` support
`--format text|json|sarif`; text is the default. JSON output is a versioned,
deterministic result document written to stdout. SARIF 2.1.0 output is intended
for GitHub Code Scanning.

`apiwatch verify <INPUT> --name <NAME> --lock <PATH>` selects declared or
observed verification from the named lock entry's provenance. It exits `0`
when no breaking finding is present, `1` for breaking drift, and `2` for
invalid input or operational failure. Declared Verify JSON version 2 includes
`coverage: full|routes` and structured limitations.

## Installation

APIWatch is published on [crates.io](https://crates.io/crates/apiwatch) and
requires Rust 1.88 or newer. APIWatch declares and checks this minimum in CI
so dependency changes cannot raise it silently.

### cargo install (recommended)

```bash
cargo install apiwatch
```

This is the fastest way to get started and pulls the latest published version.

### Source build

```bash
git clone https://github.com/hitesh518-collab/apiwatch.git
cd apiwatch
cargo build --release
```

The binary is then available at `target/release/apiwatch`.

### Homebrew

The repository includes a source-building Homebrew formula for the v1.0.2
tagged release:

```bash
git clone https://github.com/hitesh518-collab/apiwatch.git
cd apiwatch
brew install --build-from-source ./Formula/apiwatch.rb
```

This formula is not yet a Homebrew tap, so `brew install apiwatch` is not
available.

### Scoop

The repository includes a source-building Scoop manifest for the v1.0.2
tagged release:

```powershell
git clone https://github.com/hitesh518-collab/apiwatch.git
cd apiwatch
scoop install ./Scoop/apiwatch.json
```

Scoop installs Rust automatically. Rust source builds on Windows also require
Microsoft C++ Build Tools and a Windows SDK. This manifest is not yet in a
Scoop bucket.

Prebuilt binaries, a Homebrew tap, a Scoop bucket, and automated release
updates are part of the
[continuous distribution track](ROADMAP.md#continuous-distribution-track).

## GitHub Action

Use the reusable action from an Ubuntu workflow after checking out the
consumer repository:

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

The `openapi` and `name` inputs are required. `lock` defaults to `api.lock`,
and `working-directory` defaults to `.`. `sarif-file` is relative to
`working-directory`; when set, it enables Code Scanning upload and requires
`security-events: write`. A Verify drift report uploads before the action
returns exit `1`.

Pin the action to a commit SHA or release tag. The action currently builds
APIWatch from source with Cargo, propagates Verify's `0`/`1`/`2` exit codes,
and supports the `working-directory` input. It does not provide caching,
action outputs, authentication, custom headers, or configuration files.

### Known Limitations

Current v4 locks cover the completed Phase 2 audit classes;
older locks retain the coverage limitations shown below.

| Area | Current limitation | Tracked work |
|---|---|---|
| Request bodies (D-01) | Resolved for presence and requiredness in current v4 locks. | [Phase 2](ROADMAP.md#phase-2--make-the-comparison-engine-trustworthy) |
| Content types (D-02) | Resolved for canonical request and response media types. | Phase 2 |
| Response requiredness (D-03) | Resolved with directional request/response rules. | Phase 2 |
| Dictionary schemas (D-04) | Resolved for forbidden, unconstrained, and schema-valued `additionalProperties`. | Phase 2 |
| Schema formats (D-05) | Resolved as deterministic warnings. | Phase 2 |
| Servers (D-06) | Resolved for effective, privacy-safe server-template identity. | Phase 2 |
| Path templates (D-07) | Resolved through positional template identity while retaining display paths. | Phase 2 |
| Security identity (D-08) | Resolved through normalized wire identity. | Phase 2 |
| Composition (D-09) | Resolved through `allOf` intersection and order-independent `oneOf`/`anyOf` comparison. | Phase 2 |
| Array model (D-10) | Resolved with first-class array items. | Phase 2 |
| Enum severity (D-11) | Resolved with directional request/response enum policy. | Phase 2 |
| OpenAPI 3.1 (D-12) | Resolved: OpenAPI 3.1 nullable type arrays are supported. | [Phase 3](ROADMAP.md#phase-3--real-world-compatibility) |
| Strict metadata parsing (D-13) | Resolved: metadata strictness relaxed for real-world specs like DigitalOcean. | Phase 3 |
| Recursive schemas (D-14) | Resolved: cycle detection and schema expansion budget handles recursive and densely-shared schemas. | Phase 3 |
| External references (D-15) | Resolved: external `$ref` targets with `components:`-wrapped fragments are supported. | Phase 3 |
| Legacy declared locks (D-16) | Versions 1 and 2 are route-only; v3 is partial for Phase 2. All require re-locking from original sources for full v4 coverage. | [Phase 1](ROADMAP.md#phase-1--make-verify-meaningful) |
| Null observations (D-17) | Resolved with observation-floor hardening and lenient null at verify time. | [Phase 4](ROADMAP.md#phase-4--trustworthy-observed-contracts) |
| Observed requiredness (D-18) | Resolved with configurable `--required-threshold` and confidence metadata. | Phase 4 |
| Observed recording (D-19) | HAR import and live URL recording implemented; passive proxy is post-v1. | [Phase 5](ROADMAP.md#phase-5--frictionless-recording-and-ci-adoption) |
| Distribution | The Action, Homebrew formula, and Scoop manifest still build from source. | [Continuous distribution](ROADMAP.md#continuous-distribution-track) |

Repeated phase names in the table refer to the linked phase in the first row
for that group. See [ROADMAP.md](ROADMAP.md) for exit criteria.

## Product Direction

APIWatch is focused on deterministic REST contract evidence for APIs a
consumer does not control. Declared and observed contracts share one
lock-and-verify product model, while preserving the difference between
provider declarations and sampled evidence.

The correctness-first sequence, phase exit criteria, distribution work, and
v1 boundaries live in [ROADMAP.md](ROADMAP.md).

## Non-Goals

- Dashboards, web interfaces, or hosted services
- User accounts, billing, or a cloud backend
- Static code scanning for API calls
- General API testing, mock generation, or SDK generation
- GraphQL, gRPC, or AsyncAPI before the REST product is stable
- AI-powered contract decisions
- Replacing mature tools as a general-purpose OpenAPI differ

Proxy or passive runtime capture is a post-v1 exploration, not current scope.

## License

Apache-2.0

## Changelog

See [CHANGELOG.md](CHANGELOG.md).
