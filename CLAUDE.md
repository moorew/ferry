# TailCopy

Keyboard-first clipboard manager for Windows, macOS and Ubuntu GNOME (Wayland) that
syncs history between the owner's devices over their Tailscale tailnet.
`SPEC.md` is the source of truth. Read it before writing code.

## Ground rules

1. **SPEC.md wins.** If something in it is wrong, outdated or impossible, stop that
   thread, add the finding and a proposed change to the top of `docs/DECISIONS.md`,
   tell the owner, and carry on with work it doesn't affect. Never work around the
   spec silently.
2. **Hard product rules:** no telemetry, analytics, accounts, cloud services, ads,
   upgrade prompts or nags (telling the user a new release exists is not one, D-028).
   Never log clip contents, previews or labels.
3. **One milestone at a time** (SPEC 17). At each gate write `docs/M<n>-REPORT.md`,
   then stop and give the owner a short summary, exact manual test steps and anything
   needed from them. Don't start the next milestone until the owner says go.
4. **The GNOME extension** stays small, idiomatic GJS a human would write (SPEC 9.4.1,
   9.4.5): no comments addressed to an AI, no speculative code paths, no unused
   helpers. Check APIs against gjs.guide and the GNOME Shell source for every
   supported version (46 to 51) rather than guessing.
5. **Check every crate's current API** (docs.rs or the downloaded source in
   `~/.cargo/registry/src`) before using it. Prefer the crates in SPEC 3; log any
   substitution in `docs/DECISIONS.md`.
6. **Never block** the UI thread or GNOME Shell's main loop.
7. **Names** from SPEC 0.2 live only in `crates/tailcopy-core/src/names.rs`, mirrored in
   `extension/metadata.json` and the GSettings schema (a test keeps them in step).
8. **Before every commit:** `cargo fmt`, `cargo clippy --all-targets -- -D warnings`,
   `cargo test --workspace`, front-end typecheck and lint, eslint on the extension.
   Small, focused commits with clear messages.

The owner is Will: hands-on with Rust, Linux, Docker and Tailscale. Ask when a
decision is genuinely his; otherwise pick the sensible default, say what you picked,
and keep moving.

## Layout

See SPEC 4 and 21.7. `crates/` (core, sync, tailnet, platform, relay, testkit, mobile), `app/`
(Tauri 2 shell in `src-tauri`, Svelte 5 UI in `src`), `extension/` (GNOME Shell
extension, GPL-2.0-or-later), `mobile/` (Expo app for iOS and
Android, SPEC 21), `spikes/` (M0 throwaway proofs, standalone projects, not in the
workspace or CI), `docs/`.

## Commands

```sh
# Front end (needed before building the Tauri crate: it embeds app/dist)
cd app && npm ci && npm run check && npm run check:tokens && npm run lint && npm run build

# Rust workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Run the desktop app in development
cd app && npx tauri dev

# Extension lint and schema check
cd extension && npm ci && npm run lint
glib-compile-schemas --strict --dry-run extension/schemas
```

Linux builds of the Tauri crate need `libwebkit2gtk-4.1-dev libayatana-appindicator3-dev
librsvg2-dev libxdo-dev`. Without root, build in Docker (see the CI workflow for the
package list).

### Phone apps (`mobile/`)

```sh
cd mobile && npm ci && npm run check:design && npm run typecheck && npm run lint && npm test
npm run design        # regenerate theme, icon shapes and native icons from design/
npm run rust:android  # Rust core + Kotlin bindings (NDK + cargo-ndk); rust:ios on a Mac
# Android build without a local SDK (image: mobile/docker/android.Dockerfile)
docker run --rm -v "$PWD":/src -w /src/mobile -e CARGO_TARGET_DIR=/src/target/docker-android tailcopy-android \
  bash -c 'npm run rust:android && npx expo prebuild -p android --clean --no-install && cd android && ./gradlew assembleRelease'
```

Always add packages with `npx expo install` (SDK-matched versions). `ios/` and
`android/` are generated; change native config in `app.config.ts`,
`plugins/with-tailcopy.js`, `modules/` or `targets/`. Never use EAS or other Expo
cloud services, and keep `EXPO_NO_TELEMETRY=1`. Swift can't be compiled on this
machine: the Mobile workflow's iOS job is the check.

### Releases

Pushing a `v*` tag runs `.github/workflows/release.yml`: it builds the Windows NSIS
installer, the universal macOS DMG and the Ubuntu `.deb`, and publishes a GitHub
Release with notes from `docs/releases/<tag>.md` and a `SHA256SUMS`. The tag must
match the version in `app/src-tauri/tauri.conf.json` (D-022).

### Spikes

Each `spikes/<name>` is its own Cargo project (`cargo build` inside it); the app
spikes build as `tailcopy` and the tailnet one as `tailcopy-tailnet`. Test steps are
in `docs/M0-REPORT.md`. Building an app spike locally needs `binaries/tailcopy-tailnet-<target-triple>` to
exist (Tauri checks external binaries); an empty placeholder is enough for clippy.
The GNOME spikes have headless Docker tests under `test/`.

Cross-checking platform spikes from Linux: `cargo clippy --target
x86_64-pc-windows-msvc` needs an `llvm-rc` on PATH and `--target aarch64-apple-darwin`
needs a C compiler for `objc2-exception-helper`; stubs that only create the output
file are enough for `check`/`clippy`, never for linking.

## GNOME notes

- Logs: `journalctl --user -f -o cat /usr/bin/gnome-shell`
- Nested shell: GNOME 46 `dbus-run-session -- gnome-shell --nested --wayland`;
  GNOME 49+ `dbus-run-session -- gnome-shell --devkit --wayland`
- A development build must be listed in the extension's `allowed-clients` key:
  `gsettings --schemadir <ext>/schemas set org.gnome.shell.extensions.tailcopy allowed-clients "['/abs/path/to/tailcopy']"`

## Design
- Source of truth: design/HANDOFF.md. Tokens: design/tokens/tokens.json (and tokens.css for web views).
- Never hardcode a colour, radius, font size or duration. Add a token first if one is missing, and say so.
- Icons come from design/assets/icons (24 grid, 1.75 stroke, currentColor). Do not mix in another icon set.
- Device colour is never the only signal: the device name is always shown with it.
- Respect reduced motion (OS setting): fades only, max 120 ms.
