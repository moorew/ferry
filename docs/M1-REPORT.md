# M1 report: the TailCopy app

Status as of 2 November 2026. Branch `m1`, PR #2. Sync is off in this milestone.

TailCopy 0.1.0 is a working local clipboard manager with a GUI on Windows, macOS and
Ubuntu GNOME, installed with no commands after the installer. The installed Ubuntu
package passes a full end-to-end run on GNOME 46 and 50. Windows and macOS are built,
signed where it applies, and waiting for real-machine runs.

## What shipped

- **Picker** (SPEC 10.2, 10.3): opens on your shortcut over any app, newest first,
  type to search, with the `is:pinned`, `is:image`, `is:rich`, `is:text`, `from:`,
  `today` and `yesterday` tokens. Enter pastes with formatting, Shift+Enter as plain
  text, Ctrl/Cmd+1 to 9 pastes a row. Tab shows a preview, Ctrl/Cmd+P pins,
  Ctrl/Cmd+L labels, Ctrl/Cmd+Shift+Backspace deletes, Ctrl/Cmd+C copies without
  pasting, Esc closes.
- **History window**: All, Pinned, Images and Formatted text; search; multi-select
  with bulk pin and delete; a detail pane with copy, pin, label and delete; clear
  history with an "include pinned" choice.
- **Settings** that save as you change them: device name, start at login, paste for me,
  shortcuts, skip passwords, apps to ignore, how much history to keep, "Only my devices"
  for sync, Linux paste keystrokes, open log folder, copy diagnostics.
- **First run**: Tailscale check, the GNOME extension (turned on with one click, with
  Log out now when GNOME needs a fresh login), macOS Accessibility (with a button to the
  right System Settings pane), and choosing your shortcut. Every step can be skipped
  and run again from Settings.
- **Tray / menu bar**: Open, History, Pause capture, Settings, Quit. The icon goes
  hollow while paused.
- **CLI**: `tailcopy`, `tailcopy picker`, `paste-plain`, `pause`, `resume`, `status`.
- **Privacy**: OS secret markers on all three platforms (including password-manager
  hints on GNOME, which fail closed, D-020), the app list with "don't save" and "keep on
  this device", pause. Logs never contain clip content (checked in the end-to-end test).
- **Installers**: Windows per-user NSIS setup, macOS universal DMG signed with your
  Developer ID and notarized, Ubuntu `.deb` with the GNOME extension and its schema.
- **Brand**: the centred Trail mark, Commissioner throughout, black and white.

## Results

| SPEC 14.2 smoke item | Ubuntu (headless GNOME 46 and 50) | Windows | macOS |
| --- | --- | --- | --- |
| 1. Copy text; it appears in the picker within 300 ms | **Pass** (settles in 150 ms, then stored) | Your test | Untested (no Mac) |
| 2. Formatted paste with Enter, plain with Shift+Enter | **Pass** (HTML still served after the picker hides; plain offers no HTML) | Your test | Untested |
| 3. Image copy, thumbnail, paste | Not covered headless | Your test | Untested |
| 4. Password manager copy not stored | **Pass** | Your test | Untested |
| 5. Picker opens on the hotkey every time, focused | **Pass** in every run so far; 50-press run is yours | Your test | Untested |
| 6. Paste lands in the previous app, terminal override | **Pass** for a GTK app; terminals are yours | Your test | Untested |
| 7. Pause stops capture; resume restarts it | **Pass** (via the CLI) | Your test | Untested |
| 12. Lock the screen; shortcuts work after unlock | **Pass** with the lock stand-in (extension disable/enable) | n/a | n/a |

The end-to-end test (`app/test-gnome`) installs the `.deb` on stock Ubuntu 24.04 and
26.04 and passes 18/18 on both. It found and fixed a startup crash, double captures
from GNOME's repeated change events, and a password hint arriving after the first
event.

| Measure | Result | Budget |
| --- | --- | --- |
| Hotkey to focused picker, GNOME (headless) | 10 to 25 ms | 100 ms p95 |
| Search over 10,000 clips | p95 5.9 ms | 16 ms p95 |
| Building the search index for 10,000 clips at startup | about 195 ms | |

## Known gaps

- **macOS has no tester yet.** It builds, signs and notarizes; it hasn't run anywhere.
- **Windows** is compiled and packaged by CI but hasn't run on a machine.
- Image copy and paste aren't covered by the headless test.
- On Linux the picker is square: WebKitGTK windows can't be transparent, so the
  rounded corners are Windows and macOS only.
- The Debian package is named `tail-copy` (Tauri derives it from the product name);
  M3 packaging fixes that.

## Test steps for you

Installers: the **TailCopy 0.1.0 (M1 pre-release test build)** at
<https://github.com/moorew/tailcopy/releases/tag/m1-test-build>. Uninstall the M0 test
build first if you installed it.

After each platform, open **Settings > Open log folder** and send me the newest log
file, and the text from **Settings > Copy diagnostics**. Neither contains anything you
copied.

### Windows 11 (and 10 22H2 if you have it)

1. Run `TailCopy-0.1.0-windows-x64-setup.exe`. SmartScreen: More info, Run anyway.
2. TailCopy opens on its setup screens. Go through them and pick a shortcut (Ctrl+Alt+V
   is suggested).
3. Smoke items 1 to 7 from the table, using Notepad, Word or LibreOffice Writer, Chrome
   and Paint. For item 4, copy from your password manager. For item 5, press the
   shortcut about 50 times over a few minutes.
4. Open an app as administrator and paste into it: you should get a notification
   "Copied. Press Ctrl+V to paste (admin window)."
5. Sign out and back in: TailCopy should start by itself (tray icon) without opening a
   window.

### Ubuntu 24.04 and 26.04

1. Open `TailCopy-0.1.0-ubuntu-amd64.deb` with App Center (or `sudo apt install` it if
   you prefer).
2. Open TailCopy from the app grid. Setup asks you to log out and back in once (GNOME
   only notices new extensions at login) with a button that does it. After logging back
   in, open TailCopy again and finish setup.
3. Smoke items 1 to 7 with Text Editor, Firefox, LibreOffice Writer and Ptyxis (the
   paste into Ptyxis should use Ctrl+Shift+V).
4. Item 12: lock the screen (Super+L), copy nothing, unlock; the shortcut should work
   straight away.

### macOS (when someone has a Mac)

1. Open the DMG, drag TailCopy to Applications, open it. No warnings expected: it's
   signed and notarized.
2. Setup asks for Accessibility; the button opens the right pane. Switch TailCopy on;
   setup notices by itself.
3. Smoke items 1 to 7 with TextEdit and Safari. The suggested shortcut is Cmd+Shift+V.

## What I need from you

1. The Windows and Ubuntu runs above, with the log and diagnostics.
2. A decision on macOS testing: borrow a Mac for an hour, rent one, or leave macOS
   untested in this gate.
3. Your call on the open spec changes: D-009 to D-012 from M0, and D-018 (a late pin
   can bring back a cleared clip; the fix changes the wire format, so it should land
   before M2's sync).
