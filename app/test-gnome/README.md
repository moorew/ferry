# End-to-end test on GNOME

Installs the built `.deb` into a stock Ubuntu with GNOME Shell running headless, then
drives TailCopy like a user: copy in an app, open the picker with the shortcut, paste
with Enter, Shift+Enter and search, the paste-plain shortcut, `tailcopy status`,
`pause` and `resume`, a password manager's copy, and a lock-style extension restart.

```sh
(cd app && npx tauri build)          # on Ubuntu 24.04 with the Tauri build deps
docker build -t tailcopy-app-gnome46 app/test-gnome
docker build -t tailcopy-app-gnome50 --build-arg BASE=ubuntu:26.04 app/test-gnome
for img in tailcopy-app-gnome46 tailcopy-app-gnome50; do
  docker run --rm --user root \
    -v "$PWD/target/release/bundle/deb:/deb:ro" \
    -v "$PWD/app/test-gnome:/test:ro" -v /tmp:/out \
    "$img" sh /test/root.sh
done
```

The shell runs with `--unsafe-mode` so the test can press keys through
`org.gnome.Shell.Eval`; that's test-only. Screenshots of the picker land in `/out`.
