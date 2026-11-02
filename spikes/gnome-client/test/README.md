# Headless end-to-end test

Runs this Tauri client against the extension spike inside a headless GNOME Shell:
capture of text and HTML, the picker shortcut, Enter (GTK multi-format write, hide,
paste into the previous window), Shift+Enter, the paste-plain shortcut, and
re-registration after the extension is disabled and enabled.

Build the client in an Ubuntu 24.04 environment with the Tauri dependencies, then:

```sh
docker build -t tailcopy-gnome46 spikes/gnome-extension/test
docker build -t tailcopy-gnome46-e2e spikes/gnome-client/test
docker run --rm --user root \
  -v "$PWD/spikes/gnome-extension:/ext:ro" \
  -v "$PWD/spikes/gnome-client/test:/test:ro" \
  -v "$PWD/spikes/gnome-client/target/debug:/client:ro" \
  tailcopy-gnome46-e2e sh /test/e2e-root.sh
```

For GNOME 50, build `tailcopy-gnome50` from the extension test with
`--build-arg BASE=ubuntu:26.04`, then this image with `--build-arg BASE=tailcopy-gnome50`.
