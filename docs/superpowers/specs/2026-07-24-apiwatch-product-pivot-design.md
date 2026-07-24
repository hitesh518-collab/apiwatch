# APIWatch Product Pivot Design

**Date:** 2026-07-24

**Status:** Approved for documentation implementation

## Decision

APIWatch will pivot from competing as a general OpenAPI differ to protecting
applications from changes in third-party APIs they do not control.

Declared OpenAPI contracts remain essential infrastructure. They provide
deterministic contracts when a usable specification exists. Observed contracts
provide APIWatch's primary differentiation when a specification is missing,
incomplete, or unreliable.

The product is intended for a global, cross-industry audience. Documentation,
fixtures, and compatibility testing must not assume an Indian or fintech-only
user base.

## Positioning

The core product statement is:

> `oasdiff` diffs specs you own. APIWatch locks APIs you don't.

This statement defines the product boundary rather than a promise that
APIWatch will replace every OpenAPI comparison tool. APIWatch should make
third-party API dependencies explicit, reviewable, and verifiable in CI.

The product must be honest about the difference between:

- a declared contract derived from an OpenAPI document;
- an observed contract inferred from response samples;
- verified structure;
- insufficiently observed or unverified structure.

## Current Milestone Transition

The unreleased v0.6.5 planning milestone is retired. Its completed observed
JSON recording, merging, verification, privacy protections, and explicit
`--map-at` support are preserved and carried into v0.7.0.

Remaining v0.6.5 work is not completed in the old order. Output gaps and
release blockers that affect honest existing behavior move into Phase 0.
Coverage, HAR capture, live recording, and other observed-contract expansion
move behind declared-contract verification and diff-engine correctness.

No existing observed-contract work should be deleted merely because its
roadmap position changed.

## Roadmap Structure

Roadmap phase numbers describe dependency order. Release numbers are target
milestones and may be adjusted without changing the phase order.

### Phase 0 — Stabilize and release honestly (`v0.7.0`)

Align release metadata and installation documentation, declare and test the
minimum supported Rust version, reject unsupported OpenAPI 3.1 documents with
an accurate error, add a real-world compatibility smoke suite, and publish
known limitations.

Exit criterion: a new user can install APIWatch, run the documented commands,
and understand exactly what the release does and does not verify.

### Phase 1 — Make `verify` meaningful (`v0.8.0`)

Design and implement lockfile v3, store complete declared contracts, and make
`verify` use the same comparison engine as `diff`. Extend all output formats
and the GitHub Action to report complete drift findings.

The breaking v3 format is approved. Version 1 and version 2 locks remain
readable for migration, but users must re-lock from the original source to
recover contract data those formats never stored.

The v3 design must prototype real lock sizes before fixing the schema. The
default design ceiling is 5 MB per upstream API committed to Git, with
explicit endpoint scoping for larger APIs. The final v3 representation remains
a Phase 1 design decision.

Exit criterion: authentication changes, parameter changes, schema drift, and
removed responses cause `verify` to fail with correctly classified findings.

### Phase 2 — Make the comparison engine trustworthy (`v0.9.0`)

Fix confirmed false negatives before false positives, then complete lower-risk
semantic refinements. Every audit defect must first receive a failing
regression fixture.

Exit criterion: every audit reproduction produces its documented expected
result and the complete test suite passes.

### Phase 3 — Real-world compatibility (`v0.10.0`)

Add cycle-safe and external references, tolerant parsing, OpenAPI 3.1 support,
a maintained YAML parser, configurable severity and ignore rules, failure
thresholds, and secure authentication headers for remote verification. Expand
the compatibility corpus across industries and API styles.

Exit criterion: the compatibility suite passes, including OpenAPI 3.1 and
split specifications.

### Phase 4 — Trustworthy observed contracts (`v0.11.0`)

Resume observed-contract expansion with nullable and underdetermined shapes,
sample-aware requiredness, empty-collection semantics, confidence metadata,
coverage reporting, and a documented structure-only privacy threat model.
Explicit `--map-at` annotations remain the safe default; maps are not silently
inferred.

Exit criterion: users can distinguish verified structure from insufficiently
observed structure, and locks and diagnostics contain no captured values.

### Phase 5 — Frictionless recording and CI adoption (`v0.12.0`)

Prioritize HAR import, then add honest live recording, multi-entry
verification, method-and-path entry identity, `apiwatch init`, and endpoint
and field coverage commands. Examples and onboarding target a global,
cross-industry audience.

Exit criterion: a user can record an undocumented third-party API from real
traffic, commit a safe lock, and detect shape drift in CI without writing an
OpenAPI document.

### Continuous Distribution Track

Distribution begins in Phase 0 and advances alongside product phases:
crates.io, prebuilt cross-platform binaries, a fast binary-based GitHub
Action, a Docker image, Homebrew and Scoop distribution, automated release
updates, and unambiguous version metadata.

### Phase 6 — v1 Stabilization and Adoption

Stabilize the lockfile and SemVer promises, grow the compatibility corpus to
15–20 varied real-world APIs, enforce performance and fuzzing gates, document
migrations, and finish release automation.

A plugin system is not a v1 requirement unless demonstrated user demand
justifies it.

## Scope Boundaries

The following remain post-v1 or out of scope:

- proxy or passive runtime capture;
- static source scanning;
- dashboards, hosted services, accounts, billing, or cloud backends;
- general API testing, mock generation, or SDK generation;
- GraphQL, gRPC, or AsyncAPI support before the REST product is stable;
- AI-powered contract decisions;
- competing with mature tools as a general-purpose OpenAPI differ.

Proxy capture is deferred because it materially expands the product's
security, consent, performance, and operational surface.

## Documentation Architecture

`ROADMAP.md` becomes the single authoritative roadmap. Other public documents
link to it instead of maintaining competing phase lists.

The documentation implementation will:

- create `ROADMAP.md` with phase tasks and exit criteria;
- update `README.md` with the new positioning, accurate current status, and
  known limitations;
- update `IDEA.md` with the third-party API dependency problem;
- update `DESIGN.md` with declared and observed contract roles;
- update `docs/lockfile-spec.md` with the approved v3 goals and migration
  policy while leaving the exact v3 schema undecided;
- update `docs/change-rules.md` to separate intended rules from validated
  current behavior;
- preserve historical specifications and plans unchanged.

This documentation task will not change source code, release versions,
formulas, manifests, or CI.

## Quality and Safety Rules

- Correctness and deterministic output take priority over feature breadth.
- A defect is reproduced and covered by a failing regression test before it
  is fixed.
- `diff` and declared `verify` share one comparison engine.
- Observed coverage is probabilistic and must never be presented as complete
  without evidence.
- Locks and diagnostics never retain captured values, credentials, or dynamic
  map keys.
- New roadmap work begins only after the preceding phase exit criterion is
  met.

## Resolved Decisions

- The product pivot is approved.
- A breaking lockfile v3 is approved.
- Version 1 and version 2 migration requires re-locking for full contracts.
- The lockfile design target is a 5 MB default ceiling with endpoint scoping.
- The target audience is global and cross-industry.
- Correctness-first sequencing is approved.
- Distribution is continuous rather than a final cleanup phase.

## Decisions Deferred to Their Phase

- The exact lockfile v3 schema and canonical digest representation.
- The precise endpoint-scoping CLI.
- Default severity for response enum widening.
- Whether confirmed map suggestions are useful enough to supplement explicit
  annotations.
- Whether user demand justifies plugins or proxy capture after v1.
