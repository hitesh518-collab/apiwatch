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


def sha256_of_string(text):
    return hashlib.sha256(text.encode()).hexdigest()


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
                binary, "lock",
                str(spec_path),
                "--name", name,
                "--output", str(lock_out),
            ],
            capture_output=True, text=True,
        )
        if result.returncode != 0:
            lock_hash = sha256_of_string(result.stderr)
        else:
            lock_hash = sha256_of_file(lock_out)

        diff_out = tmp_dir / f"snapshot_{name}_diff.txt"
        with open(diff_out, "w") as f:
            diff_result = subprocess.run(
                [binary, "diff", str(spec_path), str(spec_path)],
                stdout=f, stderr=subprocess.PIPE, text=True,
            )
        if diff_result.returncode != 0:
            diff_hash = sha256_of_string(diff_result.stderr)
        else:
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
