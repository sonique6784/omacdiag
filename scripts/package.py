#!/usr/bin/env python3
"""Stage a CLI or desktop tar archive from Linux binaries and a supported runtime."""
import argparse
import json
import os
import platform
import shutil
import tarfile
import tomllib
from pathlib import Path

from runtime_versions import qt_version_at_least

ROOT = Path(__file__).resolve().parents[1]

def main():
    parser = argparse.ArgumentParser()
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--runtime", type=Path)
    group.add_argument("--system-qt", action="store_true")
    group.add_argument("--cli-only", action="store_true")
    parser.add_argument("--binaries", type=Path, default=ROOT / "target/release")
    parser.add_argument("--output", type=Path, default=ROOT / "dist")
    args = parser.parse_args()
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        parser.error("Release packages require x86_64 Linux binaries")
    variant = "cli" if args.cli_only else "desktop"
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    name = f"omacdiag-{version}-linux-x86_64-{variant}"
    stage = args.output / name
    archive = args.output / (name + ".tar.gz")
    if stage.exists() or archive.exists():
        parser.error("Staging directory or archive already exists; choose a fresh output directory")
    for binary in ["omacdiag", "omacdiag-helper"]:
        source = args.binaries / binary
        if not source.is_file() or not os.access(source, os.X_OK):
            parser.error(f"Missing executable: {source}")
    if args.runtime:
        manifest = args.runtime / "runtime-versions.json"
        if not manifest.is_file():
            parser.error(f"Missing runtime manifest: {manifest}")
        pinned = json.loads((ROOT / "packaging/runtime-versions.json").read_text())
        supplied = json.loads(manifest.read_text())
        if not qt_version_at_least(supplied.get("qt", ""), pinned["qt_minimum"]):
            parser.error(f"Runtime Qt must be >= {pinned['qt_minimum']}")
        if any(supplied.get(k) != pinned[k] for k in ["quickshell", "architecture"]):
            parser.error("Runtime does not match required Quickshell version and architecture")
        quickshell = args.runtime / "bin/quickshell"
        if not quickshell.is_file() or not os.access(quickshell, os.X_OK):
            parser.error("Runtime is missing executable bin/quickshell")
        for directory in ["lib", "plugins", "qml"]:
            if not (args.runtime / directory).is_dir():
                parser.error(f"Runtime is missing {directory}/")
    stage.mkdir(parents=True, exist_ok=False)
    (stage / "bin").mkdir()
    for binary in ["omacdiag", "omacdiag-helper"]:
        shutil.copy2(args.binaries / binary, stage / "bin" / binary)
    for directory in ["schemas", "packaging", "docs", "third_party"]:
        shutil.copytree(ROOT / directory, stage / directory)
    shutil.copy2(ROOT / "README.md", stage / "README.md")
    shutil.copy2(ROOT / "LICENSE", stage / "LICENSE")
    if args.runtime:
        shutil.copytree(args.runtime, stage / "runtime", symlinks=True)
    if args.system_qt:
        requirements = json.loads((ROOT / "packaging/runtime-versions.json").read_text())
        (stage / "system-qt.json").write_text(json.dumps({
            "provider": "system",
            "qt_minimum": requirements["qt_minimum"],
            "quickshell": requirements["quickshell"],
            "architecture": requirements["architecture"],
        }, indent=2) + "\n")
    if args.runtime or args.system_qt:
        shutil.copytree(ROOT / "ui", stage / "ui")
        (stage / "scripts").mkdir()
        shutil.copy2(ROOT / "scripts/launch-ui.sh", stage / "scripts/launch-ui.sh")
    with tarfile.open(archive, "w:gz") as tar:
        tar.add(stage, arcname=name)
    print(archive)

if __name__ == "__main__":
    main()
