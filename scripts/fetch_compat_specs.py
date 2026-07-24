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
