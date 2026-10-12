#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
cd "$repo_dir"

qt_root=''
quickshell_source='/tmp/omacdiag-quickshell'
runtime_mode=system
requested_private=0
explicit_system=0
quickshell_tag='v0.3.1'
quickshell_url='https://github.com/quickshell-mirror/quickshell.git'

usage() {
  cat <<'EOF'
Usage: ./precheck.sh [--system-qt | --qt-root PATH [--quickshell-source PATH]]

Default: use installed Arch Qt and Quickshell. Prepare missing Arch packages,
require Qt 6.8+ with the expected system paths, check the Git release
candidate, then start verify-plugin.sh.
Run from an interactive Omarchy desktop terminal as your normal user.

--qt-root selects an optional bundled runtime. That mode also prepares a clean
Quickshell v0.3.1 source checkout; the Qt SDK is not installed automatically.
EOF
}

fail() {
  echo "precheck: $*" >&2
  exit 1
}

need() {
  command -v "$1" >/dev/null 2>&1 || fail "missing command: $1"
}

while (( $# > 0 )); do
  case "$1" in
    --qt-root|--quickshell-source)
      (( $# >= 2 )) || fail "$1 needs a path"
      [[ -n $2 ]] || fail "$1 needs a nonempty path"
      (( ! explicit_system )) || fail "$1 conflicts with --system-qt"
      if [[ $1 == --qt-root ]]; then qt_root=$2; else quickshell_source=$2; fi
      runtime_mode=private
      requested_private=1
      shift 2
      ;;
    --system-qt)
      (( ! requested_private )) || fail '--system-qt conflicts with private-runtime options'
      runtime_mode=system
      explicit_system=1
      shift
      ;;
    -h|--help) usage; exit 0 ;;
    *) usage >&2; fail "unknown argument: $1" ;;
  esac
done

[[ $(uname -s) == Linux && $(uname -m) == x86_64 ]] || fail 'requires x86-64 Linux'
(( EUID != 0 )) || fail 'run as your normal user; sudo is used only for pacman'
[[ -t 0 ]] || fail 'requires an interactive terminal for package and UI prompts'
[[ -n ${WAYLAND_DISPLAY:-}${DISPLAY:-} ]] || fail 'run from an Omarchy desktop session'
for command_name in omarchy pacman git python3; do need "$command_name"; done

missing_packages=()
for package_name in cmake ninja cli11 quickshell; do
  if ! pacman -Qq "$package_name" >/dev/null 2>&1; then
    missing_packages+=("$package_name")
  fi
done
if (( ${#missing_packages[@]} > 0 )); then
  need sudo
  echo "Installing Arch packages: ${missing_packages[*]}"
  sudo pacman -Syu --needed "${missing_packages[@]}"
fi
for command_name in cmake ninja quickshell; do need "$command_name"; done
pacman -Qq cli11 >/dev/null 2>&1 || fail 'CLI11 is still missing after package installation'

if [[ $runtime_mode == private ]]; then
  if [[ ! -e $quickshell_source ]]; then
    echo "Cloning Quickshell $quickshell_tag into $quickshell_source"
    git clone --branch "$quickshell_tag" --depth 1 "$quickshell_url" "$quickshell_source"
  fi
  [[ -d "$quickshell_source/.git" ]] || fail "not a Quickshell Git checkout: $quickshell_source"
  source_tag=$(git -C "$quickshell_source" describe --tags --exact-match 2>/dev/null) \
    || fail "Quickshell checkout must be at $quickshell_tag: $quickshell_source"
  [[ $source_tag == "$quickshell_tag" ]] || fail "Quickshell checkout must be at $quickshell_tag; found $source_tag"
  [[ -z $(git -C "$quickshell_source" status --porcelain) ]] \
    || fail "Quickshell checkout has uncommitted changes: $quickshell_source"

  if [[ -z $qt_root ]]; then
    qt_root=$(find "$HOME/Qt" -mindepth 2 -maxdepth 2 -type d -path '*/6.*/gcc_64' 2>/dev/null \
      | sort -V | tail -n 1) || true
  fi
  if [[ -z $qt_root || ! -x "$qt_root/bin/qmake" ]]; then
    cat >&2 <<'EOF'
precheck: A complete Qt 6.8+ Linux gcc_64 SDK is required.
Install one under ~/Qt using the official Qt Online Installer (include Qt
Declarative, Quick Controls, Wayland, ShaderTools, and private headers):
https://doc.qt.io/qt-6.11/get-and-install-qt-cli.html
Then rerun ./precheck.sh --qt-root "$HOME/Qt/<version>/gcc_64".
The system qmake6 and Quickshell binary cannot supply this private runtime.
EOF
    exit 1
  fi
  qt_version=$("$qt_root/bin/qmake" -query QT_VERSION) || fail "cannot query Qt SDK: $qt_root"
  qt_minimum=$(python3 -c 'import json; print(json.load(open("packaging/runtime-versions.json"))["qt_minimum"])')
  python3 -c 'import sys; from scripts.runtime_versions import qt_version_at_least; sys.exit(0 if qt_version_at_least(sys.argv[1], sys.argv[2]) else 1)' \
    "$qt_version" "$qt_minimum" || fail "Qt SDK must be >= $qt_minimum and below 7.0.0; found $qt_version"
  for directory in lib plugins qml; do
    [[ -d "$qt_root/$directory" ]] || fail "Qt SDK is missing $directory/: $qt_root"
  done
else
  need qmake6
  qt_version=$(qmake6 -query QT_VERSION) || fail 'cannot query system Qt with qmake6'
  qt_minimum=$(python3 -c 'import json; print(json.load(open("packaging/runtime-versions.json"))["qt_minimum"])')
  python3 -c 'import sys; from scripts.runtime_versions import qt_version_at_least; sys.exit(0 if qt_version_at_least(sys.argv[1], sys.argv[2]) else 1)' \
    "$qt_version" "$qt_minimum" || fail "system Qt must be >= $qt_minimum and below 7.0.0; found $qt_version"
  [[ $(qmake6 -query QT_INSTALL_LIBS) == /usr/lib ]] || fail 'system Qt libraries must be in /usr/lib'
  [[ $(qmake6 -query QT_INSTALL_PLUGINS) == /usr/lib/qt6/plugins ]] || fail 'system Qt plugins must be in /usr/lib/qt6/plugins'
  [[ $(qmake6 -query QT_INSTALL_QML) == /usr/lib/qt6/qml ]] || fail 'system Qt QML modules must be in /usr/lib/qt6/qml'
  echo "System Qt $qt_version is available in /usr/lib and /usr/lib/qt6."
fi

if [[ -n $(git status --porcelain) ]]; then
  git status --short >&2
  fail 'review and commit the OmacDiag release candidate before full verification'
fi

if [[ $runtime_mode == system ]]; then
  echo 'Starting verify-plugin.sh with system Qt and Quickshell'
  exec "$repo_dir/verify-plugin.sh" --system-qt
fi
echo "Starting verify-plugin.sh with Qt $qt_version and Quickshell $quickshell_tag"
exec "$repo_dir/verify-plugin.sh" --qt-root "$qt_root" --quickshell-source "$quickshell_source"
