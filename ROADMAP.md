# APIWatch Roadmap

> Vision: **Prevent production outages caused by changes in third-party APIs.**

`oasdiff` diffs specs you own. **APIWatch locks APIs you don't.**

APIWatch makes external API expectations explicit, reviewable, and verifiable
in CI. It supports two sources of contract evidence:

- **Declared contracts** come from usable OpenAPI documents.
- **Observed contracts** infer value-free response structure when a
  specification is absent, incomplete, or unreliable.

The product is designed for a global, cross-industry audience. Its fixtures,
examples, and compatibility corpus should represent varied APIs rather than a
single country, industry, or design-partner project.

## Current State

The latest tagged release is v0.9.0, which bundles Phases 1 and 2. It includes
v3 and v4 declared lockfiles with the complete comparison model, full-contract
declared Verify, versioned observed JSON contracts, JSON and SARIF output, a
reusable GitHub Action, and source-building Homebrew and Scoop definitions.

Important limitations remain:

- declared v1 and v2 locks remain route-only, v3 locks have partial Phase 2
  coverage, and all need original sources to migrate to current v4;
- OpenAPI 3.1 and external or multi-file references are not supported;
- real-world specification compatibility and binary distribution are
  incomplete;
- observed contracts prove sampled structure, not complete runtime coverage.

## Milestone Transition

The former v0.6.5 roadmap was an unreleased planning milestone. It is retired
as a delivery sequence. Completed observed-contract and `--map-at` work is
preserved and carried into the planned v0.7.0 release.

Remaining v0.6.5 tasks move according to product dependency:

- output correctness, release integrity, and honest documentation move into
  Phase 0;
- observed confidence and coverage move into Phase 4;
- HAR and live recording move into Phase 5;
- proxy or passive runtime capture moves to post-v1.

New observed features remain frozen while Phases 0–3 establish a trustworthy
declared-contract foundation. No completed observed work is discarded.

Roadmap phases specify dependency order. Target release numbers communicate
intent but may change without reordering the phases.

## Phase 0 — Stabilize and Release Honestly

**Target:** v0.7.0

**Goal:** make the existing product installable, internally aligned, and
accurately represented before adding features.

### Ordered Scope

1. Align `Cargo.toml`, the changelog, README, Homebrew formula, Scoop manifest,
   and release tag (D-20).
2. Complete observed Verify output parity so successful and failing results
   honor text, JSON, and SARIF consistently.
3. Declare the minimum supported Rust version and add pinned-MSRV CI (D-22).
4. Reject OpenAPI 3.1 with an accurate unsupported-version error until Phase 3
   implements it (D-12).
5. Add a compatibility smoke suite using varied real-world public
   specifications. Track expected failures explicitly (D-13, D-14).
6. Add prominent known limitations and verify every documented command from a
   fresh installation.

### Excluded From This Phase

- lockfile v3;
- diff-engine semantic fixes;
- observed coverage or capture features.

### Exit Criterion

A new user can install the tagged release, run every documented command, and
understand exactly what APIWatch does and does not verify.

## Phase 1 — Make `verify` Meaningful

**Target:** v0.8.0

**Goal:** make a declared API lock contain enough information to detect the
contract changes APIWatch promises to catch.

### Ordered Scope

1. **Completed:** Prototype full-contract lock sizes on small and large public
   specifications. The measured recommendation is `deduplicated_yaml`; see the
   [human-readable](docs/benchmarks/phase-1-lock-size-report.md) and
   [machine-readable](docs/benchmarks/phase-1-lock-size-report.json) reports.
2. **Completed:** Design and approve the exact lockfile v3 schema (D-16).
3. **Completed:** Target a **5 MB default ceiling per upstream API** committed to Git, with
   explicit endpoint scoping for larger APIs.
4. **Completed:** Serialize complete normalized declared contracts with explicit provenance
   while excluding examples, defaults, credentials, and other sensitive
   source values.
5. **Completed:** Make declared Verify deserialize the locked contract and call the same
   `diff_contracts` comparison path used by `diff`.
6. **Completed:** Extend Verify findings with severity and messages matching diff findings.
7. **Completed:** Update text, JSON, and SARIF renderers plus the reusable Action.
8. **Completed:** Keep v1 and v2 readable with a clear route-only warning. Require re-locking
   from the original source because legacy files cannot reconstruct contract
   data they never stored.
9. **Completed:** Provide a deliberate migration workflow after the v3 format is approved.

The approved v3 YAML representation, canonical digests, endpoint scoping,
atomic update workflow, and legacy migration policy are implemented.

### Excluded From This Phase

- repairing unrelated comparison semantics;
- changing observed requiredness or adding traffic capture;
- silently expanding legacy route-only locks.

### Exit Criterion

**Met:** the [D-16 fixtures](testdata/openapi/v3_d16_old.yaml) and
[Verify regression](tests/cli_verify.rs) cover an authentication change,
parameter rename/retype, and successful-response removal. Declared Verify
exits `1` with four correctly classified breaking findings.

## Phase 2 — Make the Comparison Engine Trustworthy

**Target:** v0.9.0

**Goal:** eliminate the confirmed cases where semantic diffing misses real
breakage or reports harmless changes as breaking.

Every defect receives a failing regression fixture before its fix.

### P0: False Negatives

1. **Completed:** Request-body addition and removal (D-01).
2. **Completed:** Content-type addition and removal (D-02).
3. **Completed:** Response requiredness and its directional symmetry (D-03).
4. **Completed:** Schema format comparison (D-05).
5. **Completed:** `additionalProperties` semantics (D-04).
6. **Completed:** Server changes (D-06).

### P0: False Positives

1. **Completed:** Correct `allOf` merging and order-independent `oneOf`/`anyOf` set
   comparison (D-09).
2. **Completed:** Path-template normalization (D-07).
3. **Completed:** Authentication identity matching (D-08).

### P1 Refinements

1. **Completed:** First-class array items (D-10).
2. **Completed:** Enum-severity refinement (D-11).

### Excluded From This Phase

- new configuration surfaces beyond what a fix requires;
- observed capture features;
- new protocol support.

### Exit Criterion

**Met:** every Category A audit reproduction produces its documented expected
result, one regression fixture exists per D-01 through D-11 defect, the
production v4 payload fits the default ceiling for every normalizable pinned
corpus entry, and the complete local phase gate passes.

## Phase 3 — Real-World Compatibility

**Target:** v0.10.0

**Goal:** make declared contracts work against the specifications and delivery
patterns users encounter outside controlled fixtures.

### Ordered Scope

1. Represent cycle-breaking references safely (D-14).
2. Ignore malformed metadata that the normalizer does not consume (D-13).
3. Resolve external and multi-file `$ref` targets with path-traversal and
   remote-input protections (D-15).
4. Implement OpenAPI 3.1, including nullable type arrays (D-12).
5. Replace deprecated `serde_yaml` (D-23).
6. Add `.apiwatch.yaml` configuration for ignore rules, severity overrides,
   and `--fail-on` thresholds (D-11).
7. Add secure remote authentication headers with environment interpolation;
   never log or persist their values (D-19 adjacent).
8. Grow a globally representative compatibility corpus across API sizes,
   styles, and industries.

### Excluded From This Phase

- supporting GraphQL, gRPC, or AsyncAPI;
- becoming a general-purpose API testing framework;
- observed traffic capture.

### Exit Criterion

The compatibility suite passes, an OpenAPI 3.1 nullable-type fixture diffs
correctly, and a split specification resolves `./schemas.yaml#/User` safely.

## Phase 4 — Trustworthy Observed Contracts

**Target:** v0.11.0

**Goal:** make the confidence and boundaries of inferred response shapes
explicit enough for reliable CI use.

### Ordered Scope

1. Fix nullable and underdetermined observed shapes (D-17).
2. Add observation-count-aware requiredness and an explicit
   `--required-threshold` policy (D-18).
3. Define empty-array and empty-object evolution without premature drift.
4. Store deterministic confidence metadata such as observation counts and
   first/last-seen timestamps at the appropriate shape boundaries.
5. Report verified, insufficiently observed, and unverified structure
   distinctly.
6. Preserve explicit-only `--map-at` semantics. Suggestions may be explored
   later, but APIWatch must never silently convert an object into a map.
7. Publish a structure-only privacy threat model.
8. Add property tests for round-trips, determinism, order invariance, and the
   absence of captured values.

### Excluded From This Phase

- HAR or live capture;
- proxy operation;
- enum inference without a separate privacy review.

### Exit Criterion

Users can distinguish verified structure from insufficient evidence, repeated
input produces byte-identical locks, and locks and diagnostics contain no
captured scalar values, credentials, or dynamic map keys.

## Phase 5 — Frictionless Recording and CI Adoption

**Target:** v0.12.0

**Goal:** let users adopt observed contracts from real traffic without writing
an OpenAPI document or application harness.

### Ordered Scope

1. Add HAR import as the highest-priority adoption feature, with explicit URL
   or operation selection and deterministic multi-sample merging.
2. Exclude malformed, binary, and non-JSON bodies with honest coverage output.
3. Add live URL recording while reporting each response as one observation,
   never as proof of complete coverage.
4. Key observed entries by method and path so one lock can represent an
   upstream API.
5. Add multi-entry Verify.
6. Add `apiwatch init` to scaffold a lock and CI workflow.
7. Add endpoint and field coverage commands.
8. Provide global, industry-neutral onboarding and examples.

### Excluded From This Phase

- passive proxying;
- background daemons;
- static discovery of API calls in source.

### Exit Criterion

A user can import real traffic from an undocumented third-party API, commit a
value-free lock, and have CI fail when a recorded response shape drifts.

## Continuous Distribution Track

Distribution starts in Phase 0 and improves alongside every product phase:

1. Publish and verify `cargo install apiwatch`.
2. Produce checksummed binaries for supported Linux, macOS, and Windows
   targets.
3. Make the GitHub Action download a release binary, with a documented
   source-build fallback.
4. Publish a minimal container image for non-GitHub CI.
5. Maintain a Homebrew tap and Scoop bucket.
6. Automate tag-driven artifacts, checksums, crates.io publishing, and package
   manager updates.
7. Report both SemVer and Git revision from `apiwatch --version`.

Distribution failures block the release whose functionality they prevent
users from installing; distribution is not a final cleanup project.

## Phase 6 — v1 Stabilization and Adoption

**Target:** v1.0.0

**Goal:** make the proven REST contract workflow stable enough for long-term
automation.

### Scope

- stable lockfile and SemVer guarantees;
- migration documentation and compatibility tests;
- a 15–20-spec real-world compatibility corpus;
- parser fuzzing for untrusted inputs;
- performance budgets and regression gates;
- deterministic output snapshots;
- mature release automation and install verification.

A plugin system is not a v1 requirement. It enters the roadmap only if real
users demonstrate a need that cannot be served by stable CLI and file
interfaces.

### Exit Criterion

APIWatch can lock and verify declared and observed REST contracts with
documented compatibility, migration, privacy, performance, and release
guarantees.

## Engineering Quality Gates

These apply to every phase:

1. Reproduce each defect before fixing it.
2. Add a regression fixture before changing behavior.
3. Keep `diff` and declared Verify on one `diff_contracts` comparison path.
4. Preserve deterministic ordering and byte-stable lock output.
5. Keep Verify read-only.
6. Never retain observed values, credentials, or dynamic map keys.
7. Report probabilistic observed coverage honestly.
8. Keep documentation accurate for the tagged release.
9. Do not start a phase until its predecessor's exit criterion is met.

## Post-v1 Exploration

These require separate design and security reviews:

- consent-based proxy or passive runtime capture;
- framework-specific handler-shape analysis;
- confirmed map-suggestion assistance;
- a plugin system justified by demonstrated integrations.

## Explicit Non-Goals

APIWatch will not build the following as part of the current roadmap:

- dashboards, web interfaces, or hosted services;
- accounts, billing, or a cloud backend;
- static source scanning for API calls;
- a general API testing framework;
- mock server or SDK generation;
- GraphQL, gRPC, or AsyncAPI support before the REST product is stable;
- AI-powered contract decisions;
- a feature-for-feature replacement for mature general OpenAPI diff tools.
