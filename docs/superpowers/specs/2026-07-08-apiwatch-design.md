# apiwatch Design

## Summary

`apiwatch` is a CLI-first open-source tool that helps developers lock, diff, and verify the external API contracts their applications depend on. The first milestone is a Rust command-line tool that compares local OpenAPI 3.x files, normalizes them into an internal API contract model, and reports semantic compatibility changes.

The project should be public from day one, licensed under Apache-2.0, and positioned as:

> API lockfiles for external services.

## Goals

- Build a serious systems-style developer tool in Rust.
- Start with OpenAPI 3.x files because they provide structured API contracts.
- Produce deterministic, CI-friendly semantic diffs.
- Prefer a small set of high-confidence rules over noisy broad detection.
- Keep the core language-agnostic by diffing normalized contracts rather than raw OpenAPI documents.
- Create a public repository that is easy for future contributors to understand.

## Non-Goals

- No dashboard in the MVP.
- No user accounts or cloud backend.
- No static code scanning in the MVP.
- No runtime monitoring in the MVP.
- No AI features in the MVP.
- No support for every API description format at the start.
- No accidental stabilization of the `api.lock` format before the design has matured.

## Technology Choices

- Language: Rust.
- CLI parsing: `clap`.
- Serialization: `serde`, `serde_yaml`, and `serde_json`.
- HTTP support later: `reqwest`.
- License: Apache-2.0.
- Distribution path: `cargo install` first, then GitHub Releases and Homebrew later.

Rust is the right fit because the project aims for long-term systems-tool credibility, single-binary distribution, strong correctness guarantees, and a serious developer-tooling feel.

## MVP Boundary

The MVP focuses on semantic OpenAPI diffing.

The first useful command is:

```bash
apiwatch diff old.openapi.yaml new.openapi.yaml
```

Initial command roadmap:

```bash
apiwatch diff old.openapi.yaml new.openapi.yaml
apiwatch lock --config apiwatch.yaml
apiwatch verify
```

`diff` should be implemented first. `lock` and `verify` can be documented or stubbed until the diff engine is useful.

## Architecture

The crate should be split around product concepts:

- `cli`: parses commands and flags with `clap`.
- `openapi`: loads OpenAPI 3.x documents and resolves the supported subset.
- `contract`: defines normalized API contract structs independent of OpenAPI.
- `diff`: compares two normalized contracts and emits typed changes.
- `output`: renders human-readable text first, with JSON later.
- `lockfile`: reads and writes `api.lock` after `lock` exists.
- `config`: reads `apiwatch.yaml` after `init` and `lock` exist.

The core design choice is that diffing operates on normalized contracts, not raw OpenAPI documents. This keeps the product extensible for future sources such as JSON samples, GraphQL, Postman collections, HAR files, or runtime captures.

## Data Flow

```text
OpenAPI file A -> parse -> normalize -> Contract A
OpenAPI file B -> parse -> normalize -> Contract B
Contract A + Contract B -> diff rules -> Change list -> terminal output + exit code
```

## CLI Behavior

`apiwatch diff` should be quiet, deterministic, and CI-friendly.

Default usage:

```bash
apiwatch diff old.openapi.yaml new.openapi.yaml
```

Default output should group changes by severity:

```text
Breaking changes
- GET /repos/{owner}/{repo}: response field removed: license

Warnings
- POST /payments: response field became nullable: receipt_url

Non-breaking changes
- GET /users/{id}: optional response field added: avatar_url
```

Exit codes:

- `0`: no breaking changes.
- `1`: breaking changes found.
- `2`: invalid input, parse failure, unsupported OpenAPI shape, or internal error.

## Change Rules

Early breaking changes should include:

- Endpoint removed.
- HTTP method removed.
- Required request field added.
- Response field removed.
- Response field type changed.
- Enum value removed.
- Successful status code removed.
- Content type changed.

Early non-breaking changes should include:

- Endpoint added.
- Optional response field added.
- Optional request parameter added.

Early warnings should include:

- Nullable changed.
- Numeric type widened or narrowed.
- Format changed.
- Response field became optional.
- New error status code added.
- Unsupported or ambiguous OpenAPI shapes.

The rule philosophy is to preserve developer trust. It is better to ship fewer high-confidence rules than to report many uncertain changes.

## Testing

Testing should be fixture-driven. The repository should include small OpenAPI file pairs under `testdata/openapi/`, each proving one rule.

Initial fixtures should cover:

- Endpoint removed.
- Method removed.
- Required request field added.
- Response field removed.
- Response field type changed.
- Enum value removed.
- Content type changed.

The diff engine should be tested at the normalized contract layer as well as through CLI integration tests.

## Repository Shape

Initial public repository structure:

```text
apiwatch/
  src/
    main.rs
    cli.rs
    contract/
    diff/
    openapi/
    output/
  docs/
    superpowers/specs/
    lockfile-spec.md
    change-rules.md
  examples/
    simple-openapi/
  testdata/
    openapi/
  .github/
    workflows/
    ISSUE_TEMPLATE/
  README.md
  IDEA.md
  DESIGN.md
  CONTRIBUTING.md
  CODE_OF_CONDUCT.md
  LICENSE
  Cargo.toml
```

Repository settings:

- Name: `apiwatch`.
- Visibility: public.
- Description: `Lock, diff, and verify the APIs your code depends on.`
- Default branch: `main`.
- License: Apache-2.0.
- Topics: `api`, `openapi`, `cli`, `rust`, `contract-testing`, `developer-tools`, `ci`.

## First Public Impression

The README should lead with the problem and the mental model:

> `package-lock.json` locks packages. `api.lock` should lock external API contracts.

The repo should include:

- `README.md` with a quick overview, early CLI examples, and roadmap.
- `IDEA.md` preserving the product vision.
- `DESIGN.md` summarizing this architecture.
- `docs/change-rules.md` documenting breaking, warning, and non-breaking changes.
- `docs/lockfile-spec.md` marked as a draft.
- GitHub Actions for `cargo fmt`, `cargo clippy`, and `cargo test` once the Rust skeleton exists.

## Implementation Planning Decisions

- The implementation plan should evaluate `openapiv3` first for OpenAPI 3.x parsing, with a fallback to direct `serde_json::Value` or `serde_yaml::Value` traversal if the crate blocks the first milestone.
- v0.1 should support paths, HTTP methods, parameters, JSON request bodies, JSON responses, status codes, content types, object properties, required fields, enum values, primitive types, nullable, and format.
- v0.1 should parse YAML and JSON OpenAPI documents through the same loader.
- `apiwatch diff` should accept OpenAPI files directly in v0.1. A `--from` flag can be added later when more input types exist.
- Machine-readable JSON output should wait until the human-readable change model is stable, then mirror the typed change list emitted by the diff engine.
