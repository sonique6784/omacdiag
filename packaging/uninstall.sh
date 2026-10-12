#!/bin/sh
set -eu

# Remove only files created by packaging/install.sh.
destination=${DESTDIR:-}
app="$destination/opt/omacdiag"
link="$destination/usr/bin/omacdiag"
desktop="$destination/usr/share/applications/omacdiag.desktop"
helper="$destination/usr/libexec/omacdiag-helper"
policy="$destination/usr/share/polkit-1/actions/org.omacdiag.smart.policy"

if [ -L "$link" ] && [ "$(readlink "$link")" = /opt/omacdiag/bin/omacdiag ]; then
    rm "$link"
fi
if [ -f "$desktop" ] && [ ! -L "$desktop" ] &&
   grep -Fxq 'Exec=/opt/omacdiag/scripts/launch-ui.sh' "$desktop"; then
    rm "$desktop"
fi
if [ -f "$policy" ] && [ ! -L "$policy" ] &&
   grep -Fq 'org.omacdiag.smart' "$policy"; then
    rm "$policy"
    if [ -f "$helper" ] && [ ! -L "$helper" ]; then rm "$helper"; fi
fi
if [ -d "$app" ] && [ ! -L "$app" ] &&
   [ -f "$app/README.md" ] && [ -f "$app/bin/omacdiag" ] &&
   [ -f "$app/packaging/install.sh" ]; then
    rm -rf -- "$app"
fi
