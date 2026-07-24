import hashlib
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).resolve().parents[1] / "fetch_compat_specs.py"
SPEC = importlib.util.spec_from_file_location("fetch_compat_specs", SCRIPT)
fetcher = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(fetcher)


class FakeResponse(io.BytesIO):
    status = 200

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, traceback):
        self.close()


class FetchCompatSpecsTests(unittest.TestCase):
    def entry(self, payload=b"contract"):
        return {
            "name": "example",
            "file": "example.json",
            "url": (
                "https://raw.githubusercontent.com/example/api/"
                "0123456789abcdef0123456789abcdef01234567/openapi.json"
            ),
            "sha256": hashlib.sha256(payload).hexdigest(),
            "max_bytes": 1024,
            "status": "passing",
        }

    def test_rejects_mutable_upstream_url(self):
        entry = self.entry()
        entry["url"] = "https://raw.githubusercontent.com/example/api/main/openapi.json"
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "immutable 40-character commit"):
                fetcher.fetch_entry(entry, Path(directory))

    def test_downloads_and_reuses_a_verified_cache_entry(self):
        payload = b"contract"
        with tempfile.TemporaryDirectory() as directory:
            cache = Path(directory)
            with patch.object(
                fetcher.urllib.request,
                "urlopen",
                return_value=FakeResponse(payload),
            ) as urlopen:
                size = fetcher.fetch_entry(self.entry(payload), cache)
            self.assertEqual(size, len(payload))
            self.assertEqual((cache / "example.json").read_bytes(), payload)
            urlopen.assert_called_once()

            with patch.object(
                fetcher.urllib.request,
                "urlopen",
                side_effect=AssertionError("network should not be used"),
            ):
                reused_size = fetcher.fetch_entry(self.entry(payload), cache)
            self.assertEqual(reused_size, len(payload))

    def test_hash_mismatch_does_not_replace_cached_file(self):
        payload = b"unexpected"
        entry = self.entry(b"expected")
        with tempfile.TemporaryDirectory() as directory:
            cache = Path(directory)
            with patch.object(
                fetcher.urllib.request,
                "urlopen",
                return_value=FakeResponse(payload),
            ):
                with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
                    fetcher.fetch_entry(entry, cache)
            self.assertFalse((cache / "example.json").exists())


if __name__ == "__main__":
    unittest.main()
