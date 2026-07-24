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

APIWatch is in early development. The latest tagged release is v0.6.0.

The current repository also contains unreleased observed JSON recording,
monotonic shape merging, observed verification, and explicit `--map-at`
annotations. These changes are planned for v0.7.0 after the release,
compatibility, and documentation checks in
[Roadmap Phase 0](ROADMAP.md#phase-0--stabilize-and-release-honestly).

Current declared v1 and v2 locks contain normalized routes only. Declared
`verify` detects added or removed operations, but it does not yet verify the
complete schemas, parameters, authentication, content types, or responses
represented by the original OpenAPI document. Full-contract locking and
shared diff/Verify semantics are planned in
[Roadmap Phase 1](ROADMAP.md#phase-1--make-verify-meaningful).

## CLI

```bash
apiwatch diff old.openapi.yaml new.openapi.yaml
apiwatch lock openapi.yaml --name users --output api.lock
apiwatch verify openapi.yaml --name users --lock api.lock
apiwatch verify https://api.example.com/openapi.yaml --name users --lock api.lock
```

The declared-contract path currently targets OpenAPI 3.0 YAML and JSON.
`apiwatch diff` normalizes two documents and reports semantic changes.
`apiwatch lock` writes a deterministic route-only declared entry.
`apiwatch verify` compares the named route set with a local document or
HTTP/HTTPS URL.

Remote verification uses a 10-second timeout and a 10 MiB response limit.
Authentication, custom headers, and configuration files are not included.

## Observed JSON Contracts

When an OpenAPI specification is absent or incomplete, record the shape of a
local JSON response, then verify future local JSON responses against it:

```bash
apiwatch record --from-json body.json --name portfolio --output api.lock
apiwatch record --from-json updated.json --name portfolio --output api.lock --merge
apiwatch verify body.json --name portfolio --lock api.lock
```

APIWatch records JSON structure, never captured values. `record` is an
explicit learning command that updates a lock; `verify` only checks it.
Observed entries currently accept local JSON files only.

An observed contract represents the samples supplied to it. It does not prove
that every endpoint, response variant, conditional field, or error shape has
been observed. Confidence-aware requiredness and coverage reporting are
planned in [Roadmap Phase 4](ROADMAP.md#phase-4--trustworthy-observed-contracts).

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
observed verification from the named lock entry's provenance. It exits `0` for
a match, `1` for detected drift, and `2` for invalid input or operational
failure.

## Installation

### Homebrew

The repository includes a source-building Homebrew formula for the v0.6.0
tagged release:

```bash
git clone https://github.com/hitesh518-collab/apiwatch.git
cd apiwatch
brew install --build-from-source ./Formula/apiwatch.rb
```

This formula is not yet a Homebrew tap, so `brew install apiwatch` is not
available.

### Scoop

The repository includes a source-building Scoop manifest for the v0.6.0
tagged release:

```powershell
git clone https://github.com/hitesh518-collab/apiwatch.git
cd apiwatch
scoop install ./Scoop/apiwatch.json
```

Scoop installs Rust automatically. Rust source builds on Windows also require
Microsoft C++ Build Tools and a Windows SDK. This manifest is not yet in a
Scoop bucket.

Prebuilt binaries, crates.io installation, a Homebrew tap, a Scoop bucket, and
automated release updates are part of the
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

## Known Limitations

- **Declared locks are route-only.** Full schema, parameter, authentication,
  content-type, and response verification requires
  [Phase 1](ROADMAP.md#phase-1--make-verify-meaningful).
- **Diff semantics have confirmed gaps.** Request bodies, content types,
  response requiredness, composition, path templates, authentication identity,
  and related rules are tracked in
  [Phase 2](ROADMAP.md#phase-2--make-the-comparison-engine-trustworthy).
- **Input compatibility is incomplete.** OpenAPI 3.1, external and multi-file
  `$ref`, recursive schemas, and some real-world documents require
  [Phase 3](ROADMAP.md#phase-3--real-world-compatibility).
- **Observed evidence is sample-bound.** Requiredness confidence and coverage
  are planned in [Phase 4](ROADMAP.md#phase-4--trustworthy-observed-contracts).
- **Traffic capture is not implemented.** HAR import, live recording,
  multi-entry Verify, and onboarding automation are planned in
  [Phase 5](ROADMAP.md#phase-5--frictionless-recording-and-ci-adoption).
- **Distribution is source-heavy.** The Action, Homebrew formula, and Scoop
  manifest build from source. Binary distribution is tracked in the
  [continuous distribution work](ROADMAP.md#continuous-distribution-track).

These limitations are release constraints, not hidden implementation details.
Consult the [full roadmap](ROADMAP.md) before relying on a planned capability.

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
