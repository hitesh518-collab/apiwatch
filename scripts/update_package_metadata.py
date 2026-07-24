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
