# TailCopy design handoff

**Summary.** TailCopy gets a "Lilac utility" look: lilac paper and aubergine ink in light mode, a deep night purple in dark mode, one violet accent, and six purple-family device colours that make the tailnet visible in every row. The logo is a clipboard leaving a trail of dots (Option A, "Trail"); the same dots become the tray's syncing animation and the "trail" card that shows where an item came from. The work is split into eight Claude Code phases below, each with a paste-ready prompt, acceptance checks and a stop point. Tokens, fonts, logo, app icons, tray icons and 38 UI icons are all in this folder as real files.

Design canvas (private to Will): https://claude.ai/artifact/R58nFYxe2chH3ECjeghqKQ

---

## How to use this with Claude Code

1. Copy this folder into the repo root as `design/`.
2. Append the snippet below to the repo's `CLAUDE.md` (create it if missing).
3. Run the phase prompts in section 13 one at a time. Each phase ends with a report and screenshots; review before starting the next.

```md
## Design
- Source of truth: design/HANDOFF.md. Tokens: design/tokens/tokens.json (and tokens.css for web views).
- Never hardcode a colour, radius, font size or duration. Add a token first if one is missing, and say so.
- Icons come from design/assets/icons (24 grid, 1.75 stroke, currentColor). Do not mix in another icon set.
- Device colour is never the only signal: the device name is always shown with it.
- Respect reduced motion (OS setting): fades only, max 120 ms.
```

## What's in the folder

```
design/
  HANDOFF.md                 this file
  tokens/tokens.json         every token, light and dark
  tokens/tokens.css          the same as CSS custom properties, theme via OS or data-theme
  fonts/variable/            Bricolage Grotesque, Figtree, JetBrains Mono (variable TTF)
  fonts/static/              static instances for toolkits without variable font support
  fonts/OFL-*.txt            licences, ship these with the app
  assets/logo/               mark, mark-dark, mark-mono, wordmark, lockup (light and dark)
  assets/app-icon/           macOS, Windows, Linux SVGs, PNGs 16 to 1024, tailcopy.ico
  assets/tray/svg/           7 states at 24 grid, plus 16 px optimised versions
  assets/tray/png/           every state in black (macOS template), ink and white
  assets/icons/              38 UI icons, currentColor (phone and scan added for the phone apps, SPEC 21)
```

---

## 1. Principles

- **Keyboard first, mouse friendly.** Every action in the picker has a key; the footer always shows them.
- **Quiet sync.** Items from other devices arrive without toasts. A violet tint and a "From mac-mini" badge are enough.
- **The tailnet is visible.** Each device has a colour and a glyph. Rows, the trail card and the sidebar all use it.
- **One accent.** Violet means live: arrival, focus, toggles on, the capturing dot. Nothing else is violet.
- **Calm surfaces, characterful type.** Bricolage Grotesque only for titles; Figtree for everything you read; JetBrains Mono for anything you'd paste into a terminal.

## 2. Tokens

Full values in `tokens/tokens.json`. The roles that matter most:

| Role | Light | Dark | Notes |
|---|---|---|---|
| bg | #F3EFF8 | #17121F | Window ground |
| surface | #FCFAFE | #1F1829 | Cards, detail pane, picker |
| sunk | #EAE3F3 | #120E19 | Sidebar, code blocks |
| ink | #2A1838 | #F2EDF9 | Primary text |
| ink-2 | #66547A | #BDB0D1 | Secondary text, safe everywhere |
| ink-3 | #6E5C82 | #A091B6 | Tertiary: timestamps, placeholders |
| accent | #7B52F0 | #A989FF | Live state only |
| accent-ink | #5B33C2 | #C2ABFF | Violet text |
| accent-soft | #E7DEFD | #352659 | Arrival tint, focus halo |
| selected | #2A1838 | #F2EDF9 | Selected row, primary button |
| danger | #B42A55 | #F08AAE | Delete, errors |

Device colours (both themes, in join order): violet #9B7BF2, periwinkle #9DB2F5, orchid #E59BDD, lavender #C4B1F5, mauve #D9A8C4, plum #B07CC6. Glyphs on them use #2A1838 (light) or #17121F (dark).

Every text pairing above was checked at 4.5:1 or better (WCAG AA), including ink on all six device colours. The trap to avoid: white text fails on the light device colours, so avatars and Pinned cards always use the dark glyph and ink colours, never white.

**Type.** Page title 34/800 display, card title 26/800 display, body 14/400 (line height 20), setting label 15/600, meta 12/400, section label 12/600 uppercase with 0.08em tracking, picker search 17/400, code 13 to 14 mono.

**Shape.** Radii: keycap 6, tile 10, control 12, row 14, card 20, window 22, pill 999. Controls 40 tall (32 compact). Sidebar 248. Detail pane min 340. Picker default 960 × 640, rows 46.

**Focus.** 2 px accent ring with a 2 px surface gap on every focusable thing. Never remove it, never replace it with colour alone.

## 3. Brand

### Logo

- **Working pick: Option A "Trail".** A violet clipboard with an aubergine clip and two dots trailing from its lower left. Options B ("Linked clip") and C ("Overlap") are on the canvas if Will prefers either; swapping only touches `assets/logo` and `assets/app-icon`.
- Clearspace equals the clip height on every side. Minimum size 16 px; below 24 px the lines inside the sheet and the second dot drop out (the 16 px files already do this).
- The mark never sits on the accent colour, never gets a gradient, and never appears next to Tailscale's nine-dot logo. TailCopy is a third-party app built on Tailscale; the name and mark should not read as an official Tailscale product.
- Files: `mark.svg`, `mark-dark.svg`, `mark-mono.svg` (currentColor), `wordmark.svg` (outlined, no font needed), `lockup.svg`, `lockup-dark.svg`.

### App icons

| OS | File | Notes |
|---|---|---|
| macOS | `app-icon/macos.svg`, `png/macos-1024.png` | 824 px plate on a 1024 canvas. Build the .icns from the 1024 PNG with the platform's icon tooling. |
| Windows | `app-icon/tailcopy.ico`, `windows.svg` | No plate; the mark fills the tile. The .ico holds 16, 32, 64, 128, 256. |
| Linux | `linux.svg`, `png/linux-*.png` | Plate with a darker lip. Install PNGs to `share/icons/hicolor/<size>x<size>/apps/tailcopy.png` and the SVG to `hicolor/scalable/apps/tailcopy.svg`. |

### Tray and menu bar

Seven states, each drawn on a 24 grid with a 16 px optimised variant: `idle`, `sync1` to `sync3` (looped at 160 ms per frame while a transfer runs, then back to idle), `paused`, `offline`, `new` (violet dot, cleared when the picker opens).

- **macOS:** use the black PNGs as template images so the menu bar tints them for light, dark and highlighted states. The `new` dot is the one exception: drop template mode for that state or the violet is lost. If that feels wrong in practice, use the idle glyph plus a small badge instead and tell Will.
- **Windows:** the notification area wants 16 px at 100% scaling, with 20, 24 and 32 for higher scale factors (all provided). Pick white or ink from the taskbar theme (`HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\SystemUsesLightTheme`) and switch live when it changes.
- **Linux (GNOME):** upstream GNOME has no tray; Ubuntu ships the AppIndicator extension enabled. Use a StatusNotifierItem and a single-colour symbolic icon. When no tray host exists, the app must still be reachable through its shortcut and launcher.

## 4. Components

| Component | Spec |
|---|---|
| Button, primary | 40 tall, radius 12, `selected` fill, `on-selected` text 14/600, 18 px icon at 8 px gap. Hover lightens to #3D2752 (light). Disabled 40% opacity. |
| Button, secondary | Transparent, 1 px inset `line-strong`, ink text. |
| Button, accent | `accent` fill, `on-accent` text. Use once per screen at most (Connect device, Set up relay). |
| Button, danger | Secondary shape with `danger` text. Destructive actions always offer undo. |
| Icon button | 40 × 40 (32 compact), radius 12, transparent, 20 px icon. Needs an accessible label. |
| Search field | 42 tall, radius 12, `surface`, 1 px `line-strong`. Focus: 1.5 px accent plus 4 px `accent-soft` halo. Shows its shortcut as keycaps on the right. |
| Filter chip | 32 tall pill, 1 px `line-strong`. On: `selected` fill, no border. |
| Toggle | 44 × 26 track, 20 px white knob. On: `accent`. Off: `line-strong`. |
| Keycap | Min 22 × 22, radius 6, `surface`, 1 px `line-strong` plus a 2 px bottom inset. 11/600. |
| Badge | 24 tall pill, 12/700. Arrival: `accent` fill, white dot plus "From <device>". Pinned: `accent-soft` with `accent-ink`. Count: `tile` with ink, "×2". |
| Device avatar | Rounded square in the device colour, glyph by device type (laptop, desktop, server, board). Sizes 20, 24, 28, 40 with radius 6, 8, 9, 12. Stacks overlap by 7 px with a 2 px ring in the surface colour. |
| History row | 56 tall: 36 px type tile (radius 10), title 14 (mono for code, IPs, colours), meta line 12 with an 8 px device dot, device name and app. States: rest, hover (`hover`), selected (`selected` fill, `on-selected` text, `selected-tile` tile), arrived (`accent-soft` row, `accent` tile, "Just arrived" badge), private (outlined tile, eye-off icon, "Password skipped"). |
| Trail card | `bg` fill, radius 14. Source avatar, two small dots, a 2 px lavender line, an accent dot, destination avatar. Text: "Copied on **ubuntu-desk**, here **0.4s** later". For local items: "Copied on this device". |
| Status card | Sidebar footer. Capturing: accent dot with a 4 px `accent-soft` halo, "Capturing", "Synced with N devices", pause icon button. Paused: hollow dot, "Paused for 1 hour", "Resumes at 8:41 PM", Resume button. |
| Toast | 44 to 48 tall, radius 14, `ink` fill (light) with light text, `shadow-toast`. Check icon in `accent-ink` (dark variant #C2ABFF). Holds 1.8 s, or 5 s when it carries Undo. |

## 5. Screens

### Main window (History)

- Layout: sidebar (248, `sunk`) plus main area. Main has a header row (title, search, "Open anywhere Ctrl 1" hint), a filter chip row, then the list and the detail pane side by side. Below about 900 px wide, the detail pane stacks under the list; below about 600 px, the sidebar stacks on top.
- Sidebar: lockup, **Library** (All, Pinned, Code, Links, Images, Formatted, each with a live count), **Your tailnet** (one row per device: avatar, name, "This device" or "Live" with an accent dot or "2h ago"), then Settings and the status card pinned to the bottom. Clicking a device filters the list to items from that device.
- List: grouped by Just now, Earlier today, Yesterday, then weekday or date. No zebra striping.
- Detail pane: actions (Paste primary, Copy, Pin, Delete pushed right), content preview sized to content (min 120 px, `sunk`, mono for code), Label field, trail card, metadata list (Type, Size, Copied, App, Formats, Used, and for images Dimensions and Synced to).
- Dark board shows the Images view: rows use 64 × 44 thumbnails, the preview keeps the image's aspect ratio, and an item too large to sync says so in its meta line ("kept on mac-mini, over the 10 MB sync limit").

### Quick picker

- 960 × 640 default, remembers its size, radius 22, `surface`, `shadow-pop`. Opens on the screen that has the cursor.
- Search (17 px) with "All devices" scope on the right, chips (All, Text, Links, Code, Images, Pinned), list of 46 px rows with the number key 1 to 9 on the right, the device dot and the relative time.
- Preview pane (Tab) takes 44% of the width: source line (device, app, time), content or image, trail line.
- Footer: Paste, Plain text, Preview, Pin, Label on the left, Close on the right. The hints are clickable.
- Pinned items show first when the search is empty, with a small pin icon.
- No-results state: "Nothing matches “query”" and "Try fewer words, or switch the filter back to All."

### Pinned shelf

- Grid of cards, `repeat(auto-fit, minmax(280px, 1fr))`, gap 16, radius 22, min height 210. Cards cycle through the device palette fills plus one plain surface card; text on fills is always ink.
- Card: type and use count, a More button, title (the label, else the first line), a preview (mono block on a 40% white wash for code), and a footer with the source device avatar and date.
- Below the grid: a dashed hint row, "Pin anything in your history and it lands here, on every device."

### Settings, Tailnet section

- Settings has its own section list: General, Shortcuts, Privacy, Tailnet, History and storage, About.
- Tailnet: a status banner (`accent-soft`) with "Connected as thinkpad", "Tailscale is running · 3 other devices have TailCopy" and the device name field. Then **Devices running TailCopy** with Send and Receive toggles per device, plus tailnet peers without TailCopy ("How to install"). Then **What syncs**: Images (with size limit), Formatted text, Passwords and secrets ("Always skipped", not a toggle), Relay server (optional, "Set up").

### States

- **First run:** happy mark character, "Found your tailnet", device stack, "Start capturing" and "Choose what syncs".
- **Empty history:** resting character, "Nothing copied yet", shortcut hint.
- **Tailscale not running:** outlined character, "Tailscale isn't running", explains history keeps saving locally, "Open Tailscale" and "Try again".
- **Pause:** options for 15 minutes, 1 hour, or until resumed, each showing when it ends.
- The character is the mark with eyes and a mouth. It only appears in these states, never in everyday UI.

## 6. Interactions and motion

**Arrival** (copy on mac-mini, lands on thinkpad): copy at 0 ms, secrets skipped and the item stored locally first; around 300 ms over the tailnet; on arrival the tray gets the violet dot and nothing else happens; when the picker opens the item sits at the top with the tint and badge, slides in 8 px over 420 ms, and the tint clears once the item is used or 4 s after it has been seen.

**Paste:** Ctrl 1 opens the picker (fade plus scale 0.98 to 1 over 140 ms) with focus in search. Typing filters instantly, matches are bolded and the first match is selected. Enter, a number key or a click closes the picker (100 ms), returns focus to the previous app and pastes. A 1.8 s toast confirms "Pasted into Termius". When paste-for-me is off or not possible, the toast reads "Copied. Press Ctrl V".

**Micro-interactions:**
- Copy: the button label swaps to "Copied" with a check for 1.2 s; the button never changes size.
- Pin: the pin tips and fills violet; the sidebar count bumps with the spring curve.
- Delete: the row collapses over 180 ms, a toast offers Undo for 5 s, then the delete syncs to every device.

**Motion tokens:** instant 0, fast 120, base 200, enter 140, exit 100, arrive 420, spring 240 (counts and badges only), tray 3 × 160. Easing out `cubic-bezier(.2,.8,.2,1)`, in `cubic-bezier(.4,0,1,1)`, spring `cubic-bezier(.34,1.56,.64,1)`. With reduced motion on: fades only, max 120 ms, no translate or scale, and the tray uses the static new dot.

## 7. Keyboard

Ctrl on Windows and Linux, ⌘ on macOS.

| Keys | Action |
|---|---|
| Ctrl 1 | Open the picker over any app (user can rebind) |
| ↑ ↓ | Move selection |
| 1 to 9 | Paste that row immediately |
| Enter | Paste |
| Shift Enter | Paste as plain text |
| Tab | Toggle preview |
| Ctrl P | Pin or unpin |
| Ctrl L | Label |
| Delete | Delete with undo |
| Esc | Clear search; press again to close |
| Ctrl F | Focus search in the main window |

**Platform caveat, GNOME on Wayland:** apps cannot freely register global hotkeys or send synthetic keystrokes there. Plan for (a) a `tailcopy picker` command the user can bind as a GNOME custom shortcut, with the XDG GlobalShortcuts portal used where the running GNOME supports it, and (b) paste-for-me falling back to copy-only plus the "Copied. Press Ctrl V" toast. Phase 0 should confirm what the repo does today.

## 8. Copy deck

| Key | String |
|---|---|
| picker.search.placeholder | Search clipboard history |
| picker.scope.all | All devices |
| picker.empty.title | Nothing matches “{query}” |
| picker.empty.body | Try fewer words, or switch the filter back to All. |
| row.arrived | Just arrived |
| row.from | From {device} |
| row.private.title | Password skipped |
| row.private.meta | {device} · {app} · never saved or synced |
| row.too_large | kept on {device}, over the {limit} sync limit |
| trail.remote | Copied on {device}, here {latency} later |
| trail.local | Copied on this device |
| status.capturing | Capturing |
| status.synced | Synced with {n} devices |
| status.paused | Paused for {duration} |
| status.resumes | Resumes at {time} |
| toast.pasted | Pasted into {app} |
| toast.pasted_plain | Pasted into {app} as plain text |
| toast.copied_manual | Copied. Press Ctrl V |
| toast.deleted | Deleted from all devices |
| toast.undo | Undo |
| pinned.subtitle | Kept forever and synced to every device. They show first in the picker. |
| pinned.hint | Pin anything in your history and it lands here, on every device. |
| tailnet.connected | Connected as {device} |
| tailnet.running | Tailscale is running · {n} other devices have TailCopy |
| tailnet.not_installed | On your tailnet, TailCopy not installed |
| tailnet.last_seen | Last seen {ago} · {os} · catches up when it's back |
| sync.secrets | Never saved or synced. Detected from password managers and marked fields. |
| sync.relay | Optional. A self-hosted relay holds items for devices that are offline. |
| state.first.title | Found your tailnet |
| state.first.body | {n} of your devices already run TailCopy. Anything you copy here shows up on them in under a second. |
| state.empty.title | Nothing copied yet |
| state.empty.body | Copy anything on any device. It lands here, ready to paste with one shortcut. |
| state.offline.title | Tailscale isn't running |
| state.offline.body | History keeps saving on this device. It syncs to the rest of your tailnet as soon as Tailscale is back. |

## 9. Data the UI needs

The new UI reads fields the current build may not store. Phase 0 should map these against the real schema.

- **Device:** stable id (from Tailscale), Tailscale name, optional display name, OS, device type (laptop, desktop, server, board), `color_index` (assigned in join order and persisted, never recomputed), last seen, has TailCopy, send enabled, receive enabled.
- **Item:** id, kind (text, rich, code, link, image, colour, file), display text, formats, size in bytes, image dimensions, `origin_device_id`, origin app, `copied_at` (origin clock), `received_at` (local clock), use count, last used, pinned, label, duplicate count, sync state (local only, synced, too large), skipped reason.
- **Trail latency** is `received_at - copied_at`. Clocks drift between machines, so clamp anything under 0.1 s (or negative) to "<0.1s" and hide the number above 60 s ("here a few minutes later").
- **Kind detection** (suggested, cheap heuristics): URL → link; hex or rgb() → colour; IPv4 or IPv6 → text in mono; one line that starts with a common command or contains `--flags`, or several indented lines → code.

## 10. Accessibility checklist

- All text pairs at 4.5:1 or better (checked); do not introduce new pairs without checking.
- Device identity never relies on colour alone: the name or a tooltip always comes with the dot.
- Every icon-only button has an accessible name; decorative SVGs are hidden from assistive tech.
- Focus ring visible on every control, including rows in the picker and cards on the Pinned shelf.
- Reduced motion respected everywhere, including the tray.
- Hit targets at least 32 px in the desktop UI (40 for primary controls).
- Toasts are announced politely and never steal focus.

## 11. Fixes spotted in the current build

From the screenshots shared on Nov 3:

1. Sidebar counts read 0 (All 0, Pinned 0) while the list has items.
2. Every item says "This device"; items synced from other devices need their origin device and app.
3. Single characters and whitespace (the 1 B "I" item) clutter history. Suggest ignoring whitespace-only and single-character copies by default, with a setting.
4. The detail preview is a large empty box for short items; size it to content with a 120 px minimum.
5. The Device name field's placeholder says "Tailscale name"; show the actual Tailscale name instead.
6. Pause or resume has no default shortcut. Consider offering one, checking it doesn't clash on each OS.

## 12. Open questions for Will

1. Logo: keep Option A "Trail", or switch to B or C?
2. Default image sync limit: 10 MB is a guess.
3. Should long items arriving from other devices ever show a toast, or stay tray-only?
4. Relay server: in scope for this round of polish, or a later milestone?
5. Is Ctrl 1 the shortcut you want to ship as the default?

---

## 13. Claude Code prompts

Run these in order. Each one is self-contained; paste it as the whole message.

### Phase 0: Recon, no code changes

```text
Read design/HANDOFF.md end to end, then explore this repo. Do not change any code in this phase.

Report back with:
1. The UI stack (framework, how styles and themes work today, where colours and fonts are defined) and how the tokens in design/tokens/tokens.json should land in it. If it is web based (Tauri, webview), plan to import design/tokens/tokens.css. If it is a native Rust toolkit (egui, iced, Slint or similar), propose a generated theme module from tokens.json.
2. How the tray, global shortcut and paste-for-me work on each OS today, and what that means for section 3 (tray) and the GNOME Wayland caveat in section 7.
3. A gap list against section 9 (data the UI needs): which fields exist, which are missing, and the smallest schema change that adds them.
4. A file-by-file plan for phases 1 to 7, flagging anything in the handoff that does not fit the code.

Stop after the report.
```

### Phase 1: Tokens, fonts, theming

```text
Implement design tokens and fonts per design/HANDOFF.md section 2.
- Bring every colour, radius, size, shadow and duration from design/tokens/tokens.json into the app using the approach agreed in Phase 0. One source of truth; no hex values in components.
- Bundle the fonts from design/fonts (variable where supported, static otherwise) and ship the OFL licence files with the app.
- Theme follows the OS (light or dark) and switches live; add a Settings > General control for System, Light, Dark.
- Replace existing hardcoded colours with tokens. List any value that had no token and what you mapped it to.
Acceptance: app builds on all targets, light and dark both render with the new palette, a search for hex colours outside the token file finds none in UI code. Report with before and after screenshots of the main window in both themes, then stop.
```

### Phase 2: Brand assets, app icons, tray

```text
Wire up the brand assets per design/HANDOFF.md section 3.
- App icons: macOS from design/assets/app-icon/png/macos-1024.png, Windows from design/assets/app-icon/tailcopy.ico, Linux PNGs and SVG into the hicolor layout.
- Tray: use design/assets/tray. States idle, sync1 to sync3 (160 ms per frame while a transfer runs), paused, offline, new. macOS uses the black template PNGs; Windows picks white or ink from the taskbar theme and updates live; Linux uses a single-colour symbolic icon via StatusNotifierItem.
- The new state clears when the picker opens. Reduced motion: no sync animation, idle glyph only.
- Use the outlined design/assets/logo/lockup.svg (or lockup-dark.svg) wherever the app shows its name.
Acceptance: icons appear correctly in dock or taskbar and launcher on each OS you can test; tray switches state when you pause, go offline (stop Tailscale) and receive an item. Report what you could and could not verify per OS, then stop.
```

### Phase 3: Component primitives

```text
Build the primitives in design/HANDOFF.md section 4 as reusable components: primary, secondary, accent and danger buttons; icon button; search field; filter chip; toggle; keycap; badge (arrival, pinned, count); device avatar (sizes 20, 24, 28, 40, glyph by device type, colour from color_index); history row with all five states; trail card; status card (capturing and paused); toast.
- Icons only from design/assets/icons.
- Every focusable component gets the focus ring token (`focus` in tokens.json, `--tc-focus` in tokens.css).
- Add a hidden dev-only gallery screen showing every component in every state, light and dark.
Acceptance: the gallery matches the Components board in the design; screenshots of the gallery in both themes in the report. Stop after.
```

### Phase 4: Main window

```text
Rebuild the main window per design/HANDOFF.md section 5, "Main window".
- Sidebar: lockup, Library with live counts (fix the counts that currently read 0), Your tailnet with one row per device (live status, click to filter), Settings, status card at the bottom.
- Header with title, search (Ctrl F) and the "Open anywhere" hint; filter chips; list grouped by time; detail pane with actions, content-sized preview, label, trail card, metadata.
- Images view uses thumbnails and an aspect-correct preview; too-large items say so.
- Add any schema fields from section 9 that Phase 0 found missing, with a migration.
- Responsive: detail pane stacks under the list below about 900 px wide.
Acceptance: side by side with the "History, light" and "History, dark, Images view" boards. Screenshots in the report, then stop.
```

### Phase 5: Quick picker

```text
Polish the quick picker per design/HANDOFF.md sections 5 ("Quick picker"), 6 and 7.
- Opens on the cursor's screen with the enter animation, focus in search; Esc clears search then closes.
- Chips, 46 px rows with number keys 1 to 9, device dot, relative time; pinned first when search is empty.
- Tab toggles the preview pane (44% width) with source line, content and trail line.
- Arrival tint and "From <device>" badge for items from other devices, cleared after use or 4 s after being seen.
- Paste, Shift Enter plain text, number-key paste, toast confirmation; copy-only fallback with the "Copied. Press Ctrl V" toast where paste-for-me is off or unavailable (GNOME Wayland).
- Footer hints are clickable.
Acceptance: every key in section 7 works; compare with the "Quick picker" board (it is interactive on the canvas). Report with screenshots and a short screen recording if you can, then stop.
```

### Phase 6: Pinned, Settings, states

```text
Build the remaining screens per design/HANDOFF.md section 5.
- Pinned shelf: card grid with device palette fills, use counts, More menu, and the dashed hint row.
- Settings: section list (General, Shortcuts, Privacy, Tailnet, History and storage, About). Tailnet section with the status banner, device name field, per-device Send and Receive toggles, peers without TailCopy, and What syncs (images with size limit, formatted text, secrets always skipped, relay server).
- States: first run, empty history, Tailscale not running, pause menu with durations and the paused status card.
- Use the copy deck in section 8 for every string; keep strings in one place for future translation.
Acceptance: compare with the Pinned shelf, Settings and States boards. Screenshots in the report, then stop.
```

### Phase 7: Motion, accessibility, QA

```text
Finish per design/HANDOFF.md sections 6 and 10.
- Apply the motion tokens to every transition listed in section 6; nothing animates that is not listed.
- Reduced motion: fades only, max 120 ms, tray static.
- Accessibility pass against section 10. Fix what fails and list anything you could not fix.
- Screenshot every screen in light and dark at the board sizes and put them in design/qa/ with a short markdown table comparing each to its board: matches, differs on purpose (why), or still wrong.
Acceptance: the QA table has no "still wrong" rows, or each one has an issue drafted. Stop and report.
```
