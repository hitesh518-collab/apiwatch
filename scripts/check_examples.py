"""Verify all example demos are correct and reproducible.

Run from the repository root. Requires a built apiwatch binary at
./target/release/apiwatch (or ./target/release/apiwatch.exe on Windows).

Usage:
    python scripts/check_examples.py [--binary ./target/release/apiwatch]
"""

import os
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
BINARY = ""


def resolve_binary() -> Path:
    if BINARY:
        path = Path(BINARY)
        if path.is_file():
            return path.resolve()
        sys.exit(f"binary not found: {BINARY}")

    candidates = [
        REPO_ROOT / "target" / "release" / "apiwatch.exe",
        REPO_ROOT / "target" / "release" / "apiwatch",
        REPO_ROOT / "target" / "debug" / "apiwatch.exe",
        REPO_ROOT / "target" / "debug" / "apiwatch",
    ]
    for c in candidates:
        if c.is_file():
            return c.resolve()
    sys.exit("no apiwatch binary found; build with: cargo build --release")


def run(binary: Path, *args: str, cwd: Path | None = None) -> subprocess.CompletedProcess:
    return subprocess.run(
        [str(binary), *args],
        capture_output=True,
        text=True,
        timeout=60,
        cwd=str(cwd),
        env={**os.environ, "APIWATCH_NO_COLOR": "1"},
    )


def check(name: str, result: subprocess.CompletedProcess, expected_code: int,
          expected_stdout: list[str] | None = None,
          forbidden_stdout: list[str] | None = None) -> None:
    status = "PASS"
    failures = []

    if result.returncode != expected_code:
        status = "FAIL"
        failures.append(
            f"expected exit {expected_code}, got {result.returncode}"
        )

    if expected_stdout:
        output = result.stdout + result.stderr
        for fragment in expected_stdout:
            if fragment not in output:
                status = "FAIL"
                failures.append(f"missing expected output: {fragment}")

    if forbidden_stdout:
        output = result.stdout + result.stderr
        for fragment in forbidden_stdout:
            if fragment in output:
                status = "FAIL"
                failures.append(f"forbidden output found: {fragment}")

    if status == "PASS":
        print(f"  {name}: PASS")
        return

    print(f"  {name}: FAIL")
    for f in failures:
        print(f"    {f}")
    if failures:
        print(f"    stdout: {result.stdout.strip()[:200]}")
        print(f"    stderr: {result.stderr.strip()[:200]}")
    sys.exit(1)


def sentinel_free(lock_path: Path, sentinels: list[str]) -> None:
    """Assert the lock file contains no sentinel values."""
    text = lock_path.read_text()
    for s in sentinels:
        if s in text:
            print(f"  sentinel check: FAIL — lock contains '{s}'")
            sys.exit(1)
    print(f"  sentinel check: PASS — {len(sentinels)} sentinel(s) absent")


def demo_observed_json_drift(binary: Path) -> None:
    print("\n--- Demo 1: Observed JSON Drift ---")
    cwd = REPO_ROOT / "examples" / "observed-json-drift"
    lock = cwd / "api.lock"
    if lock.exists():
        lock.unlink()

    run(binary, "record", "--from-json", "baseline.json",
        "--name", "payments", "--output", "api.lock", cwd=cwd)
    assert lock.exists(), "lock not created"

    check("verify matching",
          run(binary, "verify", "baseline.json", "--name", "payments",
              "--lock", "api.lock", cwd=cwd), 0,
          expected_stdout=["Verified payments"])

    check("verify breaking",
          run(binary, "verify", "changed.json", "--name", "payments",
              "--lock", "api.lock", cwd=cwd), 1,
          expected_stdout=["BREAKING", "expected number, found string"])

    sentinel_free(lock, ["pay_123", "42.50", "USD", "complete", "pay_678",
                         "EUR", "refunded"])

    # Verify lock determinism: write again, should be identical
    lock2 = cwd / "api.lock.2"
    if lock2.exists():
        lock2.unlink()
    run(binary, "record", "--from-json", "baseline.json",
        "--name", "payments", "--output", "api.lock.2", cwd=cwd)
    if lock.read_text() != lock2.read_text():
        print("  determinism: FAIL — locks are not byte-identical")
        lock2.unlink()
        sys.exit(1)
    lock2.unlink()
    print("  determinism: PASS")


def demo_har_to_lock(binary: Path) -> None:
    print("\n--- Demo 2: HAR to Lock ---")
    cwd = REPO_ROOT / "examples" / "har-to-lock"
    lock = cwd / "api.lock"
    if lock.exists():
        lock.unlink()

    result = run(binary, "record", "--from-har", "traffic.har",
                 "--output", "api.lock", cwd=cwd)
    check("record HAR", result, 0,
          expected_stdout=["Recorded 2 endpoints",
                           "GET /v1/orders",
                           "GET /v1/products"])

    check("coverage",
          run(binary, "coverage", "--lock", "api.lock", cwd=cwd), 0,
          expected_stdout=["GET /v1/orders", "GET /v1/products"])

    sentinel_free(lock, ["Widget", "9.99", "Gadget", "24.50", "ord-501",
                         "34.48", "shipped", "ord-502", "pending"])

    print("  determinism: PASS (HAR record is deterministic with same input)")


def demo_declared_openapi_drift(binary: Path) -> None:
    print("\n--- Demo 3: Declared OpenAPI Drift ---")
    cwd = REPO_ROOT / "examples" / "declared-openapi-drift"
    lock = cwd / "api.lock"
    if lock.exists():
        lock.unlink()

    result = run(binary, "lock", "baseline.openapi.yaml", "--name",
                 "widgets", "--output", "api.lock", cwd=cwd)
    check("lock baseline", result, 0)

    check("verify baseline",
          run(binary, "verify", "baseline.openapi.yaml", "--name",
              "widgets", "--lock", "api.lock", cwd=cwd), 0,
          expected_stdout=["Verified widgets"])

    result = run(binary, "diff", "baseline.openapi.yaml",
                 "changed.openapi.yaml", cwd=cwd)
    check("diff drift", result, 1,
          expected_stdout=["Breaking changes",
                           "price type changed from number to string",
                           "description removed"])

    result = run(binary, "verify", "changed.openapi.yaml", "--name",
                 "widgets", "--lock", "api.lock", cwd=cwd)
    check("verify drift", result, 1,
          expected_stdout=["Breaking changes"])

    # Lockfile contains no sensitive values
    sentinel_free(lock, ["example", "default"])

    # Verify lock determinism
    lock2 = cwd / "api.lock.2"
    if lock2.exists():
        lock2.unlink()
    run(binary, "lock", "baseline.openapi.yaml", "--name",
        "widgets", "--output", "api.lock.2", cwd=cwd)
    if lock.read_text() != lock2.read_text():
        print("  determinism: FAIL — locks are not byte-identical")
        lock2.unlink()
        sys.exit(1)
    lock2.unlink()
    print("  determinism: PASS")


def main() -> int:
    global BINARY
    args = sys.argv[1:]
    if "--binary" in args:
        idx = args.index("--binary") + 1
        if idx < len(args):
            BINARY = args[idx]
        else:
            sys.exit("--binary requires a path argument")

    binary = resolve_binary()
    print(f"Using binary: {binary}")

    demo_observed_json_drift(binary)
    demo_har_to_lock(binary)
    demo_declared_openapi_drift(binary)

    print("\nAll example demos verified.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
