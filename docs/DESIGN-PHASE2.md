# Design Phase 2: brand assets, app icons, tray

3 November 2026. Branch `design`. Handoff section 3.

## What shipped

**App icons** (`app/src-tauri/icons/`)

| OS | Source | Notes |
| --- | --- | --- |
| macOS | `icon.icns` built from `macos-1024.png` with `tauri icon` | Plate on a 1024 canvas, as designed |
| Windows | `icon.ico` is `design/assets/app-icon/tailcopy.ico` as-is | 16, 32, 64, 128, 256; no plate. Also the installer icon |
| Linux | `32x32`, `64x64`, `128x128`, `128x128@2x`, `icon.png` from `linux-*.png` | The `.deb` installs hicolor 16, 32, 64, 128, 256, 512 and `scalable/apps/tailcopy.svg` (checked in a Docker-built `.deb`); `Icon=tailcopy` in the desktop file |

Tauri names its own @2x icon `hicolor/256x256@2/`, which isn't a standard folder, so
the `.deb` also installs a plain `256x256` copy.

**Tray** (`app/src-tauri/src/tray.rs`, PNGs in `icons/tray/`)

| | Asset | Behaviour |
| --- | --- | --- |
| macOS | `tray-<state>-black-48.png` as a template image | The menu bar tints it; macOS draws it 18 pt high |
| Windows | `tray-<state>-16-{white,ink}-16`, `-{white,ink}-20`, `-24`, `-16-{white,ink}-32` | White on a dark taskbar, ink on a light one, read from `SystemUsesLightTheme`. A background thread waits on `RegNotifyChangeKeyValue` and swaps the icon live when the taskbar theme changes. Size picked from the primary monitor's scale (100, 125, 150, 200%) |
| Linux | `tray-<state>-16-white-32.png` | GNOME's top bar is always dark. See "Differs from the handoff" |

States now: **idle**, **paused** (the pause icon) and **offline** (Tailscale isn't
running). Paused wins over offline. The tooltip says which. A check every 30 seconds
notices Tailscale starting or stopping and refreshes the tray and both windows; M2's
tailnet adapter will replace it.

**Logo in the app**
- `components/Lockup.svelte`: the mark plus the outlined wordmark, laid out as
  `lockup.svg`. The wordmark outline is read from `wordmark-mono.svg` at build time,
  so there's one copy of it. Used in the sidebar and in Settings > About.
- `components/Mark.svelte`: the mark alone, used on the first-run screens until Phase 6
  brings the character. Below 24 px it drops the sheet lines and the second dot, as
  the handoff says.
- Both colour themselves from the `--tc-mark-*` and `--tc-ink` tokens, so light and
  dark need no second file. `Logo.svelte` (the M1 black-and-white mark) is gone.

Also: Bricolage Grotesque now ships as the static ExtraBold (91 KB instead of 408 KB).

## Differs from the handoff

- **Syncing frames and the new dot aren't wired yet.** Both need sync (M2): there are
  no transfers to animate and no items from other devices. Shipping them now would be
  code nothing calls. Their PNGs stay in `design/assets/tray` and land with M2,
  including "new clears when the picker opens" and the reduced-motion rule for the tray.
- **Linux tray is a white PNG, not a named symbolic icon.** Tauri's tray hands the
  AppIndicator pixels, not an icon name, so GNOME can't recolour it. On Ubuntu's
  always-dark top bar the white glyph is what a symbolic icon would show anyway.
- **macOS "new" state** (drop template mode for the violet dot): M2, with the rest of
  "new".

## Checks

| Check | Result |
| --- | --- |
| `npm run check`, `lint`, `check:tokens`, `build` | Pass |
| `cargo fmt`, `clippy -D warnings` on Linux (Docker), Windows and macOS targets (stubbed C toolchain) | Pass |
| `cargo test --workspace` | Pass |
| Debug `.deb` built in Docker: icons, licences, desktop entry | As listed above |
| Lockup and mark, light and dark, in Chromium with mocks | `design/qa/phase2/` |

## What I couldn't verify (needs your machines)

- **Windows:** the taskbar and Start menu icon, the installer icon, the tray at your
  display scale, and the tray switching white ↔ ink when you change Settings >
  Personalisation > Colours > "Choose your default mode" (Windows mode), with TailCopy
  running.
- **Windows and Ubuntu:** pause from the tray (icon becomes the pause glyph), then
  quit Tailscale: within 30 seconds the icon shows offline once you resume, and the
  tooltip says "Tailscale isn't running". Start Tailscale again and it returns to idle.
- **Ubuntu:** the app grid icon and the top-bar tray icon (needs the AppIndicator
  extension Ubuntu ships).
- **macOS:** Dock icon and the template tray icon in light and dark menu bars. No Mac
  yet.
