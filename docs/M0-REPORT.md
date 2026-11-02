# M0 report: spikes

Status as of 2 November 2026. Branch `m0-spikes`, PR #1.

What could be proven without your hardware has been, in headless GNOME Shell 46 and
50 and against the live tailnet on `venus-wsl`. Everything else is built and
compile-checked against the real Windows and macOS APIs, and is waiting for you to
run it with the steps below.

## Summary

| # | M0 item (SPEC 17) | Status | Evidence |
| --- | --- | --- | --- |
| 1 | Windows: watch/read/write text, HTML, RTF, PNG; focus restore; Ctrl+V into Notepad, Word or LibreOffice, Chrome | **Ready for you** | `spikes/clipboard-win`; clippy clean for `x86_64-pc-windows-msvc`; release build on the Windows runner |
| 2 | macOS: the same into TextEdit and Safari, with the Accessibility prompt | **Ready for you** | `spikes/clipboard-mac`; clippy clean for `aarch64-apple-darwin`; universal ad-hoc-signed `.app` on the macOS runner |
| 3 | GNOME 46 and 50: extension, GetContent/SetContent, PresentPicker, paste, GTK multi-format offer surviving hide, re-register after lock, latency | **Pass (headless)**, hardware run pending | Extension test 19/19 on GNOME 46.0 and 50.1; client end-to-end 13/13 on 46.0 and 12/13 on 50.1 (one timing check, see below) |
| 4 | Tailnet: status and whois via CLI on all three, LocalAPI on Linux as non-root; Tailscale-only listener; two machines exchange `/v1/hello` with whois on both ends; Defender behaviour | **Partial pass** | Linux CLI and LocalAPI as a normal user; listener binds only the Tailscale IPv4 and IPv6 addresses; hello with whois both ways on one node; tagged node rejected. Two machines, Windows and macOS pending |
| 5 | Shift+Enter and the global paste-plain shortcut arrive as a clean Ctrl+V/Cmd+V | **Pass on GNOME (headless)**, Windows and macOS pending | Paste waits while Shift is held, refuses after 503 to 515 ms, and pastes cleanly once Shift is released |

## Measurements

| What | Result | Where |
| --- | --- | --- |
| GNOME hotkey to focused picker (shell-measured, from the keybinding to the picker becoming the focus window) | 16 to 40 ms over 8 presses (budget 100 ms p95) | Headless GNOME 46.0 and 50.1, software rendering |
| Picker focused as seen by the webview, after the client receives `ShortcutActivated` | 20 to 47 ms | Same |
| Choose to Ctrl+V sent (modifier poll + 60 ms activation delay) | 83 to 84 ms | Same |
| `GetContent`, small text or HTML | 1 to 6 ms | Same |
| `SetContent` / `GetContent`, 1 MiB | 24 to 134 ms / 49 to 127 ms | Same |
| `tailscale status --json` / LocalAPI status | 18 ms / 12 ms | venus-wsl, 50+ peers |
| whois, CLI / LocalAPI | 8 to 9 ms / 1 ms | venus-wsl |
| `/v1/hello` round trip on one node, IPv4 / IPv6 | 3 ms / 7 ms | venus-wsl |

The GNOME numbers come from a headless shell in Docker and only show the design isn't
slow; please confirm on real hardware (step G6).

## Findings and proposed spec changes

Details are in `docs/DECISIONS.md`. The ones that need your decision:

- **D-012** clipboard-rs's Windows `set()` loses formats when an image is present.
  Proposed: read with clipboard-rs, write through clipboard-win in one session.
- **D-011** GNOME emits ACTIVE just before disabling an extension, so a re-Register
  can fail once. Proposed: treat Register failure as retryable.
- **D-010** `global.get_pointer()` is the right modifier source on 46 to 51; ignore
  Caps Lock and Num Lock.
- **D-009** Tailscale JSON: nulls, `CurAddr` is empty for idle peers (path display
  needs an "idle" state), `StableID` vs numeric `ID`, LocalAPI socket permissions.

Also confirmed, no change needed: the GTK multi-format offer survives the picker
hiding and keeps serving every format (9.4.3 [M0 verify]);
`global.stage.context.get_backend()` is missing on 46 and `Clutter.get_default_backend()`
is gone in 51 (9.4.1); a single write fires several change events, and the loop guard
catches them (7.6).

## Headless GNOME details

`spikes/gnome-extension/test` runs the extension in `gnome-shell --headless` on Ubuntu
24.04 (GNOME 46.0) and 26.04 (GNOME 50.1) and drives it from a Python client.
`spikes/gnome-client/test` runs the real Tauri client against it. Covered: the caller
check before Register, Register, ClipboardChanged, GetMimeTypes, Get/SetContent
(including 1 MiB binary), SetShortcut, the picker shortcut, PresentPicker for a window
created after the call and for an already-mapped one, Paste with and without
activating the previous window, the modifier wait, the GTK offer after hide (HTML
still served), Shift+Enter offering plain text only, the paste-plain shortcut, and
re-registration after disable/enable.

Limits: no lock screen without GDM, so lock/unlock is simulated with
Disable/EnableExtension (the same thing GNOME does on lock); no real apps (Text Editor,
Firefox, Ptyxis); keystrokes are injected through a second virtual keyboard. A
headless shell sometimes never gives the first window keyboard focus; then every
focus-dependent check fails together, and a rerun passes. The 12/13 on GNOME 50 is
the editor taking longer than 800 ms to report focus; the pastes into it all passed.

## Test steps for you

Binaries come from the **Spike builds** workflow on this branch: open the latest run at
<https://github.com/moorew/ferry/actions/workflows/spikes.yml> and download
`spikes-windows`, `spikes-macos` and `spikes-linux`. Each spike prints timestamped
logs. Please send me the full output of each run.

Picker keys in every spike: Up/Down (or Ctrl+J/K) to move, **Enter** pastes all
formats, **Shift+Enter** pastes plain text, **Ctrl+T** prints the formats now on the
clipboard, **Esc** hides.

### Windows 11 (and 10 22H2 if you have it)

W1. Unzip `spikes-windows`, open PowerShell there, run `.\clipboard-win-spike.exe`.
    Leave it in the foreground of its console; logs print there.

W2. Capture: copy plain text in Notepad; a formatted paragraph in Word or LibreOffice
    Writer; a formatted paragraph in Chrome; an image in Chrome (right-click, Copy
    image). Each should log `read [...]` with the formats and a time.

W3. Secrets: copy a password from your password manager. Expect `skipped:` with the
    marker it set, or tell me which manager logged nothing special.

W4. Picker and paste: click into Notepad, press **Ctrl+Alt+V**, pick the plain-text
    clip, press **Enter**. It should paste into Notepad. Then the formatted clip with
    **Enter** into Word or Writer (formatted), and with **Shift+Enter** (plain, and in
    LibreOffice **no Paste Special dialog**). Then into Chrome (a text field or a doc).
    The image clip with **Enter** into Paint or Writer.

W5. After a rich paste, open the picker and press **Ctrl+T**: the log lists the
    formats on the clipboard. Expect HTML, RTF and text (and PNG plus a bitmap for
    images).

W6. Global plain paste: copy formatted text, click into Writer, press
    **Ctrl+Alt+Shift+V**. Plain text, no Paste Special dialog.

W7. Elevated window: open Notepad as administrator, try W4 into it. Expect
    `Copied. Press Ctrl+V to paste (admin window).`

W8. clipboard-rs comparison (D-012): quit, run
    `$env:FERRY_WRITER_CLIPBOARD_RS=1; .\clipboard-win-spike.exe`, copy an image from
    Chrome, choose it with Enter, then Ctrl+T. I expect formats missing.

W9. Tailnet: `.\tailnet-spike.exe status`, then `.\tailnet-spike.exe serve`.
    **Note exactly what Defender Firewall shows** (screenshot welcome), which boxes
    are ticked, and the output of `Get-NetConnectionProfile` (the Tailscale interface's
    category). Leave `serve` running for T2.

### macOS 13 or later

M1. Unzip `spikes-macos`. The app is ad-hoc signed, so clear quarantine:
    `xattr -dr com.apple.quarantine ferry-clipboard-mac-spike.app`

M2. Launch with logs to a file:
    `open --stdout /tmp/ferry-spike.log --stderr /tmp/ferry-spike.log ferry-clipboard-mac-spike.app`
    and `tail -f /tmp/ferry-spike.log` in Terminal. There's no Dock icon by design.

M3. First launch should show the Accessibility prompt. Grant it in System Settings,
    quit with `pkill -f clipboard-mac-spike`, launch again (M2). The log should say
    `accessibility trusted=true`.

M4. Capture in TextEdit (plain and rich) and Safari (text, formatted text, Copy Image).
    Then a password from your password manager (expect `skipped:`).

M5. Click into TextEdit, **Cmd+Shift+V**, **Enter** (rich) and **Shift+Enter**
    (plain). Then into Safari (a text field).

M6. **Ctrl+Opt+Cmd+V** in TextEdit pastes the current clipboard as plain text (a test
    stand-in for a global paste-plain shortcut; the log shows how long it waited for
    you to let go of the keys).

M7. Tailnet: `chmod +x tailnet-spike && ./tailnet-spike status` (uses the CLI inside
    `/Applications/Tailscale.app`), and `./tailnet-spike serve`.

### Ubuntu 24.04 (GNOME 46) and 26.04 (GNOME 50)

G1. Unzip `spikes-linux`. If WebKitGTK 4.1 is missing:
    `sudo apt install libwebkit2gtk-4.1-0`.

G2. Install the extension, then log out and back in (Wayland only picks up new
    extensions at login):

    cp -r ferry@moorew.github.io ~/.local/share/gnome-shell/extensions/
    glib-compile-schemas ~/.local/share/gnome-shell/extensions/ferry@moorew.github.io/schemas

    After logging in: `gnome-extensions enable ferry@moorew.github.io`

G3. Allow the spike client, then run it:

    gsettings --schemadir ~/.local/share/gnome-shell/extensions/ferry@moorew.github.io/schemas \
      set org.gnome.shell.extensions.ferry allowed-clients "['$(realpath ./gnome-client-spike)']"
    chmod +x gnome-client-spike && ./gnome-client-spike

    In a second terminal: `journalctl --user -f -o cat /usr/bin/gnome-shell | grep Ferry`

G4. Capture in Text Editor, Firefox (text, a formatted paragraph, Copy Image) and
    Ptyxis. A password from KeePassXC or your manager (expect `skipped`).

G5. Paste: click into Text Editor, **Ctrl+Alt+V**, **Enter**. Then Firefox (rich into
    a contenteditable or Docs), LibreOffice Writer with **Enter** (formatted) and
    **Shift+Enter** (plain, no Paste Special dialog), and **Ptyxis** (the log should
    show `Paste ctrl-shift-v`). An image clip into Writer or GIMP. After a paste,
    Ctrl+T in the picker checks the offer is still there.

G6. Latency: press Ctrl+Alt+V about 50 times over a few minutes (Esc between). The
    journal prints `picker focused N ms after the shortcut` each time. Send me those
    lines. This also covers smoke item 5 (opens every time).

G7. Plain paste: copy formatted text, click into Writer, **Ctrl+Alt+Shift+V**.

G8. Lock and unlock (Super+L). The spike's terminal should show
    `ExtensionStateChanged`, then `re-registered after the extension came back`, and
    Ctrl+Alt+V should work again. Copy something while locked if you can (it should
    not be captured).

G9. Tailnet as a normal user: `./tailnet-spike --localapi status`,
    `./tailnet-spike status`, then `./tailnet-spike --localapi serve`.

### Two machines (tailnet)

T1. With `serve` running on machine A, on machine B run
    `tailnet-spike hello <A's MagicDNS short name>` (add `--localapi` on Linux if you
    like). Expect `hello from ...` and `SameUser`, and A's terminal to log the inbound
    whois decision. Then the other direction.

T2. Fixtures: on two machines, `tailnet-spike fixture <a peer's 100.x IP>` writes
    `fixtures/<host>-status.json` and `-whois.json` with keys, names, IPs and logins
    replaced. Please skim them before sending.

## What I need from you

1. The runs above on Windows, macOS, Ubuntu 24.04 and Ubuntu 26.04, with their logs.
2. Whether your tailnet policy is allow-all (it decides whether the README needs the
   first grant in SPEC 8.6).
3. The fixtures from T2. I also generated a redacted pair from `venus-wsl` while
   testing; say if you're happy for that to go into the repo as a fixture too.
4. Your call on D-009 to D-012.

Note: the extension spike uses the production UUID, so don't install `extension/`
(the empty production skeleton) at the same time.
