# APIWatch Scoop Manifest Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a repository-owned Scoop manifest that source-builds the tagged APIWatch v0.6.0 release on Windows and document the local installation workflow.

**Architecture:** `Scoop/apiwatch.json` pins the immutable GitHub v0.6.0 source archive, verifies its SHA-256 checksum, relies on Scoop's `rust` dependency, and runs a locked Cargo release build in Scoop's versioned application directory. README and changelog changes make the local source-build workflow and Windows native-build prerequisite explicit; no Scoop bucket, binary release, updater, or CI job is added.

**Tech Stack:** Scoop manifest JSON, PowerShell, Cargo, GitHub source archives, existing Rust test suite.

## Global Constraints

- Create exactly one local source-build manifest at `Scoop/apiwatch.json`; do not create a Scoop bucket, binary release assets, an installer, `checkver`, `autoupdate`, or Scoop-specific GitHub Actions CI.
- Pin source to `https://github.com/hitesh518-collab/apiwatch/archive/refs/tags/v0.6.0.tar.gz` with SHA-256 `243bf768f39dac882b4cdbf02209bc0d95d83e07a81a60e0631bd039646ab948` and extraction root `apiwatch-0.6.0`.
- Manifest metadata is version `0.6.0`, homepage `https://github.com/hitesh518-collab/apiwatch`, license `Apache-2.0`, and description `Lock, diff, and verify external API contracts`.
- Depend on Scoop package `rust`, build with `cargo build --release --locked --manifest-path "$dir\\Cargo.toml"`, and expose `target\\release\\apiwatch.exe` as the `apiwatch` shim.
- Scoop supplies Cargo automatically, but the README must state that a Windows Rust source build needs Microsoft C++ Build Tools and a Windows SDK.
- Document only `scoop install ./Scoop/apiwatch.json`; do not imply that `scoop install apiwatch` works without a bucket.
- Preserve `Formula/apiwatch.rb`, Rust CLI behavior, GitHub Action behavior, and existing CI unchanged.
- Keep high-level agent records in ignored `implementation-log/` files; do not stage them.

---

### Task 1: Add the Release-Pinned Scoop Manifest

**Files:**
- Create: `Scoop/apiwatch.json`
- Create (ignored): `implementation-log/2026-07-17-apiwatch-scoop-manifest.md`

**Interfaces:**
- Consumes: the public `v0.6.0` GitHub source archive, Scoop's `rust` package, Cargo's locked release build, and Scoop's `$dir` installer variable.
- Produces: a local manifest accepted by `scoop install ./Scoop/apiwatch.json` that creates the normal `apiwatch` Scoop shim from `target\\release\\apiwatch.exe`.

- [ ] **Step 1: Verify the tagged source archive before creating the manifest**

  Run this PowerShell command. It is the source-integrity red gate: do not
  create the manifest if the archive checksum or extraction root differs.

  ```powershell
  $archive = Join-Path $env:TEMP 'apiwatch-v0.6.0.tar.gz'
  curl.exe --fail --location --silent --show-error --output $archive 'https://github.com/hitesh518-collab/apiwatch/archive/refs/tags/v0.6.0.tar.gz'
  $actual = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant()
  $expected = '243bf768f39dac882b4cdbf02209bc0d95d83e07a81a60e0631bd039646ab948'
  if ($actual -ne $expected) {
      throw "v0.6.0 archive checksum mismatch: expected $expected, got $actual"
  }
  $root = (& tar -tzf $archive | Select-Object -First 1)
  if ($root -ne 'apiwatch-0.6.0/') {
      throw "v0.6.0 archive root mismatch: expected apiwatch-0.6.0/, got $root"
  }
  $actual
  ```

  Expected: the command prints
  `243bf768f39dac882b4cdbf02209bc0d95d83e07a81a60e0631bd039646ab948` and
  exits `0`.

- [ ] **Step 2: Create the local Scoop manifest**

  Create `Scoop/apiwatch.json` with exactly this content. `installer.script`
  uses Scoop's `$dir` as the extracted application directory; `bin` is
  deliberately relative to that directory so Scoop creates the shim only
  after the release executable exists.

  ```json
  {
    "version": "0.6.0",
    "description": "Lock, diff, and verify external API contracts",
    "homepage": "https://github.com/hitesh518-collab/apiwatch",
    "license": "Apache-2.0",
    "url": "https://github.com/hitesh518-collab/apiwatch/archive/refs/tags/v0.6.0.tar.gz",
    "hash": "243bf768f39dac882b4cdbf02209bc0d95d83e07a81a60e0631bd039646ab948",
    "extract_dir": "apiwatch-0.6.0",
    "depends": "rust",
    "installer": {
      "script": "cargo build --release --locked --manifest-path \"$dir\\Cargo.toml\""
    },
    "bin": "target\\release\\apiwatch.exe"
  }
  ```

- [ ] **Step 3: Parse and structurally validate the manifest**

  Run this PowerShell assertion after creating the JSON. It validates the
  executable shim and installer string without requiring Scoop to be present.

  ```powershell
  $manifest = Get-Content -Raw Scoop/apiwatch.json | ConvertFrom-Json
  $expected = [ordered]@{
      version = '0.6.0'
      description = 'Lock, diff, and verify external API contracts'
      homepage = 'https://github.com/hitesh518-collab/apiwatch'
      license = 'Apache-2.0'
      url = 'https://github.com/hitesh518-collab/apiwatch/archive/refs/tags/v0.6.0.tar.gz'
      hash = '243bf768f39dac882b4cdbf02209bc0d95d83e07a81a60e0631bd039646ab948'
      extract_dir = 'apiwatch-0.6.0'
      depends = 'rust'
      installer_script = 'cargo build --release --locked --manifest-path "$dir\Cargo.toml"'
      bin = 'target\release\apiwatch.exe'
  }
  foreach ($field in 'version', 'description', 'homepage', 'license', 'url', 'hash', 'extract_dir', 'depends', 'bin') {
      if ($manifest.$field -ne $expected[$field]) {
          throw "Manifest $field mismatch: expected $($expected[$field]), got $($manifest.$field)"
      }
  }
  if ($manifest.installer.script -ne $expected.installer_script) {
      throw "Manifest installer.script mismatch: expected $($expected.installer_script), got $($manifest.installer.script)"
  }
  'Scoop manifest structure is valid.'
  ```

  Expected: `Scoop manifest structure is valid.` and exit `0`.

- [ ] **Step 4: Conditionally smoke-test the local manifest with Scoop**

  First discover whether Scoop is already available and whether it already
  manages `apiwatch`:

  ```powershell
  $scoop = Get-Command scoop -ErrorAction SilentlyContinue
  if ($null -eq $scoop) {
      'SKIP: Scoop is not installed in this environment.'
  }
  else {
      scoop list apiwatch 2>$null
      if ($LASTEXITCODE -eq 0) {
          'SKIP: Scoop already manages apiwatch; do not overwrite it.'
      }
      else {
          $installOutput = & scoop install ./Scoop/apiwatch.json 2>&1
          $installExit = $LASTEXITCODE
          $installOutput
          if ($installExit -ne 0) {
              "LIMITED: Scoop install exited $installExit. Record whether Windows C++ Build Tools or a Windows SDK is unavailable."
          }
          else {
              apiwatch --help
          }
      }
  }
  ```

  Expected when Scoop is available and does not already manage APIWatch:
  Scoop resolves `rust`, Cargo builds the locked release profile, and
  `apiwatch --help` exits `0` with output containing `apiwatch`. Expected when
  Scoop is unavailable or already manages APIWatch: the command prints the
  applicable `SKIP:` line. If the native source build cannot complete, the
  command prints `LIMITED:` and preserves Cargo's output for the implementation
  log; it is not a successful smoke test. Do not install Scoop, uninstall an
  existing installation, or bypass a missing Microsoft C++ Build Tools or
  Windows SDK prerequisite.

- [ ] **Step 5: Record the manifest implementation status and commit it**

  Create `implementation-log/2026-07-17-apiwatch-scoop-manifest.md` with a
  concise record containing the v0.6.0 tag and checksum, manifest fields,
  structural-validation result, Scoop smoke-test result or skip reason, and
  any missing native Windows build prerequisite. Leave this file ignored.

  Stage and commit only the manifest:

  ```powershell
  git add Scoop/apiwatch.json
  git commit -m "Add Scoop manifest"
  ```

  Expected: one commit contains only `Scoop/apiwatch.json`.

### Task 2: Document the Local Scoop Workflow

**Files:**
- Modify: `README.md` after the `## Homebrew` section and before `## GitHub Action`
- Modify: `CHANGELOG.md` under `## Unreleased` -> `### Added`
- Modify (ignored): `implementation-log/2026-07-17-apiwatch-scoop-manifest.md`

**Interfaces:**
- Consumes: `Scoop/apiwatch.json` from Task 1 and its v0.6.0 source-build contract.
- Produces: a documented local Scoop command and unreleased changelog record that accurately state the automatic Rust dependency and native Windows build requirement.

- [ ] **Step 1: Add the local Scoop installation section to the README**

  Insert this section immediately after the existing `## Homebrew` section
  and before `## GitHub Action` in `README.md`:

  ````markdown
  ## Scoop

  The repository includes a Scoop manifest for source-building the current v0.6.0 tagged release on Windows. Clone this repository, then install the local manifest:

  ```powershell
  git clone https://github.com/hitesh518-collab/apiwatch.git
  cd apiwatch
  scoop install ./Scoop/apiwatch.json
  ```

  Scoop installs the Rust dependency automatically. Rust source builds on Windows also require Microsoft C++ Build Tools and a Windows SDK. This first manifest is not in a Scoop bucket, so `scoop install apiwatch` is not available. Each apiwatch release updates the manifest's pinned source URL and SHA-256 checksum after its tag is published.
  ````

- [ ] **Step 2: Add the unreleased changelog entry**

  Under `## Unreleased` -> `### Added` in `CHANGELOG.md`, add exactly this
  bullet after the existing Homebrew entry:

  ```markdown
  - A repository-owned Scoop manifest for source-building the tagged apiwatch release on Windows.
  ```

- [ ] **Step 3: Validate documentation accuracy and placement**

  Run this PowerShell assertion. It verifies the command, required native
  prerequisite, no-bucket limitation, changelog entry, and section order.

  ```powershell
  $readme = Get-Content -Raw README.md
  foreach ($fragment in @(
      '## Scoop',
      'scoop install ./Scoop/apiwatch.json',
      'Scoop installs the Rust dependency automatically.',
      'Microsoft C++ Build Tools and a Windows SDK.',
      'scoop install apiwatch` is not available'
  )) {
      if (-not $readme.Contains($fragment)) {
          throw "README is missing: $fragment"
      }
  }
  if ($readme.IndexOf('## Homebrew') -ge $readme.IndexOf('## Scoop') -or $readme.IndexOf('## Scoop') -ge $readme.IndexOf('## GitHub Action')) {
      throw 'README Scoop section is not between Homebrew and GitHub Action.'
  }
  $changelog = Get-Content -Raw CHANGELOG.md
  $entry = '- A repository-owned Scoop manifest for source-building the tagged apiwatch release on Windows.'
  if (-not $changelog.Contains($entry)) {
      throw 'CHANGELOG is missing the Scoop manifest entry.'
  }
  'Scoop documentation is valid.'
  ```

  Expected: `Scoop documentation is valid.` and exit `0`.

- [ ] **Step 4: Update the ignored implementation record and commit the documentation**

  Update `implementation-log/2026-07-17-apiwatch-scoop-manifest.md` with the
  README and changelog changes plus the documentation-validation result. Do
  not stage the log.

  ```powershell
  git add README.md CHANGELOG.md
  git commit -m "Document Scoop installation"
  ```

  Expected: one commit contains only `README.md` and `CHANGELOG.md`.

### Task 3: Run the Final Release Gate

**Files:**
- Modify (ignored): `implementation-log/2026-07-17-apiwatch-scoop-manifest.md`

**Interfaces:**
- Consumes: the committed `Scoop/apiwatch.json`, README, changelog, Cargo workspace, and conditional Scoop runtime from Tasks 1 and 2.
- Produces: recorded evidence that the manifest remains structurally correct, the source archive remains pinned, Rust checks pass, and optional Scoop validation was run or explicitly skipped.

- [ ] **Step 1: Re-run the manifest and source-archive validation**

  Run the Task 1 archive command and Task 1 structural manifest command
  unchanged.

  Expected: the archive hash is
  `243bf768f39dac882b4cdbf02209bc0d95d83e07a81a60e0631bd039646ab948`, the
  archive root is `apiwatch-0.6.0/`, and the manifest command prints `Scoop
  manifest structure is valid.`.

- [ ] **Step 2: Run the Rust and repository integrity gate**

  ```powershell
  cargo fmt --all -- --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test
  git diff --check
  git status --short --branch
  ```

  Expected: formatting, Clippy, and all Rust tests exit `0`; Git whitespace
  validation produces no output; status shows no unexpected tracked changes
  and an ahead count that reflects only the local design and implementation
  commits that have not yet been pushed.

- [ ] **Step 3: Record final validation and report the known boundary**

  Update `implementation-log/2026-07-17-apiwatch-scoop-manifest.md` with the
  final Rust-gate results and the Scoop smoke-test outcome. When Scoop is
  unavailable, or Windows C++ Build Tools and Windows SDK are unavailable,
  record that exact reason as an environment limitation rather than claiming a
  completed end-to-end install. Do not create a CI job or install missing
  software to remove the limitation.

## Final Verification

- [ ] `Scoop/apiwatch.json` is valid JSON, pins the v0.6.0 source archive and verified SHA-256, declares extraction root `apiwatch-0.6.0`, depends on `rust`, and exposes the release executable through Scoop.
- [ ] The manifest builds with Cargo's locked release dependencies in Scoop's application directory and creates no global Cargo installation.
- [ ] README documents only `scoop install ./Scoop/apiwatch.json`, automatic Rust dependency installation, the Microsoft C++ Build Tools and Windows SDK prerequisite, and the lack of a bucket shortcut.
- [ ] The changelog and ignored implementation log record the delivery, while `Formula/apiwatch.rb`, CLI behavior, existing CI, binary releases, buckets, and auto-update tooling remain unchanged.
- [ ] Archive integrity, manifest structure, the existing Rust release gate, `git diff --check`, and conditional non-destructive Scoop validation have run with outcomes recorded.
