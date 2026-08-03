# Continuous Distribution Track — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Automate tag-driven binary releases across 6 targets, publish a container image and crates.io crate, update the GitHub Action to download binaries, and report git revision from `--version`.

**Architecture:** A single `release.yml` GitHub Actions workflow triggered by `v*` tags builds 6 platform binaries in parallel via a matrix, generates SHA256SUMS, creates a GitHub Release, publishes to crates.io, builds and pushes an Alpine container image, and auto-bumps Homebrew/Scoop metadata. A `build.rs` injects the short git commit hash for `--version` output. The consumer action gains a binary-download path with source-build fallback.

**Tech Stack:** Rust 1.88, GitHub Actions, `cross` (cross-compilation), Docker, clap 4, Python 3 (bump/version scripts)

## Global Constraints

- Rust MSRV: 1.88.0
- `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --all-features -- -D warnings` must pass
- `cargo test --workspace` must pass
- Tag must be `v` + numeric SemVer matching `Cargo.toml` version
- `--version` output format: `apiwatch <semver> (<short-hash>)`
- Binary naming: `apiwatch-{target}.tar.gz` (unix) or `.zip` (windows)
- Container base: `alpine:3.21`

## File Structure

| File | Action | Responsibility |
|---|---|---|
| `build.rs` | Create | Injects `GIT_HASH` env var at build time |
| `src/cli.rs` | Modify | Custom `--version` string with git hash |
| `src/lib.rs` | Modify | Add `version_string()` public function |
| `Dockerfile` | Create | Alpine-based container using musl binary |
| `.github/workflows/release.yml` | Create | Tag-driven release pipeline |
| `action.yml` | Modify | Binary download path with source-build fallback |
| `scripts/bump_version.py` | Modify | Enhanced version-bump script (from `update_package_metadata.py`) |
| `scripts/tests/test_bump_version.py` | Rename | Updated tests for renamed script |
| `scripts/update_package_metadata.py` | Delete | Replaced by `scripts/bump_version.py` |
| `scripts/tests/test_update_package_metadata.py` | Delete | Renamed to `test_bump_version.py` |
| `scripts/release_smoke.py` | Modify | Parameterize version check |

---

### Task 1: `--version` with Git Revision

**Files:**
- Create: `build.rs`
- Modify: `src/cli.rs:5-12`
- Modify: `src/lib.rs:1-4`

**Interfaces:**
- Produces: `pub fn apiwatch::version_string() -> String` — returns full version string e.g. `"apiwatch 0.9.0 (abc123f)"`
- Produces: `build.rs` sets `GIT_HASH` env var for `option_env!("GIT_HASH")` in lib.rs

- [ ] **Step 1: Create `build.rs`**

```rust
fn main() {
    let hash = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    println!("cargo:rustc-env=GIT_HASH={}", hash);
    println!("cargo:rerun-if-changed=.git/HEAD");
}
```

- [ ] **Step 2: Add `version_string()` to `src/lib.rs`**

Replace the first 4 lines of `src/lib.rs` with:

```rust
#![doc = "Internal APIWatch library. Public interfaces are pre-v1 and unstable."]

pub fn version_string() -> String {
    let ver = env!("CARGO_PKG_VERSION");
    match option_env!("GIT_HASH") {
        Some("") | None => format!("apiwatch {ver}"),
        Some(hash) => format!("apiwatch {ver} ({hash})"),
    }
}
```

- [ ] **Step 3: Update `src/cli.rs` to use custom version**

Change the `Cli` struct derive attributes from:
```rust
#[derive(Debug, Parser)]
#[command(name = "apiwatch")]
#[command(about = "Lock, diff, and verify the APIs your code depends on.")]
#[command(version)]
```
To:
```rust
#[derive(Debug, Parser)]
#[command(name = "apiwatch")]
#[command(about = "Lock, diff, and verify the APIs your code depends on.")]
#[command(version = crate::version_string())]
```

- [ ] **Step 4: Build and verify version output**

Run: `cargo build --release`

Run: `./target/release/apiwatch --version`

Expected: Output like `apiwatch 0.9.0 (abc123f)` where the hash is the current short commit.

Run: `cargo test --workspace` — all existing tests pass.

- [ ] **Step 5: Verify --help still works**

Run: `./target/release/apiwatch --help`

Expected: Help output includes the version string in the first line or footer.

- [ ] **Step 6: Commit**

```bash
git add build.rs src/cli.rs src/lib.rs
git commit -m "feat: report git revision in --version output"
```

---

### Task 2: Version Bump Script

**Files:**
- Modify: `scripts/update_package_metadata.py` → `scripts/bump_version.py`
- Rename: `scripts/tests/test_update_package_metadata.py` → `scripts/tests/test_bump_version.py`
- Delete: `scripts/update_package_metadata.py`
- Delete: `scripts/tests/test_update_package_metadata.py`

**Interfaces:**
- Consumes: None
- Produces: `python scripts/bump_version.py --version <X.Y.Z>` — bumps `Cargo.toml`, `CHANGELOG.md` (pre-release usage). `python scripts/bump_version.py --version <X.Y.Z> --sha256 <hash>` — additionally bumps `Formula/apiwatch.rb`, `Scoop/apiwatch.json` (post-release usage). Script is idempotent when run twice with same version.

- [ ] **Step 1: Create `scripts/bump_version.py`**

```python
#!/usr/bin/env python3
import argparse
import datetime
import json
from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[1]
VERSION = re.compile(r"0|[1-9][0-9]*")
SHA256 = re.compile(r"[0-9a-f]{64}")


def _bump_cargo_toml(root, version):
    path = root / "Cargo.toml"
    content = path.read_text(encoding="utf-8")
    if f'version = "{version}"' in content:
        return
    new_content, count = re.subn(
        r'(?m)^version = "[0-9]+\.[0-9]+\.[0-9]+"$',
        f'version = "{version}"',
        content,
        count=1,
    )
    if count != 1:
        raise ValueError("Cargo.toml must contain one version field")
    path.write_text(new_content, encoding="utf-8", newline="\n")


def _bump_changelog(root, version):
    path = root / "CHANGELOG.md"
    content = path.read_text(encoding="utf-8")
    if f"## v{version} -" in content:
        return
    today = datetime.date.today().isoformat()
    header = f"## v{version} - {today}"
    new_content = re.sub(
        r"^# Changelog\n",
        f"# Changelog\n\n{header}\n",
        content,
        count=1,
    )
    path.write_text(new_content, encoding="utf-8", newline="\n")


def _bump_formula(root, version, sha256):
    path = root / "Formula" / "apiwatch.rb"
    content = path.read_text(encoding="utf-8")
    new_content, url_count = re.subn(
        r"/v[0-9]+\.[0-9]+\.[0-9]+\.tar\.gz",
        f"/v{version}.tar.gz",
        content,
        count=1,
    )
    new_content, hash_count = re.subn(
        r'(?m)^  sha256 "[0-9a-f]{64}"$',
        f'  sha256 "{sha256}"',
        new_content,
        count=1,
    )
    if url_count != 1 or hash_count != 1:
        raise ValueError("formula must contain one release URL and one SHA-256")
    path.write_text(new_content, encoding="utf-8", newline="\n")


def _bump_scoop(root, version, sha256):
    path = root / "Scoop" / "apiwatch.json"
    scoop = json.loads(path.read_text(encoding="utf-8"))
    scoop["version"] = version
    scoop["url"] = (
        "https://github.com/hitesh518-collab/apiwatch/"
        f"archive/refs/tags/v{version}.tar.gz"
    )
    scoop["hash"] = sha256
    scoop["extract_dir"] = f"apiwatch-{version}"
    path.write_text(
        json.dumps(scoop, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )


def bump(root, version, sha256):
    if not all(VERSION.fullmatch(part) for part in version.split(".")):
        raise ValueError("version must be numeric SemVer without a prefix")
    if len(version.split(".")) != 3:
        raise ValueError("version must contain major.minor.patch")
    if sha256 is not None and not SHA256.fullmatch(sha256):
        raise ValueError("sha256 must be 64 lowercase hexadecimal characters")

    _bump_cargo_toml(root, version)
    _bump_changelog(root, version)
    if sha256 is not None:
        _bump_formula(root, version, sha256)
        _bump_scoop(root, version, sha256)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--sha256", default=None)
    parser.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args()
    bump(args.root, args.version, args.sha256)


if __name__ == "__main__":
    main()
```

- [ ] **Step 2: Create `scripts/tests/test_bump_version.py`**

```python
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "bump_version.py"
SPEC = importlib.util.spec_from_file_location("bump_version", SCRIPT)
bumper = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(bumper)


class BumpVersionTests(unittest.TestCase):
    def test_bumps_all_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)

            # Create Cargo.toml
            (root / "Cargo.toml").write_text(
                'version = "0.6.0"\n', encoding="utf-8"
            )

            # Create CHANGELOG.md
            (root / "CHANGELOG.md").write_text(
                "# Changelog\n\n## v0.6.0\n\nstuff\n", encoding="utf-8"
            )

            # Create Formula/apiwatch.rb
            formula_dir = root / "Formula"
            formula_dir.mkdir()
            (formula_dir / "apiwatch.rb").write_text(
                '  url "https://github.com/o/r/archive/refs/tags/v0.6.0.tar.gz"\n'
                '  sha256 "' + ("a" * 64) + '"\n',
                encoding="utf-8",
            )

            # Create Scoop/apiwatch.json
            scoop_dir = root / "Scoop"
            scoop_dir.mkdir()
            (scoop_dir / "apiwatch.json").write_text(
                json.dumps(
                    {
                        "version": "0.6.0",
                        "url": "https://github.com/o/r/archive/refs/tags/v0.6.0.tar.gz",
                        "hash": "a" * 64,
                        "extract_dir": "apiwatch-0.6.0",
                    }
                ),
                encoding="utf-8",
            )

            # Create scripts/release_smoke.py (should NOT be modified by bump_version.py)
            scripts_dir = root / "scripts"
            scripts_dir.mkdir()
            (scripts_dir / "release_smoke.py").write_text(
                'if "apiwatch 0.6.0" not in version:\n'
                '    raise RuntimeError("bad version")\n',
                encoding="utf-8",
            )

            bumper.bump(root, "0.7.0", "b" * 64)

            # Verify Cargo.toml
            cargo = (root / "Cargo.toml").read_text(encoding="utf-8")
            self.assertEqual(cargo, 'version = "0.7.0"\n')

            # Verify CHANGELOG.md
            changelog = (root / "CHANGELOG.md").read_text(encoding="utf-8")
            self.assertTrue(changelog.startswith("# Changelog\n\n## v0.7.0 -"))

            # Verify Formula
            formula = (formula_dir / "apiwatch.rb").read_text(encoding="utf-8")
            self.assertIn("/v0.7.0.tar.gz", formula)
            self.assertIn('sha256 "' + ("b" * 64) + '"', formula)

            # Verify Scoop
            scoop = json.loads(
                (scoop_dir / "apiwatch.json").read_text(encoding="utf-8")
            )
            self.assertEqual(scoop["version"], "0.7.0")
            self.assertEqual(scoop["hash"], "b" * 64)
            self.assertEqual(scoop["extract_dir"], "apiwatch-0.7.0")
            self.assertTrue(scoop["url"].endswith("/v0.7.0.tar.gz"))

            # Verify release_smoke.py was NOT modified
            smoke = (scripts_dir / "release_smoke.py").read_text(encoding="utf-8")
            self.assertIn('"apiwatch 0.6.0"', smoke)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 3: Run tests to verify**

Run: `python -m scripts.tests.test_bump_version`

Expected: All tests pass.

- [ ] **Step 4: Delete old files and rename**

```bash
git rm scripts/update_package_metadata.py
git rm scripts/tests/test_update_package_metadata.py
git add scripts/bump_version.py scripts/tests/test_bump_version.py
```

Note: The old files are removed and new ones added in the same commit.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat: replace update_package_metadata.py with bump_version.py covering all metadata files"
```

---

### Task 3: Container Image

**Files:**
- Create: `Dockerfile`

**Interfaces:**
- Consumes: `apiwatch` binary at repo root (copied into the image during release workflow)
- Produces: Container image `ghcr.io/hitesh518-collab/apiwatch:{version}` and `:latest`

- [ ] **Step 1: Create `Dockerfile`**

```dockerfile
FROM alpine:3.21
COPY apiwatch /usr/local/bin/apiwatch
ENTRYPOINT ["apiwatch"]
```

- [ ] **Step 2: Verify Dockerfile syntax (no build needed locally)**

Run: `docker build --check .` (if available, otherwise verify the file is syntactically valid Dockerfile)

- [ ] **Step 3: Commit**

```bash
git add Dockerfile
git commit -m "feat: add Alpine-based Dockerfile for container distribution"
```

---

### Task 4: Release Workflow

**Files:**
- Create: `.github/workflows/release.yml`

**Interfaces:**
- Consumes: `build.rs` (Task 1), `Dockerfile` (Task 3), `scripts/bump_version.py` (Task 2), `scripts/release_smoke.py`
- Produces: GitHub Release assets, `ghcr.io` container image, crates.io publish, bumped Formula/Scoop on `main`

**Prerequisites:**
- Repository secret `CARGO_REGISTRY_TOKEN` must be set with a crates.io API token before the first release

- [ ] **Step 1: Create `.github/workflows/release.yml`**

```yaml
name: Release

on:
  push:
    tags: ["v*"]

permissions:
  contents: write
  packages: write

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: stable
          components: rustfmt, clippy
      - run: cargo fmt --all -- --check
      - run: cargo clippy --workspace --all-targets --all-features -- -D warnings
      - run: cargo test --workspace

  build:
    needs: test
    runs-on: ${{ matrix.runner }}
    strategy:
      matrix:
        include:
          - target: x86_64-unknown-linux-gnu
            runner: ubuntu-latest
            ext: tar.gz
            use-cross: false
          - target: x86_64-unknown-linux-musl
            runner: ubuntu-latest
            ext: tar.gz
            use-cross: true
          - target: aarch64-unknown-linux-gnu
            runner: ubuntu-latest
            ext: tar.gz
            use-cross: true
          - target: x86_64-apple-darwin
            runner: macos-13
            ext: tar.gz
            use-cross: false
          - target: aarch64-apple-darwin
            runner: macos-latest
            ext: tar.gz
            use-cross: false
          - target: x86_64-pc-windows-msvc
            runner: windows-latest
            ext: zip
            use-cross: false
    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: stable
          targets: ${{ matrix.target }}

      - name: Install cross
        if: ${{ matrix.use-cross }}
        run: cargo install cross

      - uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-${{ matrix.target }}-cargo-${{ hashFiles('Cargo.lock') }}

      - name: Build (native)
        if: ${{ !matrix.use-cross }}
        run: cargo build --release --locked --target ${{ matrix.target }}

      - name: Build (cross)
        if: ${{ matrix.use-cross }}
        run: cross build --release --locked --target ${{ matrix.target }}

      - name: Package (unix)
        if: ${{ matrix.ext == 'tar.gz' }}
        run: |
          cd target/${{ matrix.target }}/release
          tar czf ../../../apiwatch-${{ matrix.target }}.tar.gz apiwatch

      - name: Package (windows)
        if: ${{ matrix.ext == 'zip' }}
        shell: pwsh
        run: |
          cd target\${{ matrix.target }}\release
          7z a ../../../apiwatch-${{ matrix.target }}.zip apiwatch.exe

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: apiwatch-${{ matrix.target }}
          path: apiwatch-${{ matrix.target }}.${{ matrix.ext }}

  release:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: actions/download-artifact@v4
        with:
          path: artifacts

      - name: Flatten binaries
        run: |
          mkdir -p dist
          find artifacts -type f \( -name '*.tar.gz' -o -name '*.zip' \) -exec cp {} dist/ \;

      - name: Generate SHA256SUMS
        working-directory: dist
        run: sha256sum * > ../SHA256SUMS

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          files: |
            dist/*
            SHA256SUMS
          generate_release_notes: true

  container:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: actions/download-artifact@v4
        with:
          name: apiwatch-x86_64-unknown-linux-musl
          path: binary

      - name: Prepare binary
        run: |
          tar xzf binary/apiwatch-x86_64-unknown-linux-musl.tar.gz
          chmod +x apiwatch

      - name: Log in to ghcr.io
        uses: docker/login-action@v3
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      - name: Extract metadata
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: ghcr.io/${{ github.repository }}
          tags: |
            type=semver,pattern={{version}}
            type=semver,pattern={{major}}.{{minor}}
            type=raw,value=latest

      - name: Build and push
        uses: docker/build-push-action@v6
        with:
          context: .
          push: true
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}

  cargo-publish:
    needs: test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Extract version from tag
        id: version
        run: |
          TAG="${{ github.ref_name }}"
          VERSION="${TAG#v}"
          echo "version=$VERSION" >> "$GITHUB_OUTPUT"

      - name: Verify tag matches Cargo.toml
        run: |
          CARGO_VER=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
          TAG_VER="${{ steps.version.outputs.version }}"
          if [ "$CARGO_VER" != "$TAG_VER" ]; then
            echo "Cargo.toml version ($CARGO_VER) != tag version ($TAG_VER)"
            exit 1
          fi

      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: stable

      - name: Dry-run publish
        run: cargo publish --dry-run

      - name: Publish to crates.io
        run: cargo publish --token ${{ secrets.CARGO_REGISTRY_TOKEN }}

  bump-packages:
    needs: release
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v4
        with:
          ref: main
          token: ${{ secrets.GITHUB_TOKEN }}

      - name: Extract version from tag
        id: version
        run: |
          TAG="${{ github.ref_name }}"
          VERSION="${TAG#v}"
          echo "version=$VERSION" >> "$GITHUB_OUTPUT"

      - name: Compute source tarball SHA256
        id: sha
        run: |
          URL="https://github.com/${{ github.repository }}/archive/refs/tags/${{ github.ref_name }}.tar.gz"
          HASH=$(curl -sL "$URL" | sha256sum | cut -d' ' -f1)
          echo "sha256=$HASH" >> "$GITHUB_OUTPUT"

      - name: Run bump_version.py
        run: |
          python scripts/bump_version.py \
            --version "${{ steps.version.outputs.version }}" \
            --sha256 "${{ steps.sha.outputs.sha256 }}"

      - name: Commit and push
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"
          git add Cargo.toml CHANGELOG.md Formula/apiwatch.rb Scoop/apiwatch.json
          git commit -m "chore: bump package metadata to v${{ steps.version.outputs.version }}"
          git push
```

- [ ] **Step 2: Verify YAML syntax**

Run: `python -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"`

Or use a YAML linter if available.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add tag-driven release workflow with binaries, container, and crates.io"
```

---

### Task 5: Consumer GitHub Action — Binary Download

**Files:**
- Modify: `action.yml`

**Interfaces:**
- Consumes: GitHub Release artifacts named `apiwatch-{target}.{ext}`, SHA256SUMS (Task 4)
- Produces: Same Verify behavior but with binary download as default path

- [ ] **Step 1: Update `action.yml`**

Replace the entire file content:

```yaml
name: apiwatch verify
description: Verify a local or live OpenAPI contract against a named api.lock entry.

inputs:
  openapi:
    description: OpenAPI input or local JSON body, selected by the named lock entry provenance.
    required: true
  name:
    description: Named api.lock entry to verify.
    required: true
  lock:
    description: api.lock path relative to the working directory.
    required: false
    default: api.lock
  working-directory:
    description: Consumer repository directory in which Verify runs.
    required: false
    default: .
  sarif-file:
    description: Relative SARIF output path within working-directory; enables Code Scanning upload when set.
    required: false
    default: ""
  apiwatch-version:
    description: APIWatch version tag to download (e.g. v0.10.0), or 'latest'.
    required: false
    default: latest

runs:
  using: composite
  steps:
    - name: Detect platform target
      id: platform
      shell: bash
      run: |
        case "$RUNNER_OS" in
          Linux)   TARGET="x86_64-unknown-linux-gnu" ;;
          macOS)
            if [ "$(uname -m)" = "arm64" ]; then
              TARGET="aarch64-apple-darwin"
            else
              TARGET="x86_64-apple-darwin"
            fi
            ;;
          Windows) TARGET="x86_64-pc-windows-msvc" ;;
          *)       echo "::error::unsupported runner OS: $RUNNER_OS"; exit 2 ;;
        esac
        echo "target=$TARGET" >> "$GITHUB_OUTPUT"

    - name: Download apiwatch binary
      id: download
      shell: bash
      continue-on-error: true
      env:
        VERSION: ${{ inputs.apiwatch-version }}
        TARGET: ${{ steps.platform.outputs.target }}
        GH_TOKEN: ${{ github.token }}
      run: |
        if [ "$VERSION" = "latest" ]; then
          RELEASE="latest/download"
        else
          RELEASE="download/$VERSION"
        fi

        EXT="tar.gz"
        if [ "$RUNNER_OS" = "Windows" ]; then
          EXT="zip"
        fi

        URL="https://github.com/${{ github.action_repository }}/releases/$RELEASE/apiwatch-${TARGET}.${EXT}"
        echo "Downloading $URL"
        gh release download "$VERSION" \
          --repo "${{ github.action_repository }}" \
          --pattern "apiwatch-${TARGET}.${EXT}" \
          --dir "$RUNNER_TEMP"

        if [ "$RUNNER_OS" = "Windows" ]; then
          unzip -o "$RUNNER_TEMP/apiwatch-${TARGET}.zip" -d "$RUNNER_TEMP"
        else
          tar xzf "$RUNNER_TEMP/apiwatch-${TARGET}.tar.gz" -C "$RUNNER_TEMP"
        fi

        chmod +x "$RUNNER_TEMP/apiwatch" 2>/dev/null || true
        echo "binary=$RUNNER_TEMP/apiwatch" >> "$GITHUB_OUTPUT"

    - name: Install Rust (source-build fallback)
      if: ${{ steps.download.outcome == 'failure' }}
      uses: dtolnay/rust-toolchain@stable

    - name: Build apiwatch from source (fallback)
      if: ${{ steps.download.outcome == 'failure' }}
      shell: bash
      env:
        ACTION_PATH: ${{ github.action_path }}
      run: cargo build --release --manifest-path "$ACTION_PATH/Cargo.toml"

    - name: Resolve binary path
      id: binary
      shell: bash
      run: |
        if [ "${{ steps.download.outcome }}" = "success" ]; then
          echo "path=${{ steps.download.outputs.binary }}" >> "$GITHUB_OUTPUT"
        else
          echo "path=${{ github.action_path }}/target/release/apiwatch" >> "$GITHUB_OUTPUT"
        fi

    - name: Verify API contract
      if: ${{ inputs.sarif-file == '' }}
      shell: bash
      working-directory: ${{ inputs.working-directory }}
      env:
        OPENAPI: ${{ inputs.openapi }}
        API_NAME: ${{ inputs.name }}
        LOCK: ${{ inputs.lock }}
      run: '"${{ steps.binary.outputs.path }}" verify "$OPENAPI" --name "$API_NAME" --lock "$LOCK"'

    - name: Generate SARIF
      if: ${{ inputs.sarif-file != '' }}
      shell: bash
      working-directory: ${{ inputs.working-directory }}
      env:
        OPENAPI: ${{ inputs.openapi }}
        API_NAME: ${{ inputs.name }}
        LOCK: ${{ inputs.lock }}
        SARIF_FILE: ${{ inputs.sarif-file }}
      run: |
        case "$SARIF_FILE" in
          /*|..|../*|*/..|*/../*)
            echo "error: sarif-file must be a relative path within working-directory" >&2
            exit 2
            ;;
        esac
        mkdir -p -- "$(dirname "$SARIF_FILE")"
        set +e
        "${{ steps.binary.outputs.path }}" verify "$OPENAPI" --name "$API_NAME" --lock "$LOCK" --format sarif > "$SARIF_FILE"
        status=$?
        set -e
        if [ "$status" -eq 2 ]; then
          exit 2
        fi
        echo "APIWATCH_SARIF_EXIT_CODE=$status" >> "$GITHUB_ENV"

    - name: Upload SARIF
      if: ${{ inputs.sarif-file != '' && env.APIWATCH_SARIF_EXIT_CODE != '' }}
      uses: github/codeql-action/upload-sarif@v4
      with:
        sarif_file: ${{ inputs.working-directory }}/${{ inputs.sarif-file }}
        category: apiwatch-${{ inputs.name }}

    - name: Report Verify result
      if: ${{ inputs.sarif-file != '' && env.APIWATCH_SARIF_EXIT_CODE != '' }}
      shell: bash
      env:
        APIWATCH_SARIF_EXIT_CODE: ${{ env.APIWATCH_SARIF_EXIT_CODE }}
      run: exit "$APIWATCH_SARIF_EXIT_CODE"
```

- [ ] **Step 2: Verify action-smoke CI job still passes**

The existing `action-smoke` job in `ci.yml` uses `uses: ./` which runs the action from the current checkout. After this change, since the repo won't have a release yet, the download will fail and fall back to source build. The smoke test should still pass.

Run locally (conceptual — requires GitHub Actions): review the fallback path logic.

- [ ] **Step 3: Commit**

```bash
git add action.yml
git commit -m "feat: add binary download path to consumer action with source-build fallback"
```

---

### Task 6: Release Smoke Test — Parameterize Version

**Files:**
- Modify: `scripts/release_smoke.py:49`

**Interfaces:**
- Consumes: None (version read from Cargo.toml at runtime)
- Produces: Same smoke test, version test is flexible

- [ ] **Step 1: Update the version check in `scripts/release_smoke.py`**

Change line 49 from:
```python
if "apiwatch 0.7.0" not in version:
    raise RuntimeError(f"unexpected version output: {version}")
```

To:
```python
import re
if not re.fullmatch(r"apiwatch \d+\.\d+\.\d+( \([0-9a-f]+\))?\n?$", version):
    raise RuntimeError(f"unexpected version output: {version!r}")
```

- [ ] **Step 2: Verify the regex matches expected formats**

Run: `python -c "import re; p = r'apiwatch \d+\.\d+\.\d+( \([0-9a-f]+\))?\n?$'; assert re.fullmatch(p, 'apiwatch 0.9.0\n'); assert re.fullmatch(p, 'apiwatch 0.9.0 (abc123f)\n'); print('OK')"`

Expected: `OK`

- [ ] **Step 3: Commit**

```bash
git add scripts/release_smoke.py
git commit -m "fix: parameterize version check in release smoke test"
```

---

### Task 7: Cross-Compilation Support — `Cross.toml`

**Files:**
- Create: `Cross.toml`

**Interfaces:**
- Consumes: None
- Produces: `cross` configuration used by `cross build` in release workflow

Note: The `cross` tool handles most targets with its pre-built Docker images. `Cross.toml` is usually not required, but creating an empty one documents the intent and allows future customization.

- [ ] **Step 1: Create `Cross.toml`**

```toml
[build]
xargo = false
```

This is intentionally minimal — `cross` uses its own images for musl and aarch64.

- [ ] **Step 2: Commit**

```bash
git add Cross.toml
git commit -m "build: add Cross.toml for cross-compilation targets"
```

---

### Task 8: End-to-End Verification

**Files:**
- No new files — verifies all prior tasks

- [ ] **Step 1: Run full Rust test suite**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

Expected: All pass.

- [ ] **Step 2: Run Python test suite**

```bash
python -m unittest discover -s scripts/tests -p "test_bump_version.py"
```

Expected: All pass.

- [ ] **Step 3: Verify `--version` output**

```bash
cargo build --release
./target/release/apiwatch --version
```

Expected: `apiwatch 0.9.0 (<short hash>)` with the current commit hash.

- [ ] **Step 4: Verify Dockerfile builds locally**

```bash
cp target/release/apiwatch .
docker build -t apiwatch:local .
docker run --rm apiwatch:local --version
```

Expected: Same version output as step 3.

- [ ] **Step 5: Commit (if any cleanup)**

```bash
git status
git add -A
git commit -m "chore: final verification cleanup"
```
