#!/bin/sh
set -eu
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
app_root=$(CDPATH= cd -- "$script_dir/.." && pwd)
if [ "$(uname -s)" != Linux ]; then
    echo 'The desktop app requires Linux. The Rust core can be built on other hosts.' >&2
    exit 1
fi
if [ -z "${OMACDIAG_ENGINE:-}" ]; then
    if [ -x "$app_root/bin/omacdiag" ]; then OMACDIAG_ENGINE="$app_root/bin/omacdiag"
    elif [ -x "$app_root/target/release/omacdiag" ]; then OMACDIAG_ENGINE="$app_root/target/release/omacdiag"
    elif [ -x "$app_root/target/debug/omacdiag" ]; then OMACDIAG_ENGINE="$app_root/target/debug/omacdiag"
    else OMACDIAG_ENGINE=$(command -v omacdiag); fi
fi
export OMACDIAG_ENGINE
export QT_QUICK_BACKEND=software
export QT_QUICK_CONTROLS_STYLE=Basic
runtime="$app_root/runtime"
if [ -x "$runtime/bin/quickshell" ]; then
    export LD_LIBRARY_PATH="$runtime/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
    export QT_PLUGIN_PATH="$runtime/plugins"
    export QML_IMPORT_PATH="$runtime/qml"
    quickshell_bin="$runtime/bin/quickshell"
else
    quickshell_bin=${OMACDIAG_QUICKSHELL:-quickshell}
fi
# Reports default to a writable per-user directory, independent of the installation path.
state_dir="${XDG_STATE_HOME:-$HOME/.local/state}/omacdiag"
mkdir -p "$state_dir"
cd "$state_dir"
exec "$quickshell_bin" --path "$app_root/ui"
