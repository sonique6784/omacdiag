#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"
plugin_id='io.github.sonique6784.omacdiag'
qt_root=''
quickshell_source='/tmp/omacdiag-quickshell'
runtime_mode=system
requested_private=0
explicit_system=0
mode=release

usage() {
  cat <<'EOF'
Usage: ./verify-plugin.sh [--system-qt | --qt-root PATH [--quickshell-source PATH]]
       ./verify-plugin.sh --preflight
       ./verify-plugin.sh --menu-smoke

Default: package the Rust CLI, check the installed system Qt/Quickshell, and
verify the embedded Omarchy plugin with a guided report.
System mode requires Qt 6.8+ and Quickshell 0.3.1 at /usr/bin/quickshell,
with Qt libraries in /usr/lib and plugins/QML in /usr/lib/qt6.
--qt-root selects the optional bundled-runtime build from a complete Qt SDK.
--preflight runs software and manifest checks without the Qt SDK or GUI.
--menu-smoke checks the installed CLI and plugin, then asks you to launch it
from the Omarchy menu. It does not install or remove anything.
EOF
}

fail() {
  echo "verify-plugin: $*" >&2
  exit 1
}

need() {
  command -v "$1" >/dev/null 2>&1 || fail "missing command: $1"
}

while (( $# > 0 )); do
  case "$1" in
    --qt-root)
      (( $# >= 2 )) || fail '--qt-root needs a path'
      (( ! explicit_system )) || fail '--qt-root conflicts with --system-qt'
      qt_root=$2
      runtime_mode=private
      requested_private=1
      shift 2
      ;;
    --quickshell-source)
      (( $# >= 2 )) || fail '--quickshell-source needs a path'
      (( ! explicit_system )) || fail '--quickshell-source conflicts with --system-qt'
      quickshell_source=$2
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
    --preflight|--menu-smoke)
      [[ $mode == release ]] || fail 'choose one verification mode'
      mode=${1#--}
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *) usage >&2; fail "unknown argument: $1" ;;
  esac
done

need omarchy
need jq
need python3

if [[ $mode == menu-smoke ]]; then
  [[ -t 0 ]] || fail 'menu smoke test requires an interactive terminal'
  plugin_dir="$HOME/.config/omarchy/plugins/$plugin_id"
  menu_file="$HOME/.config/omarchy/extensions/omarchy-menu.jsonc"
  [[ -x /usr/bin/omacdiag ]] || fail 'install the tested CLI archive first'
  [[ -f "$plugin_dir/manifest.json" ]] || fail 'install the Omarchy plugin first'
  omarchy plugin validate "$plugin_dir"
  omarchy plugin list --json | jq -e --arg id "$plugin_id" \
    '.[] | select(.id == $id and .enabled == true)' >/dev/null \
    || fail 'plugin is not enabled'
  [[ -f "$menu_file" ]] || fail 'install the OmacDiag menu row first'
  need rg
  rg -Fq 'trigger.hardware.omacdiag' "$menu_file" || fail 'OmacDiag menu row is missing'
  echo 'Open Omarchy menu → Trigger → Hardware → OmacDiag; close its in-shell diagnostic window after checking it.'
  read -r -p 'Did the menu action open OmacDiag inside the running Omarchy shell? Type yes: ' answer
  [[ $answer == yes ]] || fail 'menu smoke test was not confirmed'
  echo 'Menu smoke test confirmed.'
  exit 0
fi

for command_name in cargo git qmllint rg uv; do need "$command_name"; done
[[ $(uname -s) == Linux && $(uname -m) == x86_64 ]] || fail 'requires x86-64 Linux'
version=$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["package"]["version"])')
[[ $(jq -r '.version' manifest.json) == "$version" ]] || fail 'manifest and Cargo versions differ'

if [[ $mode == release ]]; then
  [[ -z $(git status --porcelain) ]] || fail 'commit the release candidate before testing its archive'
  for command_name in cmake ninja cmp ldd sha256sum tar stat omarchy-shell; do need "$command_name"; done
  [[ -t 0 ]] || fail 'the guided plugin test requires an interactive terminal'
  [[ -n ${WAYLAND_DISPLAY:-}${DISPLAY:-} ]] || fail 'run from an Omarchy desktop session'
  qt_minimum=$(jq -r '.qt_minimum' packaging/runtime-versions.json)
  if [[ $runtime_mode == system ]]; then
    need qmake6
    [[ -x /usr/bin/quickshell ]] || fail 'system Quickshell is missing at /usr/bin/quickshell'
    qt_version=$(qmake6 -query QT_VERSION)
    [[ $(qmake6 -query QT_INSTALL_LIBS) == /usr/lib ]] || fail 'system Qt libraries are not in /usr/lib'
    [[ $(qmake6 -query QT_INSTALL_PLUGINS) == /usr/lib/qt6/plugins ]] || fail 'system Qt plugins are not in /usr/lib/qt6/plugins'
    [[ $(qmake6 -query QT_INSTALL_QML) == /usr/lib/qt6/qml ]] || fail 'system Qt QML modules are not in /usr/lib/qt6/qml'
    quickshell_version=$(/usr/bin/quickshell --version)
    [[ $quickshell_version == *'Quickshell 0.3.1'* ]] || fail "system Quickshell must be 0.3.1; found $quickshell_version"
  else
    [[ -x "$qt_root/bin/qmake" ]] || fail 'Qt SDK not found; pass --qt-root PATH'
    qt_version=$("$qt_root/bin/qmake" -query QT_VERSION)
    for directory in lib plugins qml; do
      [[ -d "$qt_root/$directory" ]] || fail "Qt SDK is missing $directory/: $qt_root"
    done
    [[ -d "$quickshell_source/.git" ]] || fail "Quickshell checkout not found at $quickshell_source"
    [[ $(git -C "$quickshell_source" describe --tags --exact-match) == v0.3.1 ]] \
      || fail 'Quickshell checkout must be at tag v0.3.1'
    [[ -z $(git -C "$quickshell_source" status --porcelain) ]] \
      || fail 'Quickshell checkout must be clean'
    quickshell_version='v0.3.1 (bundled)'
  fi
  python3 -c 'import sys; from scripts.runtime_versions import qt_version_at_least; sys.exit(0 if qt_version_at_least(sys.argv[1], sys.argv[2]) else 1)' \
    "$qt_version" "$qt_minimum" || fail "Qt must be >= $qt_minimum and below 7.0.0; found $qt_version"
fi

echo 'Checking plugin, software, and contracts...'
omarchy plugin validate .
qmllint -I "${OMARCHY_PATH:-/usr/share/omarchy}/shell" plugin/Launcher.qml ui/DiagnosticView.qml ui/Session.qml ui/Patterns.qml ui/shell.qml
python3 -m unittest discover -s tests -p 'test_*.py'
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --examples
UV_CACHE_DIR=/tmp/omacdiag-uv-cache uv run scripts/validate_contracts.py

if [[ $mode == preflight ]]; then
  echo 'Preflight passed. Run without --preflight to build and test the CLI archive and plugin.'
  exit 0
fi

work_dir=$(mktemp -d "${TMPDIR:-/tmp}/omacdiag-verify-plugin.XXXXXXXX")
echo "Keeping verification files in: $work_dir"
if [[ $runtime_mode == private ]]; then
  python3 scripts/build_runtime.py --qt-root "$qt_root" \
    --quickshell-source "$quickshell_source" --output "$work_dir/runtime"
fi
cargo build --locked --release
if [[ $runtime_mode == system ]]; then
  python3 scripts/package.py --cli-only --output "$work_dir/packages"
  name="omacdiag-$version-linux-x86_64-cli"
  archive="$name.tar.gz"
  (cd "$work_dir/packages" && sha256sum "$archive" > "$archive.sha256" && sha256sum -c "$archive.sha256")
  mkdir "$work_dir/extracted" "$work_dir/staged"
  tar -xzf "$work_dir/packages/$archive" -C "$work_dir/extracted"
  package_dir="$work_dir/extracted/$name"
  [[ -x "$package_dir/bin/omacdiag" ]] || fail 'CLI archive lacks the OmacDiag engine'
  DESTDIR="$work_dir/staged" sh "$package_dir/packaging/install.sh"
  [[ -L "$work_dir/staged/usr/bin/omacdiag" ]] || fail 'staged CLI install lacks /usr/bin/omacdiag'
  echo "CLI archive: $work_dir/packages/$archive"
  echo "Install it with: sudo sh '$package_dir/packaging/install.sh'"
  echo "Install this committed plugin with: omarchy plugin add 'file://$PWD' --enable"
  echo "Add its menu row with: python3 '$HOME/.config/omarchy/plugins/$plugin_id/scripts/menu_entry.py' install"
  echo 'Return here afterward.'
  read -r -p 'When the CLI and plugin are installed, type ready: ' answer
  [[ $answer == ready ]] || fail 'installation was not confirmed'
  [[ -x /usr/bin/omacdiag ]] || fail 'installed CLI engine is missing'
  cmp /usr/bin/omacdiag "$package_dir/bin/omacdiag" || fail 'installed engine differs from the tested archive'
  plugin_dir="$HOME/.config/omarchy/plugins/$plugin_id"
  [[ -f "$plugin_dir/manifest.json" ]] || fail 'install the plugin first'
  omarchy plugin validate "$plugin_dir"
  for source_file in manifest.json plugin/Launcher.qml ui/DiagnosticView.qml ui/Session.qml ui/Patterns.qml; do
    cmp "$source_file" "$plugin_dir/$source_file" || fail "installed plugin differs from tested source: $source_file"
  done
  omarchy plugin list --json | jq -e --arg id "$plugin_id" '.[] | select(.id == $id and .enabled == true)' >/dev/null || fail 'plugin is not enabled'
  menu_file="$HOME/.config/omarchy/extensions/omarchy-menu.jsonc"
  [[ -f "$menu_file" ]] && rg -Fq 'trigger.hardware.omacdiag' "$menu_file" || fail 'OmacDiag menu row is missing'
  omarchy-shell shell summon "$plugin_id" '{}'
  read -r -p 'Did summon open the diagnostic UI in the existing Omarchy shell? Type yes: ' answer
  [[ $answer == yes ]] || fail 'in-shell summon was not confirmed'
  omarchy-shell shell hide "$plugin_id"
  read -r -p 'Did hide close the UI while Omarchy shell stayed running? Type yes: ' answer
  [[ $answer == yes ]] || fail 'in-shell close was not confirmed'
  state_dir="${XDG_STATE_HOME:-$HOME/.local/state}/omacdiag"
  echo 'Open Omarchy menu → Trigger → Hardware → OmacDiag, complete at least one Quick + Guided check, inspect the JSON report, and close the UI.'
  session_start=$(date +%s)
  read -r -p 'Enter the report path shown in the UI: ' report_path
  [[ -n $report_path ]] || fail 'a report path is required'
  [[ $report_path == /* ]] || report_path="$state_dir/$report_path"
  [[ -f "$report_path" ]] || fail "report not found: $report_path"
  (( $(stat -c '%Y' "$report_path") >= session_start )) || fail 'report predates this UI session'
  jq -e --arg version "$version" '.application_version == $version and .state == "completed" and .finished_at != null and (.results | length > 0)' "$report_path" >/dev/null || fail 'report must be a completed session with at least one result'
  [[ $(stat -c '%a' "$report_path") == 600 ]] || fail 'report permissions must be 600'
  read -r -p 'Did the menu UI, checklist, selected checks, final report, and close action work? Type yes: ' answer
  [[ $answer == yes ]] || fail 'visual plugin smoke test was not confirmed'
  echo 'The next step disables and re-enables the plugin, removes its owned menu row, and removes the plugin installation.'
  read -r -p 'To run the installation cleanup check, type cleanup: ' answer
  [[ $answer == cleanup ]] || fail 'cleanup check was not confirmed'
  omarchy plugin disable "$plugin_id"
  if omarchy plugin list --json | jq -e --arg id "$plugin_id" '.[] | select(.id == $id and .enabled == true)' >/dev/null; then
    fail 'plugin remained enabled after disable'
  fi
  omarchy plugin enable "$plugin_id"
  omarchy plugin list --json | jq -e --arg id "$plugin_id" '.[] | select(.id == $id and .enabled == true)' >/dev/null || fail 'plugin did not re-enable'
  python3 "$plugin_dir/scripts/menu_entry.py" remove
  if rg -Fq 'trigger.hardware.omacdiag' "$menu_file"; then fail 'menu row remained after removal'; fi
  omarchy plugin remove "$plugin_id" --yes
  [[ ! -e "$plugin_dir" ]] || fail 'plugin directory remained after removal'
  archive_sha=$(sha256sum "$work_dir/packages/$archive" | cut -d ' ' -f1)
  cat > "$work_dir/verification-summary.txt" <<EOF
Status: plugin and CLI archive smoke passed
Qt runtime: system $qt_version
Quickshell: $quickshell_version
Source commit: $(git rev-parse HEAD)
Omarchy version: $(omarchy version)
Kernel: $(uname -r)
Archive: $work_dir/packages/$archive
Artifact SHA-256: $archive_sha
Report: $report_path
Report state: completed
Report permissions: 600
In-shell summon, hide, and menu UI: confirmed by operator
Plugin disable, re-enable, menu removal, and plugin removal: passed
EOF
  cat "$work_dir/verification-summary.txt"
  echo 'Review third-party notices before marking release acceptance passed.'
  exit 0
fi
if [[ $runtime_mode == private ]]; then
  python3 scripts/package.py --runtime "$work_dir/runtime" --output "$work_dir/packages"
else
  python3 scripts/package.py --system-qt --output "$work_dir/packages"
fi

name="omacdiag-$version-linux-x86_64-desktop"
archive="$name.tar.gz"
(cd "$work_dir/packages" && sha256sum "$archive" > "$archive.sha256" && sha256sum -c "$archive.sha256")
mkdir "$work_dir/extracted"
tar -xzf "$work_dir/packages/$archive" -C "$work_dir/extracted"
package_dir="$work_dir/extracted/$name"
[[ -x "$package_dir/bin/omacdiag" ]] || fail 'archive lacks the OmacDiag engine'

if [[ $runtime_mode == private ]]; then
  [[ -x "$package_dir/runtime/bin/quickshell" ]] || fail 'archive lacks the private Quickshell runtime'
  quickshell_bin="$package_dir/runtime/bin/quickshell"
  LD_LIBRARY_PATH="$package_dir/runtime/lib" ldd "$quickshell_bin" > "$work_dir/quickshell-ldd.txt"
else
  [[ -f "$package_dir/system-qt.json" ]] || fail 'archive lacks system-qt.json'
  [[ ! -d "$package_dir/runtime" ]] || fail 'system Qt archive unexpectedly includes a private runtime'
  quickshell_bin=/usr/bin/quickshell
  ldd "$quickshell_bin" > "$work_dir/quickshell-ldd.txt"
fi
if rg -q 'not found' "$work_dir/quickshell-ldd.txt"; then
  cat "$work_dir/quickshell-ldd.txt" >&2
  fail 'Quickshell has unresolved runtime libraries'
fi
rg -q 'libQt6.*=>' "$work_dir/quickshell-ldd.txt" || fail 'Quickshell did not link to Qt 6 libraries'
if [[ $runtime_mode == private ]]; then
  awk '/libQt6.*=>/ { if ($3 !~ /\/runtime\/lib\//) bad=1 } END { exit bad }' \
    "$work_dir/quickshell-ldd.txt" || fail 'Quickshell resolved a Qt library outside its private runtime'
else
  awk '/libQt6.*=>/ { if ($3 !~ /^\/usr\/(bin\/\.\.\/)?lib\//) bad=1 } END { exit bad }' \
    "$work_dir/quickshell-ldd.txt" || fail 'Quickshell resolved a Qt library outside /usr/lib'
fi
"$quickshell_bin" --version

echo 'The packaged UI will open. In Quick + Guided mode, review the checklist,'
echo 'acknowledge preflight, run at least one selected check, inspect the JSON'
echo 'report tab, and close the window when the session is complete.'
session_start=$(date +%s)
sh "$package_dir/scripts/launch-ui.sh"

state_dir="${XDG_STATE_HOME:-$HOME/.local/state}/omacdiag"
read -r -p 'Enter the report path shown in the UI (absolute or relative to the OmacDiag state directory): ' report_path
[[ -n $report_path ]] || fail 'a report path is required'
[[ $report_path == /* ]] || report_path="$state_dir/$report_path"
[[ -f "$report_path" ]] || fail "report not found: $report_path"
(( $(stat -c '%Y' "$report_path") >= session_start )) || fail 'report predates this UI session'
jq -e --arg version "$version" \
  '.application_version == $version and .state == "completed" and .finished_at != null and (.results | length > 0)' \
  "$report_path" >/dev/null || fail 'report must be a completed session with at least one result'
[[ $(stat -c '%a' "$report_path") == 600 ]] || fail 'report permissions must be 600'
read -r -p 'Did the UI show the checklist, selected checks, and final report correctly? Type yes: ' answer
[[ $answer == yes ]] || fail 'visual smoke test was not confirmed'

archive_sha=$(sha256sum "$work_dir/packages/$archive" | cut -d ' ' -f1)
cat > "$work_dir/verification-summary.txt" <<EOF
Status: archive smoke passed
Qt runtime: $runtime_mode $qt_version
Quickshell: $quickshell_version
Source commit: $(git rev-parse HEAD)
Omarchy version: $(omarchy version)
Kernel: $(uname -r)
Archive: $work_dir/packages/$archive
Artifact SHA-256: $archive_sha
Report: $report_path
Report state: completed
Report permissions: 600
Visual UI check: confirmed by operator
Plugin menu smoke: run ./verify-plugin.sh --menu-smoke after installation
EOF
cat "$work_dir/verification-summary.txt"
echo 'Complete the hardware, menu, and license checks in docs/release-acceptance.template.md before marking the release passed.'
