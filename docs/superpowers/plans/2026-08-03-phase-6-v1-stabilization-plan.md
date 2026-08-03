# Phase 6 — v1 Stabilization and Adoption: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship v1.0.0 with frozen v4 lockfile format, SemVer guarantees, parser fuzzing, performance regression gates, expanded compat corpus, deterministic output snapshots, migration docs, and release install verification.

**Architecture:** Three-track plan. Track A (Safety) gates the v1.0.0 tag — feature-gate legacy formats, add fuzz targets and perf budgets. Track B (Testing) expands the compat corpus and adds SHA-256 output snapshot gates. Track C (Docs/Distribution) writes migration docs and a post-release install smoke test. Tracks B and C are independent of each other but both depend on Track A completing.

**Tech Stack:** Rust (edition 2021, MSRV 1.88), `cargo-fuzz` + `libfuzzer-sys` (nightly, dev-only), Python 3.x (scripts), GitHub Actions CI.

## Global Constraints

- Rust edition 2021, MSRV 1.88.0
- Keep `diff` and declared Verify on one `diff_contracts` comparison path
- Preserve deterministic ordering (BTreeMap) and byte-stable lock output
- Keep Verify read-only
- Never retain observed values, credentials, or dynamic map keys
- Report probabilistic observed coverage honestly
- All scripts live under `scripts/` and use existing patterns (`fetch_compat_specs.py`, `release_smoke.py`)
- New CI jobs follow existing naming: lowercase, hyphen-separated, `ci.yml` for push/PR, new workflow files for dispatch
- Commit messages follow existing convention: lowercase prefix (`feat:`, `fix:`, `docs:`, `ci:`, `test:`, `chore:`)

---

### Task A1: Lockfile SemVer Guarantees

**Files:**
- Create: `compat/semver-contract.json`
- Create: `tests/cli_semver.rs`
- Modify: `Cargo.toml:6-8` — add `[features]` section
- Modify: `src/lib.rs:1` — update doc comment
- Modify: `src/lockfile/mod.rs:275-306` — feature-gate load paths
- Modify: `CHANGELOG.md` — add SemVer policy

**Interfaces:**
- Consumes: existing `Cli` struct from `src/cli.rs`, existing `OutputFormat` enum, existing `Command` enum variants, existing `output::render_changes`, `output::render_declared_verify_text`, `output::render_observed_verify_with_tiers`
- Produces: `compat/semver-contract.json` with keys `subcommands`, `flags`, `output_keys`, `exit_codes`; `tests/cli_semver.rs` with test `semver_contract_is_satisfied`

- [ ] **Step 1: Write the semver contract JSON**

Create `compat/semver-contract.json`:

```json
{
  "version": 1,
  "subcommands": {
    "diff": {
      "description": "Compare two OpenAPI contracts",
      "flags": {
        "old": {"kind": "positional", "type": "PathBuf", "required": true},
        "new": {"kind": "positional", "type": "PathBuf", "required": true},
        "--format": {"kind": "named", "type": "OutputFormat (text|json|sarif)", "default": "text"},
        "--ref-root": {"kind": "named", "type": "Option<PathBuf>"},
        "--config": {"kind": "named", "type": "Option<PathBuf>"}
      },
      "exit_codes": {"0": "no changes detected", "1": "changes detected", "2": "error"}
    },
    "lock": {
      "description": "Create an api.lock file from one OpenAPI contract",
      "flags": {
        "openapi": {"kind": "positional", "type": "PathBuf", "required": true},
        "--name": {"kind": "named", "type": "String", "required": true},
        "--output": {"kind": "named", "type": "PathBuf", "required": true},
        "--update": {"kind": "named", "type": "bool"},
        "--include-operation": {"kind": "named", "type": "Vec<String>"},
        "--max-lock-bytes": {"kind": "named", "type": "u64", "default": "5242880"},
        "--ref-root": {"kind": "named", "type": "Option<PathBuf>"}
      },
      "exit_codes": {"0": "success", "2": "error"}
    },
    "init": {
      "description": "Scaffold a new api.lock and CI workflow",
      "flags": {
        "--output": {"kind": "named", "type": "PathBuf", "default": "api.lock"}
      },
      "exit_codes": {"0": "success", "2": "error"}
    },
    "coverage": {
      "description": "Report endpoint and field coverage for observed entries",
      "flags": {
        "--lock": {"kind": "named", "type": "PathBuf", "required": true},
        "--name": {"kind": "named", "type": "Option<String>"}
      },
      "exit_codes": {"0": "success", "2": "error"}
    },
    "record": {
      "description": "Record the observed shape of one JSON body",
      "flags": {
        "--from-har": {"kind": "named", "type": "Option<PathBuf>"},
        "--from-json": {"kind": "named", "type": "Option<PathBuf>"},
        "--from-url": {"kind": "named", "type": "Option<String>"},
        "--name": {"kind": "named", "type": "Option<String>"},
        "--method": {"kind": "named", "type": "String", "default": "GET"},
        "--header": {"kind": "named", "type": "Vec<String>"},
        "--output": {"kind": "named", "type": "PathBuf", "required": true},
        "--merge": {"kind": "named", "type": "bool"},
        "--map-at": {"kind": "named", "type": "Vec<String>"},
        "--required-threshold": {"kind": "named", "type": "Option<f64>"},
        "--path-identity": {"kind": "named", "type": "Vec<String>"},
        "--status": {"kind": "named", "type": "Vec<u16>"}
      },
      "exit_codes": {"0": "success", "2": "error"}
    },
    "verify": {
      "description": "Verify one OpenAPI contract against a named api.lock entry",
      "flags": {
        "openapi": {"kind": "positional", "type": "Option<String>"},
        "--name": {"kind": "named", "type": "Option<String>"},
        "--lock": {"kind": "named", "type": "PathBuf", "required": true},
        "--format": {"kind": "named", "type": "OutputFormat (text|json|sarif)", "default": "text"},
        "--ref-root": {"kind": "named", "type": "Option<PathBuf>"},
        "--config": {"kind": "named", "type": "Option<PathBuf>"},
        "--header": {"kind": "named", "type": "Vec<String>"},
        "--all": {"kind": "named", "type": "bool"},
        "--source-url": {"kind": "named", "type": "Option<String>"}
      },
      "exit_codes": {"0": "verified, no changes", "1": "changes detected", "2": "error"}
    }
  },
  "output_formats": {
    "text": {"description": "human-readable, not guaranteed stable for parsing"},
    "json": {"description": "machine-readable, schema versioned per command", "fields": ["version", "command", "summary", "changes"]},
    "sarif": {"description": "SARIF 2.1.0 static analysis results interchange format", "spec": "https://docs.oasis-open.org/sarif/sarif/v2.1.0/"}
  }
}
```

- [ ] **Step 2: Write the CLI semver contract test**

Create `tests/cli_semver.rs`:

```rust
use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Deserialize)]
struct SemverContract {
    #[allow(dead_code)]
    version: u8,
    subcommands: BTreeMap<String, SubcommandContract>,
}

#[derive(Deserialize)]
struct SubcommandContract {
    #[allow(dead_code)]
    description: String,
    flags: BTreeMap<String, FlagContract>,
    exit_codes: BTreeMap<String, String>,
}

#[derive(Deserialize, PartialEq, Eq)]
struct FlagContract {
    kind: String,
    #[serde(rename = "type")]
    flag_type: String,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    default: Option<String>,
}

#[test]
fn semver_contract_is_satisfied() {
    let contract_data =
        std::fs::read_to_string("compat/semver-contract.json").expect("semver contract should be readable");
    let contract: SemverContract =
        serde_json::from_str(&contract_data).expect("semver contract should be valid JSON");

    use clap::CommandFactory;
    let cmd = apiwatch::cli::Cli::command();

    for subcommand_name in contract.subcommands.keys() {
        let found = cmd.get_subcommands().any(|sc| sc.get_name() == subcommand_name);
        assert!(
            found,
            "semver contract lists subcommand '{subcommand_name}' but CLI no longer has it"
        );
    }

    for subcommand_name in contract.subcommands.keys() {
        let contract_sub = &contract.subcommands[subcommand_name];
        if let Some(cli_sub) = cmd.find_subcommand(subcommand_name) {
            for (flag_name, flag_contract) in &contract_sub.flags {
                if flag_name.starts_with("--") {
                    let long = flag_name.trim_start_matches("--");
                    let found = cli_sub.get_arguments().any(|a| {
                        a.get_long_and_visible_aliases()
                            .map_or(false, |mut longs| longs.any(|l| l == long))
                    });
                    assert!(
                        found,
                        "semver contract lists flag '{flag_name}' on subcommand '{subcommand_name}' but CLI no longer has it"
                    );
                }
            }
        }
    }
}
```

- [ ] **Step 3: Run test to verify it passes with current CLI**

Run: `cargo test tests::cli_semver::semver_contract_is_satisfied -- --nocapture`

Expected: PASS (the contract matches the current `Cli` struct)

- [ ] **Step 4: Add `legacy-lock-format` feature to Cargo.toml**

In `Cargo.toml`, after the `[dependencies]` section, add:

```toml
[features]
default = ["legacy-lock-format"]
legacy-lock-format = []
```

- [ ] **Step 5: Feature-gate v2/v3 load paths in lockfile/mod.rs**

In `src/lockfile/mod.rs`, wrap the v2/v3 load paths. In the `load` function (line ~281-304), change:

```rust
    match header.version {
        1 => serde_yml::from_str(&contents).context("failed to parse api.lock YAML"),
        2 => load_v2(&contents),
        3 => {
            let (declared, observed) = v3::load(&contents)?.into_parts();
            ...
        }
        4 => { ... }
        version => Err(anyhow!("unsupported api.lock version {version}")),
    }
```

To:

```rust
    match header.version {
        1 => serde_yml::from_str(&contents).context("failed to parse api.lock YAML"),
        #[cfg(feature = "legacy-lock-format")]
        2 => load_v2(&contents),
        #[cfg(feature = "legacy-lock-format")]
        3 => {
            let (declared, observed) = v3::load(&contents)?.into_parts();
            Ok(ApiLock {
                version: 3,
                legacy_declared: BTreeMap::new(),
                declared_v3: declared,
                declared_v4: BTreeMap::new(),
                observed,
            })
        }
        4 => {
            let (declared_v4, observed) = v4::load(&contents)?.into_parts();
            Ok(ApiLock {
                version: 4,
                legacy_declared: BTreeMap::new(),
                declared_v3: BTreeMap::new(),
                declared_v4,
                observed,
            })
        }
        #[cfg(not(feature = "legacy-lock-format"))]
        2 | 3 => Err(anyhow!(
            "api.lock version {version} requires the legacy-lock-format feature; see docs/migration.md"
        )),
        #[cfg(feature = "legacy-lock-format")]
        version => Err(anyhow!("unsupported api.lock version {version}")),
        #[cfg(not(feature = "legacy-lock-format"))]
        version => Err(anyhow!("unsupported api.lock version {version}")),
    }
```

- [ ] **Step 6: Gate v2/v3 Verify paths behind the same feature**

In `src/lockfile/mod.rs`, find `select_verify_target` and verify the legacy path is also gated. Add `#[cfg(feature = "legacy-lock-format")]` on the v2/v3 match arms in that function.

Read the function first to find the exact lines:

Run: `rg "fn select_verify_target" src/lockfile/mod.rs`

Then add the cfg gate on the match arms that handle `v1` and `v2` and `v3` (non-v4 legacy paths).

- [ ] **Step 7: Update lib.rs doc comment**

In `src/lib.rs`, change line 1 from:

```rust
#![doc = "Internal APIWatch library. Public interfaces are pre-v1 and unstable."]
```

To:

```rust
#![doc = "APIWatch v1 public library. Lock, diff, and verify REST API contracts."]
```

- [ ] **Step 8: Add SemVer policy section to CHANGELOG.md**

At the top of `CHANGELOG.md`, insert after the header:

```
## Stability Guarantees (v1.0.0+)

- The v4 lockfile format (`version: 4` in `api.lock`) is frozen. Future format
  changes require a new version number — never a silent schema change.
- CLI subcommands, flags, exit codes, and JSON/SARIF output schemas are stable
  within a major release. Additions are allowed in minor/patch; removals
  and renames require a major bump.
- Text output is human-readable and not guaranteed stable for parsing.
- v2 and v3 lockfiles remain readable behind the `legacy-lock-format` Cargo
  feature (on by default).
```

- [ ] **Step 9: Run `cargo test --workspace` to verify no regressions**

Run: `cargo test --workspace`

Expected: all 360+ tests still pass

- [ ] **Step 10: Test feature gate: build without legacy features**

Run: `cargo build --no-default-features`

Expected: builds successfully (v2/v3 load paths compiled out)

- [ ] **Step 11: Commit**

```bash
git add compat/semver-contract.json tests/cli_semver.rs Cargo.toml src/lib.rs src/lockfile/mod.rs CHANGELOG.md
git commit -m "feat: freeze v4 lockfile format, add SemVer contract with feature-gated v2/v3 backcompat"
```

---

### Task A2: Parser Fuzzing

**Files:**
- Create: `fuzz/Cargo.toml`
- Create: `fuzz/fuzz_targets/openapi_parse.rs`
- Create: `fuzz/fuzz_targets/lockfile_v4_roundtrip.rs`
- Create: `fuzz/fuzz_targets/observed_infer.rs`
- Create: `.github/workflows/fuzz.yml`
- Modify: `Cargo.toml` — add `[workspace]` member or keep separate

**Interfaces:**
- Consumes: `apiwatch::openapi::load_contract_input_with_ref_root`, `apiwatch::lockfile::load`, `apiwatch::lockfile::render`, `apiwatch::observed::infer`
- Produces: three fuzz targets callable via `cargo +nightly fuzz run <target>`; `fuzz.yml` dispatch workflow

- [ ] **Step 1: Create fuzz crate**

Create `fuzz/Cargo.toml`:

```toml
[package]
name = "apiwatch-fuzz"
version = "0.0.0"
edition = "2021"
publish = false

[package.metadata]
cargo-fuzz = true

[dependencies]
libfuzzer-sys = "0.4"
apiwatch = { path = ".." }

[[bin]]
name = "openapi_parse"
path = "fuzz_targets/openapi_parse.rs"
test = false
doc = false
bench = false

[[bin]]
name = "lockfile_v4_roundtrip"
path = "fuzz_targets/lockfile_v4_roundtrip.rs"
test = false
doc = false
bench = false

[[bin]]
name = "observed_infer"
path = "fuzz_targets/observed_infer.rs"
test = false
doc = false
bench = false
```

- [ ] **Step 2: Create openapi_parse fuzz target**

Create `fuzz/fuzz_targets/openapi_parse.rs`:

```rust
#![no_main]

use libfuzzer_sys::fuzz_target;
use std::io::Write;

fuzz_target!(|data: &[u8]| {
    if data.len() > 10_000_000 {
        return;
    }

    let mut file = tempfile::NamedTempFile::new().ok();
    let (path, mut file) = match file {
        Some(f) => {
            let p = f.path().to_path_buf();
            (p, f)
        }
        None => return,
    };

    if file.write_all(data).is_err() {
        return;
    }
    let _ = file.flush();

    let _ = apiwatch::openapi::load_contract(&path);
});
```

- [ ] **Step 3: Create lockfile_v4_roundtrip fuzz target**

Create `fuzz/fuzz_targets/lockfile_v4_roundtrip.rs`:

```rust
#![no_main]

use libfuzzer_sys::fuzz_target;
use std::io::Write;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() || data.len() > 5_000_000 {
        return;
    }

    let mut file = tempfile::NamedTempFile::new().ok();
    let (path, mut file) = match file {
        Some(f) => {
            let p = f.path().to_path_buf();
            (p, f)
        }
        None => return,
    };

    if file.write_all(data).is_err() {
        return;
    }
    let _ = file.flush();

    let lock = match apiwatch::lockfile::load(&path) {
        Ok(l) => l,
        Err(_) => return,
    };

    let rendered = match apiwatch::lockfile::render(&lock) {
        Ok(r) => r,
        Err(_) => return,
    };

    let reparse_path = match tempfile::NamedTempFile::new() {
        Ok(f) => f.path().to_path_buf(),
        Err(_) => return,
    };
    if std::fs::write(&reparse_path, &rendered).is_err() {
        return;
    }

    let roundtripped = match apiwatch::lockfile::load(&reparse_path) {
        Ok(l) => l,
        Err(_) => return,
    };

    let rerendered = match apiwatch::lockfile::render(&roundtripped) {
        Ok(r) => r,
        Err(_) => return,
    };

    assert_eq!(rendered, rerendered, "v4 roundtrip mismatch");
});
```

- [ ] **Step 4: Create observed_infer fuzz target**

Create `fuzz/fuzz_targets/observed_infer.rs`:

```rust
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let value: serde_json::Value = match serde_json::from_slice(data) {
        Ok(v) => v,
        Err(_) => return,
    };

    let shape = apiwatch::observed::infer(&value);

    let max_depth = measure_depth(&shape);
    assert!(max_depth <= 128, "shape depth {} exceeds limit", max_depth);

    let serialized = serde_json::to_value(&shape).ok();
    if let Some(ref v) = serialized {
        let _: Result<apiwatch::observed::Shape, _> = serde_json::from_value(v.clone());
    }
});

fn measure_depth(shape: &apiwatch::observed::Shape) -> usize {
    match shape {
        apiwatch::observed::Shape::Object { properties, .. } => {
            1 + properties.values().map(|p| measure_depth(&p.shape)).max().unwrap_or(0)
        }
        apiwatch::observed::Shape::Array { items } => 1 + measure_depth(items),
        apiwatch::observed::Shape::Map { values } => 1 + measure_depth(values),
        apiwatch::observed::Shape::Union { variants } => {
            1 + variants.iter().map(|v| measure_depth(v)).max().unwrap_or(0)
        }
        _ => 1,
    }
}


```

- [ ] **Step 5: Create fuzz CI workflow**

Create `.github/workflows/fuzz.yml`:

```yaml
name: Fuzz

on:
  workflow_dispatch:
    inputs:
      duration:
        description: 'Seconds per target'
        required: false
        default: '60'

jobs:
  fuzz:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        target: [openapi_parse, lockfile_v4_roundtrip, observed_infer]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: nightly
      - run: cargo install cargo-fuzz
      - run: cargo +nightly fuzz run ${{ matrix.target }} -- -max_total_time=${{ github.event.inputs.duration || 60 }}
      - uses: actions/upload-artifact@v4
        if: failure()
        with:
          name: fuzz-crash-${{ matrix.target }}
          path: fuzz/artifacts/${{ matrix.target }}
```

- [ ] **Step 6: Build fuzz targets to verify they compile**

Run: `cargo +nightly fuzz build`

Expected: all three fuzz targets compile (you may need `rustup install nightly` first)

- [ ] **Step 7: Run fuzz targets briefly (10s each) as sanity check**

Run:
```bash
cargo +nightly fuzz run openapi_parse -- -max_total_time=10
cargo +nightly fuzz run lockfile_v4_roundtrip -- -max_total_time=10
cargo +nightly fuzz run observed_infer -- -max_total_time=10
```

Expected: no crashes within 10 seconds

- [ ] **Step 8: Seed corpuses from test fixtures**

Run:
```bash
mkdir -p fuzz/corpus/openapi_parse
cp .compat-cache/*.json fuzz/corpus/openapi_parse/ 2>/dev/null || true
cp .compat-cache/*.yaml fuzz/corpus/openapi_parse/ 2>/dev/null || true
cp .compat-cache/*.yml fuzz/corpus/openapi_parse/ 2>/dev/null || true
mkdir -p fuzz/corpus/lockfile_v4_roundtrip
cp testdata/lock/v4_*.lock fuzz/corpus/lockfile_v4_roundtrip/ 2>/dev/null || true
mkdir -p fuzz/corpus/observed_infer
cp testdata/har/*.json fuzz/corpus/observed_infer/ 2>/dev/null || true
```

- [ ] **Step 9: Update .gitignore for fuzz artifacts**

Add to `.gitignore`:

```
fuzz/artifacts/
fuzz/corpus/openapi_parse/*
fuzz/corpus/lockfile_v4_roundtrip/*
fuzz/corpus/observed_infer/*
!fuzz/corpus/*/.gitkeep
```

And create empty `.gitkeep` files in each corpus dir.

- [ ] **Step 10: Run `cargo test --workspace` to verify no regressions**

Run: `cargo test --workspace`

- [ ] **Step 11: Commit**

```bash
git add fuzz/ .github/workflows/fuzz.yml .gitignore
git commit -m "feat: add cargo-fuzz targets for OpenAPI parser, v4 roundtrip, and observed infer"
```

---

### Task A3: Performance Budgets

**Files:**
- Create: `scripts/bench_perf.py`
- Create: `compat/perf-budget.json`
- Modify: `.github/workflows/ci.yml` — new `perf` job

**Interfaces:**
- Consumes: `compat/specs.json` (existing), `compat/perf-budget.json`, `.compat-cache/` directory
- Produces: benchmark report printed to stdout; CI job exits non-zero if budgets exceeded

- [ ] **Step 1: Write the performance budget baseline file**

Create `compat/perf-budget.json`:

```json
{
  "version": 1,
  "budgets": {
    "default_diff_seconds": 10.0,
    "default_lock_seconds": 15.0,
    "per_spec_overrides": {
      "github": { "diff_seconds": 60.0, "lock_seconds": 90.0 }
    }
  }
}
```

- [ ] **Step 2: Write the benchmark script**

Create `scripts/bench_perf.py`:

```python
"""Benchmark APIWatch diff and lock performance against the compat corpus."""
import json
import os
import statistics
import subprocess
import sys
import time
from pathlib import Path


def load_specs(manifest_path):
    with open(manifest_path) as f:
        manifest = json.load(f)
    return manifest["specs"]


def load_budgets(budget_path):
    with open(budget_path) as f:
        return json.load(f)


def run_timed(args, runs=3):
    times = []
    for _ in range(runs):
        start = time.perf_counter()
        result = subprocess.run(
            args, capture_output=True, text=True, timeout=120
        )
        elapsed = time.perf_counter() - start
        if result.returncode not in (0, 1):
            raise RuntimeError(
                f"command failed (exit {result.returncode}): {' '.join(args)}\n"
                f"stderr: {result.stderr[-500:]}"
            )
        times.append(elapsed)
    return statistics.median(times)


def main():
    root = Path(__file__).resolve().parent.parent
    specs_file = root / "compat" / "specs.json"
    budget_file = root / "compat" / "perf-budget.json"
    compat_dir = Path(
        os.environ.get("APIWATCH_COMPAT_DIR", str(root / ".compat-cache"))
    )

    specs = load_specs(specs_file)
    budgets = load_budgets(budget_file)

    binary = os.environ.get("APIWATCH_BINARY", "apiwatch")
    failures = []

    for spec in specs:
        if spec.get("status") != "passing":
            continue

        name = spec["name"]
        spec_path = compat_dir / spec["file"]

        if not spec_path.is_file():
            print(f"SKIP {name}: file not in compat cache")
            continue

        spec_budget = budgets["budgets"].get("per_spec_overrides", {}).get(
            name, {}
        )
        diff_budget = spec_budget.get(
            "diff_seconds", budgets["budgets"]["default_diff_seconds"]
        )
        lock_budget = spec_budget.get(
            "lock_seconds", budgets["budgets"]["default_lock_seconds"]
        )

        try:
            diff_time = run_timed(
                [binary, "diff", str(spec_path), str(spec_path)]
            )
            print(
                f"diff {name}: {diff_time:.2f}s (budget {diff_budget:.0f}s)"
            )
            if diff_time > diff_budget:
                failures.append(
                    f"diff {name}: {diff_time:.2f}s > {diff_budget:.0f}s budget"
                )
        except Exception as e:
            failures.append(f"diff {name}: {e}")

        try:
            lock_time = run_timed(
                [
                    binary,
                    "lock",
                    "--openapi",
                    str(spec_path),
                    "--name",
                    name,
                    "--output",
                    str(root / "tmp" / f"perf_{name}.lock"),
                ]
            )
            print(
                f"lock {name}: {lock_time:.2f}s (budget {lock_budget:.0f}s)"
            )
            if lock_time > lock_budget:
                failures.append(
                    f"lock {name}: {lock_time:.2f}s > {lock_budget:.0f}s budget"
                )
        except Exception as e:
            failures.append(f"lock {name}: {e}")

    if failures:
        print("\nPERFORMANCE BUDGET EXCEEDED:")
        for f in failures:
            print(f"  {f}")
        sys.exit(1)

    print("\nAll performance budgets met.")


if __name__ == "__main__":
    main()
```

- [ ] **Step 3: Run the benchmark script locally to verify it works**

Run: `python scripts/bench_perf.py`

Expected: benchmarks run against locally cached compat specs, passes (generous budgets)

- [ ] **Step 4: Add `perf` job to CI workflow**

In `.github/workflows/ci.yml`, after the `action-smoke` job, add:

```yaml
  perf:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: stable
      - uses: actions/setup-python@v5
        with:
          python-version: "3.x"
      - uses: actions/cache@v4
        with:
          path: .compat-cache
          key: compat-${{ runner.os }}-${{ hashFiles('compat/specs.json') }}
      - run: python scripts/fetch_compat_specs.py
      - run: cargo build --release
      - name: Benchmark
        run: |
          python scripts/bench_perf.py
        env:
          APIWATCH_BINARY: ./target/release/apiwatch
          APIWATCH_COMPAT_DIR: .compat-cache
```

- [ ] **Step 5: Commit**

```bash
git add scripts/bench_perf.py compat/perf-budget.json .github/workflows/ci.yml
git commit -m "feat: add performance budget gates for diff and lock on compat corpus"
```

---

### Task B1: Corpus Expansion (10 -> 15-20 Specs)

**Files:**
- Modify: `compat/specs.json` — add 5-10 new spec entries
- Modify: `tests/compat.rs` — add `#[ignore]` tests for new specs
- Create: `docs/compat-corpus.md` — corpus documentation

**Interfaces:**
- Consumes: `compat/specs.json` schema (fields: `name`, `file`, `url`, `sha256`, `max_bytes`, `status`, optional `expected_error`, optional `phase1_measurement`)
- Produces: new passing specs verified via `assert_clean_self_diff`, new known-failing specs verified via `assert_known_failure`

- [ ] **Step 1: Research and select 5-10 candidate specs**

OpenAPI specs to add (choose from this list based on availability and diversity):

Candidates for passing specs:
1. `petstore` — OpenAPI 3.0, small, classic reference (from openapitools/openapi-petstore)
2. `kubernetes` — 500+ operations, large, stress-tests v4 payload (from kubernetes/kubernetes)
3. `twilio` — complex `$ref` chains, YAML with anchors (from twilio/twilio-oai)
4. `vercel` — discriminators, response $ref (from vercel/api)
5. `adyen` — YAML, anyOf/oneOf (from Adyen/adyen-openapi)
6. `slack` — webhook/handler patterns (from slackapi/slack-api-specs)
7. `zoom` — multi-document style (from APIs-guru or official)

Candidates for known-failing specs:
8. `plaid` — uses path-level `$ref` (common failure)
9. `shopify` — complex schema references
10. `square` — potential recursive schema

- [ ] **Step 2: Verify each candidate builds with current APIWatch**

For each candidate: `apiwatch diff spec.yaml spec.yaml` and record the result (pass vs fail with what error).

- [ ] **Step 3: Add new spec entries to `compat/specs.json`**

Add entries using the existing format. Example for a passing spec:

```json
    {
      "name": "kubernetes",
      "file": "kubernetes.json",
      "url": "https://raw.githubusercontent.com/kubernetes/kubernetes/<COMMIT>/api/openapi-spec/v3/api__v1_openapi.json",
      "sha256": "<ACTUAL_SHA256>",
      "max_bytes": 52428800,
      "status": "passing",
      "phase1_measurement": {
        "operation_count": 0,
        "expanded_yaml_bytes": 0,
        "canonical_json_bytes": 0,
        "deduplicated_yaml_bytes": 0
      }
    }
```

Set `phase1_measurement` to zeros — the compat CI run will fill in actual values.

For a known-failing spec:

```json
    {
      "name": "plaid",
      "file": "plaid.yaml",
      "url": "<URL>",
      "sha256": "<ACTUAL_SHA256>",
      "max_bytes": 52428800,
      "status": "known_failing",
      "expected_error": "<actual error substring>"
    }
```

- [ ] **Step 4: Add `#[ignore]` tests to `tests/compat.rs`**

For each new passing spec, add at the end of the file (before the last `}`):

```rust
#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn kubernetes_is_compatible() {
    assert_clean_self_diff("kubernetes.json");
}
```

For each new known-failing spec:

```rust
#[test]
#[ignore = "requires commit-pinned compatibility corpus"]
fn plaid_reproduces_known_ref_failure() {
    assert_known_failure("plaid.yaml", "expected error substring here");
}
```

- [ ] **Step 5: Run `scripts/fetch_compat_specs.py` to download new specs**

Run: `python scripts/fetch_compat_specs.py`

Expected: downloads new specs into `.compat-cache/`

- [ ] **Step 6: Run new compat tests locally**

Run: `cargo test --test compat -- --ignored --nocapture`

Expected: new passing specs self-diff clean; known-failing specs produce expected errors

- [ ] **Step 7: Run the lock-size report to update measurements**

Run:
```bash
cargo run -p apiwatch-lock-size-report -- --manifest compat/specs.json --compat-dir .compat-cache --privacy-fixture testdata/openapi/privacy_sentinels.yaml --max-lock-bytes 5242880 --json-out docs/benchmarks/phase-1-lock-size-report.json --markdown-out docs/benchmarks/phase-1-lock-size-report.md --v4-json-out docs/benchmarks/phase-2-v4-lock-size-report.json --v4-markdown-out docs/benchmarks/phase-2-v4-lock-size-report.md --check
```

Then copy the `phase1_measurement` values from the benchmark JSON into `compat/specs.json` for each new spec.

- [ ] **Step 8: Write corpus documentation**

Create `docs/compat-corpus.md`:

```
# APIWatch Compatibility Corpus

The corpus verifies that APIWatch can parse, lock, and verify real-world OpenAPI
specifications. Each spec is pinned to a specific commit with a known SHA-256.

## Passing Specs

| Spec | Operations | Why it's in the corpus |
|------|-----------|------------------------|
| github | 1209 | Largest known public OpenAPI spec |
| asana | 249 | YAML with discriminated unions |
| box | 296 | JSON format, OAuth2 scopes |
| mercadopago | 142 | YAML, response $ref patterns |
| line | 73 | YAML, messaging API shape |
| humanitas-fhir | 3 | FHIR-specific schema patterns |
| ... | ... | ... |

## Known-Failing Specs

| Spec | Error | Known Issue |
|------|-------|-------------|
| stripe | circular schema reference | Recursive $ref not yet supported |
| digitalocean | missing field `responses` | Path item without responses |
| paystack | unsupported schema reference | Path-level requestBody $ref |
| deutsche-bahn | failed to parse | Swagger 2.0 spec |
| ... | ... | ... |
```

Fill in the actual spec names and details based on the selected candidates.

- [ ] **Step 9: Run cargo test --workspace to verify no regressions**

Run: `cargo test --workspace`

- [ ] **Step 10: Commit**

```bash
git add compat/specs.json tests/compat.rs docs/compat-corpus.md
git commit -m "test: expand compat corpus from 10 to N specs (X passing, Y known-failing)"
```

---

### Task B2: Deterministic Output Snapshots

**Files:**
- Create: `scripts/snapshot.py`
- Create: `compat/snapshots.json`
- Modify: `.github/workflows/ci.yml` — new `snapshot` job

**Interfaces:**
- Consumes: `compat/specs.json`, `compat/snapshots.json`, `.compat-cache/`
- Produces: `snapshot` CI job that fails on hash mismatch

- [ ] **Step 1: Write the snapshot script**

Create `scripts/snapshot.py`:

```python
"""Snapshot APIWatch lock and diff output for all passing compat specs."""
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path


def load_specs(manifest_path):
    with open(manifest_path) as f:
        manifest = json.load(f)
    return manifest["specs"]


def load_snapshots(snap_path):
    if not snap_path.is_file():
        return {"version": 1, "snapshots": {}}
    with open(snap_path) as f:
        return json.load(f)


def save_snapshots(snap_path, snapshots):
    with open(snap_path, "w") as f:
        json.dump(snapshots, f, indent=2)
        f.write("\n")


def sha256_of_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        while True:
            chunk = f.read(65536)
            if not chunk:
                break
            h.update(chunk)
    return h.hexdigest()


def main():
    root = Path(__file__).resolve().parent.parent
    specs_file = root / "compat" / "specs.json"
    snap_file = root / "compat" / "snapshots.json"
    compat_dir = Path(
        os.environ.get("APIWATCH_COMPAT_DIR", str(root / ".compat-cache"))
    )

    specs = load_specs(specs_file)
    stored = load_snapshots(snap_file)
    binary = os.environ.get("APIWATCH_BINARY", "apiwatch")
    update = "--update" in sys.argv

    tmp_dir = root / "tmp"
    tmp_dir.mkdir(exist_ok=True)

    new_snapshots = {"version": 1, "snapshots": {}}
    failures = []

    for spec in specs:
        if spec.get("status") != "passing":
            continue

        name = spec["name"]
        spec_path = compat_dir / spec["file"]

        if not spec_path.is_file():
            print(f"SKIP {name}: file not in compat cache")
            continue

        lock_out = tmp_dir / f"snapshot_{name}.lock"
        result = subprocess.run(
            [
                binary, "lock", "--openapi", str(spec_path),
                "--name", name, "--output", str(lock_out),
            ],
            capture_output=True, text=True,
        )
        if result.returncode != 0:
            failures.append(f"lock {name}: exit {result.returncode}\n{result.stderr[-500:]}")
            continue

        lock_hash = sha256_of_file(lock_out)

        diff_out = tmp_dir / f"snapshot_{name}_diff.txt"
        with open(diff_out, "w") as f:
            subprocess.run(
                [binary, "diff", str(spec_path), str(spec_path)],
                stdout=f, stderr=subprocess.PIPE, text=True,
            )
        diff_hash = sha256_of_file(diff_out)

        new_snapshots["snapshots"][name] = {
            "lock_sha256": lock_hash,
            "diff_output_sha256": diff_hash,
        }

        old = stored.get("snapshots", {}).get(name)
        if old is None:
            if update:
                print(f"NEW {name}: lock={lock_hash[:12]} diff={diff_hash[:12]}")
            else:
                failures.append(f"new {name}: no stored snapshot (run with --update)")
        else:
            if old["lock_sha256"] != lock_hash:
                msg = (
                    f"MISMATCH lock {name}:\n"
                    f"  expected: {old['lock_sha256'][:12]}\n"
                    f"  actual:   {lock_hash[:12]}"
                )
                if not update:
                    failures.append(msg)
                else:
                    print(msg)
            if old["diff_output_sha256"] != diff_hash:
                msg = (
                    f"MISMATCH diff {name}:\n"
                    f"  expected: {old['diff_output_sha256'][:12]}\n"
                    f"  actual:   {diff_hash[:12]}"
                )
                if not update:
                    failures.append(msg)
                else:
                    print(msg)
            if old["lock_sha256"] == lock_hash and old["diff_output_sha256"] == diff_hash:
                print(f"MATCH {name}")

    if update:
        save_snapshots(snap_file, new_snapshots)
        print(f"\nUpdated {snap_file}")

    if failures:
        print(f"\n{len(failures)} SNAPSHOT FAILURE(S):")
        for f in failures:
            print(f"  {f}")
        if not update:
            print("\nRun 'python scripts/snapshot.py --update' to accept changes.")
        sys.exit(1)

    print("\nAll snapshots match.")


if __name__ == "__main__":
    main()
```

- [ ] **Step 2: Generate initial snapshots**

Run: `python scripts/snapshot.py --update`

Expected: generates `compat/snapshots.json` with SHA-256 hashes for all passing specs

- [ ] **Step 3: Verify snapshots are reproducible**

Run: `python scripts/snapshot.py`

Expected: "All snapshots match." — no changes since `--update` just ran

- [ ] **Step 4: Add `snapshot` job to CI workflow**

In `.github/workflows/ci.yml`, after the `perf` job, add:

```yaml
  snapshot:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: stable
      - uses: actions/setup-python@v5
        with:
          python-version: "3.x"
      - uses: actions/cache@v4
        with:
          path: .compat-cache
          key: compat-${{ runner.os }}-${{ hashFiles('compat/specs.json') }}
      - run: python scripts/fetch_compat_specs.py
      - run: cargo build --release
      - name: Snapshot
        run: |
          python scripts/snapshot.py
        env:
          APIWATCH_BINARY: ./target/release/apiwatch
          APIWATCH_COMPAT_DIR: .compat-cache
```

- [ ] **Step 5: Commit**

```bash
git add scripts/snapshot.py compat/snapshots.json .github/workflows/ci.yml
git commit -m "test: add deterministic output snapshot gates for lock and diff on compat corpus"
```

---

### Task C1: Migration Documentation

**Files:**
- Create: `docs/migration.md`
- Create: `testdata/migration/v2_fixture.lock`
- Create: `testdata/migration/v3_fixture.lock`
- Create: `tests/cli_migration.rs`
- Modify: `README.md` — add link to migration guide

**Interfaces:**
- Consumes: `apiwatch::lockfile::load`, `apiwatch::lockfile::render`
- Produces: `tests/cli_migration.rs` with test `migrate_v2_to_v4` and `migrate_v3_to_v4`

- [ ] **Step 1: Create v2 fixture lockfile**

Create `testdata/migration/v2_fixture.lock`:

```yaml
version: 2
apis:
  demo:
    provenance: declared
    source: openapi
    operations:
      - method: GET
        path: /items
      - method: POST
        path: /items
```

- [ ] **Step 2: Create v3 fixture lockfile**

Create `testdata/migration/v3_fixture.lock`:

```yaml
version: 3
apis:
  demo-v3:
    provenance: declared
    source: openapi
    scope: all
    max_lock_bytes: 5242880
    contract_bytes: 223
    contract_digest: sha256:c839c2f77c568466f3009c5c72b2ebd2ea714de0033e000c4177abc470449e05
    contract:
      operations:
        GET /users:
          auth: {}
          parameters: {}
          request_body: null
          responses:
            '200': {}
        GET /zeta:
          auth: {}
          parameters: {}
          request_body: null
          responses:
            '200': {}
      schemas: {}
```

- [ ] **Step 3: Write the migration test**

Create `tests/cli_migration.rs`:

```rust
use std::io::Write;

use assert_cmd::Command;

#[test]
fn migrate_v2_loads_and_updates() {
    let mut tmp = tempfile::NamedTempFile::new().expect("tempfile");
    let fixture = include_str!("../testdata/migration/v2_fixture.lock");
    tmp.write_all(fixture.as_bytes()).expect("write v2 fixture");
    tmp.flush().expect("flush");

    let lock = apiwatch::lockfile::load(tmp.path()).expect("v2 lock should load");
    let rendered = apiwatch::lockfile::render(&lock).expect("v2 lock should render");
    assert!(
        rendered.contains("version: 2"),
        "rerendered v2 lock should contain version marker"
    );

    let update_path =
        tempfile::NamedTempFile::new().expect("tempfile").into_temp_path();
    update_path
        .as_os_str()
        .to_str()
        .expect("UTF-8 temp path");

    std::fs::write(&update_path, fixture).expect("write v2 fixture to update target");

    Command::cargo_bin("apiwatch")
        .expect("binary")
        .args([
            "lock",
            "--openapi",
            "testdata/openapi/verify_matching.yaml",
            "--name",
            "demo",
            "--output",
            update_path.as_os_str().to_str().expect("UTF-8"),
            "--update",
        ])
        .assert()
        .success();
}

#[test]
fn migrate_v3_loads_and_updates() {
    let mut tmp = tempfile::NamedTempFile::new().expect("tempfile");
    let fixture = include_str!("../testdata/migration/v3_fixture.lock");
    tmp.write_all(fixture.as_bytes()).expect("write v3 fixture");
    tmp.flush().expect("flush");

    let lock = apiwatch::lockfile::load(tmp.path()).expect("v3 lock should load");
    let rendered = apiwatch::lockfile::render(&lock).expect("v3 lock should render");
    assert!(
        rendered.contains("version: 3"),
        "rerendered v3 lock should contain version marker"
    );

    let update_path =
        tempfile::NamedTempFile::new().expect("tempfile").into_temp_path();
    std::fs::write(&update_path, fixture).expect("write v3 fixture to update target");

    Command::cargo_bin("apiwatch")
        .expect("binary")
        .args([
            "lock",
            "--openapi",
            "testdata/openapi/v3_scoped.yaml",
            "--name",
            "demo-v3",
            "--output",
            update_path.as_os_str().to_str().expect("UTF-8"),
            "--update",
        ])
        .assert()
        .success();

    let updated = std::fs::read_to_string(&update_path).expect("read updated lock");
    assert!(
        updated.contains("version: 4"),
        "updated lock should be v4, got:\n{updated}"
    );
}
```

- [ ] **Step 4: Run migration tests to verify they pass**

Run: `cargo test tests::cli_migration -- --nocapture`

Expected: both tests pass (v2 loads and renders; v3 loads and updates to v4)

- [ ] **Step 5: Write migration documentation**

Create `docs/migration.md`:

```
# APIWatch Lockfile Migration Guide

## Version Quick Reference

| Version | Features | Breakage Risk | Upgrade Action |
|---------|----------|---------------|----------------|
| v1 | Route-only declared entries | None | Automatic: re-serialize writes v2 |
| v2 | Observed entries, per-entry shapes | None | Run `apiwatch lock` to upgrade |
| v3 | Phase 1 payload reduction, partial coverage | None | Re-lock from original OpenAPI source |
| v4 (current) | Full contract payload, observed contracts | None | Already current |

## Compatibility Guarantee

The v4 lockfile format (`version: 4` in `api.lock`) is frozen as of APIWatch
v1.0.0. Future format changes will use a new version number — v4 will never be
silently changed.

v2 and v3 lockfiles remain readable via the `legacy-lock-format` Cargo feature
(on by default). APIWatch will always be able to read them; writing always
produces v4.

## Migrating from v2

v2 locks contain route-only declared entries and optional observed shapes.

```
apiwatch lock --openapi path/to/spec.yaml --name my-api --output api.lock --update
```

This loads the existing v2 lock, replaces the named entry with a full v4
declared entry, and writes v4 output. Observed entries are preserved.

## Migrating from v3

v3 locks contain Phase 1 contract payloads with reduced scope. For full
contract coverage:

```
apiwatch lock --openapi path/to/spec.yaml --name my-api --output api.lock --update
```

v3 locks work correctly for diff and verify, but declared verify provides only
partial coverage. Re-locking from the original OpenAPI source enables full
coverage.

## New Project Setup

```
apiwatch init --output api.lock
apiwatch lock --openapi spec.yaml --name my-api --output api.lock
apiwatch record --from-har capture.har --output api.lock
git add api.lock .github/workflows/
git commit -m "add apiwatch contract checking"
```

## Troubleshooting

### "api.lock version N requires the legacy-lock-format feature"

Your build excludes legacy format support. Enable the feature:

```toml
[dependencies]
apiwatch = { version = "1", features = ["legacy-lock-format"] }
```

### "warning: api.lock v3 lacks Phase 2 contract fields"

This warning appears during v3 declared verify. Re-lock from the original
OpenAPI source to upgrade to v4 and enable full coverage.
```

- [ ] **Step 6: Add migration link to README**

In `README.md`, add a line under the appropriate section:

```
See [docs/migration.md](docs/migration.md) for lockfile version upgrade instructions.
```

- [ ] **Step 7: Run `cargo test --workspace` to verify no regressions**

Run: `cargo test --workspace`

- [ ] **Step 8: Commit**

```bash
git add docs/migration.md testdata/migration/ tests/cli_migration.rs README.md
git commit -m "docs: add lockfile migration guide, fixtures, and migration tests"
```

---

### Task C2: Release Install Verification

**Files:**
- Create: `scripts/install_smoke.py`
- Modify: `.github/workflows/release.yml` — new `install-verify` job

**Interfaces:**
- Consumes: release artifacts (binaries, container tag) from prior `release` and `container` jobs
- Produces: summary appended to release body

- [ ] **Step 1: Write the install smoke script**

Create `scripts/install_smoke.py`:

```python
"""Verify that released binaries and container images install and run."""
import hashlib
import os
import subprocess
import sys
import tempfile
import urllib.request
from pathlib import Path


def download(url, dest):
    urllib.request.urlretrieve(url, dest)


def verify_sha256(path, expected_hash):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        while True:
            chunk = f.read(65536)
            if not chunk:
                break
            h.update(chunk)
    actual = h.hexdigest()
    if actual != expected_hash:
        raise RuntimeError(
            f"SHA256 mismatch: expected {expected_hash}, got {actual}"
        )


def run_version_check(binary_path):
    result = subprocess.run(
        [str(binary_path), "--version"],
        capture_output=True, text=True, timeout=10,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"--version failed: {result.stderr.strip()}"
        )
    version_output = result.stdout.strip()
    tag = os.environ.get("GITHUB_REF_NAME", "").lstrip("v")
    if tag and tag not in version_output:
        raise RuntimeError(
            f"Version mismatch: expected {tag} in output, got: {version_output}"
        )
    print(f"  version: {version_output}")
    return version_output


def run_diff_check(binary_path, spec_url):
    spec_dir = Path(tempfile.mkdtemp())
    spec_path = spec_dir / "test_spec.yaml"
    download(spec_url, spec_path)
    result = subprocess.run(
        [str(binary_path), "diff", str(spec_path), str(spec_path)],
        capture_output=True, text=True, timeout=30,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"diff self-check failed: exit {result.returncode}\n{result.stderr.strip()}"
        )
    if "No changes detected" not in result.stdout:
        raise RuntimeError(
            f"diff output unexpected: {result.stdout.strip()}"
        )
    print("  diff self-check: No changes detected")


def main():
    spec_url = (
        "https://raw.githubusercontent.com/hitesh518-collab/apiwatch"
        f"/{os.environ.get('GITHUB_SHA', 'main')}"
        "/testdata/openapi/verify_matching.yaml"
    )

    binary_checks = {
        "linux-x86_64": {
            "asset": "apiwatch-x86_64-unknown-linux-gnu.tar.gz",
            "binary": "apiwatch",
            "sha256_env": None,
        },
    }

    tag = os.environ.get("GITHUB_REF_NAME", "")
    repo = os.environ.get("GITHUB_REPOSITORY", "hitesh518-collab/apiwatch")
    results = []

    for label, info in binary_checks.items():
        print(f"\n--- {label} ---")
        try:
            asset_url = (
                f"https://github.com/{repo}/releases/download/{tag}/{info['asset']}"
            )
            tmp_dir = Path(tempfile.mkdtemp())
            archive_path = tmp_dir / info["asset"]
            download(asset_url, archive_path)

            if archive_path.suffix == ".gz":
                subprocess.run(
                    ["tar", "xzf", str(archive_path), "-C", str(tmp_dir)],
                    check=True, capture_output=True,
                )
            elif archive_path.suffix == ".zip":
                subprocess.run(
                    ["unzip", "-q", str(archive_path), "-d", str(tmp_dir)],
                    check=True, capture_output=True,
                )

            binary_path = tmp_dir / info["binary"]
            binary_path.chmod(0o755)

            run_version_check(binary_path)
            run_diff_check(binary_path, spec_url)
            results.append((label, "PASS", ""))
        except Exception as e:
            results.append((label, "FAIL", str(e)))
            print(f"  FAIL: {e}")

    print("\n--- Container ---")
    try:
        image = f"ghcr.io/{repo}:{tag}"
        subprocess.run(["docker", "pull", image], check=True)
        version = subprocess.run(
            ["docker", "run", "--rm", image, "--version"],
            capture_output=True, text=True, check=True,
        )
        print(f"  version: {version.stdout.strip()}")
        results.append(("container", "PASS", ""))
    except Exception as e:
        results.append(("container", "FAIL", str(e)))
        print(f"  FAIL: {e}")

    summary_lines = ["## Install Verification", ""]
    all_pass = True
    for label, status, detail in results:
        emoji = "PASS" if status == "PASS" else "FAIL"
        summary_lines.append(f"- **{label}**: {emoji}")
        if detail:
            summary_lines.append(f"  - Error: {detail}")
        if status != "PASS":
            all_pass = False

    summary = "\n".join(summary_lines)
    print(f"\n{summary}")

    summary_file = Path(os.environ.get("GITHUB_STEP_SUMMARY", "/dev/null"))
    if summary_file.exists() or str(summary_file) != "/dev/null":
        with open(summary_file, "a") as f:
            f.write(summary + "\n")

    if not all_pass:
        sys.exit(1)


if __name__ == "__main__":
    main()
```

- [ ] **Step 2: Add `install-verify` job to release workflow**

In `.github/workflows/release.yml`, after the `bump-packages` job, add:

```yaml
  install-verify:
    needs: [release, container]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: actions/setup-python@v5
        with:
          python-version: "3.x"

      - name: Run install smoke tests
        run: python scripts/install_smoke.py
        env:
          GITHUB_REF_NAME: ${{ github.ref_name }}
          GITHUB_REPOSITORY: ${{ github.repository }}
          GITHUB_SHA: ${{ github.sha }}

      - name: Append results to release
        uses: softprops/action-gh-release@v2
        with:
          body_path: ${{ github.step_summary }}
```

- [ ] **Step 3: Run `cargo test --workspace` to verify no regressions**

Run: `cargo test --workspace`

- [ ] **Step 4: Commit**

```bash
git add scripts/install_smoke.py .github/workflows/release.yml
git commit -m "ci: add post-release install verification smoke test across all platforms"
```

---

### Task Z: Bump Version to 1.0.0

**Files:**
- Modify: `Cargo.toml:3` — version number
- Modify: `README.md` — any version references
- Modify: `src/lib.rs:1` — already updated in A1
- Modify: `CHANGELOG.md` — v1.0.0 release entry

**Note:** This is the FINAL task after all other tasks are merged and CI is green on all new jobs.

- [ ] **Step 1: Update version in Cargo.toml**

Change `version = "0.10.0"` to `version = "1.0.0"`.

- [ ] **Step 2: Update README version references**

Search README for `0.10.0` or `v0.10.0` and replace with `1.0.0`.

- [ ] **Step 3: Add v1.0.0 release entry to CHANGELOG.md**

At the top of `CHANGELOG.md`, add:

```
## [1.0.0] — 2026-08-DD

### Added
- Frozen v4 lockfile format with SemVer guarantees
- Legacy lock format feature gate (`legacy-lock-format`)
- Parser fuzzing with `cargo-fuzz` for OpenAPI, v4 roundtrip, and observed infer
- Performance budget regression gates (diff and lock on compat corpus)
- Expanded compatibility corpus (N specs, X passing, Y known-failing)
- Deterministic output snapshot hashes for lock and diff
- Migration documentation and version upgrade tests
- Post-release install verification smoke test

### Changed
- v2/v3 lock loading gated behind `legacy-lock-format` feature (on by default)
- Public library interface marked as v1 stable
```

- [ ] **Step 4: Run `cargo test --workspace` (final gate)**

Run: `cargo test --workspace`

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml README.md CHANGELOG.md
git commit -m "chore: bump version to 1.0.0"
```

- [ ] **Step 6: Create annotated tag**

```bash
git tag -a v1.0.0 -m "v1.0.0: frozen v4 lockfile, SemVer guarantees, fuzzing, perf budgets, expanded corpus, snapshots, migration docs"
```

---

## Task Dependency Graph

```
A1 (SemVer) ──────┬──> A2 (Fuzzing)
                  │
                  ├──> A3 (Perf)
                  │         │
                  └──> B1 (Corpus) ──> B2 (Snapshots)
                            │
                            └──> C1 (Migration)
                                      │
                            C2 (Install Verify, depends on release.yml structure, independent)

All tracks ──> Z (Bump to 1.0.0 + tag)
```

Tasks within a track are sequential; tracks B and C start after A1 completes (A2 and A3 can run in parallel with B1/C1).
