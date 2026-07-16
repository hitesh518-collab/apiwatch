# APIWatch Homebrew Formula Design

## Goal

Make `apiwatch` installable through Homebrew from a repository-owned formula, without creating a separate tap or release-bottle pipeline.

## Scope

- Add `Formula/apiwatch.rb` to this repository.
- Package the existing `v0.1.0` tagged source using a pinned GitHub source-archive URL and SHA-256 checksum.
- Build the Rust CLI from source with Homebrew-provided Rust.
- Document the repository-local Homebrew installation workflow.
- Verify formula syntax and source integrity alongside the existing Rust release gate.

## Formula Contract

`Formula/apiwatch.rb` defines `Apiwatch < Formula` with:

- A concise description, the GitHub repository homepage, and `Apache-2.0` license.
- A stable `v0.1.0` GitHub source archive URL and its exact SHA-256 checksum.
- `depends_on "rust" => :build`.
- A locked Cargo release build that installs only the `apiwatch` executable into Homebrew's `bin` directory.
- A `test do` block that runs the installed executable with `--help` and asserts a stable command name in the output.

The formula is source-only. It does not define bottles, a `head` build, optional dependencies, action outputs, or runtime configuration.

## User Experience

The formula remains in the main repository rather than a Homebrew tap. The documented installation path is:

```bash
git clone https://github.com/hitesh518-collab/apiwatch.git
cd apiwatch
brew install --build-from-source ./Formula/apiwatch.rb
```

Users receive a source build with the formula's pinned source checksum and `Cargo.lock` dependency resolution. This first delivery does not promise `brew install apiwatch` or `brew install hitesh518-collab/tap/apiwatch`; those require a hosted tap or a future core submission.

## Release Maintenance

Each new apiwatch release updates the formula's source URL, version, and checksum in the same repository. The formula version must always match a released Git tag and must not follow the mutable default branch.

## Validation

- Validate Ruby syntax for `Formula/apiwatch.rb` locally when Ruby is available.
- Download the exact source archive and verify its SHA-256 matches the formula.
- Run the existing Rust formatter, Clippy, and full test suite.
- When Homebrew is available on macOS or Linux, run `brew install --build-from-source ./Formula/apiwatch.rb` followed by `brew test apiwatch`.

The repository does not add a Homebrew-specific CI job in this slice. GitHub-hosted Homebrew installation validation is deferred until a broader release automation task.

## Deferred Scope

- Separate Homebrew tap and `brew install <tap>/apiwatch` shortcut.
- Prebuilt bottles and cross-platform release artifacts.
- Automated formula bumps when a Git tag is published.
- Shell completions and configuration-file support.
