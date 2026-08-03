import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "bump_version.py"
SPEC = importlib.util.spec_from_file_location("bump_version", SCRIPT)
bumper = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(bumper)


class BumpVersionTests(unittest.TestCase):
    def test_bumps_all_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)

            # Create Cargo.toml
            (root / "Cargo.toml").write_text(
                'version = "0.6.0"\n', encoding="utf-8"
            )

            # Create CHANGELOG.md
            (root / "CHANGELOG.md").write_text(
                "# Changelog\n\n## v0.6.0\n\nstuff\n", encoding="utf-8"
            )

            # Create Formula/apiwatch.rb
            formula_dir = root / "Formula"
            formula_dir.mkdir()
            (formula_dir / "apiwatch.rb").write_text(
                '  url "https://github.com/o/r/archive/refs/tags/v0.6.0.tar.gz"\n'
                '  sha256 "' + ("a" * 64) + '"\n',
                encoding="utf-8",
            )

            # Create Scoop/apiwatch.json
            scoop_dir = root / "Scoop"
            scoop_dir.mkdir()
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

            # Create scripts/release_smoke.py (should NOT be modified by bump_version.py)
            scripts_dir = root / "scripts"
            scripts_dir.mkdir()
            (scripts_dir / "release_smoke.py").write_text(
                'if "apiwatch 0.6.0" not in version:\n'
                '    raise RuntimeError("bad version")\n',
                encoding="utf-8",
            )

            bumper.bump(root, "0.7.0", "b" * 64)

            # Verify Cargo.toml
            cargo = (root / "Cargo.toml").read_text(encoding="utf-8")
            self.assertEqual(cargo, 'version = "0.7.0"\n')

            # Verify CHANGELOG.md
            changelog = (root / "CHANGELOG.md").read_text(encoding="utf-8")
            self.assertTrue(changelog.startswith("# Changelog\n\n## v0.7.0 -"))

            # Verify Formula
            formula = (formula_dir / "apiwatch.rb").read_text(encoding="utf-8")
            self.assertIn("/v0.7.0.tar.gz", formula)
            self.assertIn('sha256 "' + ("b" * 64) + '"', formula)

            # Verify Scoop
            scoop = json.loads(
                (scoop_dir / "apiwatch.json").read_text(encoding="utf-8")
            )
            self.assertEqual(scoop["version"], "0.7.0")
            self.assertEqual(scoop["hash"], "b" * 64)
            self.assertEqual(scoop["extract_dir"], "apiwatch-0.7.0")
            self.assertTrue(scoop["url"].endswith("/v0.7.0.tar.gz"))

            # Verify release_smoke.py was NOT modified
            smoke = (scripts_dir / "release_smoke.py").read_text(encoding="utf-8")
            self.assertIn('"apiwatch 0.6.0"', smoke)


if __name__ == "__main__":
    unittest.main()
