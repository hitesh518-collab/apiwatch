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
        run(
            [
                "cargo",
                "install",
                "--path",
                ROOT,
                "--root",
                install_root,
                "--locked",
                "--force",
            ]
        )
        binary = install_root / "bin" / (
            "apiwatch.exe" if sys.platform == "win32" else "apiwatch"
        )

        version = run([binary, "--version"]).stdout
        if "apiwatch 0.7.0" not in version:
            raise RuntimeError(f"unexpected version output: {version}")

        run(
            [
                binary,
                "diff",
                ROOT / "testdata/openapi/no_breaking_old.yaml",
                ROOT / "testdata/openapi/no_breaking_new.yaml",
            ]
        )

        declared_lock = temporary / "declared.lock"
        run(
            [
                binary,
                "lock",
                ROOT / "testdata/openapi/verify_matching.yaml",
                "--name",
                "users",
                "--output",
                declared_lock,
            ]
        )
        run(
            [
                binary,
                "verify",
                ROOT / "testdata/openapi/verify_matching.yaml",
                "--name",
                "users",
                "--lock",
                declared_lock,
            ]
        )
        run(
            [
                binary,
                "verify",
                ROOT / "testdata/openapi/verify_current.yaml",
                "--name",
                "users",
                "--lock",
                declared_lock,
            ],
            expected=1,
        )

        observed_lock = temporary / "observed.lock"
        run(
            [
                binary,
                "record",
                "--from-json",
                ROOT / "testdata/observed/portfolio-empty.json",
                "--name",
                "portfolio",
                "--output",
                observed_lock,
            ]
        )
        run(
            [
                binary,
                "record",
                "--from-json",
                ROOT / "testdata/observed/portfolio-populated.json",
                "--name",
                "portfolio",
                "--output",
                observed_lock,
                "--merge",
            ]
        )
        run(
            [
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
                "--map-at",
                "$.state.by_region",
            ]
        )
        run(
            [
                binary,
                "verify",
                ROOT / "testdata/observed/portfolio-map-matching.json",
                "--name",
                "portfolio-map",
                "--lock",
                observed_lock,
            ]
        )

        json_match = run(
            [
                binary,
                "verify",
                ROOT / "testdata/observed/portfolio-matching.json",
                "--name",
                "portfolio",
                "--lock",
                observed_lock,
                "--format",
                "json",
            ]
        )
        rendered = json.loads(json_match.stdout)
        if rendered["summary"] != {"breaking": 0} or rendered["changes"] != []:
            raise RuntimeError("matching observed JSON output is not empty")

        sarif_match = run(
            [
                binary,
                "verify",
                ROOT / "testdata/observed/portfolio-matching.json",
                "--name",
                "portfolio",
                "--lock",
                observed_lock,
                "--format",
                "sarif",
            ]
        )
        sarif = json.loads(sarif_match.stdout)
        if sarif["runs"][0]["results"] != []:
            raise RuntimeError("matching observed SARIF results are not empty")

        run(
            [
                binary,
                "verify",
                ROOT / "testdata/observed/portfolio-type-drift.json",
                "--name",
                "portfolio",
                "--lock",
                observed_lock,
            ],
            expected=1,
        )
        run(
            [
                binary,
                "diff",
                ROOT / "testdata/openapi/invalid_yaml.yaml",
                ROOT / "testdata/openapi/no_breaking_new.yaml",
            ],
            expected=2,
        )

    print("release smoke passed")


if __name__ == "__main__":
    main()
