"""Omarchy plugin menu integration checks."""

import importlib.util
import json
import tempfile
import tomllib
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("menu_entry", ROOT / "scripts/menu_entry.py")
MENU = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MENU)


class MenuEntryTests(unittest.TestCase):
    def test_manifest_matches_engine_version_and_entry_point(self):
        manifest = json.loads((ROOT / "manifest.json").read_text())
        cargo = tomllib.loads((ROOT / "Cargo.toml").read_text())
        self.assertEqual(manifest["version"], cargo["package"]["version"])
        self.assertEqual(manifest["id"], "io.github.sonique6784.omacdiag")
        self.assertEqual(manifest["kinds"], ["panel"])
        self.assertTrue((ROOT / manifest["entryPoints"]["panel"]).is_file())

    def test_round_trip_preserves_existing_comments_and_entries(self):
        original = '{\n  // keep this comment\n  "personal.notes": {"label":"Notes","action":"echo hi"},\n}\n'
        installed = MENU.install(original)
        self.assertIn(MENU.ROW_ID, MENU.parse_jsonc(installed))
        self.assertEqual(MENU.remove(installed), original)

    def test_empty_menu_round_trip(self):
        original = '{\n  // user notes\n}\n'
        self.assertEqual(MENU.remove(MENU.install(original)), original)

    def test_conflict_and_modified_block_are_refused(self):
        with self.assertRaises(ValueError):
            MENU.install('{"trigger.hardware.omacdiag":{"label":"Mine"}}')
        installed = MENU.install("{}\n")
        with self.assertRaises(ValueError):
            MENU.remove(installed.replace('"label":"OmacDiag"', '"label":"Changed"'))

    def test_cli_writes_backup_and_refuses_symlink(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "menu.jsonc"
            path.write_text("{}\n")
            MENU.write_atomically(path, MENU.install(path.read_text()))
            self.assertEqual(len(list(path.parent.glob("menu.jsonc.bak.*"))), 1)
            self.assertIn(MENU.ROW_ID, MENU.parse_jsonc(path.read_text()))


if __name__ == "__main__":
    unittest.main()
