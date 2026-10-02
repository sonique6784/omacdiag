#!/bin/sh
set -eu
# Run from an extracted release archive. DESTDIR supports package staging without root.
package_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
destination="${DESTDIR:-}"
install -d "$destination/opt/omacdiag" "$destination/usr/bin"
cp -R "$package_root/." "$destination/opt/omacdiag/"
ln -sf /opt/omacdiag/bin/omacdiag "$destination/usr/bin/omacdiag"
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
