# Phase 6 — v1 Stabilization and Adoption: Design

**Target:** v1.0.0
**Date:** 2026-08-03
**Status:** approved

## Goal

Make the proven REST contract workflow stable enough for long-term automation.
Deliver v1.0.0 with documented compatibility, migration, privacy, performance,
and release guarantees.

## Approach

Three parallel workstreams, gated by a shared v1.0.0 tag. Track A (Safety and
Robustness) gates the tag — it must complete first since its lockfile/SemVer
decisions anchor everything else. Tracks B and C start in parallel after Track A
stabilizes.

| Track | Items | Touches |
|-------|-------|---------|
| A: Safety and Robustness | Lockfile SemVer, Parser Fuzzing, Performance Budgets | `src/lockfile/`, `src/openapi/`, fuzz targets, CI |
| B: Testing and Verification | Corpus Expansion, Deterministic Snapshots | `compat/`, `tests/compat.rs`, `scripts/snapshot.py`, CI |
| C: Distribution and Docs | Migration Docs, Release Install Verification | `docs/migration.md`, `.github/workflows/release.yml` |

## Track A — Safety and Robustness

### A1. Lockfile SemVer Guarantees

**v4 Format Stability Contract:** The v4 lockfile schema is frozen at v1.0.0.
Future format changes require a v5 — never a silent v4 schema change. v2/v3
remain readable via the existing `load_v2()` / `v3::load()` paths, marked
`#[doc(hidden)]`, gated behind a `legacy-lock-format` feature flag (on by
default). Deprecation warnings documented in `docs/migration.md`, not emitted
at runtime.

**CLI Stability Contract:** Subcommands and flags: no removals or renames in
minor/patch. New flags may be added. Exit codes are stable (0 = no changes,
1 = changes detected, 2 = error). Text output is human-readable, not guaranteed
stable for parsing. JSON and SARIF schemas are versioned and stable within a
major release.

**Guarding Mechanism:** A CI contract file `compat/semver-contract.json` lists
every CLI flag, subcommand, output key, and exit code. A test
`tests/cli_semver.rs` compares the current `Cli` struct + output keys against the
contract, failing on removals or changes. The contract is additive-only in
minor/patch releases. Enforced by a `semver` job in `ci.yml` on every push/PR.

**Files:**
- `compat/semver-contract.json` — machine-readable stability surface
- `tests/cli_semver.rs` — contract enforcement test
- `src/lib.rs` — update doc comment from "pre-v1" to "v1 public interface"
- `src/lockfile/mod.rs` — `#[cfg(feature = "legacy-lock-format")]` gates on v2/v3 load paths
- `Cargo.toml` — add `legacy-lock-format` feature
- `CHANGELOG.md` — SemVer policy section

### A2. Parser Fuzzing

**Fuzzing target:** `openapi::load_contract_input_with_ref_root` — the full
public entry point that accepts YAML/JSON bytes, resolves `$ref`, and produces an
`ApiContract`. This is the untrusted-input boundary identified in the privacy
threat model.

**Tooling:** `cargo-fuzz` with `libfuzzer-sys`. Dev-only, not in the release
binary. Three fuzz targets under `fuzz/`:

| Target | Input | Checks |
|--------|-------|--------|
| `openapi_parse` | Arbitrary bytes as YAML/JSON | No panic, no OOM (max 10 MB input), no hang (2 s timeout) |
| `lockfile_v4_roundtrip` | Arbitrary bytes as lockfile | Parse v4 -> serialize -> re-parse -> structural equality. No panic. |
| `observed_infer` | Arbitrary bytes as JSON | `infer()` never panics, never retains values, shape depth bounded |

**Corpus seeding:**
- `openapi_parse`: `.compat-cache/*.json` and `.compat-cache/*.yaml` files
- `lockfile_v4_roundtrip`: `testdata/lock/` lockfile fixtures
- `observed_infer`: `testdata/` JSON fixtures

**CI Integration:** Not in the push/PR path. A new `fuzz` workflow dispatch job:
build targets with `cargo +nightly fuzz build`, run each for a configurable
duration (default 60 s per target), report crashes as CI artifacts. Initially
advisory (exit 0 even with findings). Hardened to blocking (exit non-zero on
findings) after 3 consecutive clean dispatch runs with no new crashes.

**Files:**
- `fuzz/Cargo.toml` — fuzz crate manifest
- `fuzz/fuzz_targets/openapi_parse.rs`
- `fuzz/fuzz_targets/lockfile_v4_roundtrip.rs`
- `fuzz/fuzz_targets/observed_infer.rs`
- `.github/workflows/fuzz.yml` — workflow dispatch job

### A3. Performance Budgets

**What gets measured:** Wall-clock time for `diff` (self-diff) and `lock` on each
compat spec. These are the two user-facing operations whose latency matters in
CI loops.

**Mechanism:** Python script `scripts/bench_perf.py`, following the pattern of
`fetch_compat_specs.py` and `release_smoke.py`. For each spec in
`compat/specs.json`: run `apiwatch diff {spec} {spec}` and
`apiwatch lock --openapi {spec}` 3 times, record median, compare against budgets.

**Budget file `compat/perf-budget.json`:**
```json
{
  "version": 1,
  "budgets": {
    "default_diff_seconds": 10.0,
    "default_lock_seconds": 15.0,
    "per_spec_overrides": {
      "github": { "diff_seconds": 30.0, "lock_seconds": 45.0 }
    }
  }
}
```

**CI Integration:** New `perf` job in `ci.yml`, runs on push to `main` and PR.
Runs `scripts/fetch_compat_specs.py` to populate `.compat-cache` (using the same
cache key as the `compat` dispatch job), then benchmarks each spec. Fails if any
spec exceeds its budget. Initial budgets set at 2x the baseline measurements from
current `main` to avoid flaky failures, tightened after observing CI variance over
several runs.

**Non-goals:** Memory profiling, lock payload size (already gated by
`DEFAULT_MAX_LOCK_BYTES`), cold-start vs warm-cache, statistical rigor beyond
median-of-3. This is a regression smoke alarm, not a benchmark suite.

**Files:**
- `scripts/bench_perf.py` — benchmark runner
- `compat/perf-budget.json` — budget thresholds
- `.github/workflows/ci.yml` — new `perf` job

## Track B — Testing and Verification

### B1. Corpus Expansion (10 -> 15-20 Specs)

**Goal:** Broaden the compatibility corpus to cover a wider range of OpenAPI
features and failure modes.

**Selection criteria for new specs:**

1. **Diverse schemas:** Specs with `anyOf`/`oneOf`/`allOf` compositions,
   polymorphic schemas, discriminator mappings — stress contract normalization
2. **Large specs:** At least 2 specs with 500+ operations — verify v4 payload
   scaling
3. **YAML variety:** YAML with anchors/aliases, multi-document, non-ASCII —
   stress the YAML parser
4. **Remote refs:** Specs with external `$ref` (at least one known to resolve in
   CI) — validate `--ref-root`
5. **New passing specs (7+):** Bring total passing to 12+ (currently 6)
6. **New known-failing specs (3+):** Broaden failure catalog beyond the current 4

**Process:** Add entries to `compat/specs.json` with pinned commit, SHA-256, max
bytes, and status. Run `scripts/fetch_compat_specs.py` to populate
`.compat-cache`. Add `#[ignore]` tests to `tests/compat.rs`. Run compat CI job
manually to verify, then update CI cache key. Document findings in
`docs/compat-corpus.md`.

**Non-goal:** Fixing existing known-failing specs. They document parser
limitations. If a spec starts passing due to unrelated improvements, update its
status — do not block on it.

**Files:**
- `compat/specs.json` — new spec entries
- `tests/compat.rs` — new `#[ignore]` tests
- `docs/compat-corpus.md` — corpus documentation (one paragraph per spec)

### B2. Deterministic Output Snapshots

**What gets snapshotted:** For each compat spec: `lock` output (v4 api.lock YAML)
and `diff` output (text mode, self-diff = "No changes detected."). These prove
the core output path is byte-stable across code changes.

**Mechanism:** Python script `scripts/snapshot.py`. For each spec: run
`apiwatch lock --openapi {spec} --name {name} --output tmp/api.lock`, hash the
result (SHA-256), compare against the stored manifest `compat/snapshots.json`.

**Snapshot manifest:**
```json
{
  "version": 1,
  "snapshots": {
    "github": {"lock_sha256": "...", "diff_output_sha256": "..."}
  }
}
```

**CI Integration:** New `snapshot` job in `ci.yml`, runs on push/PR. Runs
`scripts/fetch_compat_specs.py` to populate `.compat-cache` (same cache key as
`compat`), then hashes each spec's output. If a hash changes, the job prints old
vs new output diff and fails. Intentional changes require running
`python scripts/snapshot.py --update` to regenerate the manifest hashes and
committing the updated `compat/snapshots.json` in the same PR.

**Non-goals:** Snapshotting JSON/SARIF formats, snapshotting verify output
(depends on live URLs or recorded data — not deterministic), golden-file review
workflow beyond text diff on failure.

**Files:**
- `scripts/snapshot.py` — snapshot runner
- `compat/snapshots.json` — hash manifest
- `.github/workflows/ci.yml` — new `snapshot` job

## Track C — Distribution and Docs

### C1. Migration Documentation

**Artifact:** `docs/migration.md` — a single document covering every lockfile
version transition, new user onboarding, and the compatibility guarantee policy.

**Content structure:**
1. Quick reference table: version -> features, breakage risk, upgrade action
2. Per-version-pair instructions (v1->v2, v2->v3, v3->v4, pre-APIWatch->v4)
3. Compatibility guarantee statement (v4 is v1.0.0 stable; v2/v3 read forever)
4. Troubleshooting — common errors and resolutions

**Validation:** A CI test `tests/cli_migration.rs` that ships v2 and v3 fixture
lockfiles in `testdata/migration/`, runs `apiwatch lock --update` against them,
and asserts the output is valid v4.

**Files:**
- `docs/migration.md`
- `testdata/migration/v2_fixture.lock`
- `testdata/migration/v3_fixture.lock`
- `tests/cli_migration.rs`
- `README.md` — add link to migration guide

### C2. Release Install Verification

**What it does:** After a tag-triggered release builds all artifacts, a new
smoke-test job downloads the binary (Linux, macOS, Windows), the container, and
runs `apiwatch --version` + a basic `diff` on a test fixture. Reports results in
the release notes.

**Mechanism:** New `install-verify` job in `release.yml`, running after `release`
and `container` succeed. Steps:
1. Download each platform binary from the release artifacts
2. Run `./apiwatch --version` — verify it prints the tagged version
3. Download `testdata/openapi/verify_matching.yaml` from the repo
4. Run `./apiwatch diff verify_matching.yaml verify_matching.yaml` — expect "No changes detected."
5. Pull the ghcr.io container, run the same checks
6. Append a summary table to the release body via `softprops/action-gh-release`

**Files:**
- `.github/workflows/release.yml` — new `install-verify` job
- `scripts/install_smoke.py` — orchestrates download + verify steps

## Non-Goals for v1.0.0

- Plugin system or hook architecture
- GraphQL, gRPC, or AsyncAPI support
- Proxy or passive runtime capture
- Dashboard, web interface, or hosted service
- AI-powered contract decisions
- SLSA provenance attestation or artifact signing
- Fixing existing known-failing compat specs
- Memory profiling or detailed benchmark statistics

## Quality Gates (Inherited)

All existing quality gates from ROADMAP apply to this phase:

1. Reproduce each defect before fixing it
2. Add a regression fixture before changing behavior
3. Keep `diff` and declared Verify on one `diff_contracts` comparison path
4. Preserve deterministic ordering and byte-stable lock output
5. Keep Verify read-only
6. Never retain observed values, credentials, or dynamic map keys
7. Report probabilistic observed coverage honestly
8. Keep documentation accurate for the tagged release
9. Do not start a phase until its predecessor's exit criterion is met

## CI Workflow Changes Summary

| Workflow | Change |
|----------|--------|
| `ci.yml` | New `perf` job (push/PR), new `snapshot` job (push/PR) |
| `ci.yml` | New `semver` job (push/PR) — runs `tests/cli_semver.rs` |
| `release.yml` | New `install-verify` job (post-release gate) |
| `fuzz.yml` | New workflow (dispatch only, `workflow_dispatch`) |
