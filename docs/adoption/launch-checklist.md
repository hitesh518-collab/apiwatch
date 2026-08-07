# Launch Checklist

## Before Launch

### Release Status
- [x] Latest tagged release: v1.0.2
- [x] main is preparing v1.0.3 (Cargo.toml + CHANGELOG aligned)
- [x] Version references consistent: README/Formula/Scoop at v1.0.2
- [ ] Tag v1.0.3 when ready (owner action)

### CI Green
- [x] `cargo fmt` passes
- [x] `cargo clippy` passes
- [x] `cargo test --workspace` passes
- [x] MSRV check (1.88.0) passes
- [x] Compatibility corpus passes (14 passing + 6 known-failing)
- [x] Performance budget passes
- [x] Snapshot gate passes
- [x] Action smoke test passes
- [x] Example smoke test passes
- [x] Dogfood CI passes

### Install Paths Verified
- [x] `cargo install apiwatch` (crates.io)
- [x] `cargo build --release` (source)
- [x] Prebuilt binaries on releases page
- [x] Homebrew source-build formula
- [x] Scoop source-build manifest
- [ ] GHCR container pullable: now public

### Demo Verified
- [x] Observed JSON drift demo (local, deterministic)
- [x] HAR to lock demo
- [x] Declared OpenAPI drift demo
- [x] `check_examples.py` passes (9/9 checks)
- [x] All README commands executable

### Dogfood Evidence
- [x] GitHub PR API locked and verified weekly
- [x] Lock is 88 KB, value-free, committed
- [x] CI workflow: `.github/workflows/dogfood.yml`

### Known Limitations (clearly documented)
- [x] Swagger 2.0 not supported
- [x] Path-level $ref not supported
- [x] Schema expansion budget (Stripe)
- [x] Observed contracts are sampled, not complete
- [x] No cheksum verification in consumer Action (Phase 4 added checksum to action downloads)
- [x] Homebrew/Scoop are source-build, not taps/buckets

### How to Report Bugs
- [x] Issue templates exist (Bug report, Feature request)
- [x] CONTRIBUTING.md updated with reporting instructions
- [x] Repository has issues enabled

### Contract Explanation
- [x] Observed vs Declared explained in README
- [x] Privacy boundary explained (value-free, structure only)
- [x] Sampling limitations explicit

## Launch Messaging

### One-sentence pitch
"Catch third-party API changes before they break your app."

### Short description (for repo, social)
"APIWatch records or locks the API contract your application relies on,
stores value-free evidence in Git, and fails CI when that external dependency
drifts. Works with OpenAPI specs and observed JSON responses — no provider spec
required."

### Technical summary (for posts)
"I built a Git-committed contract lock for third-party APIs, including APIs
with no reliable OpenAPI spec. Record a response once, commit the shape, and
get CI drift detection for free. Rust CLI, cross-platform binaries, GitHub
Action, and SARIF Code Scanning support."

### Don't lead with
- Lockfile version numbers
- Internal parser architecture
- Comparison engine defect classes
- Corpus size or benchmark numbers

### Key differentiators
1. Observed contracts — works without provider OpenAPI docs
2. Value-free evidence — types and structure only, never data
3. CI-native — exits 0/1/2, SARIF, reusable GitHub Action
4. Deterministic — byte-stable locks, reproducible across platforms

## Social/Outreach Checklist
- [ ] Pin one clear use case (e.g. "caught Stripe changing a field type")
- [ ] Link the hero demo (observed-json-drift) first
- [ ] Mention `cargo install apiwatch` as the quickest install
- [ ] Link to GitHub repo
- [ ] State what it is NOT (not an uptime monitor, not a functional test)
