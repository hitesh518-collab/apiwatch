# APIWatch Scoop Manifest Design

## Goal

Add a repository-owned Scoop manifest that source-builds the current APIWatch
release on Windows. It should mirror the repository-local Homebrew model while
letting Scoop install the Rust toolchain dependency automatically.

## Decision

The first Scoop delivery is a local manifest at `Scoop/apiwatch.json`. It
downloads the immutable `v0.6.0` source archive, verifies its SHA-256, builds
the CLI with Cargo, and exposes the resulting executable through a Scoop shim.

The install command is:

```powershell
scoop install ./Scoop/apiwatch.json
```

This is deliberately not a separate Scoop bucket, a binary release, or an
automatic manifest updater. Those options add release and maintenance scope
without improving the first source-build path enough to justify their cost.

## Manifest Contract

`Scoop/apiwatch.json` has the following fixed release contract:

- `version`: `0.6.0`.
- `description`: accurately names APIWatch as a tool that locks, diffs, and
  verifies external API contracts.
- `homepage`: `https://github.com/hitesh518-collab/apiwatch`.
- `license`: `Apache-2.0`.
- `url`: `https://github.com/hitesh518-collab/apiwatch/archive/refs/tags/v0.6.0.tar.gz`.
- `hash`: `243bf768f39dac882b4cdbf02209bc0d95d83e07a81a60e0631bd039646ab948`.
- `extract_dir`: `apiwatch-0.6.0`.
- `depends`: `rust`, so Scoop resolves Cargo before the APIWatch installer
  script runs.
- `installer.script`: runs
  `cargo build --release --locked --manifest-path "$dir\\Cargo.toml"`.
- `bin`: `target\\release\\apiwatch.exe`.

Scoop verifies the archive hash before extraction. The installer builds within
Scoop's versioned app directory; the `bin` entry then creates Scoop's normal
`apiwatch` shim from the built executable. No files are copied to a global
Cargo install directory.

The manifest intentionally omits `checkver` and `autoupdate`. A source archive
checksum cannot be updated safely before its tag exists, so each APIWatch
release must first publish its immutable tag, then update this manifest in the
following repository commit. This is the same ordering used by the Homebrew
formula and avoids a self-referential archive hash.

## Prerequisites And Failure Behavior

Scoop itself and the manifest's `rust` dependency provide the package manager
and Cargo. A Windows Rust source build also requires Microsoft's C++ Build
Tools and a Windows SDK, as documented by Scoop's `rust` package. The README
will state that prerequisite explicitly.

The manifest does not attempt to guess Visual Studio installation paths or add
a fragile compiler preflight. If the required Windows build tools are absent,
Cargo's own error is the authoritative failure and no usable `apiwatch` shim
is created. The pinned download remains hash-verified before Cargo runs.

## Documentation And Changelog

Add a `## Scoop` README section after `## Homebrew` and before `## GitHub
Action`. It documents the local manifest command, automatic Rust dependency,
the Windows C++ Build Tools and Windows SDK prerequisite, and the fact that no
Scoop bucket makes `scoop install apiwatch` available yet.

Add one concise Scoop entry under `## Unreleased` / `### Added` in
`CHANGELOG.md`. It remains unreleased because the manifest is added after the
immutable `v0.6.0` release tag.

## Validation

1. Parse the manifest with PowerShell `ConvertFrom-Json` and structurally
   verify its version, URL, SHA-256, `extract_dir`, Rust dependency, installer
   command, and executable shim path.
2. Download the public `v0.6.0` archive and verify that its SHA-256 matches the
   manifest and that it contains the declared `apiwatch-0.6.0` extraction root.
3. Run `cargo fmt --all -- --check`,
   `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`,
   and `git diff --check`.
4. If Scoop is already available and `apiwatch` is not installed through it,
   run `scoop install ./Scoop/apiwatch.json` followed by `apiwatch --help`.
   Do not install Scoop, overwrite an existing user-managed Scoop APIWatch
   install, or add a Scoop-specific CI job solely for this validation.
5. Record unavailable Scoop or native Windows build-tool validation in the
   ignored implementation log.

## Deferred Scope

- A public or private Scoop bucket.
- Windows binary release assets, installers, or checksums beyond the source
  archive hash.
- Scoop manifest auto-update tooling.
- Scoop-specific GitHub Actions CI.
- Shell completions and configuration files, which are separate v0.6 roadmap
  tasks.
