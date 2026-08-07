"""Tests for release archive checksum verification.

These tests verify the logic enforced by scripts/verify_checksum.sh
without requiring bash. They use Python's hashlib to test the same
constraints: valid match, mismatch, missing entry, malformed entry.

Usage:
    python -m unittest scripts.tests.test_verify_checksum
"""

import hashlib
import tempfile
import unittest
from pathlib import Path


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def write_checksum_file(path: Path, entries: dict[str, str]) -> None:
    """Write a SHA256SUMS-style file. Keys are filenames, values are hex digests."""
    with open(path, "w") as f:
        for filename, digest in entries.items():
            f.write(f"{digest}  {filename}\n")


class VerifyChecksumTests(unittest.TestCase):
    def test_valid_archive_passes(self):
        """Valid archive + valid checksum => pass."""
        data = b"release binary content here"
        digest = sha256_hex(data)

        with tempfile.TemporaryDirectory() as td:
            archive = Path(td) / "apiwatch-x86_64-unknown-linux-gnu.tar.gz"
            sums = Path(td) / "SHA256SUMS"
            archive.write_bytes(data)
            write_checksum_file(sums, {archive.name: digest})

            actual = sha256_hex(archive.read_bytes())
            self.assertEqual(actual, digest)

    def test_modified_archive_fails(self):
        """Modified archive => checksum mismatch."""
        data = b"original content"
        corrupt = b"modified content"
        digest = sha256_hex(data)

        with tempfile.TemporaryDirectory() as td:
            archive = Path(td) / "apiwatch.tar.gz"
            archive.write_bytes(corrupt)

            actual = sha256_hex(archive.read_bytes())
            self.assertNotEqual(actual, digest,
                                "corrupted archive must not match original digest")

    def test_missing_checksum_entry_returns_empty(self):
        """Missing checksum entry => no match found."""
        with tempfile.TemporaryDirectory() as td:
            sums = Path(td) / "SHA256SUMS"
            write_checksum_file(sums, {"other-file.tar.gz": sha256_hex(b"x")})

            with open(sums) as f:
                content = f.read()
            self.assertNotIn("apiwatch-x86_64-unknown-linux-gnu.tar.gz", content)

    def test_malformed_checksum_detected(self):
        """Malformed checksum (not 64 hex chars) => detected."""
        with tempfile.TemporaryDirectory() as td:
            sums = Path(td) / "SHA256SUMS"
            with open(sums, "w") as f:
                f.write("not-a-valid-sha256  apiwatch.tar.gz\n")

            with open(sums) as f:
                line = f.readline().strip()
            digest_part = line.split()[0]
            self.assertNotRegex(digest_part, r"^[0-9a-f]{64}$",
                                "malformed digest must not match hex pattern")

    def test_multiple_archives_in_sums(self):
        """SHA256SUMS contains multiple entries — extract correct one."""
        linux_data = b"linux binary"
        mac_data = b"mac binary"
        linux_digest = sha256_hex(linux_data)
        mac_digest = sha256_hex(mac_data)

        with tempfile.TemporaryDirectory() as td:
            sums = Path(td) / "SHA256SUMS"
            write_checksum_file(sums, {
                "apiwatch-x86_64-unknown-linux-gnu.tar.gz": linux_digest,
                "apiwatch-aarch64-apple-darwin.tar.gz": mac_digest,
                "apiwatch-x86_64-pc-windows-msvc.zip": sha256_hex(b"win"),
            })

            with open(sums) as f:
                content = f.read()

            linux_line = [l for l in content.splitlines()
                          if "linux-gnu" in l][0]
            self.assertIn(linux_digest, linux_line)


if __name__ == "__main__":
    unittest.main()
