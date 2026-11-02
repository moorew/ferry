#!/bin/sh
rm -rf /run/systemd/seats
mkdir -p /run/dbus && dbus-daemon --system --fork
exec runuser -u dev -- env HOME=/home/dev sh /test/run.sh
