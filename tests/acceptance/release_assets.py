#!/usr/bin/env python3
"""Deterministic asset-set guard tests; fixture bytes are not built artifacts."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("release_assets", ROOT / "scripts/verify-release-assets.py")
assert spec is not None and spec.loader is not None
assets = importlib.util.module_from_spec(spec)
spec.loader.exec_module(assets)


class ReleaseAssets(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="foglio-release-assets-")
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        for name in assets.expected_assets("0.5.0"):
            (self.directory / name).write_bytes(b"fixture")
        self.asset = self.directory / "Foglio_0.5.0_amd64.deb"

    def test_complete_set(self):
        assets.verify(self.directory, "0.5.0")

    def test_missing_or_stale_asset(self):
        self.asset.unlink()
        with self.assertRaisesRegex(ValueError, "missing assets"):
            assets.verify(self.directory, "0.5.0")
        self.asset.write_bytes(b"fixture")
        (self.directory / "Foglio_0.4.0_amd64.deb").write_bytes(b"stale fixture")
        with self.assertRaisesRegex(ValueError, "unexpected assets"):
            assets.verify(self.directory, "0.5.0")

    def test_empty_directory_or_symlink_is_not_an_asset(self):
        self.asset.write_bytes(b"")
        with self.assertRaises(ValueError):
            assets.verify(self.directory, "0.5.0")
        self.asset.unlink()
        self.asset.mkdir()
        with self.assertRaises(ValueError):
            assets.verify(self.directory, "0.5.0")
        self.asset.rmdir()
        target = self.directory / "Foglio_0.5.0_x64.dmg"
        self.asset.symlink_to(target)
        with self.assertRaises(ValueError):
            assets.verify(self.directory, "0.5.0")

    def test_invalid_version(self):
        for version in ("v0.5.0", "0.5", "0.5.0-rc1", "../0.5.0"):
            with self.assertRaises(ValueError):
                assets.expected_assets(version)


if __name__ == "__main__":
    unittest.main(verbosity=2)
