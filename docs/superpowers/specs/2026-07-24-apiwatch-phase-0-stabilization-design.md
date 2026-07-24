# APIWatch Phase 0 Stabilization Design

**Date:** 2026-07-24

**Status:** Approved for implementation planning

## Goal

Release APIWatch v0.7.0 as an honest, installable stabilization release. The
release must expose the already-built observed-contract workflow without
misrepresenting declared Verify, OpenAPI compatibility, observed confidence,
or distribution maturity.

Phase 0 is corrective. It adds no new contract semantics, lockfile format,
capture mode, protocol, or product surface.

## Scope and Sequence

Phase 0 is implemented in four sequential slices:

1. Correctness and toolchain gate.
2. Compatibility smoke suite.
3. Honesty and release preparation.
4. Approval-gated publication and post-tag packaging.

Each slice must pass its own checks before the next begins. Publication is a
separate external-action gate after all repository work passes.

## Slice 1: Correctness and Toolchain Gate

### Observed Verify Output Parity

Current observed Verify returns early on a match and always prints
`Verified <name>`, ignoring `--format json` and `--format sarif`. The fix will
remove that format-bypassing success path.

Observed Verify will calculate changes once, render through the selected
formatter for both matching and drifting inputs, and derive its exit code from
whether changes exist:

| Format | Matching output | Drifting output |
|---|---|---|
| Text | `Verified <name>` | Existing observed breaking lines |
| JSON | Observed v2 envelope with `breaking: 0` and `changes: []` | Existing observed v2 findings |
| SARIF | SARIF 2.1.0 document with zero results | Existing observed SARIF findings |

Exit codes remain:

- `0` for a match;
- `1` for observed drift;
- `2` for invalid input or operational failure.

The existing JSON schema version, SARIF rules, privacy redaction, and text
output remain unchanged.

The implementation should keep formatter selection in `src/main.rs` and reuse
the existing empty-safe functions in `src/output/mod.rs`. It must not create a
second success-only JSON or SARIF renderer.

### OpenAPI 3.1 Rejection

The current typed version check accepts every `3.x` string and runs only after
typed deserialization. Some 3.1 documents therefore fail first with misleading
schema parse errors.

Raw OpenAPI preflight will:

1. Parse the top-level JSON or YAML value before typed `openapiv3`
   deserialization.
2. Read the top-level `openapi` string.
3. Accept `3.0.x`.
4. Reject `3.1.x` with `OpenAPI 3.1 is not yet supported`.
5. Reject other versions with an error stating that OpenAPI 3.0 is required.
6. Preserve sanitized location-only context for remote parse failures.

The existing raw path validation already parses an untyped document. The
implementation should extend that preflight rather than introduce a second
untyped parse solely for version detection. The post-deserialization guard may
remain as a defensive invariant, but it must require 3.0 rather than all 3.x.

The behavior must be covered through the public `diff`, `lock`, and `verify`
commands so every loader path is protected.

### Minimum Supported Rust Version

Rust 1.85 is the candidate MSRV because the committed lockfile and dependency
graph require edition-2024-aware Cargo.

Before declaring the floor:

1. Install or select Rust 1.85.
2. Run `cargo +1.85 check --locked`.
3. If the command fails because the project itself requires a newer compiler,
   stop and revise the design rather than declaring an unverified MSRV.

After verification:

- add `rust-version = "1.85"` to `Cargo.toml`;
- allow Cargo to update package metadata in `Cargo.lock`;
- document Rust 1.85 for source-based installation;
- add a dedicated `msrv` CI job using exactly Rust 1.85 and
  `cargo check --locked`;
- retain stable CI for formatting, Clippy, tests, and the Action smoke test.

The MSRV job exists to fail when source or dependency changes raise the real
floor without an explicit project decision.

## Slice 2: Compatibility Smoke Suite

### Corpus Manifest

`compat/specs.json` will record the five audited real-world documents:

- GitHub REST;
- Asana;
- Box;
- Stripe;
- DigitalOcean.

Each entry contains:

- a stable name;
- a raw URL pinned to an immutable upstream commit;
- an expected SHA-256;
- a maximum byte size;
- `passing` or `known_failing` status;
- the expected stable error fragment for a known failure.

No mutable `main` or `master` URL is allowed. Exact commit identifiers and
hashes are empirical manifest data gathered and verified during
implementation.

### Fetching and Storage

`scripts/fetch-compat-specs.py` will use only the Python 3 standard library to:

1. Read the manifest.
2. Download into `.compat-cache/`.
3. Stream content while enforcing a 50 MiB per-file limit and a 100 MiB total
   corpus limit.
4. Calculate SHA-256 while downloading.
5. Reject a hash or size mismatch.
6. Replace a cached file atomically only after validation succeeds.
7. Reuse an existing file only after verifying its hash.

`.compat-cache/` is gitignored. Upstream specifications are not committed.
Python 3 is an explicit developer prerequisite only for refreshing or running
the optional compatibility corpus; it is not a runtime dependency of
APIWatch.

Errors identify the corpus entry and failure class without printing downloaded
document content.

### Compatibility Tests

`tests/compat.rs` will contain five `#[ignore]` integration tests invoked with:

```text
cargo test --test compat -- --ignored --nocapture
```

Tests exercise the public CLI against local cached files:

- GitHub, Asana, and Box must parse, and diffing each document against itself
  must exit `0` with no changes.
- Stripe must fail with the pinned circular-schema error class.
- DigitalOcean must fail with the pinned irrelevant-metadata parse error
  class.

A known-failure test passes only when the expected failure is reproduced. If
the document begins parsing successfully or fails differently, the test fails
and requires human reclassification.

Normal `cargo test` remains offline and reports these tests as ignored.

### Compatibility CI

A dedicated `compat` job will:

1. Check out the repository.
2. Install stable Rust and Python 3.
3. Restore `.compat-cache/` using a key derived from `compat/specs.json`.
4. Run the fetcher, which revalidates cached hashes.
5. Run the ignored compatibility tests.

Network, HTTP, size, and hash failures are infrastructure failures. Parser
expectation mismatches are test failures. Neither is silently skipped.

## Slice 3: Honesty and Release Preparation

### Known-Limitations Register

The README will contain a compact register for all audited D-01 through D-19
limitations, grouped as:

- diff false negatives: D-01 through D-06;
- diff false positives and model/severity limitations: D-07 through D-11;
- OpenAPI parsing and reference limitations: D-12 through D-15;
- route-only declared locking and Verify: D-16;
- observed confidence and input limitations: D-17 through D-19.

Each group links to the roadmap phase that corrects it. The register describes
user-visible consequences in plain language rather than implying that an
internal defect identifier is sufficient explanation.

The OpenAPI 3.1 entry changes after Slice 1 from "accepted but fails
misleadingly" to "explicitly rejected until Phase 3."

### v0.7.0 Metadata

The release-preparation commit will:

- set the crate version to `0.7.0`;
- update `Cargo.lock`;
- add a dated v0.7.0 changelog section;
- document observed JSON recording, merging, maps, value-free verification,
  formatter parity, explicit OpenAPI 3.1 rejection, MSRV, and compatibility
  smoke testing;
- update README release status and source-install requirements;
- keep future Phase 1+ behavior labeled as planned.

`action.yml` will be reviewed for accurate current wording. It changes only if
its metadata or behavior claims are stale; Phase 0 does not convert it to
binary distribution.

Homebrew and Scoop remain pinned to v0.6.0 in the release-preparation commit
because the v0.7.0 GitHub tag archive does not exist yet and its checksum
cannot be known.

### Release Verification

The release candidate must pass, from the repository root:

```text
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo +1.85 check --locked
cargo build --release --locked
python scripts/fetch-compat-specs.py
cargo test --test compat -- --ignored --nocapture
```

A clean-install smoke test will install the release candidate into a temporary
prefix and exercise:

- `apiwatch --version`;
- declared `diff`, `lock`, and `verify`;
- observed `record`, `--merge`, `--map-at`, and `verify`;
- matching observed JSON and SARIF output;
- exit codes `0`, `1`, and `2`.

The smoke test must not overwrite a developer's global Cargo installation,
existing lockfiles, or package-manager state.

## Slice 4: Approval-Gated Publication

After Slice 3 passes, work stops for human review. The handoff includes:

- the release commit and branch;
- all verification results;
- the five-spec compatibility result;
- proposed annotated `v0.7.0` tag message;
- release notes;
- exact commands that would push `main` and the tag.

No push or public tag is authorized by approval of this design alone.

After explicit publication approval:

1. Merge the release branch into local `main`.
2. Re-run the release verification on merged `main`.
3. Push `main`.
4. Create and push annotated tag `v0.7.0`.
5. Download the public v0.7.0 tag archive.
6. Calculate its SHA-256.
7. Update `Formula/apiwatch.rb` and `Scoop/apiwatch.json`, including version,
   URL, hash, and Scoop `extract_dir`.
8. Validate package definitions to the extent supported by the current
   platform.
9. Commit the post-tag packaging update.
10. Push that packaging commit only when the publication approval explicitly
    includes it; otherwise stop for a second approval.

The package-manager update is necessarily post-tag. A checksum of a generated
tag archive cannot be embedded inside the same archive whose bytes determine
that checksum.

## Testing Strategy

Implementation follows test-driven development:

1. Add matching observed JSON and SARIF integration tests and confirm they fail
   by receiving plain text.
2. Add OpenAPI 3.1 CLI tests and confirm they fail with the current misleading
   parse path or acceptance.
3. Apply the minimal implementation for each behavior.
4. Run focused tests, then the complete suite.
5. Verify Rust 1.85 before committing the MSRV declaration.
6. Add compatibility expectations only after each pinned document and hash is
   independently obtained.

No existing assertion is silently rewritten. If a current test encodes
behavior that conflicts with this design, implementation stops for review.

## Error and Safety Boundaries

- Observed Verify never prints captured values or dynamic map keys.
- Remote OpenAPI errors retain sanitized context.
- Compatibility downloads never become CLI runtime behavior.
- Mutable upstream URLs are rejected from the corpus.
- Hash mismatch never falls back to unverified content.
- Compatibility cache writes are atomic.
- Release smoke tests use temporary paths.
- Public pushes and tags require a separate explicit approval.
- No existing public tag is moved or rewritten.

## Non-Goals

Phase 0 does not include:

- lockfile v3 or full-contract declared Verify;
- semantic diff fixes D-01 through D-11;
- OpenAPI 3.1 implementation;
- recursive or external reference support;
- tolerant metadata parsing;
- observation-aware requiredness or coverage;
- HAR, live, or proxy recording;
- crates.io publication;
- prebuilt release binaries;
- Docker distribution;
- Homebrew tap or Scoop bucket creation;
- migration away from deprecated `serde_yaml`.

## Exit Criterion

Repository work is release-ready when:

- observed matching output honors text, JSON, and SARIF;
- OpenAPI 3.1 fails with the intentional unsupported message;
- Rust 1.85 is declared and checked;
- the compatibility job reproduces three passing and two known-failing specs;
- all audited limitations are visible in the README;
- all release verification and clean-install smoke commands pass.

Phase 0 is fully released only after the user approves publication, v0.7.0 is
publicly tagged, package definitions contain the verified public archive hash,
and the documented installation and observed-contract workflow has been
checked against the released artifact.
