# apiwatch

**Catch third-party API changes before they break your app.**

APIWatch records or locks the API contract your application relies on, stores
value-free evidence in Git, and fails CI when that external dependency drifts.

```text
package-lock.json : packages
api.lock          : external APIs
```

`oasdiff` diffs specs you own. **APIWatch locks APIs you don't.**

When a provider publishes a usable OpenAPI document, APIWatch normalizes it
into a deterministic declared contract. When a specification is absent,
incomplete, or unreliable, APIWatch infers a value-free observed response shape
from samples. Both paths produce reviewable, CI-verifiable evidence of what your
code expects from an API you do not control.

---

## 60-second demo

Record the shape of a payment API response once, then catch when it drifts:

```bash
# Record the expected structure (stores types and paths, never values)
apiwatch record --from-json testdata/observed/demo-baseline.json  --name payments --output api.lock

# Verify the same structure — passes
apiwatch verify testdata/observed/demo-baseline.json --name payments --lock api.lock
# Verified payments (observed) — exit 0

# Verify a breaking change — `amount` changed from number to string
apiwatch verify testdata/observed/demo-breaking.json --name payments --lock api.lock
```

```
Drift detected in payments (observed)
BREAKING $.amount: expected number, found string — exit 1
```

The lockfile stores only the fact that `$.amount` is a `number`. It never
captures `42.50`, `pay_123`, or any payment data.

All demo fixtures are committed in `testdata/observed/`. No network, no
credentials, no live API required.

---

## How it works

**1. Record what the API looks like.**
Point APIWatch at a JSON response, a HAR capture, or an OpenAPI document. It
normalizes the structure into a deterministic, value-free contract entry.

**2. Commit the evidence.**
`api.lock` is a reviewable lockfile you commit to your repository. It records
types, paths, requiredness, and observation counts — never captured values,
credentials, or dynamic keys.

**3. Verify in CI.**
`apiwatch verify` checks a live or local response against the recorded contract.
It exits `0` when the structure matches, `1` when it breaks, and `2` on invalid
input. Add it to your workflow and catch provider-side changes before they reach
production.

---

## Declared OpenAPI contracts

When a usable OpenAPI specification exists, APIWatch locks the declared
contract:

```bash
apiwatch lock  openapi.yaml --name users --output api.lock
apiwatch diff  old.openapi.yaml new.openapi.yaml
apiwatch verify openapi.yaml --name users --lock api.lock
```

Declared contracts support OpenAPI 3.0 and 3.1 YAML and JSON with local and
external `$ref` resolution. `apiwatch lock` creates deterministic v4 lock
entries. `apiwatch diff` normalizes two documents and reports semantic changes.
`apiwatch verify` reconstructs the locked contract and runs the same comparison
engine used by `diff`.

Scoping, configuration, and remote features:

```bash
apiwatch lock openapi.yaml --name users --output api.lock \
  --include-operation "GET /users/{id}" --max-lock-bytes 5242880
apiwatch verify https://api.example.com/openapi.yaml --name users --lock api.lock \
  --header "Authorization:${AUTH_TOKEN}" --config .apiwatch.yaml
apiwatch verify openapi.yaml --name users --lock api.lock --format json
apiwatch verify openapi.yaml --name users --lock api.lock --format sarif
```

Remote verification uses a 10-second timeout and a 10 MiB response limit.

---

## Installation

### Prebuilt binaries (recommended)

Download the latest binary from the
[releases page](https://github.com/hitesh518-collab/apiwatch/releases/latest):

| Platform | Asset |
|----------|-------|
| Linux x86_64 | `apiwatch-x86_64-unknown-linux-gnu.tar.gz` |
| Linux x86_64 (musl) | `apiwatch-x86_64-unknown-linux-musl.tar.gz` |
| Linux aarch64 | `apiwatch-aarch64-unknown-linux-gnu.tar.gz` |
| macOS x86_64 | `apiwatch-x86_64-apple-darwin.tar.gz` |
| macOS arm64 | `apiwatch-aarch64-apple-darwin.tar.gz` |
| Windows x86_64 | `apiwatch-x86_64-pc-windows-msvc.zip` |

Extract the archive and place the binary on your `PATH`.

### cargo install

```bash
cargo install apiwatch
```

Requires Rust 1.88 or newer.

### Source build

```bash
git clone https://github.com/hitesh518-collab/apiwatch.git
cd apiwatch
cargo build --release
```

### Homebrew (source build)

```bash
git clone https://github.com/hitesh518-collab/apiwatch.git
cd apiwatch
brew install --build-from-source ./Formula/apiwatch.rb
```

### Scoop (source build)

```powershell
git clone https://github.com/hitesh518-collab/apiwatch.git
cd apiwatch
scoop install ./Scoop/apiwatch.json
```

Homebrew and Scoop are source-build integrations; no published tap or bucket
exists yet.

---

## GitHub Action

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

Pin the action to a commit SHA or release tag. The action downloads a matching
release binary and falls back to building from source with Cargo. It propagates
Verify's exit codes and supports Code Scanning upload via `sarif-file`.

---

## What APIWatch catches

When an API response changes, APIWatch detects:

- **Missing required fields** — a field present in the recorded contract disappears
- **Type changes** — a `number` becomes a `string`, an `object` becomes an array
- **Nullability changes** — a field goes from nullable to non-nullable (or vice versa)
- **Object shape changes** — nested fields added, removed, or retyped
- **Array item changes** — items inside an array change structure

For declared OpenAPI contracts, it also catches:

- Endpoint and operation removal, authentication changes, parameter
  addition/removal/type changes, schema requiredness, enum value changes,
  content-type changes, server changes, and composition branch differences.

See [docs/change-rules.md](docs/change-rules.md) for the complete semantic rule
catalog.

---

## What APIWatch does not guarantee

- **Observed contracts prove sampled structure, not complete coverage.**
  Endpoints, response variants, conditional fields, and error shapes that
  were never recorded are not verified.

- **It does not inspect captured values.**
  APIWatch stores types and structure. It cannot detect semantic changes such
  as "the `status` field now returns `rejected` instead of `declined`" — both
  are valid strings.

- **It is not a functional API test.**
  APIWatch verifies structure compatibility, not business-logic correctness.
  Use integration tests for behavioral assertions.

- **It is not an uptime monitor.**
  APIWatch checks whether the response shape still matches. It does not poll
  for availability or latency.

- **It does not replace contract testing for APIs you own.**
  If you control the API, versioned specs and generated clients may be
  sufficient. APIWatch is for APIs you depend on but do not control.

---

## Output and exit codes

```bash
apiwatch diff   old.yaml new.yaml --format text|json|sarif
apiwatch verify openapi.yaml --name users --lock api.lock --format text|json|sarif
```

| Exit code | Meaning |
|-----------|---------|
| `0` | Clean — no breaking change detected |
| `1` | Drift — breaking change found |
| `2` | Invalid input or operational failure |

Text output is human-readable. JSON output is a versioned, deterministic result
document. SARIF 2.1.0 output works with GitHub Code Scanning.

---

## Observed JSON maps

When object keys are dynamic data rather than API fields, mark the object with
`--map-at`:

```bash
apiwatch record --from-json response.json --name portfolio --output api.lock \
  --map-at $.by_broker --map-at $.state.by_region
```

Map keys may be added, removed, or renamed without triggering drift. Each map
value is still verified structurally. APIWatch never infers maps silently —
choosing map semantics changes compatibility, so an explicit annotation is
required.

Diagnostics use a stable `<map-value>` segment in place of dynamic keys (e.g.
`$.by_broker.<map-value>.pnl_pct`). Text, JSON, and SARIF output never expose
the actual dynamic key.

---

## Comparison: when to use APIWatch

### APIWatch is a fit when

- your application depends on third-party REST APIs,
- API breakage can reach production before you notice,
- provider specs are missing, incomplete, or unreliable,
- you want contract evidence in Git and CI,
- you maintain multiple integrations or connectors.

### APIWatch is probably not needed when

- the API is entirely controlled within one repo or team and existing contract
  testing already solves the problem,
- the provider has strong versioned specs plus generated clients and your risk
  is low,
- you need request/response functional testing rather than structural
  compatibility evidence,
- you primarily need GraphQL, gRPC, or AsyncAPI support.

---

## Lockfile versions

| Version | Declared coverage | Status |
|---------|-------------------|--------|
| 4 | Full Phase 2 contract | Current |
| 3 | Partial Phase 2 | Readable (requires re-lock for full coverage) |
| 1–2 | Routes only | Readable (requires re-lock from original source) |

Observed entries are v2 and will migrate to a content-addressed format in v2.0.0.

See [docs/migration.md](docs/migration.md) and [docs/lockfile-spec.md](docs/lockfile-spec.md).

---

## Known limitations

| Area | Limitation |
|------|-----------|
| Swagger 2.0 | Not supported |
| Path-level `$ref` | Not supported (Paystack corpus entry is known-failing) |
| Schema expansion | Some densely-shared schema graphs exceed the resolution budget (Stripe) |
| Distribution | Action does not yet verify release checksums; Homebrew/Scoop are source-build only |
| Observed format | Content-addressed observed entries planned for v2.0.0 |
| Passive capture | Post-v1 exploration |

The compatibility corpus tracks 20 real-world specs (14 passing, 6 known-failing).
See [docs/compat-corpus.md](docs/compat-corpus.md).

---

## Product direction

APIWatch is focused on deterministic REST contract evidence for APIs a consumer
does not control. Declared and observed contracts share one lock-and-verify
product model, while preserving the difference between provider declarations and
sampled evidence.

The roadmap, phase exit criteria, and v1 boundaries live in
[ROADMAP.md](ROADMAP.md).

---

## Non-goals

- Dashboards, web interfaces, or hosted services
- User accounts, billing, or a cloud backend
- Static code scanning for API calls
- General API testing, mock generation, or SDK generation
- GraphQL, gRPC, or AsyncAPI before the REST product is stable
- AI-powered contract decisions
- Replacing mature tools as a general-purpose OpenAPI differ

---

## License

Apache-2.0. See [CHANGELOG.md](CHANGELOG.md) for release history.
