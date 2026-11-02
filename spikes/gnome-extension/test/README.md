# Headless test for the extension spike

Runs the spike extension inside a headless GNOME Shell in Docker and drives every
D-Bus method and signal from a Python GTK client: Register and the caller check,
ClipboardChanged, Get/SetContent, SetShortcut, the picker shortcut, PresentPicker,
Paste with and without activating the previous window, the modifier wait, and
re-registration after the extension is disabled and enabled (what a screen lock does).

```sh
docker build -t ferry-gnome46 spikes/gnome-extension/test
docker build -t ferry-gnome50 --build-arg BASE=ubuntu:26.04 spikes/gnome-extension/test
for img in ferry-gnome46 ferry-gnome50; do
  docker run --rm --user root \
    -v "$PWD/spikes/gnome-extension:/ext:ro" \
    -v "$PWD/spikes/gnome-extension/test:/test:ro" \
    "$img" sh /test/root.sh
done
```

The shell runs with `--unsafe-mode` so the client can inject key presses through
`org.gnome.Shell.Eval`; that is test-only and the extension does not rely on it.
A headless shell sometimes never hands keyboard focus to the first window; when
that happens every focus-dependent check fails together. Run it again.
