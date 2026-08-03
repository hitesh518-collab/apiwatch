"""Verify that released binaries and container images install and run."""
import os
import subprocess
import sys
import tempfile
import urllib.request
from pathlib import Path


def download(url, dest):
    urllib.request.urlretrieve(url, dest)


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
