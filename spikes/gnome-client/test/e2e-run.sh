#!/bin/sh
# Inside the container: install the spike extension, start a headless shell, run the client.
set -e
UUID=ferry@moorew.github.io
EXT=$HOME/.local/share/gnome-shell/extensions/$UUID
mkdir -p "$EXT/schemas"
cp /ext/extension.js /ext/metadata.json "$EXT/"
cp /ext/schemas/*.xml "$EXT/schemas/"
glib-compile-schemas "$EXT/schemas"
export XDG_RUNTIME_DIR=/tmp/xdg && mkdir -p -m 700 $XDG_RUNTIME_DIR
exec dbus-run-session -- sh /test/e2e-inner.sh
