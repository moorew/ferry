# Design handoff, Phase 0: recon

3 November 2026. Branch `m1`. No code changed. Covers `design/HANDOFF.md` ("Lilac
utility") against the repo and SPEC.md. Spec conflicts are logged as D-021.

## 1. UI stack and where the tokens go

- **Stack:** Tauri 2 with a Svelte 5 + TypeScript + Vite front end in `app/src`, as
  SPEC 3 says. One bundle and one `index.html`; `main.ts` mounts `Picker` or
  `Manager` depending on the window label. No component library.
- **Styles today:** one global `app/src/app.css` holds the tokens as unprefixed CSS
  variables (`--bg`, `--fg`, `--fg-muted`, `--hairline`, ...) and a light/dark switch
  on `prefers-color-scheme`, plus shared `.btn`, `.field` and `kbd` classes.
  Components use scoped `<style>` blocks that read those variables. Commissioner is
  bundled as a woff2 in `app/src/assets/fonts`. Icons are hand-drawn 16-grid paths in
  `components/Icon.svelte`; the mark is inline SVG in `components/Logo.svelte`.
- **Plan:** import `design/tokens/tokens.css` as the single token source and delete
  the old variables. Two small adjustments, both kept in the design folder so it stays
  the source of truth:
  - Its `@font-face` URLs point at `../fonts/variable/*.ttf`, relative to
    `design/tokens/`. Vite resolves that fine if `app.css` imports
    `../../design/tokens/tokens.css`, so the fonts get bundled with no copying. The CSP
    already allows `font-src 'self'`.
  - The type scale (34/26/17/15/14/12) and the device-avatar sizes are in
    `tokens.json` but not in `tokens.css`. I'll add them to `tokens.css` as
    `--tc-text-*` and say so (the handoff rule is "add a token first").
  - Theme: `tokens.css` already supports `data-theme` on `<html>`, so the new
    System / Light / Dark setting is one attribute plus a `general.theme` config key.
- **Lint for hex colours:** a small eslint/stylelint-free check (a `npm run
  check:tokens` grep over `app/src` for `#[0-9a-f]{3,8}` and `rgb(`) so the acceptance
  test in Phase 1 is a command, not a promise.
- **Screenshots:** the front end already has `lib/mock.ts`, and Playwright's Chromium
  is installed here, so I can screenshot every screen headless in both themes from
  `vite dev` with mocks. The real webviews (WebView2, WKWebView, WebKitGTK) can still
  differ slightly; the GNOME Docker images can screenshot the real Linux build.

## 2. Tray, shortcut and paste today

| | Windows | macOS | Ubuntu GNOME (Wayland) |
| --- | --- | --- | --- |
| Tray | Tauri tray, coloured "tile" PNG, hollow when paused | Template PNG | Tauri tray through libayatana-appindicator (needs Ubuntu's AppIndicator extension) |
| Picker shortcut | `tauri-plugin-global-shortcut`, default Ctrl+Alt+V | same, Cmd+Shift+V | **The TailCopy GNOME extension** binds it with `Main.wm.addKeybinding` (SPEC 9.4.1); chosen in first run |
| Paste for me | enigo Ctrl+V after focus returns; elevated windows fall back to copy + notification | enigo Cmd+V (Accessibility) | Extension sends the keystroke through a Clutter virtual keyboard, with per-app overrides (Ctrl+Shift+V for terminals) |
| Copy-only fallback | `auto_paste = false`, or automatic for admin windows | `auto_paste = false` | `auto_paste = false` |

What that means for the handoff:

- **Section 7's GNOME caveat doesn't apply.** Because TailCopy ships its own Shell
  extension, global shortcuts and paste-for-me both work on GNOME Wayland today
  (M1's end-to-end test passes on GNOME 46 and 50). No portal or `tailcopy picker`
  custom-shortcut workaround is needed; the copy-only path already exists for
  when the user turns paste-for-me off.
- **Tray states:** `idle`, `paused` and `new` can land now. `sync1-3` and `offline`
  need M2 (there are no transfers yet; "offline" can mean "Tailscale not running",
  which we already detect). Per OS:
  - macOS: black template PNGs; drop template mode only for `new`, as the handoff says.
  - Windows: read `SystemUsesLightTheme` and listen for `WM_SETTINGCHANGE` /
    a registry notify on a background thread, then swap white/ink. The `windows`
    crate is already a dependency.
  - Linux: Tauri's tray can't pass a themed *symbolic icon name* to the
    AppIndicator, only pixels. GNOME's top bar is always dark, so the white PNG is
    right in practice. I'll use that and note it rather than add a second tray stack.
  - Reduced motion for the tray: no OS exposes this to a tray icon directly; on
    Windows and GNOME I can read the system setting (`SPI_GETCLIENTAREAANIMATION`,
    `org.gnome.desktop.interface enable-animations`), on macOS
    `accessibilityDisplayShouldReduceMotion`.
- **Toasts after paste** don't fit as drawn. The picker hides before the paste, so
  "Pasted into Termius" would have to be a separate overlay window or an OS
  notification, and SPEC 1.1 says no notifications unless something is wrong. See
  D-021, item 3.

## 3. Data gap list (handoff section 9)

Nothing below needs a wire-format change. Everything new is local-only columns or
derived at read time.

### Device

| Field | Today | Smallest change |
| --- | --- | --- |
| stable id | `devices.device_id` (UUIDv7), `ts_node_id` | none |
| Tailscale name | `ts_hostname` | none |
| display name | `display_name` | none |
| OS | not stored | add `os TEXT` (from Tailscale status `OS`) |
| device type | not stored | add `device_type TEXT`. Tailscale has no reliable laptop/desktop signal, so default to `server` for relays and tagged nodes, `desktop` otherwise, editable in Settings |
| `color_index` | not stored | add `color_index INTEGER`, assigned `max+1 mod 6` on first insert, never recomputed |
| last seen | `last_seen_ms` | none |
| has TailCopy | implicit (in `devices` at all) | none; peers without TailCopy come from Tailscale status, not the table |
| send / receive | not stored | add `send_enabled`, `receive_enabled INTEGER DEFAULT 1`. These are new sync behaviour (D-021, item 5) |

The `devices` table is filled by M2 discovery; today it holds only this device.

### Item

| Field | Today | Smallest change |
| --- | --- | --- |
| id, origin device, preview, size, pinned, label, formats | stored | none |
| kind text/rich/image | stored | none |
| kind code/link/colour | `looksLikeCode` only, computed in `dto.rs` | derive at index time in `tailcopy-core` (one `display_kind()` with the handoff's heuristics) so search chips and sidebar counts can filter on it. Not stored, not synced |
| kind file | n/a | out of scope (SPEC 2.3); drop from the design |
| image dimensions | in `preview` ("image WxH"), already parsed into `width`/`height` | none |
| origin app | `source_app`; only synced when `sync_source_app` is on (default off) | none; remote rows show device only unless the user opts in |
| `copied_at` | `created_hlc` wall time | none |
| `received_at` | not stored | add local-only `received_ms INTEGER` to `clips`, set when the `ClipAdded` is first applied (equals capture time for local clips) |
| use count, last used | `last_used_hlc` only | add local `use_count INTEGER DEFAULT 0`, bumped when materializing `ClipUsed` (counts every device's uses, so it converges) |
| duplicate count | `copies` from group_key grouping | none |
| sync state local/synced/too large | `local_only` | none in M1; "synced" means "ops sent", which is M2 |
| skipped reason ("Password skipped" rows) | secrets are never stored | **don't add**: see D-021, item 6 |

So: one migration (`0002_design.sql`) adding `clips.received_ms`, `clips.use_count`,
and the four `devices` columns. `received_ms` and `use_count` sit in materialized
state, so the "clips equals recomputation from ops" property test needs
`use_count` included and `received_ms` excluded (it's arrival time, inherently local).

## 4. Plan for phases 1 to 7

Where this sits: everything sync-related (arrival tint, trail card with real latency,
the tailnet sidebar, per-device toggles, sync tray animation) can be **built and
shown with mock data now, and wired to real data in M2**. I'd do Phases 1 to 3 and the
non-sync parts of 4 to 7 before M2, then finish the sync parts inside M2.

**Phase 1, tokens and fonts**
- `app/src/app.css`: import `design/tokens/tokens.css`, remove old variables, rename
  every `var(--…)` in components to `--tc-…`; delete `assets/fonts/Commissioner*`.
- `design/tokens/tokens.css` (+ `tokens.json`): add the type-scale tokens.
- `app/src/main.ts`: set `data-theme` from settings; listen for changes.
- `crates/tailcopy-core/src/config.rs`, `app/src-tauri/src/dto.rs`, `lib/types.ts`,
  `manager/Settings.svelte`: `general.theme = "system" | "light" | "dark"`.
- `app/src-tauri/tauri.conf.json` bundle resources: ship the three `OFL-*.txt`.
- `app/package.json`: `check:tokens` script.

**Phase 2, brand, icons, tray**
- `app/src-tauri/icons/*`: regenerate with `npx tauri icon design/assets/app-icon/png/macos-1024.png`,
  then replace `icon.ico` with `design/assets/app-icon/tailcopy.ico` (Windows uses the
  plate-less mark) and the Linux PNGs with `linux-*.png`.
- `packaging/` (deb): hicolor PNGs and `scalable/apps/tailcopy.svg`.
- `app/src-tauri/src/tray.rs`: state enum (idle, paused, offline, new, sync1-3),
  per-OS asset choice, Windows theme watcher (new `tray_theme.rs`), `new` cleared on
  `picker-shown`. Copy the needed PNGs into `app/src-tauri/icons/tray/`.
- `components/Logo.svelte` → `Lockup.svelte` / `Mark.svelte` from `design/assets/logo`.

**Phase 3, primitives**
- `components/`: `Button`, `IconButton`, `SearchField`, `Chip`, `Toggle`, `Keycap`,
  `Badge`, `DeviceAvatar`, `HistoryRow`, `TrailCard`, `StatusCard`, `Toast`.
- `components/Icon.svelte`: rewritten to load `design/assets/icons/*.svg` with Vite's
  `?raw` import (24 grid, currentColor), same `name` prop. Old 16-grid icons deleted.
- `app/src/gallery/Gallery.svelte`, mounted when the window label is `manager` and
  the URL has `#gallery` in a dev build only (`import.meta.env.DEV`).

**Phase 4, main window**
- `manager/Manager.svelte`: new sidebar (Library with live counts, Your tailnet,
  status card). **Counts bug:** the Manager fetches status once and only updates on
  `status-changed`, which isn't sent on `clips-changed`, so counts stay at whatever they
  were at open (0 on a fresh install). Fix: recompute counts on `clips-changed`, and
  return per-category counts from one `get_counts` command.
- `manager/History.svelte`: header, chips, time-grouped list, detail pane,
  content-sized preview (min 120 px), trail card, metadata, image view.
- `crates/tailcopy-core`: migration 0002, `display_kind`, `received_ms`, `use_count`,
  category counts; `search.rs` gains `is:code`, `is:link` tokens for the chips.
- Ignore whitespace-only and single-character copies (handoff 11.3): `capture.rs`
  plus a `[general] ignore_tiny = true` setting.
- Device name field shows the real Tailscale hostname as its placeholder (11.5):
  `tailscale.rs` already reads status; pass `Self.HostName` through `get_status`.

**Phase 5, picker**
- `app/src-tauri/src/picker.rs`: size (D-021, item 2), remember size, cursor screen
  (already: it centres on the cursor's monitor).
- `picker/Picker.svelte`, `Preview.svelte`: chips, 46 px rows, number hints,
  pinned first on empty search, Esc clears then closes, clickable footer, arrival tint.
- `crates/tailcopy-core/src/search.rs`: "pinned first when query is empty" ordering.

**Phase 6, pinned, settings, states**
- `manager/Pinned.svelte` (new), `Settings.svelte` (new section list, Tailnet
  section with mock peers until M2), `FirstRun.svelte`, `states/*` (empty, offline,
  first run), pause menu with durations (needs a "paused until" in `state.rs` and
  the tray menu).
- `app/src/lib/strings.ts`: the copy deck, one place.

**Phase 7, motion, a11y, QA**
- Motion tokens applied per section 6; `design/qa/` screenshots via the Playwright
  script from Phase 1.

## 5. Things in the handoff that don't fit the code

1. GNOME caveat (section 7): not needed, see section 2 above.
2. "Every item says This device" (11.2) is correct in M1: sync is off, so every item
   *is* from this device. It changes in M2.
3. "Kept on mac-mini, over the 10 MB sync limit" (section 5, dark board): local-only
   clips never produce ops, so other devices never learn they exist. That line can
   only appear on the device that holds the clip ("kept on this device, over the
   limit"). Also, the current cap (`max_image_bytes`, 20 MB) is a capture cap, not a
   sync cap; there's no separate sync limit in the spec.
4. Manager responsive rule "below 600 px the sidebar stacks on top": the manager's
   minimum width is 720 (SPEC 10.1), so that breakpoint never triggers. Harmless.
5. Picker radius 22 and shadow: Windows and macOS only; WebKitGTK can't do
   transparent windows (M1 known gap), so on Linux the picker stays square.
6. Device type (laptop/desktop/server/board) has no reliable source in Tailscale
   status. Default from role and OS, editable per device.
