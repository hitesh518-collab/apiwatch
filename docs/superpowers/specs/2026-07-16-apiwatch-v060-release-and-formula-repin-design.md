# APIWatch v0.6.0 Release and Formula Repin Design

## Goal

Publish an immutable `v0.6.0` release that contains the current full CLI, then update the repository-owned Homebrew formula to build from that release archive.

## Release Ordering

The formula cannot pin the SHA-256 of the same tag archive that contains the formula: changing the formula changes the archive checksum. Release and formula maintenance therefore happen in this order:

1. Create a v0.6.0 release-preparation commit without the Homebrew formula.
2. Create and push the annotated `v0.6.0` tag on that commit.
3. Publish a non-draft GitHub release using the v0.6.0 changelog notes.
4. Download the immutable tag archive and calculate its SHA-256.
5. Add the formula in the following `main` commit, pinned to the v0.6.0 archive and checksum.

The existing local-only `Add Homebrew formula` commit must not be pushed or tagged with its v0.1.0 source pin. Execution rebuilds it after the release tag with the v0.6.0 archive pin.

## v0.6.0 Release Contract

- `Cargo.toml` package version becomes `0.6.0`.
- `CHANGELOG.md` moves the current non-Homebrew `Unreleased` entries into a dated `## v0.6.0 - 2026-07-16` release section.
- The release includes lockfiles, local and live Verify, the GitHub Action, JSON output, and SARIF output.
- The release does not include a formula that points to a stale source archive, bottles, binary assets, a Homebrew tap, or release automation.
- Create an annotated Git tag named `v0.6.0` and a non-draft GitHub release titled `v0.6.0` using the corresponding changelog notes.

## Post-Release Formula Contract

The post-tag `Formula/apiwatch.rb` uses the exact v0.6.0 GitHub source-archive URL and SHA-256 checksum. Its description, README installation text, and formula smoke test describe the full apiwatch CLI accurately. The Homebrew changelog entry remains under `Unreleased`, because the formula maintenance commit follows the immutable release tag.

The formula remains source-only and repository-local: it uses Homebrew Rust, `std_cargo_args`, and a help-output test. It does not add a tap, bottles, a `head` build, release binaries, or a Homebrew CI job.

## Validation

- Run the existing Rust formatter, Clippy, and full test suite before tagging and after the formula repin.
- Verify the v0.6.0 archive SHA-256 after the tag exists and ensure it matches the post-tag formula declaration.
- Confirm the GitHub release points to the annotated v0.6.0 tag and publishes the versioned changelog notes.
- Run Ruby syntax, `brew install --build-from-source ./Formula/apiwatch.rb`, and `brew test apiwatch` only when Ruby and Homebrew are available on macOS or Linux. Do not install those tools or create a new CI job solely for this validation.

## Deferred Scope

- Bottles and release binary artifacts.
- A Homebrew tap or `brew install <tap>/apiwatch` shortcut.
- Automated release tagging and formula bumping.
- A Homebrew-specific GitHub Actions workflow.
