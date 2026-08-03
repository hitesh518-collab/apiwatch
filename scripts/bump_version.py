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
