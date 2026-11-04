# Design Phase 4: main window

3 November 2026. Branch `design`. Handoff section 5 ("Main window") and fixes 11.1 to
11.5, compared with the History and HistoryDark boards.

## What shipped

**Sidebar** (`manager/Manager.svelte`)
- Lockup, then **Library**: All, Pinned, Code, Links, Images, Formatted, each with a
  live count.
- **Your tailnet**: this device (avatar, name, "This device"); clicking it filters to
  items copied here. Other devices join this list in M2.
- Settings and the status card at the bottom.
- **Fix 11.1, counts stuck at 0:** the window fetched status once and never again
  when clips changed. It now refreshes on `clips-changed` (one fetch per burst), and
  the counts come from one pass over the search index (`SearchIndex::counts`).

**History** (`manager/History.svelte`)
- Header: title in Bricolage, search ("Search 248 items", Ctrl/⌘+F focuses it), and
  "Open anywhere" with the picker shortcut.
- Chips on the mixed views (All, Pinned, a device): All, Text, Links, Code, Images.
  "Clear history…" moved to the end of the chip row.
- The list grouped by Just now (last 15 minutes), Earlier today, Yesterday, then
  weekday or date. Rows are the Phase 3 `HistoryRow`: type tile (the colour itself
  for colours), mono for code, colours and IPs, device dot and name, app, ×N, pin.
  Images view uses 64 × 44 thumbnails.
- Detail card: Copy (the label swaps to "Copied" for 1.2 s and the button keeps its
  width), Pin, Delete pushed right; preview sized to content (min 120 px, `sunk`,
  mono for code), Label, trail card, and Type, Size, Dimensions, Copied, Last used,
  Used, App, Formats, Copies, Sync.
- **Delete offers Undo for 5 s**: the row disappears at once and the delete is sent
  when the toast ends (or straight away if you clear history or leave).
- Responsive: the card sits beside the list from 900 px wide and wraps under it below.

**Fixes from handoff section 11**
- 11.3: whitespace-only copies were already ignored; single characters now are too,
  by default. New setting **Skip single characters** (`general.skip_single_characters`).
- 11.4: the preview is sized to its content.
- 11.5: the Device name field shows the actual Tailscale name as its placeholder.
- 11.2 ("every item says This device") is correct until sync (M2).

## Data (handoff section 9)

Migration `0002_design.sql`, all local, no wire change:

| Column | Notes |
| --- | --- |
| `clips.use_count` | Number of `ClipUsed` ops, computed in the merge like `last_used_hlc`, so it converges and the consistency check covers it. Backfilled from the ops log |
| `clips.received_ms` | When this device first held the clip, by its own clock. Trail latency is `received_ms − copied`; local clips get the capture time. Backfilled from `created_hlc` |
| `devices.color_index` | Join order, assigned on insert and never recomputed; this device is 0 |
| `devices.device_type`, `os`, `send_enabled`, `receive_enabled` | Read by M2 (and a device-type picker later); defaults keep today's behaviour |

**Derived look** (`tailcopy-core/src/look.rs`): text, rich, image, code, link or colour,
from the kind and preview with the handoff's cheap heuristics. Never stored or
synced. Search gains `is:code` and `is:link`; `DisplayRow` carries `look` and `mono`.

This device's display name now follows the Device name setting (empty means the
Tailscale hostname), so rows and the sidebar show the current name.

SPEC 6.2 (new columns), 10.2 (`is:code`, `is:link`) and 11 (`skip_single_characters`)
updated.

## Differs from the board

- **No Paste button in the detail card.** The History window is a normal window: there
  is no "previous app" to paste into, so Copy stays the primary action. Paste lives in
  the picker.
- **"From other devices" chip** is left out until M2: there are no such items yet,
  and it needs a core filter for "not this device".
- **No "Password skipped" rows** (D-021).
- **"$" prompt in code previews** is not drawn: the look heuristics can't tell a shell
  command from SQL reliably, and a prompt on the wrong item would mislead.
- The sidebar shows the lockup rather than the board's mark plus Bricolage text,
  per Phase 2.
- Arrival tint and "Just arrived" need sync, so they stay off until M2.

## Checks

| Check | Result |
| --- | --- |
| `npm run check`, `lint`, `check:tokens`, `build` | Pass |
| `cargo fmt`, `clippy -D warnings` (Linux in Docker, Windows and macOS targets) | Pass |
| `cargo test --workspace` | Pass: 83 core tests, including new ones for looks, `is:code`/`is:link` and counts, single characters, colour order on join, and the v1 → v2 migration backfill |

Screenshots (Chromium, mock backend, light and dark, Images view, narrow, Undo):
`design/qa/phase4/`.

## To check on your machines

- Upgrade over the M1 install: history, pins and labels intact; the sidebar counts
  match the list and change as you copy.
- Copy "x" (one letter): not stored. Turn off Settings > Skip single characters and
  it is.
- Delete an item, then Undo within 5 seconds: it comes back. Let the toast run out:
  it's gone after a restart too.
- Settings > Device name shows your Tailscale name greyed out when empty.
