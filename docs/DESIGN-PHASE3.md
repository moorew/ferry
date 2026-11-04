# Design Phase 3: component primitives

3 November 2026. Branch `design`. Handoff section 4, checked against the canvas's
Components board (`project/brand/Components.dc.html`).

## What shipped (`app/src/components/`)

| Component | Notes |
| --- | --- |
| `Button` | primary, secondary, accent, danger; `compact` (32 px); optional 18 px icon. Styles live in `app.css` as `.btn` so screens not yet rebuilt match too |
| `IconButton` | 40 or 32 px, `danger`, `pressed`; `label` is the accessible name and tooltip |
| `SearchField` | 42 px, accent ring plus soft halo on focus, shortcut keycaps while empty |
| `Chip` | 32 px pill, `aria-pressed`; on = selected fill |
| `Toggle` | 44 × 26, accent when on, `role="switch"`. Settings now uses it |
| `Keycap` | a whole shortcut ("Ctrl+Alt+V") as keys; bare `<kbd>` is styled globally |
| `Badge` | arrival, pinned, count, offline (the board's fourth badge) |
| `DeviceAvatar`, `DeviceDot`, `DeviceStack` | sizes 20, 24, 28, 40; glyph by type (laptop, desktop, server, board); colour from `colorIndex`; avatar carries the device name as its label and tooltip |
| `HistoryRow` | rest, hover, selected, just arrived; mono titles, label, pin, ×N count, image thumbnail |
| `TrailCard` | remote with latency (`<0.1s`, `0.4s`, "a few minutes later" over 60 s) and local |
| `StatusCard` | capturing (synced with N, or "On this device only" while sync is off), paused (for a duration or until resumed), and not capturing. The sidebar uses it now |
| `Toast` | "done" (dark pill, accent check, optional Undo) and "notice" (surface, danger icon); `role="status"`, polite, never focused |
| `Icon` | rebuilt on `design/assets/icons`: the 36 SVGs are split into shapes at build time, no copies and no raw HTML |
| `lib/strings.ts` | the copy deck from handoff section 8, with `t(key, vars)` |

**Gallery:** `npm run dev`, then <http://localhost:1420/?view=gallery>. Development
builds only; it's not in the production bundle. Screenshots: `design/qa/phase3/`.

## Tokens added

- `--tc-text-small` 13 px (chips, toast and trail detail; the board uses 13 there).
- `--tc-on-selected-accent` (#C2ABFF light, #5B33C2 dark): the toast's check on its
  inverted fill.
- `--tc-r-avatar-20/24/28/40` (6, 8, 9, 12): avatar radii by size.

## Differs from the handoff or board

- **No "private" row state or "Password skipped" badge**: dropped in D-021.
- **StatusCard has a third state, "Not capturing"**, for degraded mode (GNOME
  extension off and so on). The M1 sidebar had it; the design didn't.
- **Icons the screens needed but the set lacks:** info, play and a right arrow. First
  run now uses an empty ring for "to do", the close icon for a problem and the chevron
  for Continue; the sidebar's play button became the status card's Resume button.
  Phase 6 redraws first run anyway.
- **Secondary buttons use the surface fill** with a line, as on the board, rather than
  the handoff text's "transparent".
- **Toast success copy:** the gallery shows "Copied", not "Pasted into Termius" (no
  success toasts after paste, D-021).

## Also fixed

`vite.config.ts` now lets the dev server read `design/`. Without it, `npm run dev`
and `npx tauri dev` got 403s for the fonts and icons (production builds were fine).

## Checks

`npm run check`, `lint`, `check:tokens` and `build` pass. No Rust changed in this phase.
