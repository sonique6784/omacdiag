"""Preflight checks for Linux archive packaging."""

import json
import os
import platform
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PACKAGE = ROOT / "scripts/package.py"
NAME = "omacdiag-0.1.0-linux-x86_64-cli"


@unittest.skipUnless(platform.system() == "Linux" and platform.machine() == "x86_64", "Linux x86-64 package")
class PackageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="omacdiag-package-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.binaries = self.root / "binaries"
        self.binaries.mkdir()
        self.output = self.root / "output"

    def run_package(self, *options):
        return subprocess.run(
            ["python3", str(PACKAGE), "--binaries", str(self.binaries), "--output", str(self.output), *options],
            capture_output=True, text=True, check=False,
        )

    def executable(self, name):
        path = self.binaries / name
        path.write_bytes(b"#!/bin/sh\nexit 0\n")
        path.chmod(0o755)

    def test_missing_binary_does_not_create_stage(self):
        self.executable("omacdiag")
        result = self.run_package("--cli-only")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("omacdiag-helper", result.stderr)
        self.assertFalse(self.output.exists())

    def test_invalid_runtime_does_not_create_stage(self):
        self.executable("omacdiag")
        self.executable("omacdiag-helper")
        runtime = self.root / "runtime"
        runtime.mkdir()
        (runtime / "runtime-versions.json").write_text(json.dumps({"qt": "wrong"}))
        result = self.run_package("--runtime", str(runtime))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("pinned versions", result.stderr)
        self.assertFalse(self.output.exists())

    def test_incomplete_pinned_runtime_does_not_create_stage(self):
        self.executable("omacdiag")
        self.executable("omacdiag-helper")
        runtime = self.root / "runtime"
        runtime.mkdir()
        pinned = ROOT / "packaging/runtime-versions.json"
        (runtime / "runtime-versions.json").write_bytes(pinned.read_bytes())
        result = self.run_package("--runtime", str(runtime))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("bin/quickshell", result.stderr)
        self.assertFalse(self.output.exists())

    def test_cli_archive_and_existing_artifact_guard(self):
        self.executable("omacdiag")
        self.executable("omacdiag-helper")
        first = self.run_package("--cli-only")
        self.assertEqual(first.returncode, 0, first.stderr)
        self.assertTrue((self.output / NAME / "bin/omacdiag").is_file())
        archive = self.output / (NAME + ".tar.gz")
        self.assertTrue(archive.is_file())
        original_size = os.stat(archive).st_size
        second = self.run_package("--cli-only")
        self.assertNotEqual(second.returncode, 0)
        self.assertIn("already exists", second.stderr)
        self.assertEqual(os.stat(archive).st_size, original_size)


if __name__ == "__main__":
    unittest.main()
