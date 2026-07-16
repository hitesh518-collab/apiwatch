# APIWatch v0.6.0 Release and Formula Repin Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish `v0.6.0` for the full current CLI, then add a repository-owned Homebrew formula pinned to that immutable release archive.

**Architecture:** Preserve the current unpushed v0.1 formula work on a local backup branch, then rebuild a clean temporary release branch from `origin/main`. Prepare and publish the Cargo/changelog release before calculating the tag archive checksum; only then add and push the source-only Homebrew formula that pins the release archive.

**Tech Stack:** Cargo, Git annotated tags and local branches, GitHub CLI, GitHub source archives, Homebrew Formula DSL, PowerShell, existing GitHub Actions CI.

## User-Approved Branch Delivery Override

All release preparation and formula work stays on `codex/v0.6.0-release` for review. Push that branch so GitHub Actions can validate it, but do not push it to `origin/main` or repoint local `main` during this plan. The annotated `v0.6.0` tag and published GitHub release are created from the validated release-branch snapshot before the formula commit, which is necessary to calculate the public archive checksum. After Task 3 and the final review are clean, present the branch for user approval; only a later, explicit approval may fast-forward it to `origin/main`.

## Global Constraints

- Do not push, tag, merge, or release the current local `Add Homebrew formula` commit that pins `v0.1.0`; preserve it only on `codex/homebrew-formula-pre-release-backup`.
- Build the release branch from `origin/main`, then cherry-pick only the v0.6.0 release design and this implementation plan; do not cherry-pick the stale Homebrew formula commit.
- Push and validate `codex/v0.6.0-release`, but leave `origin/main` unchanged until the user reviews and explicitly approves the final branch merge.
- Release package version is exactly `0.6.0`, annotated tag is exactly `v0.6.0`, and release date is `2026-07-16`.
- `v0.6.0` contains lock, local/live Verify, GitHub Action, JSON, and SARIF work; the Homebrew formula is added only after the tag and remains under `Unreleased`.
- The GitHub release is published, non-draft, non-prerelease, uses the verified annotated tag, and has no binary assets.
- Post-tag formula source is `https://github.com/hitesh518-collab/apiwatch/archive/refs/tags/v0.6.0.tar.gz`; calculate its exact SHA-256 after the tag is publicly available and use that value in the formula.
- Keep the source-only/no-tap/no-bottles/no-`head` formula design. Do not add release automation or a Homebrew-specific CI job.
- Run Rust format, Clippy with warnings denied, full tests, and `git diff --check` before publishing the release snapshot and after the formula repin.
- Ruby syntax and Homebrew install/test remain conditional on an existing macOS/Linux Ruby and Homebrew environment; do not install those tools in this Windows environment.
- Keep high-level agent records in ignored `implementation-log/` files.

---

### Task 1: Rebuild a Clean v0.6.0 Release Snapshot

**Files:**
- Modify: `Cargo.toml`
- Modify: `CHANGELOG.md`
- Create (ignored): `implementation-log/2026-07-16-apiwatch-v060-release-and-formula-repin.md`
- Preserve (local-only): `codex/homebrew-formula-pre-release-backup`
- Create (temporary): `codex/v0.6.0-release`

**Interfaces:**
- Consumes: `origin/main` at `b66f8d1`, the committed release design, and this plan.
- Produces: a clean local `codex/v0.6.0-release` branch with `Cargo.toml` version `0.6.0`, a versioned changelog section, and no `Formula/` directory.
- Later tasks tag and publish the release branch, then add the formula on that same branch.

- [ ] **Step 1: Verify and preserve the current local-only state**

  Run the following commands before switching branches. They ensure the stale formula commit cannot be lost or accidentally sent to the remote:

  ```powershell
  git status --short --branch
  git log --oneline origin/main..main
  git branch codex/homebrew-formula-pre-release-backup main
  git show --stat --oneline codex/homebrew-formula-pre-release-backup
  ```

  Expected: `main` is ahead of `origin/main` only with the local Homebrew formula and v0.6.0 design/plan commits. The backup branch points at the current local `main` HEAD. Stop if the working tree is not clean or `origin/main` is not `b66f8d1`.

- [ ] **Step 2: Start a clean release branch and retain the design artifacts**

  Capture the committed documentation revisions before switching to the remote base, then create the clean branch and cherry-pick only those documentation commits:

  ```powershell
  $releaseDesignCommit = git log -1 --format=%H -- docs/superpowers/specs/2026-07-16-apiwatch-v060-release-and-formula-repin-design.md
  $releasePlanCommit = git log -1 --format=%H -- docs/superpowers/plans/2026-07-16-apiwatch-v060-release-and-formula-repin.md
  if (-not $releaseDesignCommit -or -not $releasePlanCommit) {
      throw 'Release design or implementation plan commit was not found'
  }
  git switch -c codex/v0.6.0-release origin/main
  git cherry-pick $releaseDesignCommit $releasePlanCommit
  if (Test-Path Formula) {
      throw 'Clean release branch unexpectedly contains Formula/'
  }
  git status --short --branch
  ```

  Expected: the branch is `codex/v0.6.0-release`, contains the design and plan documents, and has no `Formula/` directory. The stale local formula commit remains only on the backup branch.

- [ ] **Step 3: Prepare the v0.6.0 version and changelog release section**

  In `Cargo.toml`, change only the package version line:

  ```toml
  version = "0.6.0"
  ```

  Rewrite the beginning of `CHANGELOG.md` to this exact structure. Preserve the existing `## v0.1.0` section below it unchanged.

  ````markdown
  # Changelog

  ## Unreleased

  ### Added

  ## v0.6.0 - 2026-07-16

  ### Added

  - SARIF 2.1.0 output for `apiwatch diff` and `apiwatch verify`, plus opt-in GitHub Code Scanning upload from the reusable action.
  - Deterministic, versioned JSON output for `apiwatch diff` and `apiwatch verify` via `--format json`.
  - `apiwatch lock <OPENAPI> --name <NAME> --output <PATH>` writes a deterministic v1 `api.lock` file with normalized operation metadata.
  - `apiwatch verify <OPENAPI> --name <NAME> --lock <PATH>` compares a local OpenAPI contract to one named v1 `api.lock` entry and exits `1` for deterministic operation drift.
  - `apiwatch verify <OPENAPI_OR_URL> --name <NAME> --lock <PATH>` supports HTTP/HTTPS OpenAPI URLs for live verification; remote fetch failures exit `2`.
  - Invalid `verify` input and lockfile data errors exit `2`.
  - Reusable `apiwatch verify` composite GitHub Action that builds from source and propagates Verify exit codes.

  ````

  Do not carry the Homebrew changelog entry into `v0.6.0`; Task 3 adds it back under `Unreleased` after the tag.

- [ ] **Step 4: Run the release-snapshot gate before committing**

  Run:

  ```powershell
  cargo fmt --all -- --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test
  git diff --check
  git status --short --branch
  ```

  Expected: all Rust checks pass with 111 tests, whitespace validation passes, and the only tracked changes are `Cargo.toml` and `CHANGELOG.md`.

- [ ] **Step 5: Record release intent and commit the v0.6.0 snapshot**

  Create the ignored implementation log with the planned release sequence, current branch, backup branch, expected tag, validation result, and deferred Ruby/Homebrew checks. Do not stage it.

  Then commit the tracked release files:

  ```powershell
  git add Cargo.toml CHANGELOG.md
  git commit -m "Prepare v0.6.0 release"
  git rev-parse HEAD
  ```

  Expected: the printed release commit is the exact commit Task 2 pushes to the release branch and later tags as `v0.6.0`.

### Task 2: Publish the Verified v0.6.0 Tag and GitHub Release

**Files:**
- Modify (ignored): `implementation-log/2026-07-16-apiwatch-v060-release-and-formula-repin.md`
- Create (ignored): `.superpowers/sdd/apiwatch-v0.6.0-release-notes.md`

**Interfaces:**
- Consumes: the committed `codex/v0.6.0-release` snapshot from Task 1.
- Produces: `origin/codex/v0.6.0-release` at the release snapshot, remote annotated tag `v0.6.0`, and a published GitHub release for that exact tag.
- Supplies: the immutable tag archive Task 3 downloads and hashes.

- [ ] **Step 1: Push the clean release snapshot to its review branch and verify GitHub Actions**

  Record the release commit, push the temporary release branch, then wait for the CI run that uses that commit:

  ```powershell
  $releaseCommit = git rev-parse HEAD
  $releaseBranch = git branch --show-current
  if ($releaseBranch -ne 'codex/v0.6.0-release') {
      throw "Expected codex/v0.6.0-release, got $releaseBranch"
  }
  git push --set-upstream origin $releaseBranch
  $run = gh run list --branch $releaseBranch --limit 20 --json databaseId,headSha,status,conclusion | ConvertFrom-Json |
      Where-Object { $_.headSha -eq $releaseCommit } |
      Select-Object -First 1
  if (-not $run) {
      throw "No GitHub Actions run found for release commit $releaseCommit"
  }
  gh run watch $run.databaseId --exit-status
  ```

  Expected: `origin/codex/v0.6.0-release` contains the clean snapshot while `origin/main` remains at `b66f8d1`; the matching CI run has green `rust` and `action-smoke` jobs. Do not create the tag or GitHub release if this run fails.

- [ ] **Step 2: Create and push the annotated v0.6.0 tag**

  Confirm the release commit remains the branch head, then create an annotated tag and publish it:

  ```powershell
  if ((git rev-parse HEAD) -ne $releaseCommit) {
      throw 'Release branch HEAD changed after CI verification'
  }
  git tag -a v0.6.0 -m "apiwatch v0.6.0" $releaseCommit
  git push origin v0.6.0
  $remoteTag = git ls-remote --tags origin refs/tags/v0.6.0 | ForEach-Object { ($_ -split "`t")[0] }
  if (-not $remoteTag) {
      throw 'Remote v0.6.0 tag was not found after push'
  }
  git show --no-patch --format=fuller v0.6.0
  ```

  Expected: `git show` identifies an annotated `v0.6.0` tag at `$releaseCommit`.

- [ ] **Step 3: Extract versioned changelog notes and publish the GitHub release**

  Write release notes from only the v0.6.0 section, then create a published release guarded by the existing remote tag:

  ```powershell
  $notesPath = '.superpowers/sdd/apiwatch-v0.6.0-release-notes.md'
  $capturing = $false
  $notes = foreach ($line in Get-Content CHANGELOG.md) {
      if ($line -eq '## v0.6.0 - 2026-07-16') {
          $capturing = $true
          continue
      }
      if ($capturing -and $line -match '^## ') {
          break
      }
      if ($capturing) {
          $line
      }
  }
  if (@($notes).Count -eq 0) {
      throw 'The v0.6.0 changelog section could not be extracted'
  }
  Set-Content -Path $notesPath -Value ("# apiwatch v0.6.0`n`n" + ($notes -join "`n"))
  gh release create v0.6.0 --verify-tag --title v0.6.0 --notes-file $notesPath
  gh release view v0.6.0 --json tagName,isDraft,isPrerelease,name,url,targetCommitish
  ```

  Expected: the release JSON reports `tagName` `v0.6.0`, `isDraft` false, `isPrerelease` false, name `v0.6.0`, and target commit `$releaseCommit`. Do not upload assets, use generated notes, or create a draft.

- [ ] **Step 4: Record the published release state**

  Update the ignored release log with the release commit, CI run URL, tag object, GitHub release URL, and the intent to add the formula in the next commit. Keep the log untracked.

### Task 3: Repin and Publish the Homebrew Formula Against v0.6.0

**Files:**
- Create: `Formula/apiwatch.rb`
- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Modify (ignored): `implementation-log/2026-07-16-apiwatch-homebrew-formula.md`
- Modify (ignored): `implementation-log/2026-07-16-apiwatch-v060-release-and-formula-repin.md`

**Interfaces:**
- Consumes: the public `v0.6.0` tag from Task 2.
- Produces: a source-only local Homebrew formula pinned to the v0.6.0 archive and its exact SHA-256 checksum.
- Does not change the v0.6.0 tag, Cargo package version, CLI behavior, GitHub Action, or CI workflow.

- [ ] **Step 1: Download and verify the immutable release archive before creating the formula**

  Run the source-integrity gate. The computed `$sha256` becomes the formula's exact checksum:

  ```powershell
  $archiveUrl = 'https://github.com/hitesh518-collab/apiwatch/archive/refs/tags/v0.6.0.tar.gz'
  $archive = Join-Path $env:TEMP 'apiwatch-v0.6.0.tar.gz'
  curl.exe --fail --location --silent --show-error --output $archive $archiveUrl
  $sha256 = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant()
  if ($sha256.Length -ne 64) {
      throw "Expected a 64-character SHA-256 checksum, got $sha256"
  }
  $sha256
  ```

  Expected: the download succeeds only after the public tag exists and prints one 64-character lowercase checksum. Do not use the local checkout archive or a mutable branch URL.

- [ ] **Step 2: Create the v0.6.0 source-only formula**

  Create `Formula/apiwatch.rb` from the checked `$sha256` value. This PowerShell command creates the directory and writes the complete Formula DSL:

  ```powershell
  New-Item -ItemType Directory -Force Formula | Out-Null
  $formula = @'
  class Apiwatch < Formula
    desc "Lock, diff, and verify external API contracts"
    homepage "https://github.com/hitesh518-collab/apiwatch"
    url "https://github.com/hitesh518-collab/apiwatch/archive/refs/tags/v0.6.0.tar.gz"
    sha256 "{0}"
    license "Apache-2.0"

    depends_on "rust" => :build

    def install
      system "cargo", "install", *std_cargo_args
    end

    test do
      assert_match "apiwatch", shell_output("#{bin}/apiwatch --help")
    end
  end
  '@ -f $sha256
  Set-Content -Path Formula/apiwatch.rb -Value $formula -NoNewline
  ```

  The formula must remain source-only and must not define a `head` block, bottles, options, extra dependencies, or custom Cargo environment variables.

- [ ] **Step 3: Restore the post-release Homebrew documentation and changelog entry**

  In `README.md`, insert the following section between `## JSON Output` and `## GitHub Action`:

  ````markdown
  ## Homebrew

  The repository includes a source-building Homebrew formula for the current v0.6.0 tagged release. Clone this repository, then install the local formula:

  ```bash
  git clone https://github.com/hitesh518-collab/apiwatch.git
  cd apiwatch
  brew install --build-from-source ./Formula/apiwatch.rb
  ```

  This first formula is not a Homebrew tap, so `brew install apiwatch` is not available. Each apiwatch release updates the formula's pinned source URL and SHA-256 checksum.
  ````

  Under `## Unreleased` -> `### Added` in `CHANGELOG.md`, add this exact entry before `## v0.6.0 - 2026-07-16`:

  ```markdown
  - A repository-owned Homebrew formula for source-building the tagged apiwatch release.
  ```

  Do not move that entry into the v0.6.0 section, and do not change the existing GitHub Action or output documentation.

- [ ] **Step 4: Verify the formula pin and full available release gate**

  First verify the formula declares exactly the computed archive pin:

  ```powershell
  $formula = Get-Content -Raw Formula/apiwatch.rb
  if ($formula -notmatch [regex]::Escape($archiveUrl)) {
      throw "Formula does not pin $archiveUrl"
  }
  $formulaSha = [regex]::Match($formula, 'sha256 "([0-9a-f]{64})"').Groups[1].Value
  if ($formulaSha -ne $sha256) {
      throw "Formula checksum mismatch: expected $sha256, got $formulaSha"
  }
  Write-Output "Formula checksum verified: $formulaSha"
  ```

  Then run:

  ```powershell
  cargo fmt --all -- --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test
  git diff --check
  git status --short --branch
  ```

  Expected: the formula pin matches, all 111 Rust tests pass, and only `Formula/apiwatch.rb`, `README.md`, and `CHANGELOG.md` are tracked changes. If Ruby and Homebrew are available on macOS or Linux, additionally run `ruby -c Formula/apiwatch.rb`, `brew install --build-from-source ./Formula/apiwatch.rb`, and `brew test apiwatch`; otherwise record the skipped checks and Windows/WSL reason in both ignored implementation logs.

- [ ] **Step 5: Commit, push, and verify the post-release formula delivery on the review branch**

  Update the ignored logs with the v0.6.0 archive checksum and validation results, then commit and publish the formula:

  ```powershell
  git add Formula/apiwatch.rb README.md CHANGELOG.md
  git commit -m "Add Homebrew formula"
  $formulaCommit = git rev-parse HEAD
  $releaseBranch = git branch --show-current
  if ($releaseBranch -ne 'codex/v0.6.0-release') {
      throw "Expected codex/v0.6.0-release, got $releaseBranch"
  }
  git push origin $releaseBranch
  $run = gh run list --branch $releaseBranch --limit 20 --json databaseId,headSha,status,conclusion | ConvertFrom-Json |
      Where-Object { $_.headSha -eq $formulaCommit } |
      Select-Object -First 1
  if (-not $run) {
      throw "No GitHub Actions run found for formula commit $formulaCommit"
  }
  gh run watch $run.databaseId --exit-status
  ```

  Expected: `origin/codex/v0.6.0-release` contains the formula commit while `origin/main` is unchanged, and the matching CI run has green `rust` and `action-smoke` jobs.

- [ ] **Step 6: Preserve the review branch for final approval**

  Keep the preserved stale work on its backup branch and confirm the review branch is ready for the final whole-branch review. Do not push to `origin/main` or repoint local `main`:

  ```powershell
  git log --oneline origin/main..HEAD
  git status --short --branch
  git show-ref --verify refs/heads/codex/homebrew-formula-pre-release-backup
  ```

  Expected: the review branch contains the release and formula commits; `origin/main` remains unchanged; `codex/homebrew-formula-pre-release-backup` still preserves the original local-only work; neither the backup branch nor the temporary release branch is deleted in this task.

## Final Verification

- [ ] `v0.6.0` is an annotated remote tag at the clean release snapshot, whose Cargo version is `0.6.0` and changelog section is dated `2026-07-16`.
- [ ] The published GitHub release is non-draft, non-prerelease, uses the verified tag, and contains the v0.6.0 changelog notes with no binary assets.
- [ ] The source-only Homebrew formula is added after the tag, accurately describes the full CLI, and pins the public v0.6.0 source archive plus its exact SHA-256 checksum.
- [ ] README documents only the repository-local `brew install --build-from-source ./Formula/apiwatch.rb` workflow, and the Homebrew changelog entry remains under `Unreleased`.
- [ ] Both release-branch pushes receive green `rust` and `action-smoke` GitHub Actions runs. Ruby/Homebrew validations are either passed on macOS/Linux or explicitly recorded as unavailable in Windows/WSL.
- [ ] `origin/main` remains unchanged during this plan; the final review branch is presented for explicit user approval before it is fast-forwarded to `main`. The pre-release backup branch remains local and is never pushed.
