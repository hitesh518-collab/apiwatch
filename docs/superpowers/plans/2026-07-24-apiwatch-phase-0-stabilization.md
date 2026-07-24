# APIWatch Phase 0 Stabilization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Use
> superpowers:test-driven-development for Tasks 1, 2, 4, 5, and 7. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prepare an honest, tested, release-ready APIWatch v0.7.0 candidate,
then stop for explicit approval before any public push or tag.

**Architecture:** Correct the two release-blocking CLI boundaries in the
existing command dispatch and OpenAPI preflight, declare and enforce the real
Rust floor, and add an offline-by-default compatibility harness around
commit-pinned public specifications. Keep release tooling and documentation
separate from contract semantics; public release and post-tag package hashes
remain approval-gated.

**Tech Stack:** Rust 2021, Cargo, Python 3 standard library, GitHub Actions,
Markdown, Homebrew Ruby formula, Scoop JSON

## Global Constraints

- Phase 0 is corrective; do not implement Phase 1+ semantics.
- Preserve exit codes `0` clean, `1` drift, and `2` invalid/operational error.
- Preserve observed JSON schema version 2 and SARIF 2.1.0.
- Never expose captured JSON values or dynamic map keys.
- Accept OpenAPI 3.0 only; explicitly reject 3.1 without implementing it.
- Verify Rust 1.86 before declaring it as MSRV.
- Normal `cargo test` must remain offline.
- Compatibility URLs must contain immutable 40-character upstream commits.
- Compatibility files stay in gitignored `.compat-cache/`.
- Do not silently reclassify a known compatibility failure.
- Do not modify semantic diff rules D-01 through D-11.
- Do not migrate `serde_yaml`.
- Do not publish to crates.io or add binary/container distribution.
- Do not push `main`, create a public tag, or update public package metadata
  before the explicit publication gate.
- Preserve unrelated user changes and historical design/plan records.

## File Map

- Modify `src/main.rs`: observed Verify formatter parity.
- Modify `src/openapi/mod.rs`: raw OpenAPI 3.0/3.1 preflight.
- Modify `tests/cli_verify.rs`: observed-match and Verify 3.1 integration tests.
- Modify `tests/cli_diff.rs`: diff 3.1 integration test.
- Modify `tests/cli_lock.rs`: lock 3.1 integration test.
- Create `testdata/openapi/unsupported_31.yaml`: valid minimal 3.1 fixture.
- Modify `Cargo.toml`: Rust 1.86 and later crate version 0.7.0.
- Modify `Cargo.lock`: crate version 0.7.0.
- Modify `.github/workflows/ci.yml`: MSRV and compatibility jobs.
- Modify `.gitignore`: compatibility cache.
- Create `compat/specs.json`: immutable compatibility corpus manifest.
- Create `scripts/fetch_compat_specs.py`: verified corpus fetcher.
- Create `scripts/tests/test_fetch_compat_specs.py`: fetcher unit tests.
- Create `tests/compat.rs`: ignored-by-default real-world smoke tests.
- Create `scripts/release_smoke.py`: temporary-prefix release smoke test.
- Create `scripts/update_package_metadata.py`: deterministic post-tag metadata
  updater.
- Create `scripts/tests/test_update_package_metadata.py`: updater unit tests.
- Modify `README.md`: MSRV, v0.7.0 status, and D-01–D-19 register.
- Modify `CHANGELOG.md`: v0.7.0 release notes.
- Review and modify `action.yml` only if current wording is inaccurate.
- Modify `Formula/apiwatch.rb` and `Scoop/apiwatch.json` only after public tag
  approval and archive hashing.
- Create/update
  `implementation-log/2026-07-24-phase-0-stabilization.md`: local status.

---

### Task 1: Honor Output Format for Matching Observed Verify

**Files:**
- Modify: `tests/cli_verify.rs:338`
- Modify: `src/main.rs:91-113`

**Interfaces:**
- Consumes: `OutputFormat`, `ObservedChange`, existing observed renderers
- Produces: matching text/JSON/SARIF output with unchanged schemas and exits

- [ ] **Step 1: Add failing JSON-match integration test**

Add after `verify_observed_json_body_with_matching_shape`:

```rust
#[test]
fn verify_matching_observed_json_honors_json_format() {
    let lock = observed_lock_path();
    record_portfolio(&lock);
    let lock_arg = lock.to_str().expect("temp path should be valid UTF-8");

    let output = verify_command(
        "testdata/observed/portfolio-matching.json",
        "portfolio",
        lock_arg,
    )
    .args(["--format", "json"])
    .output()
    .expect("verify should run");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(
        parse_json_output(&output),
        json!({
            "version": 2,
            "command": "verify",
            "name": "portfolio",
            "provenance": "observed",
            "summary": {"breaking": 0},
            "changes": []
        })
    );

    fs::remove_file(lock).ok();
}
```

- [ ] **Step 2: Run the focused JSON test and confirm red**

Run:

```powershell
cargo test --test cli_verify verify_matching_observed_json_honors_json_format -- --exact
```

Expected: FAIL because stdout is `Verified portfolio`, not JSON.

- [ ] **Step 3: Add failing SARIF-match integration test**

Add:

```rust
#[test]
fn verify_matching_observed_json_honors_sarif_format() {
    let lock = observed_lock_path();
    record_portfolio(&lock);
    let lock_arg = lock.to_str().expect("temp path should be valid UTF-8");

    let output = verify_command(
        "testdata/observed/portfolio-matching.json",
        "portfolio",
        lock_arg,
    )
    .args(["--format", "sarif"])
    .output()
    .expect("verify should run");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let rendered = parse_json_output(&output);
    assert_eq!(rendered["version"], "2.1.0");
    assert_eq!(
        rendered["runs"][0]["results"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        sarif_rule_ids(&rendered),
        vec![
            "apiwatch/verify-observed-missing-required-field",
            "apiwatch/verify-observed-incompatible-shape"
        ]
    );

    fs::remove_file(lock).ok();
}
```

- [ ] **Step 4: Run the focused SARIF test and confirm red**

Run:

```powershell
cargo test --test cli_verify verify_matching_observed_json_honors_sarif_format -- --exact
```

Expected: FAIL because stdout is plain text, not SARIF JSON.

- [ ] **Step 5: Implement one observed formatter path**

Replace the observed branch in `src/main.rs` from `let changes = ...` through
its return with:

```rust
let changes = observed::compare(expected, &current);
let has_changes = !changes.is_empty();
let rendered = match format {
    OutputFormat::Text if changes.is_empty() => {
        format!("Verified {}\n", target.name())
    }
    OutputFormat::Text => output::render_observed_verify_changes(&changes),
    OutputFormat::Json => {
        output::render_observed_verify_changes_json(target.name(), &changes)?
    }
    OutputFormat::Sarif => output::render_observed_verify_changes_sarif(
        &lock_path,
        target.name(),
        &changes,
    )?,
};
print!("{rendered}");
return Ok(if has_changes { 1 } else { 0 });
```

Do not modify `src/output/mod.rs`; its JSON and SARIF renderers already support
an empty change vector.

- [ ] **Step 6: Run observed Verify integration tests**

Run:

```powershell
cargo test --test cli_verify observed -- --nocapture
```

Expected: all observed Verify tests PASS, including existing privacy checks.

- [ ] **Step 7: Run formatting and commit**

Run:

```powershell
cargo fmt --all -- --check
git add src/main.rs tests/cli_verify.rs
git commit -m "fix: honor observed Verify output format"
```

### Task 2: Reject OpenAPI 3.1 Before Typed Deserialization

**Files:**
- Create: `testdata/openapi/unsupported_31.yaml`
- Modify: `tests/cli_diff.rs`
- Modify: `tests/cli_lock.rs`
- Modify: `tests/cli_verify.rs`
- Modify: `src/openapi/mod.rs:18-108`

**Interfaces:**
- Consumes: raw JSON/YAML input and current path preflight
- Produces: OpenAPI 3.0-only loader with stable 3.1 error

- [ ] **Step 1: Add minimal OpenAPI 3.1 fixture**

Create `testdata/openapi/unsupported_31.yaml`:

```yaml
openapi: 3.1.0
info:
  title: Unsupported OpenAPI 3.1
  version: 1.0.0
paths:
  /users:
    get:
      responses:
        "200":
          description: OK
  /zeta:
    get:
      responses:
        "200":
          description: OK
```

- [ ] **Step 2: Add failing diff test**

Add to `tests/cli_diff.rs`:

```rust
#[test]
fn diff_rejects_openapi_31_with_an_accurate_message() {
    let mut command = Command::cargo_bin("apiwatch").expect("binary should build");

    command
        .args([
            "diff",
            "testdata/openapi/unsupported_31.yaml",
            "testdata/openapi/unsupported_31.yaml",
        ])
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains(
            "OpenAPI 3.1 is not yet supported",
        ));
}
```

- [ ] **Step 3: Add failing lock test**

Add to `tests/cli_lock.rs`:

```rust
#[test]
fn lock_rejects_openapi_31_with_an_accurate_message() {
    let output = temp_lock_path("unsupported-31");
    let output_arg = output.to_str().expect("temp path should be valid UTF-8");

    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args([
            "lock",
            "testdata/openapi/unsupported_31.yaml",
            "--name",
            "users",
            "--output",
            output_arg,
        ])
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains(
            "OpenAPI 3.1 is not yet supported",
        ));

    fs::remove_file(output).ok();
}
```

Use the existing `temp_lock_path` and `std::fs` imports in that file.

- [ ] **Step 4: Add failing Verify test**

Add to `tests/cli_verify.rs`:

```rust
#[test]
fn verify_rejects_openapi_31_with_an_accurate_message() {
    verify_command(
        "testdata/openapi/unsupported_31.yaml",
        "users",
        "testdata/lock/verify_users.lock",
    )
    .assert()
    .code(2)
    .stdout(predicate::str::is_empty())
    .stderr(predicate::str::contains(
        "OpenAPI 3.1 is not yet supported",
    ));
}
```

- [ ] **Step 5: Run all three focused tests and confirm red**

Run:

```powershell
cargo test openapi_31_with_an_accurate_message -- --nocapture
```

Expected: all three new tests FAIL because current code accepts the minimal
3.1 fixture.

- [ ] **Step 6: Extend raw preflight**

Rename `validate_raw_openapi_paths` to `validate_raw_openapi` and replace it
with:

```rust
fn validate_raw_openapi(raw: &str, is_json: bool) -> Result<()> {
    if is_json {
        let document: serde_json::Value =
            serde_json::from_str(raw).context("failed to parse OpenAPI JSON")?;
        validate_openapi_version(
            document
                .get("openapi")
                .and_then(serde_json::Value::as_str),
        )?;
        let Some(paths) = document
            .get("paths")
            .and_then(serde_json::Value::as_object)
        else {
            return Ok(());
        };

        for path in paths.keys() {
            validate_raw_openapi_path(path)?;
        }
    } else {
        let document: serde_yaml::Value =
            serde_yaml::from_str(raw).context("failed to parse OpenAPI YAML")?;
        let openapi_key = serde_yaml::Value::String("openapi".to_string());
        validate_openapi_version(
            document
                .as_mapping()
                .and_then(|document| document.get(&openapi_key))
                .and_then(serde_yaml::Value::as_str),
        )?;

        let paths_key = serde_yaml::Value::String("paths".to_string());
        let Some(paths) = document
            .as_mapping()
            .and_then(|document| document.get(&paths_key))
            .and_then(serde_yaml::Value::as_mapping)
        else {
            return Ok(());
        };

        for path in paths.keys() {
            let path = path
                .as_str()
                .ok_or_else(|| anyhow!("OpenAPI path must be a string"))?;
            validate_raw_openapi_path(path)?;
        }
    }

    Ok(())
}

fn validate_openapi_version(version: Option<&str>) -> Result<()> {
    let Some(version) = version else {
        return Ok(());
    };

    if version == "3.0" || version.starts_with("3.0.") {
        return Ok(());
    }

    if version == "3.1" || version.starts_with("3.1.") {
        return Err(anyhow!("OpenAPI 3.1 is not yet supported"));
    }

    Err(anyhow!(
        "unsupported OpenAPI version {version}; expected OpenAPI 3.0"
    ))
}
```

Change `load_contract_text` to call:

```rust
validate_raw_openapi(text, is_json)?;
```

Replace `ensure_openapi_3` with:

```rust
fn ensure_openapi_3(document: &OpenAPI) -> Result<()> {
    validate_openapi_version(Some(&document.openapi))
}
```

- [ ] **Step 7: Run focused and parser tests**

Run:

```powershell
cargo test openapi_31_with_an_accurate_message -- --nocapture
cargo test --test cli_diff unsupported_openapi_version -- --nocapture
cargo test openapi::tests -- --nocapture
```

Expected: all PASS. Existing OpenAPI 2.0 rejection still contains
`unsupported OpenAPI version 2.0.0`.

- [ ] **Step 8: Run format, Clippy, and commit**

Run:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
git add src/openapi/mod.rs tests/cli_diff.rs tests/cli_lock.rs tests/cli_verify.rs testdata/openapi/unsupported_31.yaml
git commit -m "fix: reject unsupported OpenAPI 3.1"
```

### Task 3: Verify and Declare Rust 1.86

**Files:**
- Modify: `Cargo.toml`
- Modify: `.github/workflows/ci.yml`
- Modify: `README.md`

**Interfaces:**
- Consumes: committed Cargo lockfile and dependency graph
- Produces: explicit local and CI MSRV contract

- [ ] **Step 1: Install the candidate toolchain with approval**

Run only after environment-change approval:

```powershell
rustup toolchain install 1.86.0 --profile minimal
```

Expected: Rust and Cargo 1.86.0 install successfully.

- [ ] **Step 2: Verify the candidate before declaring it**

Run:

```powershell
cargo +1.86.0 check --locked
```

Expected: PASS. Rust 1.85 was tested first and correctly rejected because ICU
2.2 and `idna_adapter` require Rust 1.86.

- [ ] **Step 3: Declare MSRV**

Add to `[package]` in `Cargo.toml`:

```toml
rust-version = "1.86"
```

- [ ] **Step 4: Add exact-version CI job**

Add before `action-smoke` in `.github/workflows/ci.yml`:

```yaml
  msrv:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.86.0
      - run: cargo check --locked
```

- [ ] **Step 5: Document source-build requirement**

Add under `## Installation` in `README.md`:

```markdown
Source builds require Rust 1.86 or newer. APIWatch declares and checks this
minimum in CI so dependency changes cannot raise it silently.
```

- [ ] **Step 6: Verify stable and MSRV builds**

Run:

```powershell
cargo +1.86.0 check --locked
cargo check --locked
```

Expected: both PASS.

- [ ] **Step 7: Commit**

```powershell
git add Cargo.toml .github/workflows/ci.yml README.md
git commit -m "build: declare and check Rust 1.86 MSRV"
```

### Task 4: Build the Verified Compatibility Fetcher

**Files:**
- Modify: `.gitignore`
- Create: `compat/specs.json`
- Create: `scripts/fetch_compat_specs.py`
- Create: `scripts/tests/test_fetch_compat_specs.py`

**Interfaces:**
- Consumes: immutable raw GitHub URLs and SHA-256 manifest
- Produces: verified local files under `.compat-cache/`

- [ ] **Step 1: Add failing fetcher unit tests**

Create `scripts/tests/test_fetch_compat_specs.py`:

```python
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).resolve().parents[1] / "fetch_compat_specs.py"
SPEC = importlib.util.spec_from_file_location("fetch_compat_specs", SCRIPT)
fetcher = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(fetcher)


class FakeResponse(io.BytesIO):
    status = 200

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, traceback):
        self.close()


class FetchCompatSpecsTests(unittest.TestCase):
    def entry(self, payload=b"contract"):
        return {
            "name": "example",
            "file": "example.json",
            "url": (
                "https://raw.githubusercontent.com/example/api/"
                "0123456789abcdef0123456789abcdef01234567/openapi.json"
            ),
            "sha256": hashlib.sha256(payload).hexdigest(),
            "max_bytes": 1024,
            "status": "passing",
        }

    def test_rejects_mutable_upstream_url(self):
        entry = self.entry()
        entry["url"] = (
            "https://raw.githubusercontent.com/example/api/main/openapi.json"
        )
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "immutable 40-character commit"):
                fetcher.fetch_entry(entry, Path(directory))

    def test_downloads_and_reuses_a_verified_cache_entry(self):
        payload = b"contract"
        with tempfile.TemporaryDirectory() as directory:
            cache = Path(directory)
            with patch.object(
                fetcher.urllib.request,
                "urlopen",
                return_value=FakeResponse(payload),
            ) as urlopen:
                size = fetcher.fetch_entry(self.entry(payload), cache)
            self.assertEqual(size, len(payload))
            self.assertEqual((cache / "example.json").read_bytes(), payload)
            urlopen.assert_called_once()

            with patch.object(
                fetcher.urllib.request,
                "urlopen",
                side_effect=AssertionError("network should not be used"),
            ):
                reused_size = fetcher.fetch_entry(self.entry(payload), cache)
            self.assertEqual(reused_size, len(payload))

    def test_hash_mismatch_does_not_replace_cached_file(self):
        payload = b"unexpected"
        entry = self.entry(b"expected")
        with tempfile.TemporaryDirectory() as directory:
            cache = Path(directory)
            with patch.object(
                fetcher.urllib.request,
                "urlopen",
                return_value=FakeResponse(payload),
            ):
                with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
                    fetcher.fetch_entry(entry, cache)
            self.assertFalse((cache / "example.json").exists())


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run unit tests and confirm red**

Run:

```powershell
python -m unittest discover -s scripts/tests -p "test_fetch_compat_specs.py"
```

Expected: FAIL because `scripts/fetch_compat_specs.py` does not exist.

- [ ] **Step 3: Create exact corpus manifest**

Create `compat/specs.json`:

```json
{
  "version": 1,
  "max_total_bytes": 104857600,
  "specs": [
    {
      "name": "github",
      "file": "github.json",
      "url": "https://raw.githubusercontent.com/github/rest-api-description/5c88ff6bc3c36a12ccd69b8e0fee479b7202188a/descriptions/api.github.com/api.github.com.json",
      "sha256": "17d0cf71ec30e78bd1dc27085be8371504b98e9a9326cf2a0802ab88c37fbfb5",
      "max_bytes": 52428800,
      "status": "passing"
    },
    {
      "name": "asana",
      "file": "asana.yaml",
      "url": "https://raw.githubusercontent.com/Asana/openapi/56796a67a3c093eedf55fd9682357957a2ebfd85/defs/asana_oas.yaml",
      "sha256": "cb3b90f4e0af56035eab0c648974f625b942a28a7144aa6c2326e38ca0bb3d56",
      "max_bytes": 52428800,
      "status": "passing"
    },
    {
      "name": "box",
      "file": "box.json",
      "url": "https://raw.githubusercontent.com/box/box-openapi/f28eec5d49b9597d7df82f3a0c75bd92478b699a/openapi.json",
      "sha256": "0db1ffa51e52b9f1cb779bc4a37f200ac5f978630cab5178141687b2fed24e7a",
      "max_bytes": 52428800,
      "status": "passing"
    },
    {
      "name": "stripe",
      "file": "stripe.json",
      "url": "https://raw.githubusercontent.com/stripe/openapi/86b6ae4db114ff06968dcc191ff4a898e9b5db7c/openapi/spec3.json",
      "sha256": "e24a26de4188fd64dec4c043d5d3726277fdcb07556a493ea481c305b0a223d8",
      "max_bytes": 52428800,
      "status": "known_failing",
      "expected_error": "circular schema reference detected: #/components/schemas/file"
    },
    {
      "name": "digitalocean",
      "file": "digitalocean.yaml",
      "url": "https://raw.githubusercontent.com/digitalocean/openapi/7667351a0c8a1a526343160e1778cb5e97b2c9da/specification/DigitalOcean-public.v2.yaml",
      "sha256": "cda2db55fb97ceef551a3e35682dca49ad331b486f88f712f7c93f4ba05eefbc",
      "max_bytes": 52428800,
      "status": "known_failing",
      "expected_error": "tags[0].description: invalid type: map, expected a string"
    }
  ]
}
```

- [ ] **Step 4: Implement the standard-library fetcher**

Create `scripts/fetch_compat_specs.py`:

```python
#!/usr/bin/env python3
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import urllib.request


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "compat" / "specs.json"
DEFAULT_CACHE = ROOT / ".compat-cache"
COMMIT_URL = re.compile(
    r"^https://raw\.githubusercontent\.com/"
    r"[^/]+/[^/]+/[0-9a-f]{40}/.+$"
)


def sha256_file(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def validate_entry(entry):
    filename = entry["file"]
    if Path(filename).name != filename:
        raise ValueError(f"{entry['name']}: file must be a plain filename")
    if not COMMIT_URL.fullmatch(entry["url"]):
        raise ValueError(
            f"{entry['name']}: URL must contain an immutable 40-character commit"
        )
    if not re.fullmatch(r"[0-9a-f]{64}", entry["sha256"]):
        raise ValueError(f"{entry['name']}: sha256 must be 64 lowercase hex characters")
    if not isinstance(entry["max_bytes"], int) or entry["max_bytes"] <= 0:
        raise ValueError(f"{entry['name']}: max_bytes must be a positive integer")


def fetch_entry(entry, cache_dir):
    validate_entry(entry)
    cache_dir.mkdir(parents=True, exist_ok=True)
    target = cache_dir / entry["file"]
    expected_hash = entry["sha256"]
    max_bytes = entry["max_bytes"]

    if target.exists():
        size = target.stat().st_size
        if size <= max_bytes and sha256_file(target) == expected_hash:
            print(f"verified cached {entry['name']} ({size} bytes)")
            return size

    temporary = target.with_name(target.name + ".tmp")
    temporary.unlink(missing_ok=True)
    digest = hashlib.sha256()
    size = 0
    request = urllib.request.Request(
        entry["url"],
        headers={"User-Agent": "apiwatch-compat-fetch/1"},
    )

    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            status = getattr(response, "status", 200)
            if status != 200:
                raise ValueError(f"{entry['name']}: HTTP status {status}")
            with temporary.open("wb") as destination:
                while True:
                    chunk = response.read(1024 * 1024)
                    if not chunk:
                        break
                    size += len(chunk)
                    if size > max_bytes:
                        raise ValueError(
                            f"{entry['name']}: download exceeds {max_bytes} bytes"
                        )
                    digest.update(chunk)
                    destination.write(chunk)

        actual_hash = digest.hexdigest()
        if actual_hash != expected_hash:
            raise ValueError(
                f"{entry['name']}: SHA-256 mismatch; "
                f"expected {expected_hash}, got {actual_hash}"
            )
        os.replace(temporary, target)
    finally:
        temporary.unlink(missing_ok=True)

    print(f"downloaded and verified {entry['name']} ({size} bytes)")
    return size


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--cache-dir", type=Path, default=DEFAULT_CACHE)
    args = parser.parse_args()

    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    if manifest.get("version") != 1:
        raise ValueError("unsupported compatibility manifest version")

    total = 0
    for entry in manifest["specs"]:
        total += fetch_entry(entry, args.cache_dir)
        if total > manifest["max_total_bytes"]:
            raise ValueError(
                f"compatibility corpus exceeds {manifest['max_total_bytes']} bytes"
            )
    print(f"verified {len(manifest['specs'])} specs ({total} bytes total)")


if __name__ == "__main__":
    main()
```

- [ ] **Step 5: Ignore the cache**

Append to `.gitignore`:

```gitignore
.compat-cache/
__pycache__/
*.pyc
```

- [ ] **Step 6: Run fetcher unit tests and syntax check**

Run:

```powershell
python -m unittest discover -s scripts/tests -p "test_fetch_compat_specs.py"
python -m py_compile scripts/fetch_compat_specs.py
```

Expected: 3 tests PASS; syntax check exits 0.

- [ ] **Step 7: Fetch and independently verify the real corpus**

Run:

```powershell
python scripts/fetch_compat_specs.py
python scripts/fetch_compat_specs.py
```

Expected first run: five downloads verified, total 25,626,695 bytes. Expected
second run: all five entries reused only after hash verification.

- [ ] **Step 8: Commit**

```powershell
git add .gitignore compat/specs.json scripts/fetch_compat_specs.py scripts/tests/test_fetch_compat_specs.py
git commit -m "test: add pinned compatibility corpus fetcher"
```

### Task 5: Add Compatibility Tests and CI

**Files:**
- Create: `tests/compat.rs`
- Modify: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: verified `.compat-cache/` files
- Produces: three expected passes and two explicit expected failures in CI

- [ ] **Step 1: Add ignored real-world integration tests**

Create `tests/compat.rs`:

```rust
use std::ffi::OsString;
use std::path::PathBuf;

use assert_cmd::Command;
use predicates::prelude::*;

fn corpus_file(filename: &str) -> PathBuf {
    let root = std::env::var_os("APIWATCH_COMPAT_DIR")
        .unwrap_or_else(|| OsString::from(".compat-cache"));
    let path = PathBuf::from(root).join(filename);
    assert!(
        path.is_file(),
        "missing compatibility fixture {}; run python scripts/fetch_compat_specs.py",
        path.display()
    );
    path
}

fn assert_clean_self_diff(filename: &str) {
    let path = corpus_file(filename);
    let path = path.to_str().expect("compatibility path should be UTF-8");
    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args(["diff", path, path])
        .assert()
        .success()
        .stdout("No changes detected.\n")
        .stderr(predicate::str::is_empty());
}

fn assert_known_failure(filename: &str, expected_error: &str) {
    let path = corpus_file(filename);
    let path = path.to_str().expect("compatibility path should be UTF-8");
    Command::cargo_bin("apiwatch")
        .expect("binary should build")
        .args(["diff", path, path])
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains(expected_error));
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn github_rest_is_compatible() {
    assert_clean_self_diff("github.json");
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn asana_is_compatible() {
    assert_clean_self_diff("asana.yaml");
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn box_is_compatible() {
    assert_clean_self_diff("box.json");
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn stripe_reproduces_known_recursive_schema_failure() {
    assert_known_failure(
        "stripe.json",
        "circular schema reference detected: #/components/schemas/file",
    );
}

#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn digitalocean_reproduces_known_metadata_failure() {
    assert_known_failure(
        "digitalocean.yaml",
        "tags[0].description: invalid type: map, expected a string",
    );
}
```

- [ ] **Step 2: Confirm normal tests stay offline**

Run:

```powershell
cargo test --test compat
```

Expected: 0 executed, 5 ignored, exit 0; no network access.

- [ ] **Step 3: Run the downloaded corpus**

Run:

```powershell
cargo test --test compat -- --ignored --nocapture
```

Expected: 5 passed, 0 failed.

- [ ] **Step 4: Add compatibility CI job**

Add to `.github/workflows/ci.yml`:

```yaml
  compat:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: actions/setup-python@v5
        with:
          python-version: "3.x"
      - uses: actions/cache@v4
        with:
          path: .compat-cache
          key: compat-${{ runner.os }}-${{ hashFiles('compat/specs.json') }}
      - run: python -m unittest discover -s scripts/tests -p "test_fetch_compat_specs.py"
      - run: python scripts/fetch_compat_specs.py
      - run: cargo test --test compat -- --ignored --nocapture
```

- [ ] **Step 5: Validate YAML, format, and complete tests**

Run:

```powershell
cargo fmt --all -- --check
cargo test
cargo test --test compat -- --ignored --nocapture
```

Expected: normal suite passes with five ignored compat tests; dedicated compat
run passes all five.

- [ ] **Step 6: Commit**

```powershell
git add tests/compat.rs .github/workflows/ci.yml
git commit -m "test: add real-world compatibility smoke suite"
```

### Task 6: Publish the Complete Known-Limitations Register

**Files:**
- Modify: `README.md:185-208`
- Review: `action.yml`

**Interfaces:**
- Consumes: audit defects D-01–D-19 and roadmap phase links
- Produces: user-visible, release-accurate risk register

- [ ] **Step 1: Replace summarized limitations with exact grouped table**

Replace `## Known Limitations` content with a table containing these exact
user-facing rows:

```markdown
## Known Limitations

APIWatch is pre-v1. A clean result does not yet prove that every change class
below was checked.

| Area | Current limitation | Tracked work |
|---|---|---|
| Request bodies (D-01) | Adding or removing an entire request body may be missed. | [Phase 2](ROADMAP.md#phase-2--make-the-comparison-engine-trustworthy) |
| Content types (D-02) | Adding or removing a request or response media type may be missed. | Phase 2 |
| Response requiredness (D-03) | Required/optional response-field changes are not compared correctly. | Phase 2 |
| Dictionary schemas (D-04) | `additionalProperties` constraints are not represented. | Phase 2 |
| Schema formats (D-05) | Formats such as `int32`, `int64`, and date-time are normalized but not compared. | Phase 2 |
| Servers (D-06) | Server and base-URL changes are not tracked. | Phase 2 |
| Path templates (D-07) | Renaming a path parameter may appear as endpoint removal plus addition. | Phase 2 |
| Security identity (D-08) | Renaming an equivalent security scheme may be reported as breaking. | Phase 2 |
| Composition (D-09) | Reordering `allOf`, `oneOf`, or `anyOf` branches can cause false breaking findings. | Phase 2 |
| Array model (D-10) | Array items are represented internally as a synthetic property, limiting some comparisons. | Phase 2 |
| Enum severity (D-11) | Direction is handled, but response enum-widening severity is not yet a stable policy. | Phase 2 |
| OpenAPI 3.1 (D-12) | OpenAPI 3.1 is explicitly rejected until it is implemented. | [Phase 3](ROADMAP.md#phase-3--real-world-compatibility) |
| Strict metadata parsing (D-13) | Irrelevant malformed metadata can reject an otherwise usable specification. | Phase 3 |
| Recursive schemas (D-14) | Circular schema references are currently rejected. | Phase 3 |
| External references (D-15) | External and multi-file `$ref` targets are unsupported. | Phase 3 |
| Declared locks (D-16) | Version 1 and 2 declared locks store routes only; Verify cannot detect full semantic drift. | [Phase 1](ROADMAP.md#phase-1--make-verify-meaningful) |
| Null observations (D-17) | A null-only sample can make an observed shape too narrow. | [Phase 4](ROADMAP.md#phase-4--trustworthy-observed-contracts) |
| Observed requiredness (D-18) | Requiredness does not yet use a configurable confidence threshold. | Phase 4 |
| Observed inputs (D-19) | Observed Verify accepts local JSON only; HAR and live capture are not implemented. | [Phase 5](ROADMAP.md#phase-5--frictionless-recording-and-ci-adoption) |
| Distribution | The Action, Homebrew formula, and Scoop manifest still build from source. | [Continuous distribution](ROADMAP.md#continuous-distribution-track) |

Repeated phase names in the table refer to the linked phase in the first row
for that group. See [ROADMAP.md](ROADMAP.md) for exit criteria.
```

- [ ] **Step 2: Review Action metadata**

Check `action.yml` against current behavior. Its description already says
local or live OpenAPI, and the `openapi` input already states provenance-based
OpenAPI or local JSON. If no stale claim exists, make no change and do not
create an empty diff.

- [ ] **Step 3: Validate every defect appears once**

Run:

```powershell
$ids = 1..19 | ForEach-Object { 'D-{0:D2}' -f $_ }
$missing = $ids | Where-Object { -not (Select-String -Quiet -Path README.md -SimpleMatch $_) }
if ($missing) { throw "Missing limitations: $missing" }
```

Expected: no missing IDs.

- [ ] **Step 4: Validate Markdown and commit**

Run:

```powershell
git diff --check
git add README.md
git commit -m "docs: publish complete known limitations"
```

### Task 7: Prepare v0.7.0 Metadata and Release Smoke Tooling

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `CHANGELOG.md`
- Modify: `README.md`
- Create: `scripts/release_smoke.py`
- Create: `scripts/update_package_metadata.py`
- Create: `scripts/tests/test_update_package_metadata.py`

**Interfaces:**
- Consumes: completed Phase 0 behavior and existing fixtures
- Produces: release candidate and deterministic post-tag package updater

- [ ] **Step 1: Add package-updater unit test**

Create `scripts/tests/test_update_package_metadata.py`:

```python
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "update_package_metadata.py"
SPEC = importlib.util.spec_from_file_location("update_package_metadata", SCRIPT)
updater = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(updater)


class UpdatePackageMetadataTests(unittest.TestCase):
    def test_updates_formula_and_scoop(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            formula_dir = root / "Formula"
            scoop_dir = root / "Scoop"
            formula_dir.mkdir()
            scoop_dir.mkdir()
            (formula_dir / "apiwatch.rb").write_text(
                '  url "https://github.com/o/r/archive/refs/tags/v0.6.0.tar.gz"\n'
                '  sha256 "' + ("a" * 64) + '"\n',
                encoding="utf-8",
            )
            (scoop_dir / "apiwatch.json").write_text(
                json.dumps({
                    "version": "0.6.0",
                    "url": "https://github.com/o/r/archive/refs/tags/v0.6.0.tar.gz",
                    "hash": "a" * 64,
                    "extract_dir": "apiwatch-0.6.0",
                }),
                encoding="utf-8",
            )

            updater.update(root, "0.7.0", "b" * 64)

            formula = (formula_dir / "apiwatch.rb").read_text(encoding="utf-8")
            self.assertIn("/v0.7.0.tar.gz", formula)
            self.assertIn('sha256 "' + ("b" * 64) + '"', formula)
            scoop = json.loads(
                (scoop_dir / "apiwatch.json").read_text(encoding="utf-8")
            )
            self.assertEqual(scoop["version"], "0.7.0")
            self.assertEqual(scoop["hash"], "b" * 64)
            self.assertEqual(scoop["extract_dir"], "apiwatch-0.7.0")
            self.assertTrue(scoop["url"].endswith("/v0.7.0.tar.gz"))


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Confirm updater test is red**

Run:

```powershell
python -m unittest discover -s scripts/tests -p "test_update_package_metadata.py"
```

Expected: FAIL because the updater does not exist.

- [ ] **Step 3: Implement package updater**

Create `scripts/update_package_metadata.py`:

```python
#!/usr/bin/env python3
import argparse
import json
from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[1]
VERSION = re.compile(r"0|[1-9][0-9]*")
SHA256 = re.compile(r"[0-9a-f]{64}")


def update(root, version, sha256):
    if not all(VERSION.fullmatch(part) for part in version.split(".")):
        raise ValueError("version must be numeric SemVer without a prefix")
    if len(version.split(".")) != 3:
        raise ValueError("version must contain major.minor.patch")
    if not SHA256.fullmatch(sha256):
        raise ValueError("sha256 must be 64 lowercase hexadecimal characters")

    formula_path = root / "Formula" / "apiwatch.rb"
    formula = formula_path.read_text(encoding="utf-8")
    formula, url_replacements = re.subn(
        r"/v[0-9]+\.[0-9]+\.[0-9]+\.tar\.gz",
        f"/v{version}.tar.gz",
        formula,
        count=1,
    )
    formula, hash_replacements = re.subn(
        r'(?m)^  sha256 "[0-9a-f]{64}"$',
        f'  sha256 "{sha256}"',
        formula,
        count=1,
    )
    if url_replacements != 1 or hash_replacements != 1:
        raise ValueError("formula must contain one release URL and one SHA-256")
    formula_path.write_text(formula, encoding="utf-8", newline="\n")

    scoop_path = root / "Scoop" / "apiwatch.json"
    scoop = json.loads(scoop_path.read_text(encoding="utf-8"))
    scoop["version"] = version
    scoop["url"] = (
        "https://github.com/hitesh518-collab/apiwatch/"
        f"archive/refs/tags/v{version}.tar.gz"
    )
    scoop["hash"] = sha256
    scoop["extract_dir"] = f"apiwatch-{version}"
    scoop_path.write_text(
        json.dumps(scoop, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--sha256", required=True)
    parser.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args()
    update(args.root, args.version, args.sha256)


if __name__ == "__main__":
    main()
```

- [ ] **Step 4: Run updater tests**

Run:

```powershell
python -m unittest discover -s scripts/tests -p "test_update_package_metadata.py"
```

Expected: 1 test PASS.

- [ ] **Step 5: Bump crate version**

Change `Cargo.toml`:

```toml
version = "0.7.0"
```

Run:

```powershell
cargo check
cargo check --locked
```

Expected: first command updates the root package entry in `Cargo.lock` from
0.6.0 to 0.7.0; second command passes without further changes.

- [ ] **Step 6: Write v0.7.0 changelog**

Replace `## Unreleased` in `CHANGELOG.md` with:

```markdown
## v0.7.0 - 2026-07-24

### Added

- Versioned observed JSON contracts with local shape recording, monotonic
  merging, and read-only verification.
- Explicit repeatable `--map-at` annotations for value-free dynamic-key maps.
- Matching observed Verify output in text, versioned JSON, and SARIF 2.1.0.
- A commit-pinned, hash-verified compatibility suite for five public OpenAPI
  specifications.
- A declared and CI-checked minimum supported Rust version of 1.86.

### Changed

- OpenAPI 3.1 documents now fail with an explicit unsupported-version message
  instead of entering the OpenAPI 3.0 parser.
- Documentation now distinguishes route-only declared Verify from full
  semantic verification and lists all audited limitations.

### Security

- Observed locks and diagnostics retain structure only and redact dynamic map
  keys consistently across text, JSON, SARIF, and fingerprints.
```

- [ ] **Step 7: Update README release status**

Change:

```markdown
The latest tagged release is v0.6.0.

The current repository also contains unreleased ...
```

to:

```markdown
The v0.7.0 release adds observed JSON recording, monotonic shape merging,
value-free observed verification, and explicit `--map-at` annotations. It also
adds output-format parity, an explicit Rust 1.86 floor, accurate OpenAPI 3.1
rejection, and a pinned real-world compatibility smoke suite.
```

Keep Phase 1 full-contract language unchanged. Leave Homebrew and Scoop
sections at v0.6.0 until the public post-tag hash task.

- [ ] **Step 8: Create clean-install release smoke script**

Create `scripts/release_smoke.py`:

```python
#!/usr/bin/env python3
import json
from pathlib import Path
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[1]


def run(command, expected=0):
    completed = subprocess.run(
        [str(part) for part in command],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if completed.returncode != expected:
        raise RuntimeError(
            f"expected exit {expected}, got {completed.returncode}: "
            f"{' '.join(str(part) for part in command)}\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )
    return completed


def main():
    with tempfile.TemporaryDirectory(prefix="apiwatch-release-smoke-") as directory:
        temporary = Path(directory)
        install_root = temporary / "install"
        run([
            "cargo",
            "install",
            "--path",
            ROOT,
            "--root",
            install_root,
            "--locked",
            "--force",
        ])
        binary = install_root / "bin" / (
            "apiwatch.exe" if sys.platform == "win32" else "apiwatch"
        )

        version = run([binary, "--version"]).stdout
        if "apiwatch 0.7.0" not in version:
            raise RuntimeError(f"unexpected version output: {version}")

        run([
            binary,
            "diff",
            ROOT / "testdata/openapi/no_breaking_old.yaml",
            ROOT / "testdata/openapi/no_breaking_new.yaml",
        ])

        declared_lock = temporary / "declared.lock"
        run([
            binary,
            "lock",
            ROOT / "testdata/openapi/verify_matching.yaml",
            "--name",
            "users",
            "--output",
            declared_lock,
        ])
        run([
            binary,
            "verify",
            ROOT / "testdata/openapi/verify_matching.yaml",
            "--name",
            "users",
            "--lock",
            declared_lock,
        ])
        run([
            binary,
            "verify",
            ROOT / "testdata/openapi/verify_current.yaml",
            "--name",
            "users",
            "--lock",
            declared_lock,
        ], expected=1)

        observed_lock = temporary / "observed.lock"
        run([
            binary,
            "record",
            "--from-json",
            ROOT / "testdata/observed/portfolio-empty.json",
            "--name",
            "portfolio",
            "--output",
            observed_lock,
        ])
        run([
            binary,
            "record",
            "--from-json",
            ROOT / "testdata/observed/portfolio-populated.json",
            "--name",
            "portfolio",
            "--output",
            observed_lock,
            "--merge",
        ])
        run([
            binary,
            "record",
            "--from-json",
            ROOT / "testdata/observed/portfolio-map-initial.json",
            "--name",
            "portfolio-map",
            "--output",
            observed_lock,
            "--map-at",
            "$.by_broker",
        ])
        run([
            binary,
            "verify",
            ROOT / "testdata/observed/portfolio-map-matching.json",
            "--name",
            "portfolio-map",
            "--lock",
            observed_lock,
        ])

        json_match = run([
            binary,
            "verify",
            ROOT / "testdata/observed/portfolio-matching.json",
            "--name",
            "portfolio",
            "--lock",
            observed_lock,
            "--format",
            "json",
        ])
        rendered = json.loads(json_match.stdout)
        if rendered["summary"] != {"breaking": 0} or rendered["changes"] != []:
            raise RuntimeError("matching observed JSON output is not empty")

        sarif_match = run([
            binary,
            "verify",
            ROOT / "testdata/observed/portfolio-matching.json",
            "--name",
            "portfolio",
            "--lock",
            observed_lock,
            "--format",
            "sarif",
        ])
        sarif = json.loads(sarif_match.stdout)
        if sarif["runs"][0]["results"] != []:
            raise RuntimeError("matching observed SARIF results are not empty")

        run([
            binary,
            "verify",
            ROOT / "testdata/observed/portfolio-type-drift.json",
            "--name",
            "portfolio",
            "--lock",
            observed_lock,
        ], expected=1)
        run([
            binary,
            "diff",
            ROOT / "testdata/openapi/invalid_yaml.yaml",
            ROOT / "testdata/openapi/no_breaking_new.yaml",
        ], expected=2)

    print("release smoke passed")


if __name__ == "__main__":
    main()
```

- [ ] **Step 9: Run script tests and release smoke**

Run:

```powershell
python -m unittest discover -s scripts/tests -p "test_*.py"
python -m py_compile scripts/release_smoke.py scripts/update_package_metadata.py
python scripts/release_smoke.py
```

Expected: 4 Python unit tests PASS; smoke prints `release smoke passed`.

- [ ] **Step 10: Commit release candidate metadata**

```powershell
git add Cargo.toml Cargo.lock CHANGELOG.md README.md scripts/release_smoke.py scripts/update_package_metadata.py scripts/tests/test_update_package_metadata.py
git commit -m "release: prepare apiwatch v0.7.0"
```

### Task 8: Full Release-Candidate Verification and Handoff

**Files:**
- Create/update:
  `implementation-log/2026-07-24-phase-0-stabilization.md`

**Interfaces:**
- Consumes: all Phase 0 repository changes
- Produces: verified release candidate and explicit publication gate

- [ ] **Step 1: Run full Rust quality gate**

Run:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo +1.86.0 check --locked
cargo build --release --locked
```

Expected: every command exits 0.

- [ ] **Step 2: Run Python and compatibility gates**

Run:

```powershell
python -m unittest discover -s scripts/tests -p "test_*.py"
python scripts/fetch_compat_specs.py
cargo test --test compat -- --ignored --nocapture
python scripts/release_smoke.py
```

Expected: 4 Python tests pass; all five corpus entries verify; compat reports
5 passed; release smoke passes.

- [ ] **Step 3: Check documentation and repository state**

Run:

```powershell
git diff --check main...HEAD
git status --short
git log --oneline main..HEAD
```

Expected: no whitespace errors; working tree clean; only Phase 0 design, plan,
behavior, tests, CI, tooling, and release-candidate commits are present.

- [ ] **Step 4: Write the required implementation log**

Record:

- goal and approved scope;
- decisions and pinned corpus commits/hashes;
- files and areas touched;
- focused red/green evidence;
- full verification results;
- blockers;
- next step: publication approval.

Keep it high-level and do not stage it.

- [ ] **Step 5: Stop at the publication gate**

Present:

- current branch and release commit SHA;
- exact Rust, Python, compatibility, and smoke results;
- proposed annotated tag `v0.7.0`;
- release notes;
- the exact publication commands in Task 9.

Do not merge, push, tag, or update Formula/Scoop yet.

### Task 9: Publish v0.7.0 After Explicit Approval

**Files:**
- Git refs only before the tag archive exists

**Interfaces:**
- Consumes: explicit user approval at Task 8 gate
- Produces: public main and annotated v0.7.0 tag

- [ ] **Step 1: Reconfirm clean candidate**

```powershell
git status --short
git branch --show-current
```

Expected: clean `codex/phase-0-stabilization`.

- [ ] **Step 2: Merge locally and verify**

```powershell
git switch main
git pull --ff-only
git merge codex/phase-0-stabilization
cargo test
```

Expected: merge succeeds and full test suite passes on `main`.

- [ ] **Step 3: Push main and annotated tag**

Run only with explicit approval:

```powershell
git push origin main
git tag -a v0.7.0 -m "apiwatch v0.7.0"
git push origin v0.7.0
```

Expected: public `main` and `v0.7.0` resolve to the verified release commit.

### Task 10: Finalize Post-Tag Package Metadata

**Files:**
- Modify: `Formula/apiwatch.rb`
- Modify: `Scoop/apiwatch.json`
- Update: `implementation-log/2026-07-24-phase-0-stabilization.md`

**Interfaces:**
- Consumes: public GitHub v0.7.0 archive
- Produces: package definitions pinned to the real archive hash

- [ ] **Step 1: Download and hash public archive**

```powershell
$archive = 'C:\tmp\apiwatch-v0.7.0.tar.gz'
Invoke-WebRequest -Uri 'https://github.com/hitesh518-collab/apiwatch/archive/refs/tags/v0.7.0.tar.gz' -OutFile $archive
$archiveHash = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant()
if ($archiveHash -notmatch '^[0-9a-f]{64}$') { throw 'invalid archive hash' }
```

Expected: one 64-character lowercase SHA-256.

- [ ] **Step 2: Apply generated metadata**

```powershell
python scripts/update_package_metadata.py --version 0.7.0 --sha256 $archiveHash
```

- [ ] **Step 3: Verify metadata**

```powershell
Select-String -Path Formula\apiwatch.rb -SimpleMatch 'v0.7.0.tar.gz', $archiveHash
$scoop = Get-Content -Raw Scoop\apiwatch.json | ConvertFrom-Json
if ($scoop.version -ne '0.7.0' -or $scoop.hash -ne $archiveHash -or $scoop.extract_dir -ne 'apiwatch-0.7.0') { throw 'Scoop metadata mismatch' }
```

Expected: Formula and Scoop both reference v0.7.0 and the generated hash.

- [ ] **Step 4: Commit packaging metadata**

```powershell
git add Formula/apiwatch.rb Scoop/apiwatch.json
git commit -m "release: repin package metadata to v0.7.0"
```

- [ ] **Step 5: Push only if publication approval included this commit**

```powershell
git push origin main
```

Otherwise stop for a second approval with the commit SHA and validation
results.
