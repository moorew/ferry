set -e
UUID=tailcopy@moorew.github.io
EXT=$HOME/.local/share/gnome-shell/extensions/$UUID
CLIENT=/client/tailcopy
gsettings set org.gnome.shell disable-user-extensions false
gsettings set org.gnome.shell enabled-extensions "['$UUID']"
gsettings --schemadir "$EXT/schemas" set org.gnome.shell.extensions.tailcopy allowed-clients "['$CLIENT']"
gnome-shell --version
gnome-shell --headless --wayland --virtual-monitor 1280x800 --no-x11 --unsafe-mode > /tmp/shell.log 2>&1 &
gdbus wait --session --timeout 30 org.gnome.Shell
for i in $(seq 60); do
  gdbus introspect --session -d org.gnome.Shell -o /org/gnome/Shell/Extensions/TailCopy 2>/dev/null | grep -qF "interface org.gnome.Shell.Extensions.TailCopy" && break
  sleep 0.5
done
export GDK_BACKEND=wayland WAYLAND_DISPLAY=wayland-0 WEBKIT_DISABLE_DMABUF_RENDERER=1 WEBKIT_DISABLE_COMPOSITING_MODE=1
python3 /test/e2e.py || echo "E2E FAILED"
echo "---- client log ----"
cat /tmp/client.log | grep -v "^$" | tail -60
echo "---- shell log ----"
grep -iE "tailcopy|JS ERROR" /tmp/shell.log | head -20
