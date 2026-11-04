# Design Phase 5: quick picker

3 November 2026. Branch `design`. Handoff sections 5 ("Quick picker"), 6 and 7, as
amended by D-021. Compared with the Picker board.

## What shipped

- **Window:** 960 × 640 by default, resizable down to 640 × 420, and it remembers its
  size (saved in `<data dir>/picker-size` each time it hides, so dragging doesn't
  write on every frame). Radius 22 with the system shadow on Windows and macOS; still
  square on Linux (WebKitGTK has no transparent windows).
- **Opens** on the monitor with the cursor, as before (on GNOME the extension centres
  it on the focused window's monitor), with the 140 ms fade and 0.98 → 1 scale, or a
  120 ms fade with reduced motion. Focus lands in the search field.
- **Header:** 17 px search, filter tokens shown as pills (`is:code`, `from:mac-mini`),
  the paused marker, and "All devices" on the right (a label until M2 makes it a menu).
- **Chips:** All, Text, Links, Code, Images, Pinned.
- **Rows:** 46 px, 30 px tile (the colour itself for colours, thumbnails for images),
  mono for code, colours and IPs, label in bold, pin, ×N, device dot **and name** for
  items from other devices (hidden for this device, as SPEC 10.2 says), relative time,
  and the number for Ctrl/⌘+1 to 9.
- **Pinned first** when nothing is typed and no chip is on.
- **Preview (Tab):** 44% of the width on the `bg` ground: source line (device, app,
  time), the content in a card (mono when it reads as code), and the trail line.
- **Footer:** Paste, Plain text, Preview (shows pressed while open), Pin, Label, and
  Close on the right. Every hint is a button that does what it says.
- **Esc** clears the search and chip first, then closes (D-021).
- Empty states use the copy deck: "Nothing matches “query”" with "Try fewer words, or
  switch the filter back to All.", and "Nothing copied yet" for an empty history.

## Keys (handoff section 7, with D-021)

| Key | Result |
| --- | --- |
| ↑ ↓, Ctrl+J/K, Page Up/Down | Move |
| Enter / Shift+Enter | Paste / paste as plain text |
| Ctrl/⌘+1 to 9 | Paste that row (bare digits type into search, D-021) |
| Tab | Preview |
| Ctrl/⌘+P, Ctrl/⌘+L | Pin, label |
| Ctrl/⌘+Shift+Backspace | Delete (the Delete key edits the search, D-021) |
| Ctrl/⌘+C | Copy without pasting, when no search text is selected |
| Esc | Clear search, then close |

The global shortcut stays Ctrl+Alt+V / Cmd+Shift+V (D-021).

## Differs from the handoff or board

- **No success toast** after paste ("Pasted into Termius"); the picker hides before
  pasting and SPEC 1.1 keeps notifications for problems. "Copied. Press Ctrl+V"
  (and the admin-window message) still appear when a paste can't happen. (D-021)
- **Arrival tint and "From mac-mini" badge**, and clearing them 4 s after being seen:
  they need items from other devices, so they land with sync in M2.
- **Device name shown next to the dot** on rows from other devices; the board shows
  the dot alone with a tooltip, which breaks the "colour is never the only signal" rule.
- **"All devices" scope** is a label for now; picking a device needs other devices.

## Checks

`npm run check`, `lint`, `check:tokens`; `cargo fmt`, `clippy -D warnings` on Linux,
Windows and macOS targets. Screenshots in `design/qa/phase5/`: default, dark with
preview, Code chip, no results, and the list after Esc cleared a search.

## To check on your machines

- Open the picker, drag its edge to resize it, close and reopen: same size, also after
  restarting TailCopy.
- Pin an item, open the picker: it's at the top. Type anything: normal order.
- Type a search, press Esc: the search clears. Esc again: the picker closes.
- Click each footer hint with the mouse.
- On Windows, look at the corners and shadow over a busy background.
