# APIWatch Homebrew Formula Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a repository-owned Homebrew formula that builds the tagged apiwatch source with Rust and document its local-formula installation workflow.

**Architecture:** `Formula/apiwatch.rb` pins the GitHub `v0.1.0` source archive and uses Homebrew's standard locked Cargo arguments to install the binary into the formula prefix. README and changelog changes make the source-only, repository-local install workflow explicit; no tap, bottles, release automation, or Homebrew CI job is introduced.

**Tech Stack:** Homebrew Formula DSL, Ruby syntax validation, Cargo, PowerShell, GitHub source archives, existing Rust test suite.

## Global Constraints

- Create exactly one source-only formula at `Formula/apiwatch.rb`; do not create a tap, bottles, a `head` build, options, action outputs, or runtime configuration.
- Pin source to `https://github.com/hitesh518-collab/apiwatch/archive/refs/tags/v0.1.0.tar.gz` with SHA-256 `b740a199e8d00b49a6ebdbc48765e578fd729dc89b11eece85fb6ba15e1df2d8`.
- The formula class is `Apiwatch`, formula name is `apiwatch`, homepage is `https://github.com/hitesh518-collab/apiwatch`, and license is `Apache-2.0`.
- Use `depends_on "rust" => :build` and `system "cargo", "install", *std_cargo_args`; `std_cargo_args` supplies `--locked`, the formula prefix root, and the current source path.
- The formula test runs the installed executable with `--help` and asserts the output contains `apiwatch`.
- Document only repository-local installation through `brew install --build-from-source ./Formula/apiwatch.rb`; do not imply `brew install apiwatch` or a tap shortcut is available.
- Do not add a Homebrew-specific GitHub Actions job in this slice. The standard Rust CI remains unchanged.
- Keep high-level agent records in ignored `implementation-log/` files.

---

### Task 1: Add and Document the Repository-Owned Formula

**Files:**
- Create: `Formula/apiwatch.rb`
- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Create (ignored): `implementation-log/2026-07-16-apiwatch-homebrew-formula.md`

**Interfaces:**
- Provides a Homebrew formula named `apiwatch` that installs the `apiwatch` executable from the v0.1.0 source archive.
- Provides a documented local invocation: `brew install --build-from-source ./Formula/apiwatch.rb`.
- Does not change the Rust CLI, GitHub Action, or repository CI interfaces.

- [ ] **Step 1: Confirm the current source archive checksum before creating the formula**

  Run this PowerShell command. It is the source-integrity red gate: do not create the formula if the actual checksum differs from the pinned value.

  ```powershell
  $archive = Join-Path $env:TEMP 'apiwatch-v0.1.0.tar.gz'
  curl.exe --fail --location --silent --show-error --output $archive 'https://github.com/hitesh518-collab/apiwatch/archive/refs/tags/v0.1.0.tar.gz'
  $actual = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant()
  $expected = 'b740a199e8d00b49a6ebdbc48765e578fd729dc89b11eece85fb6ba15e1df2d8'
  if ($actual -ne $expected) {
      throw "v0.1.0 archive checksum mismatch: expected $expected, got $actual"
  }
  $actual
  ```

  Expected: the command prints `b740a199e8d00b49a6ebdbc48765e578fd729dc89b11eece85fb6ba15e1df2d8` and exits `0`.

- [ ] **Step 2: Create the source-only Homebrew formula**

  Create `Formula/apiwatch.rb` with exactly this formula. `std_cargo_args` carries Homebrew's current `--locked`, `--root`, and `--path` arguments, so do not duplicate them or add custom `CARGO_*` environment handling.

  ```ruby
  class Apiwatch < Formula
    desc "Lock, diff, and verify external API contracts"
    homepage "https://github.com/hitesh518-collab/apiwatch"
    url "https://github.com/hitesh518-collab/apiwatch/archive/refs/tags/v0.1.0.tar.gz"
    sha256 "b740a199e8d00b49a6ebdbc48765e578fd729dc89b11eece85fb6ba15e1df2d8"
    license "Apache-2.0"

    depends_on "rust" => :build

    def install
      system "cargo", "install", *std_cargo_args
    end

    test do
      assert_match "apiwatch", shell_output("#{bin}/apiwatch --help")
    end
  end
  ```

- [ ] **Step 3: Validate the formula's syntax and its pinned source integrity**

  Run Ruby syntax validation:

  ```powershell
  ruby -c Formula/apiwatch.rb
  ```

  Expected: `Syntax OK`.

  Then run the checksum verification against the formula's actual `sha256` declaration:

  ```powershell
  $formula = Get-Content -Raw Formula/apiwatch.rb
  $expected = [regex]::Match($formula, 'sha256 "([0-9a-f]{64})"').Groups[1].Value
  if ($expected.Length -ne 64) {
      throw 'Formula does not declare a 64-character SHA-256 checksum'
  }
  $archive = Join-Path $env:TEMP 'apiwatch-v0.1.0.tar.gz'
  curl.exe --fail --location --silent --show-error --output $archive 'https://github.com/hitesh518-collab/apiwatch/archive/refs/tags/v0.1.0.tar.gz'
  $actual = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant()
  if ($actual -ne $expected) {
      throw "Formula checksum mismatch: expected $expected, got $actual"
  }
  $actual
  ```

  Expected: `Syntax OK`, followed by the exact SHA-256 value from Step 1. Do not add a fake formula parser test or modify CI to compensate for a missing local Homebrew runtime.

- [ ] **Step 4: Add the repository-local Homebrew installation documentation**

  In `README.md`, insert this section between `## JSON Output` and `## GitHub Action`:

  ````markdown
  ## Homebrew

  The repository includes a source-building Homebrew formula for the current v0.1.0 tagged release. Clone this repository, then install the local formula:

  ```bash
  git clone https://github.com/hitesh518-collab/apiwatch.git
  cd apiwatch
  brew install --build-from-source ./Formula/apiwatch.rb
  ```

  This first formula is not a Homebrew tap, so `brew install apiwatch` is not available. Each apiwatch release updates the formula's pinned source URL and SHA-256 checksum.
  ````

  The example must not tell users to download an arbitrary formula URL, use a tap, install a bottle, or expect release automation.

- [ ] **Step 5: Add the changelog entry and ignored implementation record**

  Under `## Unreleased` -> `### Added` in `CHANGELOG.md`, add:

  ```markdown
  - A repository-owned Homebrew formula for source-building the tagged apiwatch release.
  ```

  Create `implementation-log/2026-07-16-apiwatch-homebrew-formula.md`. Record the pinned tag and checksum, source-only/no-tap decision, files changed, validation results, inability to run Homebrew locally on Windows, and the follow-up to exercise `brew install --build-from-source ./Formula/apiwatch.rb` on macOS or Linux. Do not stage this ignored file.

- [ ] **Step 6: Run the release gate and conditionally run Homebrew validation**

  Run:

  ```powershell
  ruby -c Formula/apiwatch.rb
  cargo fmt --all -- --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test
  git diff --check
  git status --short --branch
  ```

  Expected: Ruby reports `Syntax OK`; Rust format, lint, and tests pass; whitespace validation passes; only `Formula/apiwatch.rb`, `README.md`, and `CHANGELOG.md` are tracked changes before commit.

  If `brew` is available on macOS or Linux, additionally run:

  ```bash
  brew install --build-from-source ./Formula/apiwatch.rb
  brew test apiwatch
  ```

  Expected: source installation completes and the formula test passes. If this environment lacks Homebrew, record the skipped command and platform reason in the implementation log; do not install Homebrew, create a bottle, or add CI.

- [ ] **Step 7: Commit the complete Homebrew delivery**

  ```powershell
  git add Formula/apiwatch.rb README.md CHANGELOG.md
  git commit -m "Add Homebrew formula"
  ```

## Final Verification

- [ ] `Formula/apiwatch.rb` has valid Ruby syntax, pins the intended v0.1.0 GitHub source archive, and declares the verified SHA-256 checksum.
- [ ] The formula uses Homebrew's Rust build dependency and `std_cargo_args`, installs the CLI, and smoke-tests `apiwatch --help`.
- [ ] README documents only the repository-local source-build workflow and accurately states that no tap shortcut exists.
- [ ] Changelog and ignored implementation log record the delivery and its validation limits.
- [ ] The standard Rust release gate passes without a Homebrew-specific CI change.
