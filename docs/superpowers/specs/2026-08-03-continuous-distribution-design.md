# Continuous Distribution Track — Design Spec

**Date**: 2026-08-03
**Version**: v0.10.0 / Phase 3

## Overview

Make APIWatch distributable via checksummed binaries, a container image, and
automated crates.io publishing. Update the consumer GitHub Action to prefer
binary downloads. Automate Homebrew and Scoop version bumps. Report both SemVer
and git revision from `--version`.

## Approach

Single `release.yml` workflow triggered by `v*` tag pushes. A build matrix
covers all 6 targets in parallel. A post-build assembly job creates the GitHub
Release, uploads assets, generates checksums, pushes the container, publishes
to crates.io, and bumps package manager metadata.

## Binary Targets

| Target triple | Runner | Build tool |
|---|---|---|
| `x86_64-unknown-linux-gnu` | `ubuntu-latest` | native `cargo` |
| `x86_64-unknown-linux-musl` | `ubuntu-latest` | `cross` |
| `aarch64-unknown-linux-gnu` | `ubuntu-latest` | `cross` |
| `x86_64-apple-darwin` | `macos-13` | native `cargo` |
| `aarch64-apple-darwin` | `macos-latest` | native `cargo` |
| `x86_64-pc-windows-msvc` | `windows-latest` | native `cargo` |

### Artifact naming

- Unix: `apiwatch-{target}.tar.gz`
- Windows: `apiwatch-{target}.zip`

## Release Workflow (`release.yml`)

### Trigger

```yaml
on:
  push:
    tags: ["v*"]
```

### Jobs

1. **Test** — `cargo test --workspace`, `cargo clippy -- -D warnings`. Shared
   preflight gate for all build jobs.

2. **Build** — Matrix of 6 targets. Each job:
   - Checks out the repo at the tag
   - Installs `cross` for cross-compilation targets
   - Installs the appropriate Rust target via `rustup`
   - Builds with `cargo build --release --target {triple}` for native, or
     `cross build --release --target {triple}` for cross
   - Archives the binary as `apiwatch-{target}.{ext}`
   - Uploads the artifact

3. **Release** — Runs after all 6 build jobs succeed:
   - Downloads all 6 artifacts
   - Generates `SHA256SUMS` with `sha256sum`
   - Creates a GitHub Release via `softprops/action-gh-release` with all 7
     assets (6 binaries + SHA256SUMS)
   - Generates release notes from the CHANGELOG entry for the tag's version

4. **Container** — Depends on the Linux musl build:
   - Downloads the musl binary artifact
   - Builds Docker image from `Dockerfile`
   - Pushes to `ghcr.io/hitesh518-collab/apiwatch:{version}` and `:latest`
   - Labels with OCI metadata (version, revision, source)

5. **Cargo Publish** — Depends on test passing and tag version matching Cargo.toml:
   - Runs `cargo publish --dry-run` first
   - Runs `cargo publish --token ${{ secrets.CARGO_REGISTRY_TOKEN }}`
   - Fails if the version is already published (idempotent — retry-safe)

6. **Bump Packages** — Depends on release creation:
   - Checks out the repo at `main`
   - Computes the source tarball SHA256 for the tag
   - Runs `scripts/bump_version.py --version {version} --sha256 {hash}`
   - Commits changes to `Formula/apiwatch.rb`, `Scoop/apiwatch.json`
   - Pushes to `main`

### Pre-release validation gates

- Tag prefix must be `v` followed by valid SemVer (e.g., `v0.10.0`)
- Stripped version must match `Cargo.toml` version exactly
- Release smoke test (`scripts/release_smoke.py`) passes using the built binary
- All lint and test jobs pass

### Caching

Standard Rust cache action keys: `{runner.os}-{target}-cargo-{hash-of-Cargo.lock}`.

## Container Image

### Dockerfile (repo root)

```dockerfile
FROM alpine:3.21
COPY apiwatch /usr/local/bin/apiwatch
ENTRYPOINT ["apiwatch"]
```

- Uses the `x86_64-unknown-linux-musl` binary from the build
- Total image size: ~10 MB
- Pushed to `ghcr.io/hitesh518-collab/apiwatch`

## `--version` with Git Revision

### Current behavior

Uses clap's `#[command(version)]` which prints `apiwatch 0.9.0`.

### Target behavior

Prints `apiwatch 0.9.0 (abc123f)` where `abc123f` is the short git commit
hash.

### Implementation

1. **`build.rs`** (new or existing):
   - Runs `git rev-parse --short HEAD`
   - Sets `GIT_HASH` environment variable for the compiler
   - Falls back to empty string (rendered as `unknown`) if git is unavailable

2. **`src/cli.rs`**:
   - Remove `#[command(version)]` from the `Cli` struct
   - Replace with `#[command(version = custom_version())]` or manually handle
     `--version` via a version function
   - The custom version function reads `env!("CARGO_PKG_VERSION")` and
     `option_env!("GIT_HASH").unwrap_or("unknown")`

## Consumer GitHub Action Update

### Current behavior (`action.yml`)

Builds from source every time:
```yaml
- uses: dtolnay/rust-toolchain@stable
- run: cargo build --release --manifest-path "$ACTION_PATH/Cargo.toml"
```

### Target behavior

1. **Check `apiwatch-version` input** (new, default `latest`)
2. **Download binary** for the runner's OS/arch from GitHub Releases
3. **Verify checksum** against the published SHA256SUMS
4. **Fallback**: if download fails, build from source as today
5. **Run Verify** using the resolved binary path

### New input

| Input | Required | Default | Description |
|---|---|---|---|
| `apiwatch-version` | No | `latest` | Version tag to download (e.g., `v0.10.0`), or `latest` |

### Platform detection in the action

| Runner OS | Arch | Download target |
|---|---|---|
| `ubuntu-latest` | `x86_64` | `x86_64-unknown-linux-gnu` |
| `macos-latest` | `aarch64` | `aarch64-apple-darwin` |
| `macos-13` | `x86_64` | `x86_64-apple-darwin` |
| `windows-latest` | `x86_64` | `x86_64-pc-windows-msvc` |

## Version Bump Script (`scripts/bump_version.py`)

Enhances and replaces `scripts/update_package_metadata.py`.

### Usage

```bash
python scripts/bump_version.py --version 0.10.0 [--sha256 <hash>]
```

### Files updated

| File | What changes |
|---|---|
| `Cargo.toml` | `version = "0.10.0"` |
| `CHANGELOG.md` | Prepends `## v0.10.0 - YYYY-MM-DD` after the header |
| `Formula/apiwatch.rb` | Tag in URL, `sha256` field |
| `Scoop/apiwatch.json` | `version`, `url`, `hash`, `extract_dir` |
| `scripts/release_smoke.py` | Version string assertion |

### Validation

- Version must be numeric SemVer (`major.minor.patch`) without prefix
- SHA256 must be 64 hex chars (when provided)
- Each file must have exactly one version reference to replace
- Fails with a clear error if any file is missing or has unexpected format

## Homebrew and Scoop Strategy

Both formulas remain **source-build** (depend on Rust, run `cargo install`).
The bump script updates the source tarball URL and SHA256 for each release.
No binary bottles are produced — the binary download path is for the GitHub
Action and container users only.

The release workflow auto-commits bumped Formula and Scoop files back to
`main` after each release.

## Files Changed

| File | Action |
|---|---|
| `.github/workflows/release.yml` | Create — tag-driven release workflow |
| `action.yml` | Modify — binary download with fallback |
| `Dockerfile` | Create — Alpine-based container image |
| `build.rs` | Create — git hash injection |
| `src/cli.rs` | Modify — custom version string |
| `scripts/bump_version.py` | Modify — enhanced from `update_package_metadata.py` |
| `scripts/release_smoke.py` | Modify — version string parameterized |

## Non-Goals

- No binary bottles for Homebrew
- No ARM64 Windows or musl ARM64 builds
- No multi-arch container manifests (single arch: x86_64)
- No cargo-binstall support metadata (can add later)
- No changelog auto-generation (manual entries remain)

## Verification

1. Push `v0.10.0` tag → workflow creates GitHub Release with 6 binaries + SHA256SUMS
2. `cargo install apiwatch` installs the published crate
3. `docker pull ghcr.io/hitesh518-collab/apiwatch:0.10.0` works
4. Consumer action with `apiwatch-version: v0.10.0` downloads binary and verifies
5. `apiwatch --version` prints `apiwatch 0.10.0 (abc123f)`
6. `python scripts/bump_version.py --version 0.10.0` updates all 5 files correctly
