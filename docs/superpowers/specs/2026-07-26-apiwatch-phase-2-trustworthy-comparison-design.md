# APIWatch Phase 2 Trustworthy Comparison Design

**Date:** 2026-07-26

**Status:** Approved

## Goal

Make APIWatch's declared-contract comparison engine trustworthy for the eleven
audited Phase 2 defects while preserving correct existing behavior and keeping
direct `diff` and locked `verify` on one semantic comparison path.

Phase 2 introduces a complete normalized contract model and lockfile version 4
before applying the defect fixes in roadmap order. Every defect begins with a
failing regression fixture, and every stored semantic is verified through the
same `diff_contracts` function used by direct comparison.

## Design Principles

- Prefer false-negative fixes before false-positive and policy refinements.
- Preserve correct diagnostic wording, ordering, JSON structure, SARIF rules,
  and fingerprints.
- Change public output only when a corrected semantic or honest coverage
  statement requires it.
- Normalize source syntax into explicit semantic identities before comparing.
- Use deterministic structural identities rather than declaration order or
  component labels.
- Never infer semantics absent from an older lock.
- Preserve atomic writes, deterministic bytes, the 5,242,880-byte default
  declared-entry ceiling, and the established privacy boundary.
- Keep new configuration, observed-contract work, and protocol expansion out
  of Phase 2.

## Selected Architecture

Phase 2 uses a contract-model-first v4 foundation:

```text
OpenAPI 3.0 source
  -> semantic normalization
  -> normalized ApiContract
  -> diff_contracts(old, new)
  -> existing text / JSON / SARIF renderers
```

Both direct `diff` inputs and v4 locked contracts reconstruct the same
`ApiContract`. Comparison corrections remain in normalization and
`diff_contracts`; the output renderers continue consuming ordered `Change`
records.

The normalized model gains explicit representations for:

- request-body presence and requiredness;
- canonical request and response media types;
- effective operation server templates;
- first-class array items;
- `additionalProperties` policies;
- canonical composition branches;
- semantic authentication identity;
- canonical path-template identity plus a diagnostic display path.

This approach was selected over evolving the wire format defect by defect,
which would create repeated schema churn, and over fixing direct diff first,
which would make `diff` and `verify` disagree again.

## Normalized Contract Components

### Operation Identity and Display

An operation has a canonical identity and a display path. Canonical identity
uses the HTTP method and positional path placeholders:

```text
GET /users/{userId}/orders/{orderId}
-> GET /users/{0}/orders/{1}
```

Placeholder names do not participate in endpoint identity. Path parameters
bind to their canonical placeholder position; query, header, and cookie
parameters retain name-based identity. Diagnostics use the relevant original
normalized display path: the old path for removals, the new path for
additions, and the new path for matched-operation changes.

Two operations in one document that collapse to the same canonical identity
are rejected as ambiguous input. Scoped operation selectors are normalized
through the same identity function, so a placeholder rename does not make a
scoped operation disappear.

### Request Bodies and Media Types

`RequestBody` stores its OpenAPI `required` value and a canonical media-type
map. Missing `required` means `false`.

Media types are canonicalized by lowercasing the case-insensitive type,
subtype, and parameter names and sorting parameters. Parameter values remain
semantically significant but must pass the same control-character and privacy
validation as other stored strings.

### Schemas

`Schema` retains kind, nullability, format, enum values, and object properties,
and adds:

- `items: Option<Box<Schema>>` for arrays;
- `additional_properties`, represented as forbidden, unconstrained, or a
  schema-constrained value;
- canonical structural branch sets for `oneOf` and `anyOf`.

An omitted OpenAPI 3.0 `additionalProperties` value normalizes to
unconstrained. A schema-valued policy recursively uses the normal schema
model.

`allOf` does not remain an index-addressed branch list. Normalization
recursively intersects supported constraints into one effective schema:

- properties are combined;
- requiredness is the union of required property names;
- compatible enum constraints are intersected;
- nullability is narrowed;
- compatible item and additional-property schemas merge recursively;
- kind and format constraints must be compatible.

Structurally contradictory or unsupported intersections are rejected as
ambiguous input rather than guessed. `oneOf` and `anyOf` branches retain their
kind but use deterministic, duplicate-free structural identities. Phase 2
does not attempt general JSON Schema logical-equivalence, subsumption, or
overlap proofs.

First-class array traversal retains the established diagnostic display path,
such as `items.name`, even though `items` is no longer a synthetic property.

### Authentication Identity

Authentication component keys are display labels, not semantic identity.
Requirements match by normalized wire-relevant identity:

- API keys use location and transmitted parameter name;
- HTTP authentication uses the normalized HTTP scheme;
- OAuth2 uses normalized flow kinds and safe endpoint-template identity;
- OpenID Connect uses safe discovery-template identity;
- scopes remain requirement-level sets.

Authentication endpoint templates use the server-template sanitizer: user-info
is rejected, while descriptions, literal query values, and variable defaults
are excluded.

Equivalent component renames therefore produce no finding. Two declarations
that collapse to one semantic identity but carry different requirements are
rejected as ambiguous. Existing requirement-addition, removal, type, and
scope directionality remains unchanged.

### Effective Servers

Each operation stores the effective server set after applying OpenAPI
precedence:

1. operation-level servers;
2. otherwise path-level servers;
3. otherwise root-level servers;
4. otherwise the OpenAPI default relative server.

Server declaration order is irrelevant. Canonical server templates retain
scheme, host, port, path, placeholders, and query-key structure. URL user-info
is rejected. Literal query values, descriptions, and server-variable defaults
are excluded. This detects origin, port, base-path, placeholder, and query-key
changes without persisting arbitrary defaults or credentials.

Phase 2 does not claim coverage for changes that occur only in excluded
server-variable defaults or literal query values.

## Semantic Classification

### D-01: Request-Body Addition and Removal

| Change | Severity |
|---|---|
| Required request body added | Breaking |
| Optional request body added | Non-breaking |
| Request body removed | Breaking |
| Optional body becomes required | Breaking |
| Required body becomes optional | Non-breaking |

A body removal is breaking because an existing consumer may still send the
previously accepted body, even when it was optional.

### D-02: Content-Type Addition and Removal

| Change | Severity |
|---|---|
| Request media type removed | Breaking |
| Request media type added | Non-breaking |
| Response media type removed | Breaking |
| Response media type added | Breaking |

A response addition is breaking because it expands the set of representations
a consumer may receive. Phase 3 severity configuration may allow projects to
downgrade that policy later.

### D-03: Response Requiredness

| Change | Severity |
|---|---|
| Required response property becomes optional | Breaking |
| Optional response property becomes required | Non-breaking |

Request requiredness retains its existing inverse direction. Both directions
receive explicit regression coverage.

### D-05: Schema Formats

Any exact format change, including addition or removal of a format, is a
warning. Formats are open strings in OpenAPI 3.0 and are compared exactly
after input validation.

### D-04: `additionalProperties`

Compatibility follows data-flow direction:

- a request schema must not narrow the inputs the provider accepts;
- a response schema must not broaden the outputs a consumer may receive.

Therefore, unconstrained-to-forbidden is breaking for requests, while
forbidden-to-unconstrained is breaking for responses. The inverse changes are
non-breaking. Schema-constrained transitions recurse through the same
request- or response-oriented schema comparison. A change between
unconstrained and schema-constrained is classified as narrowing or broadening
according to usage.

### D-06: Server Changes

Removing an effective server template is breaking. Adding a template is
non-breaking. Replacing a template produces one removal and one addition.
Declaration reordering produces no finding.

### D-09: Composition

Equivalent `allOf` schemas normalize to the same effective schema regardless
of branch order. `oneOf` and `anyOf` compare canonical branch sets:

| Change | Request usage | Response usage |
|---|---|---|
| Branch added | Non-breaking | Breaking |
| Branch removed | Breaking | Non-breaking |

Pure reordering and duplicate equivalent branches produce no finding.
Structural changes within a matched schema continue to produce focused
property, type, format, enum, item, and requiredness diagnostics.

### D-07: Path Templates

Renaming corresponding placeholders produces no endpoint or parameter
finding. Moving, adding, or removing a placeholder changes canonical route
identity and retains the existing endpoint addition/removal behavior.

### D-08: Authentication

Requirements first match by semantic identity, so an equivalent component
rename produces no finding. Remaining requirements with the same component
label match as an identity/type change and use the existing type-change
diagnostic. Other unmatched old and new identities use the existing removal
and addition diagnostics. Scope changes on a semantic-identity match retain
their current directional severities.

### D-10: Arrays

Array items are first-class schema edges. Item kind, format, nullability,
enum, composition, object properties, nested arrays, and
`additionalProperties` recurse normally. Diagnostic paths remain compatible
with the current `items` wording.

### D-11: Enum Severity

The consumer-oriented matrix becomes the stable Phase 2 policy:

| Change | Severity |
|---|---|
| Request enum value added | Non-breaking |
| Request enum value removed | Breaking |
| Response enum value added | Breaking |
| Response enum value removed | Non-breaking |

User-configurable severity overrides remain Phase 3 work.

## Lockfile Version 4

Version 4 extends the deterministic, content-addressed v3 representation
rather than mutating v3 semantics in place.

Operations store:

- canonical operation key and display path;
- effective server templates;
- semantic authentication identities and display labels;
- parameters;
- request-body requiredness and canonical content;
- responses and canonical content.

Schemas store:

- kind, nullability, format, and canonical enum values;
- object properties and requiredness;
- first-class item references;
- explicit `additionalProperties`;
- canonical `oneOf` and `anyOf` branch references.

Schema IDs and contract digests advance to distinct v4 domain separators.
Every stored reference must exist and be reachable. Schema collisions,
orphans, invalid identities, altered byte counts, and altered digests remain
hard errors. Canonical maps and sets make render output independent of source
declaration order.

The existing maximum measures the final standalone serialized contract
payload, including its final newline. The default remains 5,242,880 bytes per
declared API, with exact operation scoping available for larger contracts.

## Compatibility and Migration

Versions 1 through 3 remain readable:

| Lock version | Declared verification coverage |
|---|---|
| v1/v2 | `routes` |
| v3 | `partial` |
| v4 | `full` |

v3 retains all Phase 1 comparisons it can reconstruct and reports a
`phase2_relock_required` limitation. Text emits a visible warning, JSON uses
`coverage: "partial"` plus the limitation code, and SARIF emits the
corresponding tool execution notification. v3 never receives invented
defaults for semantics it did not store.

New `apiwatch lock` files use v4. `apiwatch lock ... --update` is the deliberate
v3-to-v4 migration workflow and requires the original OpenAPI source. Parsing,
normalization, scoping, size enforcement, canonicalization, and integrity
checks finish before an atomic replacement.

Observed entries are preserved. Migration is refused if other declared
entries would remain in an older representation because their source
contracts are unavailable. Failed create, update, migration, scope, size, or
integrity operations preserve existing destination bytes.

## Output Compatibility and Ordering

Existing correct messages, severity labels, JSON fields, SARIF rule IDs, and
fingerprint construction remain unchanged. New semantic cases receive focused
messages following the current vocabulary. The v3 partial-coverage statement
is the intentional compatibility change required for honest verification.

Existing comparison category and ordered-map traversal remain stable. New
set-like data uses canonical sorting. Source reordering of maps, composition
branches, server declarations, or security-scheme declarations cannot change
lock bytes or diagnostic order.

## Error Handling

The following are input or integrity errors and exit with code `2` before
comparison output or lock modification:

- canonical operation identity collisions;
- unbound or ambiguous path parameters;
- invalid or control-character-bearing stored identities;
- credential-bearing server user-info;
- contradictory or unsupported `allOf` intersections;
- ambiguous duplicate authentication identities;
- missing, cyclic, orphaned, colliding, or tampered v4 schema data;
- contract byte-count or digest mismatches.

Warnings and non-breaking findings exit `0`; breaking findings exit `1`, as
they do today.

## Regression and Acceptance Strategy

Every D-01 through D-11 defect follows this sequence:

1. Add the smallest old/new OpenAPI fixture that reproduces the audit result.
2. Add a focused normalization or `diff_contracts` regression and observe the
   expected failure.
3. Add CLI coverage for message, severity, ordering, and exit code.
4. Implement only the semantic slice needed for that defect.
5. Prove a v4 lock and Verify run produce the same `Change` records.

Directional rules are tested both ways. Equivalence regressions cover:

- reordered `allOf`, `oneOf`, and `anyOf` branches;
- renamed path placeholders;
- renamed equivalent security schemes;
- reordered server declarations.

Cross-cutting v4 tests cover:

- deterministic rendering and parse/render round trips;
- schema and contract digest tampering;
- reference reachability and digest collisions;
- canonical sorting and duplicate rejection;
- atomic v3-to-v4 migration with observed-entry preservation;
- refusal when other declared entries cannot migrate;
- v3 partial coverage in text, JSON, and SARIF;
- credential, default, and privacy-sentinel absence;
- exact scoped verification across placeholder renames;
- first-class array diagnostics retaining `items.name` wording;
- pinned-corpus payload sizes remaining below the default ceiling where the
  source is currently supported.

Phase 2 acceptance requires:

- all eleven Category A audit reproductions produce their documented result;
- one regression fixture pair exists per defect;
- direct `diff` and v4 `verify` agree;
- formatting and strict Clippy pass;
- the full workspace and Python suites pass;
- Rust 1.86 checks the locked workspace;
- the pinned compatibility suite and release smoke pass;
- deterministic report and whitespace checks pass;
- README, change rules, lockfile specification, changelog, and roadmap match
  the implemented behavior.

## Delivery Sequence

1. Freeze the semantic matrix and v4 schema.
2. Add normalized identities and v4 wire, integrity, privacy, and migration
   foundations without changing comparison behavior ahead of regressions.
3. Fix P0 false negatives in roadmap order: D-01, D-02, D-03, D-05, D-04,
   D-06.
4. Fix P0 false positives: D-09, D-07, D-08.
5. Complete P1 refinements: D-10 and D-11.
6. Run phase-wide parity, compatibility, size, documentation, and release
   readiness gates.

Each defect is a reviewable regression-first commit or commit group.
Publishing, tagging, pushing, or package-manager repinning is a separate
explicitly authorized release action.

## Excluded Scope

- New configuration files, ignore rules, severity overrides, and `--fail-on`
  thresholds.
- OpenAPI 3.1, external references, recursive reference support, or parser
  replacement.
- General logical-equivalence, subsumption, or overlap proofs for composed
  JSON Schemas.
- Server-variable default and literal query-value comparison.
- New observed-contract confidence, coverage, or capture behavior.
- New protocols, hosted services, dashboards, or plugin systems.
