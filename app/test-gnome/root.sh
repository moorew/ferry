#!/bin/sh
# Inside the GNOME test image as root: install the .deb, then run the test as the user.
set -e
rm -rf /run/systemd/seats
mkdir -p /run/dbus && dbus-daemon --system --fork
dpkg -i /deb/*.deb >/tmp/dpkg.log 2>&1 || { cat /tmp/dpkg.log; exit 1; }
echo "installed $(dpkg-query -W -f '${Package} ${Version}' 'tail*')"
exec runuser -u dev -- env HOME=/home/dev XDG_RUNTIME_DIR=/tmp/xdg sh -c 'mkdir -p -m 700 /tmp/xdg && exec dbus-run-session -- sh /test/inner.sh'
