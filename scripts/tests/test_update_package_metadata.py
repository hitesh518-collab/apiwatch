import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "update_package_metadata.py"
SPEC = importlib.util.spec_from_file_location("update_package_metadata", SCRIPT)
updater = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(updater)


class UpdatePackageMetadataTests(unittest.TestCase):
    def test_updates_formula_and_scoop(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            formula_dir = root / "Formula"
            scoop_dir = root / "Scoop"
            formula_dir.mkdir()
            scoop_dir.mkdir()
            (formula_dir / "apiwatch.rb").write_text(
                '  url "https://github.com/o/r/archive/refs/tags/v0.6.0.tar.gz"\n'
                '  sha256 "' + ("a" * 64) + '"\n',
                encoding="utf-8",
            )
            (scoop_dir / "apiwatch.json").write_text(
                json.dumps(
                    {
                        "version": "0.6.0",
                        "url": "https://github.com/o/r/archive/refs/tags/v0.6.0.tar.gz",
                        "hash": "a" * 64,
                        "extract_dir": "apiwatch-0.6.0",
                    }
                ),
                encoding="utf-8",
            )

            updater.update(root, "0.7.0", "b" * 64)

            formula = (formula_dir / "apiwatch.rb").read_text(encoding="utf-8")
            self.assertIn("/v0.7.0.tar.gz", formula)
            self.assertIn('sha256 "' + ("b" * 64) + '"', formula)
            scoop = json.loads(
                (scoop_dir / "apiwatch.json").read_text(encoding="utf-8")
            )
            self.assertEqual(scoop["version"], "0.7.0")
            self.assertEqual(scoop["hash"], "b" * 64)
            self.assertEqual(scoop["extract_dir"], "apiwatch-0.7.0")
            self.assertTrue(scoop["url"].endswith("/v0.7.0.tar.gz"))


if __name__ == "__main__":
    unittest.main()
