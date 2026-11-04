# Design Phase 7: motion, accessibility, QA

3 November 2026. Branch `design`. Handoff sections 6 and 10.

## Motion (section 6)

Everything uses the motion tokens; nothing else animates.

| Moment | Motion | Reduced motion |
| --- | --- | --- |
| Picker opens | Fade, scale 0.98 → 1, `--tc-dur-enter` (140 ms), ease-out | 120 ms fade |
| Picker closes | None: the window hides at once so the paste lands sooner (SPEC 10.4) | None |
| Toast in | Fade and rise 8 px, `--tc-dur-base` (200 ms) | 120 ms fade |
| Copy | Label swaps to "Copied" with a check for `--tc-hold-copied` (1.2 s); width fixed | Same (no motion) |
| Pin | The pin tips in on the spring curve, `--tc-dur-spring`; in the detail card it turns violet | 120 ms fade |
| Sidebar counts | Bump on the spring curve when the number changes, never on first draw | None |
| Delete | Row collapses over `--tc-dur-collapse` (180 ms), Undo toast for `--tc-hold-undo` (5 s) | 120 ms fade |
| Hover, toggles | `--tc-dur-fast` (120 ms) | Cut to 0 |
| Arrival slide and tint | M2 (needs items from other devices) | |
| Tray syncing frames | M2 | Static dot |

New tokens: `--tc-dur-collapse` (180), `--tc-hold-toast`, `--tc-hold-undo`,
`--tc-hold-copied` (from `motion.toast` and a new `motion.copied` in `tokens.json`).

## Accessibility (section 10)

- **Contrast:** every token pair the UI uses passes 4.5:1 in both themes, checked
  with a script over `tokens.css` (the WCAG relative-luminance formula). Text on the
  six device colours: at least 5.06:1 light, 5.7:1 dark.
- **Device colour never alone:** the picker now shows the device name next to its
  dot (the board had the dot alone); avatars carry the name as label and tooltip.
- **Icon-only buttons:** `IconButton` requires a label.
- **Focus:** the 2 px accent ring with a surface gap is global. The picker's search is
  now a combobox whose `aria-activedescendant` follows the selected row, so a screen
  reader hears each row as you arrow through while focus stays in search. Pinned cards
  take focus (Enter copies, Ctrl/⌘+P unpins).
- **Hit targets:** picker chips, picker footer hints and the Appearance options are
  now 32 px tall.
- **Toasts** are `role="status"`, polite, and never take focus.
- **Reduced motion:** verified in Chromium with `prefers-reduced-motion: reduce`: the
  toast fades with no transform.
- **Strings:** all user-facing copy, accessible labels included, is in
  `lib/strings.ts` (first run and the shortcut recorder moved in this phase).

## QA

`design/qa/README.md` compares every screen, light and dark, with its board:
matches, or differs on purpose with the reason. No row is "still wrong".

## End-to-end on GNOME

The M1 end-to-end test (`app/test-gnome`) passes **18/18 on GNOME 46 and 50** with a
debug `.deb` of this branch: capture, formatted and plain paste, search, the
paste-plain shortcut, status, pause and resume, password-manager skip, re-register
after an extension restart, and no clip content in the log.

One thing to look at: the app logs `Gtk-CRITICAL gtk_widget_get_scale_factor:
assertion 'GTK_IS_WIDGET (widget)' failed` a few times around the extension restart.
The tests pass, and I haven't checked whether the M1 build logs it too; it may come
from the tray (AppIndicator) or the native theme call. Worth a look before release.

## Checks

`npm run check`, `lint`, `check:tokens`, `build`; `cargo fmt`, `clippy -D warnings` on
Linux, Windows and macOS targets; `cargo test --workspace`; extension lint; debug
`.deb` build; GNOME 46 and 50 end-to-end.
