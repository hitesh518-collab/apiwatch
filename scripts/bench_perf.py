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
            lock_times = []
            for run_i in range(3):
                lock_output = str(
                    root / "tmp" / f"perf_{name}_{run_i}.lock"
                )
                start = time.perf_counter()
                result = subprocess.run(
                    [
                        binary,
                        "lock",
                        "--name",
                        name,
                        "--output",
                        lock_output,
                        str(spec_path),
                    ],
                    capture_output=True,
                    text=True,
                    timeout=120,
                )
                elapsed = time.perf_counter() - start
                if result.returncode not in (0, 1):
                    raise RuntimeError(
                        f"command failed (exit {result.returncode}): apiwatch lock --name {name} --output {lock_output} {spec_path}\n"
                        f"stderr: {result.stderr[-500:]}"
                    )
                lock_times.append(elapsed)
            lock_time = statistics.median(lock_times)
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
