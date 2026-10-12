"""Isolated control-flow tests for precheck.sh; no system packages are touched."""

import os
import pty
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


@unittest.skipUnless(os.name == "posix", "requires a pseudo-terminal")
class PrecheckTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="omacdiag-precheck-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        self.bin = self.root / "bin"
        self.bin.mkdir()
        (self.repo / "scripts").mkdir()
        (self.repo / "packaging").mkdir()
        shutil.copy2(ROOT / "precheck.sh", self.repo / "precheck.sh")
        shutil.copy2(ROOT / "scripts/runtime_versions.py", self.repo / "scripts/runtime_versions.py")
        shutil.copy2(ROOT / "packaging/runtime-versions.json", self.repo / "packaging/runtime-versions.json")
        self.write_command(self.repo / "verify-plugin.sh", '#!/bin/sh\nprintf "%s\\n" "$*" > "$PRECHECK_VERIFY_LOG"\n')
        self.write_command(self.bin / "pacman", '''#!/bin/bash
if [[ $1 == -Qq ]]; then
  if [[ -e $PRECHECK_INSTALLED_MARKER || ,$PRECHECK_MISSING, != *,$2,* ]]; then
    echo "$2"
    exit 0
  fi
  exit 1
fi
exit 2
''')
        self.write_command(self.bin / "sudo", '''#!/bin/bash
printf '%s\\n' "$*" >> "$PRECHECK_LOG"
touch "$PRECHECK_INSTALLED_MARKER"
''')
        self.write_command(self.bin / "git", '''#!/bin/bash
if [[ $1 == clone ]]; then
  printf '%s\\n' "$*" >> "$PRECHECK_LOG"
  source_dir=${@: -1}
  mkdir -p "$source_dir/.git"
elif [[ $1 == -C && $3 == describe ]]; then
  echo "${PRECHECK_SOURCE_TAG:-v0.3.1}"
elif [[ $1 == -C && $3 == status ]]; then
  :
elif [[ $1 == status && $2 == --porcelain ]]; then
  [[ -z ${PRECHECK_DIRTY:-} ]] || echo ' M README.md'
elif [[ $1 == status && $2 == --short ]]; then
  [[ -z ${PRECHECK_DIRTY:-} ]] || echo ' M README.md'
else
  exit 2
fi
''')
        for name in ("omarchy", "cmake", "ninja", "quickshell"):
            self.write_command(self.bin / name, "#!/bin/sh\nexit 0\n")
        self.write_command(self.bin / "qmake6", '''#!/bin/sh
case "$2" in
  QT_VERSION) echo "${PRECHECK_QT_VERSION:-6.11.2}" ;;
  QT_INSTALL_LIBS) echo /usr/lib ;;
  QT_INSTALL_PLUGINS) echo /usr/lib/qt6/plugins ;;
  QT_INSTALL_QML) echo /usr/lib/qt6/qml ;;
  *) exit 2 ;;
esac
''')
        self.env = {
            **os.environ,
            "HOME": str(self.root / "home"),
            "PATH": f"{self.bin}:{os.environ['PATH']}",
            "DISPLAY": ":0",
            "PRECHECK_LOG": str(self.root / "actions.log"),
            "PRECHECK_VERIFY_LOG": str(self.root / "verify.log"),
            "PRECHECK_INSTALLED_MARKER": str(self.root / "installed"),
            "PRECHECK_MISSING": "",
        }
        (self.root / "home").mkdir()

    @staticmethod
    def write_command(path, content):
        path.write_text(content)
        path.chmod(0o755)

    def make_sdk(self):
        sdk = self.root / "Qt/6.11.2/gcc_64"
        (sdk / "bin").mkdir(parents=True)
        self.write_command(sdk / "bin/qmake", '#!/bin/sh\necho 6.11.2\n')
        for name in ("lib", "plugins", "qml"):
            (sdk / name).mkdir()
        return sdk

    def run_precheck(self, *args, **extra_env):
        master, slave = pty.openpty()
        try:
            return subprocess.run(
                [str(self.repo / "precheck.sh"), *map(str, args)],
                stdin=slave, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                text=True, env={**self.env, **extra_env}, timeout=15, check=False,
            )
        finally:
            os.close(master)
            os.close(slave)

    def test_installs_missing_packages_and_guides_missing_sdk(self):
        source = self.root / "quickshell"
        result = self.run_precheck("--quickshell-source", source,
                                   PRECHECK_MISSING="cmake,ninja,cli11")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("complete Qt 6.8+", result.stdout)
        actions = (self.root / "actions.log").read_text()
        self.assertIn("pacman -Syu --needed cmake ninja cli11", actions)
        self.assertIn("clone --branch v0.3.1 --depth 1", actions)
        self.assertTrue((source / ".git").is_dir())
        self.assertFalse((self.root / "verify.log").exists())

    def test_dirty_candidate_stops_before_verifier(self):
        source = self.root / "quickshell"
        (source / ".git").mkdir(parents=True)
        result = self.run_precheck("--qt-root", self.make_sdk(),
                                   "--quickshell-source", source, PRECHECK_DIRTY="1")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("review and commit", result.stdout)
        self.assertFalse((self.root / "verify.log").exists())

    def test_forwards_selected_paths_to_verifier(self):
        source = self.root / "quickshell"
        (source / ".git").mkdir(parents=True)
        sdk = self.make_sdk()
        result = self.run_precheck("--qt-root", sdk, "--quickshell-source", source)
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertEqual((self.root / "verify.log").read_text().strip(),
                         f"--qt-root {sdk} --quickshell-source {source}")
        self.assertFalse((self.root / "actions.log").exists())

    def test_default_uses_system_qt_without_sdk_or_source_checkout(self):
        result = self.run_precheck()
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn("System Qt 6.11.2 is available", result.stdout)
        self.assertEqual((self.root / "verify.log").read_text().strip(), "--system-qt")
        self.assertFalse((self.root / "actions.log").exists())

    def test_system_qt_accepts_minimum(self):
        result = self.run_precheck(PRECHECK_QT_VERSION="6.8.0")
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn("System Qt 6.8.0 is available", result.stdout)

    def test_system_qt_rejects_old_version_before_git_check(self):
        result = self.run_precheck(PRECHECK_QT_VERSION="6.7.9", PRECHECK_DIRTY="1")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("system Qt must be >= 6.8.0", result.stdout)
        self.assertNotIn("review and commit", result.stdout)
        self.assertFalse((self.root / "verify.log").exists())

    def test_system_qt_rejects_qt_seven(self):
        result = self.run_precheck(PRECHECK_QT_VERSION="7.0.0")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("found 7.0.0", result.stdout)
        self.assertFalse((self.root / "verify.log").exists())


if __name__ == "__main__":
    unittest.main()
