#!/usr/bin/env python3
"""Build pinned Quickshell against a supported Qt SDK; stage a private runtime."""
import argparse
import json
import platform
import shutil
import subprocess
from pathlib import Path

from runtime_versions import qt_version_at_least

ROOT = Path(__file__).resolve().parents[1]

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--qt-root", type=Path, required=True)
    parser.add_argument("--quickshell-source", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "dist/runtime")
    args = parser.parse_args()
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        parser.error("Build the release runtime on x86_64 Linux")
    versions = json.loads((ROOT / "packaging/runtime-versions.json").read_text())
    qt = args.qt_root.resolve()
    source = args.quickshell_source.resolve()
    version = subprocess.check_output([str(qt / "bin/qmake"), "-query", "QT_VERSION"], text=True).strip()
    if not qt_version_at_least(version, versions["qt_minimum"]):
        parser.error(f"Qt must be >= {versions['qt_minimum']}; found {version}")
    tag = subprocess.check_output(["git", "-C", str(source), "describe", "--tags", "--exact-match"], text=True).strip()
    if tag != "v" + versions["quickshell"]:
        parser.error("Quickshell source must be checked out at the pinned release tag")
    if subprocess.check_output(["git", "-C", str(source), "status", "--porcelain"], text=True).strip():
        parser.error("Quickshell checkout must be clean")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    build = output / "build"
    flags = ["-DCRASH_HANDLER=OFF", "-DUSE_JEMALLOC=OFF", "-DSCREENCOPY=OFF", "-DHYPRLAND=OFF", "-DI3=OFF",
             "-DSERVICE_PIPEWIRE=OFF", "-DSERVICE_PAM=OFF", "-DSERVICE_POLKIT=OFF", "-DSERVICE_MPRIS=OFF",
             "-DSERVICE_STATUS_NOTIFIER=OFF", "-DDISTRIBUTOR=omacdiag private runtime"]
    subprocess.run(["cmake", "-S", str(source), "-B", str(build), "-G", "Ninja", "-DCMAKE_BUILD_TYPE=Release",
                    f"-DCMAKE_PREFIX_PATH={qt}", f"-DCMAKE_INSTALL_PREFIX={output}", *flags], check=True)
    subprocess.run(["cmake", "--build", str(build), "--parallel", "2"], check=True)
    subprocess.run(["cmake", "--install", str(build)], check=True)
    for name in ["lib", "plugins", "qml"]:
        shutil.copytree(qt / name, output / name, symlinks=True, dirs_exist_ok=True)
    (output / "bin/qt.conf").write_text("[Paths]\nPrefix=..\nLibraries=lib\nPlugins=plugins\nQmlImports=qml\n")
    licenses = output / "licenses"
    licenses.mkdir()
    for candidate in source.glob("*LICENSE*"):
        shutil.copy2(candidate, licenses / ("quickshell-" + candidate.name))
    for name in ["LICENSES", "licenses"]:
        if (qt / name).is_dir():
            shutil.copytree(qt / name, licenses / "qt", dirs_exist_ok=True)
    (output / "runtime-versions.json").write_text(json.dumps({**versions, "qt": version}, indent=2) + "\n")
    # Delete only this script's newly created build artifacts, not an existing checkout.
    shutil.rmtree(build)
    print(output)

if __name__ == "__main__":
    main()
