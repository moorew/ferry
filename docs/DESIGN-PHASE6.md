# Design Phase 6: Pinned, Settings and states

3 November 2026. Branch `design`. Handoff section 5 (Pinned shelf, Settings, States)
and 8 (copy deck). Compared with the Pinned, Settings and States boards.

## What shipped

**Pinned shelf** (`manager/Pinned.svelte`, the sidebar's Pinned entry)
- Cards in a `repeat(auto-fit, minmax(280px, 1fr))` grid, gap 16, radius 22, at least
  210 tall. Fills cycle lavender, violet, periwinkle, orchid, mauve, then one plain
  card. Text on the fills is always the dark glyph colour, in both themes.
- Each card: type and use count ("Code · used 31 times"), a More menu (Copy, Unpin,
  Delete), the label or first line as the title, a preview (code in mono on a white
  wash, the swatch for colours, a thumbnail for images), and the source device with
  the date. The dashed hint row closes the grid.
- Cards take focus: Enter or Ctrl/⌘+C copies, Ctrl/⌘+P unpins.

**Settings** (`manager/Settings.svelte`)
- Its own sidebar with a way back to History, and six sections: General, Shortcuts,
  Privacy, Tailnet, History and storage, About, shown one at a time.
- **Tailnet:** with Tailscale running, the `accent-soft` banner ("Connected as
  thinkpad") with the device-name field ("Show this device as"). Without it, the
  Tailscale-isn't-running state with the outlined character, **Open Tailscale**
  (Windows and macOS) and **Try again** (asks Tailscale straight away). Then "What
  syncs": passwords and secrets "Always skipped", and Only my devices.
- Every Settings string now comes from `lib/strings.ts`.

**States**
- `components/Character.svelte`: the mark with a face, happy, resting or offline,
  coloured by tokens. Used only on first run, empty history and Tailscale offline.
- **Empty history:** resting character, "Nothing copied yet", and the picker shortcut.
- **First run:** the happy character on the welcome and done steps.
- **Pause:** the status card's pause button opens For 15 minutes, For 1 hour, Until I
  resume, each with when it ends. A timed pause resumes by itself; the card then says
  "Resumes at 8:41 PM". The tray menu has the same three choices plus Resume capture.
  Backend: `state::pause_for`, `set_paused(paused, minutes)`, `pausedUntilMs` in status.

**Data:** display rows now carry `use_count` (the Pinned cards need it), so the detail
view's separate copy went away.

## Differs from the boards

- **First run "Found your tailnet"** with the device stack needs other devices running
  TailCopy, so it arrives with M2. Today's first run keeps its steps (Tailscale, GNOME
  extension, Accessibility, shortcut) with the new look and character.
- **Tailnet device list, per-device Send and Receive, Images and Formatted text
  toggles, and Relay "Set up"**: sync features (M2, relay M3). The section says sync
  arrives in the next version rather than showing switches that do nothing.
- **Paused card** says "Paused" and "Resumes at …" rather than "Paused for 1 hour": the
  app keeps when the pause ends, not how long it was.
- **The device-name field** moved from General to Tailnet, as on the board;
  it shows in the offline state too, so the name can always be changed.

## Strings

`lib/strings.ts` holds the copy deck plus everything in History, Pinned, Settings, the
picker and the components. **Still inline:** the first-run screens (their sentences
have keycaps in the middle, so they need a small formatting helper first) and a few
accessible labels. Listed for Phase 7 follow-up.

## Checks

`npm run check`, `lint`, `check:tokens`, `build`; `cargo fmt`, `clippy -D warnings` on
Linux, Windows and macOS targets; `cargo test --workspace`. Screenshots in
`design/qa/phase6/`.

## To check on your machines

- Pin a few items and open Pinned: cards, use counts, More menu.
- Pause for 15 minutes from the status card, then from the tray: copy something, it's
  not saved; the card says when it resumes; it resumes by itself.
- Quit Tailscale, open Settings > Tailnet: the offline state; Open Tailscale (Windows),
  start it, Try again: the banner with your Tailscale name.
- Clear history (with pinned): the resting character and your shortcut.
