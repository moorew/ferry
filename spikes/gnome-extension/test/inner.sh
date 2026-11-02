set -e
UUID=ferry@moorew.github.io
EXT=$HOME/.local/share/gnome-shell/extensions/$UUID
gsettings set org.gnome.shell disable-user-extensions false
gsettings set org.gnome.shell enabled-extensions "['$UUID']"
gsettings --schemadir "$EXT/schemas" set org.gnome.shell.extensions.ferry allowed-clients "['$(readlink -f /usr/bin/python3)']"
gnome-shell --version
gnome-shell --headless --wayland --virtual-monitor 1280x800 --no-x11 --unsafe-mode > /tmp/shell.log 2>&1 &
SHELL_PID=$!
gdbus wait --session --timeout 30 org.gnome.Shell
for i in $(seq 60); do
  gdbus introspect --session -d org.gnome.Shell -o /org/gnome/Shell/Extensions/Ferry 2>/dev/null | grep -qF "interface org.gnome.Shell.Extensions.Ferry" && break
  sleep 0.5
done
echo "extension object up after ~$((i / 2)) s"
GDK_BACKEND=wayland WAYLAND_DISPLAY=wayland-0 python3 /test/client.py || echo "CLIENT FAILED"
echo "---- shell log (Ferry and errors) ----"
grep -iE "ferry|JS ERROR|error|CRITICAL|warning.*extension|TypeError|ReferenceError" /tmp/shell.log | grep -v RTKit | head -40
kill $SHELL_PID 2>/dev/null || true
