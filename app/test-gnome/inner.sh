set -e
gsettings set org.gnome.shell disable-user-extensions false
gsettings set org.gnome.shell enabled-extensions "['tailcopy@moorew.github.io']"
mkdir -p ~/.config/tailcopy ~/.local/share/tailcopy
printf '[hotkeys]\npicker = "Ctrl+Alt+V"\npaste_plain = "Ctrl+Alt+Shift+V"\n' > ~/.config/tailcopy/config.toml
touch ~/.local/share/tailcopy/setup-complete
gnome-shell --headless --wayland --virtual-monitor 1280x800 --no-x11 --unsafe-mode > /tmp/shell.log 2>&1 &
gdbus wait --session --timeout 30 org.gnome.Shell
for i in $(seq 60); do
  gdbus introspect --session -d org.gnome.Shell -o /org/gnome/Shell/Extensions/TailCopy 2>/dev/null | grep -qF "interface org.gnome.Shell.Extensions.TailCopy" && break
  sleep 0.5
done
export XDG_CURRENT_DESKTOP=GNOME XDG_SESSION_TYPE=wayland GDK_BACKEND=wayland WAYLAND_DISPLAY=wayland-0 WEBKIT_DISABLE_DMABUF_RENDERER=1 WEBKIT_DISABLE_COMPOSITING_MODE=1
python3 /test/e2e.py || echo "E2E FAILED"
cp /tmp/*.png /out/ 2>/dev/null || true
echo "---- app log (last lines) ----"
grep -vE "libEGL|MESA|^$" /tmp/app.log | tail -25
grep -iE "JS ERROR" /tmp/shell.log | head -5
