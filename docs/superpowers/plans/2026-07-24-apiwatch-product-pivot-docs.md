# APIWatch Product Pivot Documentation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the approved correctness-first product pivot the single,
consistent direction presented by APIWatch's public documentation.

**Architecture:** `ROADMAP.md` becomes the authoritative source for sequencing,
while shorter project documents explain the product from their own perspective
and link back to it. Historical specifications remain immutable records.
Current capabilities, known limitations, planned capabilities, and approved
but not-yet-designed work are labeled separately.

**Tech Stack:** Markdown, Git, PowerShell, ripgrep

## Global Constraints

- The target audience is global and cross-industry.
- The unreleased v0.6.5 planning milestone is retired; completed observed
  contract work is carried into the planned v0.7.0 release.
- Correctness-first sequencing is Phase 0 stabilization, Phase 1 full-contract
  lock and Verify, Phase 2 diff correctness, Phase 3 real-world compatibility,
  Phase 4 observed trust, Phase 5 recording and adoption, then Phase 6 v1.
- Distribution is a continuous track beginning in Phase 0.
- The breaking lockfile v3 direction is approved, but its exact schema and CLI
  remain Phase 1 design work.
- Version 1 and version 2 locks cannot be upgraded into full contracts without
  re-locking from the original source.
- Lockfile v3 design targets a 5 MB default per-upstream ceiling with endpoint
  scoping for larger APIs.
- Existing observed JSON and explicit `--map-at` behavior remains supported.
- Proxy capture, static source scanning, dashboards, hosted services, general
  API testing, new API protocols, and AI contract decisions are not pre-v1
  work.
- Do not change Rust source, release versions, CI, formulas, or manifests.
- Do not rewrite historical files under `docs/superpowers/specs/` or
  `docs/superpowers/plans/`.

---

### Task 1: Establish the authoritative roadmap

**Files:**
- Create: `ROADMAP.md`

**Interfaces:**
- Consumes: Approved decisions in
  `docs/superpowers/specs/2026-07-24-apiwatch-product-pivot-design.md`
- Produces: The canonical phase sequence linked by all summary documents

- [ ] **Step 1: Write the roadmap header and transition**

Create `ROADMAP.md` with:

- the vision, "Prevent production outages caused by changes in third-party
  APIs";
- the positioning, "`oasdiff` diffs specs you own. APIWatch locks APIs you
  don't";
- a "Current State" section that distinguishes released v0.6.0 from unreleased
  observed-contract work;
- a "Milestone Transition" section retiring the v0.6.5 planning label and
  carrying completed work into planned v0.7.0;
- an explicit statement that roadmap phases are dependency order and release
  numbers are targets.

- [ ] **Step 2: Write Phases 0 through 3**

For each phase, include its target release, goal, ordered scope, excluded
scope, and exit criterion:

1. Phase 0: honest, installable v0.7.0 stabilization.
2. Phase 1: lockfile v3 and Verify through the shared diff engine.
3. Phase 2: audit-driven diff correctness, false negatives before false
   positives.
4. Phase 3: real-world parsing, references, OpenAPI 3.1, configuration, and
   secure remote authentication.

List the audit defect identifiers next to relevant work without reproducing
the audit's full defect descriptions.

- [ ] **Step 3: Write Phases 4 through 6 and distribution**

Add:

1. Phase 4: confidence-aware, privacy-preserving observed contracts.
2. Phase 5: HAR-first recording, live recording, coverage, multi-entry Verify,
   and `apiwatch init`.
3. Continuous distribution: crates.io, binaries, Action, container, package
   managers, release automation, and version provenance.
4. Phase 6: v1 stability, compatibility corpus, fuzzing, performance, and
   migration guarantees.

Move proxy/runtime capture to post-v1. Remove static discovery and a plugin
system from committed pre-v1 work.

- [ ] **Step 4: Add global quality gates and non-goals**

Document:

- regression-before-fix;
- shared `diff_contracts` comparison path;
- deterministic output;
- structure-only observed data;
- phase exit gates;
- the global audience rule;
- post-v1 and explicit non-goals.

- [ ] **Step 5: Validate the authoritative roadmap**

Run:

```powershell
rg -n "^#|v0\.6\.5|v0\.7\.0|lockfile v3|5 MB|global|HAR|proxy|AI" ROADMAP.md
```

Expected: all approved transition, audience, phase, size, adoption, and scope
decisions appear; proxy and AI appear only in excluded or post-v1 context.

- [ ] **Step 6: Commit the roadmap**

```powershell
git add ROADMAP.md
git commit -m "docs: add correctness-first product roadmap"
```

### Task 2: Reposition the public README

**Files:**
- Modify: `README.md`

**Interfaces:**
- Consumes: `ROADMAP.md`
- Produces: Accurate first-contact product positioning and capability claims

- [ ] **Step 1: Rewrite the introduction and status**

Keep "API lockfiles for external services," then explain:

- APIWatch protects consumers of APIs they do not control;
- declared contracts come from usable OpenAPI documents;
- observed contracts cover absent, incomplete, or unreliable specifications;
- released version v0.6.0 and unreleased branch capabilities are distinct;
- current declared locks are route-only and do not yet provide the Phase 1
  full-contract Verify guarantee.

- [ ] **Step 2: Preserve accurate CLI documentation**

Retain working `diff`, `lock`, `verify`, `record`, `--merge`, `--map-at`, JSON,
SARIF, Homebrew, Scoop, and GitHub Action examples. Do not document Phase 1+
commands or flags as available.

Replace "OpenAPI 3.x" claims with accurate OpenAPI 3.0 wording until Phase 3.
Ensure observed verification remains described as local JSON only.

- [ ] **Step 3: Add known limitations**

Add a prominent section covering:

- route-only declared locks and Verify;
- no OpenAPI 3.1;
- no external or multi-file references;
- known diff-engine semantic gaps tracked in Phase 2;
- partial real-world specification compatibility;
- source-build distribution and Action latency;
- observed contracts proving only sampled structure;
- no HAR, live record, coverage, or multi-entry Verify yet.

Link each limitation group to the corresponding `ROADMAP.md` phase.

- [ ] **Step 4: Replace MVP and non-goal framing**

Replace the old OpenAPI-first "MVP Scope" with "Product Direction" and link to
`ROADMAP.md`. Expand non-goals to match the approved design without claiming
post-v1 exploration as committed work.

- [ ] **Step 5: Validate README claims**

Run:

```powershell
rg -n "OpenAPI 3\.x|OpenAPI 3\.0|route-only|Known Limitations|ROADMAP|HAR|live recording|static code|AI" README.md
```

Expected: no unqualified OpenAPI 3.x support claim; current and future
capabilities are clearly separated; the roadmap and limitations are linked.

- [ ] **Step 6: Commit the README**

```powershell
git add README.md
git commit -m "docs: reposition APIWatch around external API contracts"
```

### Task 3: Align the idea and design summaries

**Files:**
- Modify: `IDEA.md`
- Modify: `DESIGN.md`

**Interfaces:**
- Consumes: `ROADMAP.md` and the pivot design record
- Produces: Concise strategic and architectural summaries with no competing
  roadmap

- [ ] **Step 1: Rewrite `IDEA.md`**

Explain the globally applicable problem:

- uptime monitoring does not detect contract drift;
- provider specifications may be unavailable or untrustworthy;
- APIWatch makes external API expectations explicit and reviewable;
- declared and observed inputs serve the same lock-and-verify workflow;
- structure-only observation is a privacy property.

End with a link to `ROADMAP.md`; do not include a separate phase list.

- [ ] **Step 2: Rewrite `DESIGN.md`**

Describe:

- Rust CLI and deterministic normalized contract model;
- declared-contract path: OpenAPI → normalized contract → lock/diff/Verify;
- observed-contract path: JSON samples → value-free shape → merge/Verify;
- the Phase 1 target that declared `verify` shares `diff_contracts`;
- formatters and stable exit-code boundaries;
- lock privacy and provenance;
- links to the original design, pivot design, lockfile spec, change rules, and
  roadmap.

Label full-contract v3 as planned, not implemented.

- [ ] **Step 3: Validate summary consistency**

Run:

```powershell
rg -n "OpenAPI-first|language scanners|global|declared|observed|diff_contracts|ROADMAP|v3" IDEA.md DESIGN.md
```

Expected: no old language-scanner trajectory; both files describe the dual
contract model; v3 is explicitly future work.

- [ ] **Step 4: Commit the summaries**

```powershell
git add IDEA.md DESIGN.md
git commit -m "docs: align product idea and design with pivot"
```

### Task 4: Clarify lockfile and semantic-rule status

**Files:**
- Modify: `docs/lockfile-spec.md`
- Modify: `docs/change-rules.md`

**Interfaces:**
- Consumes: Approved v3 direction and audit-derived roadmap
- Produces: Honest format and behavior documentation without prematurely
  specifying Phase 1 implementation

- [ ] **Step 1: Reframe lockfile v1 and v2**

Keep the exact existing v1 and v2 examples and current behavior. Add:

- a format status table for v1, v2, and planned v3;
- a warning that declared v1/v2 entries store routes only;
- a migration policy explaining why re-locking is required;
- approved v3 goals: complete declared contracts, provenance symmetry,
  deterministic/human-reviewable serialization, forward-compatible reading,
  no captured values, 5 MB target ceiling, and endpoint scoping;
- a link to Phase 1 and the pivot design.

Do not publish a concrete v3 YAML schema or migration command syntax.

- [ ] **Step 2: Separate intended change rules from validated behavior**

Preserve the change classification catalog as the intended semantic contract.
Add an opening status notice explaining:

- `diff` currently implements the catalog incompletely;
- declared v1/v2 `verify` compares routes only;
- Phase 1 unifies declared Verify with the diff engine;
- Phase 2 resolves the audited false-negative and false-positive classes;
- callers must consult README known limitations until those exit criteria pass.

- [ ] **Step 3: Correct unsupported-version wording**

Replace any implication that all OpenAPI 3.x input is supported. State that
OpenAPI 3.0 is the current target and OpenAPI 3.1 is a Phase 3 item.

- [ ] **Step 4: Validate format and behavior claims**

Run:

```powershell
rg -n "Version 1|Version 2|Version 3|route|re-lock|5 MB|OpenAPI 3\.0|OpenAPI 3\.1|Phase 1|Phase 2" docs/lockfile-spec.md docs/change-rules.md
```

Expected: legacy formats remain documented, v3 remains a planned design, and
the semantic catalog is not presented as completely implemented.

- [ ] **Step 5: Commit the reference documentation**

```powershell
git add docs/lockfile-spec.md docs/change-rules.md
git commit -m "docs: clarify lockfile and change-rule maturity"
```

### Task 5: Cross-document verification and session record

**Files:**
- Modify: `ROADMAP.md`
- Modify: `README.md`
- Modify: `IDEA.md`
- Modify: `DESIGN.md`
- Modify: `docs/lockfile-spec.md`
- Modify: `docs/change-rules.md`
- Create: `implementation-log/2026-07-24-product-pivot-docs.md`

**Interfaces:**
- Consumes: All documentation outputs from Tasks 1–4
- Produces: A consistent documentation set and local final status record

- [ ] **Step 1: Scan for conflicting roadmap language**

Run:

```powershell
rg -n "v0\.6\.5|Static Discovery|Plugin system|OpenAPI-first|OpenAPI 3\.x|Indian|fintech" README.md ROADMAP.md IDEA.md DESIGN.md docs
```

Expected:

- v0.6.5 appears only as a retired planning milestone or in historical files;
- static discovery and plugins appear only in historical or excluded context;
- no current document claims OpenAPI 3.x support;
- no current positioning limits the audience by country or industry.

- [ ] **Step 2: Check links and Markdown hygiene**

Run:

```powershell
git diff --check
rg -n "\[[^]]+\]\([^)]*ROADMAP\.md[^)]*\)" README.md IDEA.md DESIGN.md docs/lockfile-spec.md docs/change-rules.md
```

Expected: `git diff --check` exits 0 and the current summary/reference
documents link to the authoritative roadmap.

- [ ] **Step 3: Compare documentation against the approved design**

Read:

```powershell
Get-Content -Raw docs/superpowers/specs/2026-07-24-apiwatch-product-pivot-design.md
git diff -- ROADMAP.md README.md IDEA.md DESIGN.md docs/lockfile-spec.md docs/change-rules.md
```

Verify every resolved decision appears consistently and no deferred decision
was silently resolved.

- [ ] **Step 4: Write the implementation log**

Create `implementation-log/2026-07-24-product-pivot-docs.md` with:

- goal: replace the old roadmap with the approved correctness-first pivot;
- decisions: global audience, breaking v3, 5 MB design target, observed work
  preserved, distribution continuous;
- files touched;
- verification commands and results;
- blockers or the explicit statement that there are none;
- next step: begin Phase 0 only after the documentation review.

Do not stage the implementation log because the directory is intentionally
gitignored.

- [ ] **Step 5: Run the final repository check**

Run:

```powershell
git status --short
git log -6 --oneline
```

Expected: only intended documentation changes are present before the final
documentation commit; the implementation log remains ignored.

- [ ] **Step 6: Commit any final consistency edits**

If Task 5 changes tracked documentation:

```powershell
git add ROADMAP.md README.md IDEA.md DESIGN.md docs/lockfile-spec.md docs/change-rules.md
git commit -m "docs: finalize product pivot documentation"
```

If no tracked files changed, do not create an empty commit.
