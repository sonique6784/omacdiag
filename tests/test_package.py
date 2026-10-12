"""Preflight checks for Linux archive packaging."""

import json
import os
import platform
import subprocess
import tempfile
import tomllib
import unittest
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from runtime_versions import qt_version_at_least

PACKAGE = ROOT / "scripts/package.py"
VERSION = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
NAME = f"omacdiag-{VERSION}-linux-x86_64-cli"


class QtVersionTests(unittest.TestCase):
    def test_qt_six_minimum_and_newer_patches(self):
        self.assertTrue(qt_version_at_least("6.8.0", "6.8.0"))
        self.assertTrue(qt_version_at_least("6.11.2", "6.8.0"))
        for version in ("6.7.9", "7.0.0", "6.8", "6.8.0-rc1", "wrong", None):
            with self.subTest(version=version):
                self.assertFalse(qt_version_at_least(version, "6.8.0"))


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
        self.assertIn("Runtime Qt must be >= 6.8.0", result.stderr)
        self.assertFalse(self.output.exists())

    def test_incomplete_pinned_runtime_does_not_create_stage(self):
        self.executable("omacdiag")
        self.executable("omacdiag-helper")
        runtime = self.root / "runtime"
        runtime.mkdir()
        pinned = ROOT / "packaging/runtime-versions.json"
        (runtime / "runtime-versions.json").write_text(json.dumps({
            **json.loads(pinned.read_text()), "qt": "6.8.0",
        }))
        result = self.run_package("--runtime", str(runtime))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("bin/quickshell", result.stderr)
        self.assertFalse(self.output.exists())

    def test_desktop_accepts_newer_qt_and_records_exact_version(self):
        self.executable("omacdiag")
        self.executable("omacdiag-helper")
        runtime = self.root / "runtime"
        (runtime / "bin").mkdir(parents=True)
        (runtime / "bin/quickshell").write_text("#!/bin/sh\n")
        (runtime / "bin/quickshell").chmod(0o755)
        for directory in ("lib", "plugins", "qml"):
            (runtime / directory).mkdir()
        versions = json.loads((ROOT / "packaging/runtime-versions.json").read_text())
        (runtime / "runtime-versions.json").write_text(json.dumps({**versions, "qt": "6.11.2"}))
        result = self.run_package("--runtime", str(runtime))
        self.assertEqual(result.returncode, 0, result.stderr)
        stage = self.output / f"omacdiag-{VERSION}-linux-x86_64-desktop"
        staged = json.loads((stage / "runtime/runtime-versions.json").read_text())
        self.assertEqual(staged["qt"], "6.11.2")

    def test_desktop_rejects_older_qt(self):
        self.executable("omacdiag")
        self.executable("omacdiag-helper")
        runtime = self.root / "runtime"
        runtime.mkdir()
        versions = json.loads((ROOT / "packaging/runtime-versions.json").read_text())
        (runtime / "runtime-versions.json").write_text(json.dumps({**versions, "qt": "6.7.9"}))
        result = self.run_package("--runtime", str(runtime))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Runtime Qt must be >= 6.8.0", result.stderr)
        self.assertFalse(self.output.exists())

    def test_system_qt_desktop_archive_and_staged_install(self):
        self.executable("omacdiag")
        self.executable("omacdiag-helper")
        result = self.run_package("--system-qt")
        self.assertEqual(result.returncode, 0, result.stderr)
        stage = self.output / f"omacdiag-{VERSION}-linux-x86_64-desktop"
        self.assertTrue((stage / "ui/shell.qml").is_file())
        self.assertTrue((stage / "scripts/launch-ui.sh").is_file())
        self.assertFalse((stage / "runtime").exists())
        requirements = json.loads((stage / "system-qt.json").read_text())
        self.assertEqual(requirements["provider"], "system")
        self.assertEqual(requirements["qt_minimum"], "6.8.0")
        dest = self.root / "system"
        install = subprocess.run(
            ["sh", str(stage / "packaging/install.sh")],
            env={**os.environ, "DESTDIR": str(dest)}, capture_output=True, text=True,
        )
        self.assertEqual(install.returncode, 0, install.stderr)
        self.assertTrue((dest / "opt/omacdiag/system-qt.json").is_file())

    def test_cli_archive_and_existing_artifact_guard(self):
        self.executable("omacdiag")
        self.executable("omacdiag-helper")
        first = self.run_package("--cli-only")
        self.assertEqual(first.returncode, 0, first.stderr)
        self.assertTrue((self.output / NAME / "bin/omacdiag").is_file())
        self.assertTrue((self.output / NAME / "third_party/README.md").is_file())
        self.assertTrue((self.output / NAME / "third_party/licenses/unicode-ident-1.0.25/LICENSE-UNICODE").is_file())
        archive = self.output / (NAME + ".tar.gz")
        self.assertTrue(archive.is_file())
        original_size = os.stat(archive).st_size
        second = self.run_package("--cli-only")
        self.assertNotEqual(second.returncode, 0)
        self.assertIn("already exists", second.stderr)
        self.assertEqual(os.stat(archive).st_size, original_size)

    def test_staged_install_and_uninstall_leave_unrelated_file(self):
        self.executable("omacdiag")
        self.executable("omacdiag-helper")
        result = self.run_package("--cli-only")
        self.assertEqual(result.returncode, 0, result.stderr)
        stage = self.output / NAME
        dest = self.root / "system"
        unrelated = dest / "usr/bin/other-app"
        unrelated.parent.mkdir(parents=True)
        unrelated.write_text("keep")
        install = subprocess.run(
            ["sh", str(stage / "packaging/install.sh")],
            env={**os.environ, "DESTDIR": str(dest)}, capture_output=True, text=True,
        )
        self.assertEqual(install.returncode, 0, install.stderr)
        self.assertTrue((dest / "opt/omacdiag/bin/omacdiag").exists())
        repeat = subprocess.run(
            ["sh", str(stage / "packaging/install.sh")],
            env={**os.environ, "DESTDIR": str(dest)}, capture_output=True, text=True,
        )
        self.assertNotEqual(repeat.returncode, 0)
        self.assertIn("already installed", repeat.stderr)
        uninstall = subprocess.run(
            ["sh", str(dest / "opt/omacdiag/packaging/uninstall.sh")],
            env={**os.environ, "DESTDIR": str(dest)}, capture_output=True, text=True,
        )
        self.assertEqual(uninstall.returncode, 0, uninstall.stderr)
        self.assertFalse((dest / "opt/omacdiag").exists())
        self.assertFalse((dest / "usr/bin/omacdiag").exists())
        self.assertEqual(unrelated.read_text(), "keep")

    def test_incomplete_staged_archive_does_not_create_install_tree(self):
        self.executable("omacdiag")
        self.executable("omacdiag-helper")
        result = self.run_package("--cli-only")
        self.assertEqual(result.returncode, 0, result.stderr)
        stage = self.output / NAME
        (stage / "bin/omacdiag-helper").unlink()
        dest = self.root / "system"
        install = subprocess.run(
            ["sh", str(stage / "packaging/install.sh")],
            env={**os.environ, "DESTDIR": str(dest)}, capture_output=True, text=True,
        )
        self.assertNotEqual(install.returncode, 0)
        self.assertIn("Incomplete OmacDiag archive", install.stderr)
        self.assertFalse((dest / "opt/omacdiag").exists())


if __name__ == "__main__":
    unittest.main()
