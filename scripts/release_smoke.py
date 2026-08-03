#!/usr/bin/env python3
import json
from pathlib import Path
import re
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
        if not re.fullmatch(r"apiwatch \d+\.\d+\.\d+( \([0-9a-f]+\))?\n?$", version):
            raise RuntimeError(f"unexpected version output: {version!r}")

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
        declared_json = json.loads(
            run(
                [
                    binary,
                    "verify",
                    ROOT / "testdata/openapi/verify_matching.yaml",
                    "--name",
                    "users",
                    "--lock",
                    declared_lock,
                    "--format",
                    "json",
                ]
            ).stdout
        )
        if (
            declared_json["coverage"] != "full"
            or declared_json["limitations"] != []
            or declared_json["changes"] != []
        ):
            raise RuntimeError("matching v4 JSON Verify did not report full coverage")
        declared_sarif = json.loads(
            run(
                [
                    binary,
                    "verify",
                    ROOT / "testdata/openapi/verify_matching.yaml",
                    "--name",
                    "users",
                    "--lock",
                    declared_lock,
                    "--format",
                    "sarif",
                ]
            ).stdout
        )
        if declared_sarif["runs"][0]["results"] != []:
            raise RuntimeError("matching v4 SARIF Verify reported findings")
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

        phase2_lock = temporary / "phase2.lock"
        run(
            [
                binary,
                "lock",
                ROOT / "testdata/openapi/phase2_d01_request_body_old.yaml",
                "--name",
                "phase2",
                "--output",
                phase2_lock,
            ]
        )
        run(
            [
                binary,
                "verify",
                ROOT / "testdata/openapi/phase2_d01_request_body_new.yaml",
                "--name",
                "phase2",
                "--lock",
                phase2_lock,
            ],
            expected=1,
        )

        legacy_v3 = json.loads(
            run(
                [
                    binary,
                    "verify",
                    ROOT / "testdata/openapi/verify_matching.yaml",
                    "--name",
                    "users",
                    "--lock",
                    ROOT / "testdata/lock/v3_users.lock",
                    "--format",
                    "json",
                ]
            ).stdout
        )
        if (
            legacy_v3["coverage"] != "partial"
            or legacy_v3["limitations"][0]["code"] != "phase2_relock_required"
        ):
            raise RuntimeError("v3 Verify did not report partial Phase 2 coverage")

        original_v4_bytes = declared_lock.read_bytes()
        run(
            [
                binary,
                "lock",
                ROOT / "testdata/openapi/verify_matching.yaml",
                "--name",
                "users",
                "--output",
                declared_lock,
                "--max-lock-bytes",
                "1",
                "--update",
            ],
            expected=2,
        )
        if declared_lock.read_bytes() != original_v4_bytes:
            raise RuntimeError("failed v4 update changed existing lock bytes")

        d16_lock = temporary / "d16.lock"
        run(
            [
                binary,
                "lock",
                ROOT / "testdata/openapi/v3_d16_old.yaml",
                "--name",
                "d16",
                "--output",
                d16_lock,
            ]
        )
        d16_verify = run(
            [
                binary,
                "verify",
                ROOT / "testdata/openapi/v3_d16_new.yaml",
                "--name",
                "d16",
                "--lock",
                d16_lock,
                "--format",
                "json",
            ],
            expected=1,
        )
        d16_json = json.loads(d16_verify.stdout)
        if (
            d16_json["coverage"] != "full"
            or d16_json["summary"]["breaking"] != 4
        ):
            raise RuntimeError("D-16 did not report full coverage and four breakages")

        legacy_verify = run(
            [
                binary,
                "verify",
                ROOT / "testdata/openapi/verify_current.yaml",
                "--name",
                "users",
                "--lock",
                ROOT / "testdata/lock/verify_users.lock",
                "--format",
                "json",
            ],
            expected=1,
        )
        legacy_json = json.loads(legacy_verify.stdout)
        if (
            legacy_json["coverage"] != "routes"
            or legacy_json["limitations"][0]["code"] != "route_only_lock"
        ):
            raise RuntimeError("legacy Verify did not report route-only coverage")

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
