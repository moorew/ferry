<p align="center">
  <img src="design/assets/logo/lockup.svg" alt="TailCopy" width="260">
</p>

<p align="center">A keyboard-first clipboard manager for Windows, macOS and Ubuntu.<br>
Everything you copy, one shortcut away. No account, no cloud, no tracking.</p>

![The History window](docs/images/history.png)

## Status

**v0.2** adds sync: every device signed in to your own
[Tailscale](https://tailscale.com) account that runs TailCopy shares one history,
directly over your tailnet, with nothing in between. What you copy never leaves your
tailnet, and only your own devices can connect. Sync is new, so please report
anything that goes missing or arrives twice.

TailCopy is an independent project built on Tailscale. It isn't made by or affiliated
with Tailscale Inc.

## What it does

- **Keeps what you copy:** plain text, formatted text (HTML and RTF) and images, in a
  searchable history.
- **The picker:** press your shortcut over any app, type a few letters, press Enter,
  and it pastes into the app you were using. Shift+Enter pastes as plain text.
  Ctrl+1 to 9 (⌘ on macOS) pastes a row straight away.
- **Search** with fuzzy matching on what you copied and its label, plus filters:
  `is:pinned`, `is:text`, `is:rich`, `is:image`, `is:code`, `is:link`, `today`,
  `yesterday`.
- **Knows what it's looking at:** links, code and shell commands, colours and IP
  addresses get their own icon (colours show the colour) and code reads in a
  monospace font. Filter by them in the sidebar and the picker.
- **Pin** what you reuse: pinned items are kept forever and come first in the picker.
  **Label** anything ("prod deploy") so it's easy to find again.
- **History window** with counts by type, items grouped by time, a detail view with
  the full content, and Undo for 5 seconds after you delete.
- **Private by default:** copies that password managers and other apps mark as secret
  are never saved. You can ignore whole apps, or keep their copies on this device only.
  Single characters and blank copies are skipped (you can turn that off).
- **Pause** capturing for 15 minutes, an hour, or until you resume, from the app or
  the tray (or a shortcut you set, on Windows and macOS).
- **Light and dark**, following your system or set in Settings.
- **Works on GNOME on Wayland** through a small GNOME Shell extension that's installed
  with the app, so the shortcut and paste-for-you work where other clipboard managers
  can't.

<table>
  <tr>
    <td><img src="docs/images/picker.png" alt="The picker with its preview open"></td>
    <td><img src="docs/images/picker-dark.png" alt="The picker in dark mode, filtered to code"></td>
  </tr>
  <tr>
    <td><img src="docs/images/pinned.png" alt="Pinned items as cards"></td>
    <td><img src="docs/images/history-dark.png" alt="History in dark mode"></td>
  </tr>
</table>

## Install

Download the installer for your system from the
[latest release](https://github.com/moorew/tailcopy/releases/latest), install it, open
TailCopy and follow the short setup. Nothing needs a terminal.

| System | File | Notes |
| --- | --- | --- |
| Windows 10 (22H2) and 11, x64 | `TailCopy-…-windows-x64-setup.exe` | Installs for your user only. The installer isn't code-signed yet, so Windows SmartScreen asks first: **More info**, then **Run anyway** |
| macOS 13 Ventura and later | `TailCopy-…-macos-universal.dmg` | Drag TailCopy to Applications. Setup asks for Accessibility so TailCopy can paste for you, with a button to the right settings pane |
| Ubuntu 24.04 and 26.04 (GNOME on Wayland) | `TailCopy-…-ubuntu-amd64.deb` | Open it with App Center or `sudo apt install ./TailCopy-…deb`. Setup turns on the GNOME extension; GNOME needs you to log out and back in once, and setup has a button for that |

Each release has a `SHA256SUMS` file to check the downloads against.

Install TailCopy on each device and sign in to Tailscale with the same account
everywhere; the devices find each other by themselves. Without Tailscale, TailCopy
still keeps your history on the device you're using.

On Windows, the first time TailCopy starts syncing, Windows Firewall may ask whether
TailCopy can accept connections. Allow it for private networks so your other devices
can reach this one. If you don't, this device still syncs by connecting out to the
others; it just can't be reached first.

## Sync

- **Your devices only.** TailCopy listens only on your Tailscale addresses, and every
  connection is checked with Tailscale before anything is read: only untagged devices
  signed in to your own Tailscale login can sync. Other people on a shared tailnet are
  refused, even when the tailnet policy allows everything.
- **Live:** copy on one device and it's in the picker on the others a moment later,
  and on their clipboards too (Settings > Tailnet > Put arrivals on the
  clipboard). Something you copied more recently is never overwritten, and catching up
  after sleep never touches the clipboard.
- **Catches up:** a device that was off gets everything it missed, in order, when it's
  back. Pins, labels, deletes and clears sync the same way.
- **Per device:** Settings > Tailnet lists your devices, how they're connected
  (direct or relayed), and Send and Receive switches for each.
- **What stays put:** passwords and secrets are never stored, apps you mark "keep on
  this device" stay local, and images over 10 MB (adjustable) stay on the device they
  were copied on. On Ubuntu, clips that arrive go on the clipboard as plain text or an
  image; formatted text still pastes formatted from the picker.
- It uses port 7441 on your tailnet. If your tailnet policy isn't allow-all, allow
  your devices to reach each other on `tcp:7441`.

## Using it

| Keys (⌘ instead of Ctrl on macOS) | Does |
| --- | --- |
| **Ctrl+Alt+V** (macOS **⌘⇧V**) | Open the picker over any app. Ubuntu asks you to pick it during setup; change it any time in Settings > Shortcuts |
| Type | Search |
| ↑ ↓ | Move |
| Enter / Shift+Enter | Paste / paste as plain text |
| Ctrl+1 to Ctrl+9 | Paste that row |
| Tab | Show the preview |
| Ctrl+P / Ctrl+L | Pin / label |
| Ctrl+Shift+Backspace | Delete |
| Ctrl+C | Copy without pasting |
| Esc | Clear the search, then close |
| Ctrl+Alt+Shift+V | Paste what's on the clipboard as plain text (Windows; on Ubuntu, set it in Settings > Shortcuts) |

TailCopy lives in the system tray (the menu bar on macOS): open the History window,
pause, open Settings or quit from there. The icon shows when capture is paused and
when Tailscale isn't running.

When "Paste for me" is off (Settings > General), or a window can't take a pasted
keystroke (an app running as administrator on Windows), choosing an item copies it
and TailCopy tells you to press Ctrl+V.

### Command line

On Ubuntu `tailcopy` is on your path; on Windows and macOS it's the app's own binary.

```
tailcopy              open TailCopy, or bring up its window
tailcopy picker       open the picker (bind this to a key in any launcher)
tailcopy paste-plain  paste the clipboard as plain text
tailcopy pause        stop saving what you copy
tailcopy resume       start again
tailcopy status       print the status as JSON
```

## Privacy

- **Nothing leaves your device in v0.1.** There are no accounts, no servers, no
  analytics, no crash reports and no update checks.
- Your history is a SQLite database and a folder of content files in TailCopy's data
  folder. Logs never contain anything you copied (Settings > About > Diagnostics has
  a summary you can share for bug reports, and it's the same).
- Secret copies are skipped: Windows' "exclude from history" flag, macOS concealed
  and transient types, and the password-manager hints KeePassXC and others set on
  Linux. 1Password, Bitwarden, KeePassXC and Proton Pass are also ignored by app name.
- History is kept for 1,000 items or 30 days, whichever is smaller, except pinned
  items. Change it in Settings > History and storage.

| System | Settings (`config.toml`) | History and logs |
| --- | --- | --- |
| Windows | `%APPDATA%\moorew\TailCopy\config` | `%LOCALAPPDATA%\moorew\TailCopy\data` |
| macOS | `~/Library/Application Support/io.github.moorew.TailCopy` | the same folder |
| Linux | `~/.config/tailcopy` | `~/.local/share/tailcopy` |

## What's next

The optional self-hosted relay (so devices that were asleep catch up even when your
other devices are off), signed updates, and a listing on extensions.gnome.org. The
full plan is in [SPEC.md](SPEC.md).

## Building from source

You need Rust (stable), Node 22, and on Linux
`libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev`.

```sh
cd app && npm ci && npm run build   # the front end, embedded into the app
cargo test --workspace
cd app && npx tauri dev             # run it
cd app && npx tauri build           # make an installer for this system
```

On GNOME, a development build has to be allowed by the extension first; see
`CLAUDE.md` ("GNOME notes"). Design tokens, fonts, icons and the brand live in
[`design/`](design/HANDOFF.md).

## Licence

The app is MIT or Apache-2.0, at your option ([LICENSE-MIT](LICENSE-MIT),
[LICENSE-APACHE](LICENSE-APACHE)). The GNOME extension in `extension/` is
GPL-2.0-or-later. Fonts: Bricolage Grotesque, Figtree and JetBrains Mono, under the
SIL Open Font License (`design/fonts/`).
