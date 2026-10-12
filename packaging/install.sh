#!/bin/sh
set -eu
# Run from an extracted release archive. DESTDIR supports package staging without root.
package_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
destination="${DESTDIR:-}"
for required in "$package_root/bin/omacdiag" "$package_root/bin/omacdiag-helper" \
    "$package_root/README.md" "$package_root/packaging/install.sh"; do
    if [ ! -f "$required" ]; then
        echo "Incomplete OmacDiag archive: missing $required" >&2
        exit 1
    fi
done
if [ -d "$package_root/ui" ]; then
    for required in "$package_root/scripts/launch-ui.sh" "$package_root/packaging/omacdiag.desktop"; do
        if [ ! -f "$required" ]; then
            echo "Incomplete OmacDiag desktop archive: missing $required" >&2
            exit 1
        fi
    done
    if [ ! -x "$package_root/runtime/bin/quickshell" ] && [ ! -f "$package_root/system-qt.json" ]; then
        echo 'Incomplete OmacDiag desktop archive: missing a private runtime or system-qt.json' >&2
        exit 1
    fi
    if [ -z "$destination" ] && [ -f "$package_root/system-qt.json" ] && [ ! -x /usr/bin/quickshell ]; then
        echo 'System Qt desktop archive requires /usr/bin/quickshell' >&2
        exit 1
    fi
fi
if [ -e "$destination/opt/omacdiag" ] || [ -L "$destination/opt/omacdiag" ]; then
    echo 'OmacDiag is already installed at /opt/omacdiag; uninstall it before replacing it.' >&2
    exit 1
fi
if [ -e "$destination/usr/bin/omacdiag" ] || [ -L "$destination/usr/bin/omacdiag" ]; then
    echo '/usr/bin/omacdiag already exists; refusing to replace it.' >&2
    exit 1
fi
if [ -d "$package_root/ui" ] &&
   { [ -e "$destination/usr/share/applications/omacdiag.desktop" ] || [ -L "$destination/usr/share/applications/omacdiag.desktop" ]; }; then
    echo 'The OmacDiag desktop entry already exists; refusing to replace it.' >&2
    exit 1
fi
if [ "${INSTALL_SMART_HELPER:-0}" = 1 ]; then
    for target in "$destination/usr/libexec/omacdiag-helper" "$destination/usr/share/polkit-1/actions/org.omacdiag.smart.policy"; do
        if [ -e "$target" ] || [ -L "$target" ]; then
            echo "Refusing to replace $target" >&2
            exit 1
        fi
    done
fi
install -d "$destination/opt/omacdiag" "$destination/usr/bin"
cp -R "$package_root/." "$destination/opt/omacdiag/"
ln -s /opt/omacdiag/bin/omacdiag "$destination/usr/bin/omacdiag"
if [ -d "$package_root/ui" ]; then
    install -d "$destination/usr/share/applications"
    install -m 0644 "$package_root/packaging/omacdiag.desktop" "$destination/usr/share/applications/omacdiag.desktop"
    chmod 0755 "$destination/opt/omacdiag/scripts/launch-ui.sh"
fi
# Optional helper is installed only when explicitly requested by the administrator.
if [ "${INSTALL_SMART_HELPER:-0}" = 1 ]; then
    install -d "$destination/usr/libexec" "$destination/usr/share/polkit-1/actions"
    install -m 0755 "$package_root/bin/omacdiag-helper" "$destination/usr/libexec/omacdiag-helper"
    install -m 0644 "$package_root/packaging/org.omacdiag.smart.policy" "$destination/usr/share/polkit-1/actions/org.omacdiag.smart.policy"
fi
