# Decisions

Deviations from SPEC.md and findings that change it, newest first. Each entry says
whether it is **proposed** (needs the owner's approval before SPEC.md changes),
**applied** (a default we picked within the spec's latitude) or **landed** (agreed and
written into SPEC.md).

---

## D-013 · Renamed Ferry to TailCopy · landed

The owner renamed the product because another clipboard manager is already called
Ferry. Every SPEC 0.2 identifier changed with it: app id `io.github.moorew.tailcopy`,
binaries `tailcopy` and `tailcopy-relay`, crates `tailcopy-*`, capability
`moorew.github.io/cap/tailcopy`, relay tag `tag:tailcopy-relay`, extension UUID
`tailcopy@moorew.github.io`, D-Bus `org.gnome.Shell.Extensions.TailCopy` at
`/org/gnome/Shell/Extensions/TailCopy`, GSettings `org.gnome.shell.extensions.tailcopy`.
The sync port stays 7441. SPEC.md is updated, and the GitHub repository is now
`moorew/tailcopy` (GitHub redirects the old `moorew/ferry` URLs).

## D-012 · Windows multi-format writes go through clipboard-win · proposed

**Finding.** `clipboard-rs` 0.3.5 `Clipboard::set` on Windows handles an image by
calling `set_image`, which opens the clipboard again, empties it and closes it on
return. Formats before the image are wiped and formats after it fail silently
(`continue` on error), because the clipboard is closed. A clip with an image plus HTML
or text therefore loses formats. Found by reading the source
(`src/platform/win.rs`, `set` and `set_image`); the spike can show it on hardware
(`TAILCOPY_WRITER_CLIPBOARD_RS=1`).

**Proposal.** SPEC 3: keep `clipboard-rs` for watching and reading on Windows, and
write through `clipboard-win` (already its dependency) in one
open/empty/set/close session: Unicode text, CF_HTML via `set_html_with`, RTF, the
`PNG` format and a DIB via `set_bitmap_with`. The spike does exactly this.

## D-011 · Retry Register after ExtensionStateChanged · proposed

**Finding.** When the extension is disabled (which a screen lock does), GNOME Shell
emits `ExtensionStateChanged` with `state = 1` (ACTIVE) once more, immediately before
`7` (DEACTIVATING) and `2` (INACTIVE). A client that calls `Register` on every ACTIVE
gets "Object does not exist" for that first one. The real ACTIVE after re-enabling
follows and succeeds. Seen on GNOME 46 and 50 in the headless test.

**Proposal.** SPEC 9.4.7: "calls `Register` again" becomes "calls `Register` again,
treating failure as retryable (the shell can report ACTIVE just before disabling)".

## D-010 · Modifier state on GNOME comes from `global.get_pointer()` · proposed

**Finding.** Resolves the 9.4.1 [M0 verify]. `global.get_pointer()` returns the
seat's modifier state, including keyboard modifiers: on the native backend
`clutter_seat_query_state` folds in the xkb state (mutter 46.8 and 51.1 source), and
the headless test shows Paste waiting about 500 ms and refusing while Shift is held.
`Clutter.Keymap.get_modifier_state()` only exists from GNOME 51, so it isn't an
option for 46 to 50. The mask must leave out Lock (Caps Lock) and Mod2 (Num Lock), or
Paste would always refuse with Num Lock on.

**Proposal.** SPEC 9.4.1: drop the [M0 verify] and add "count Shift, Control, Alt
(Mod1), Super/Mod4, Hyper and Meta; ignore Caps Lock and Num Lock".

## D-009 · Tailscale JSON details · proposed

Measured against Tailscale 1.102.4 (Linux).

1. **Nulls.** `whois` returns `"CapMap": null` for peers without capabilities, and
   other collections can be `null`. `#[serde(default)]` doesn't accept an explicit
   `null`; the parser needs a null-as-default helper. SPEC 8.1 "Parse JSON leniently"
   should say "treat `null` as empty".
2. **Path.** `CurAddr` is empty for every idle peer, including ones on the same LAN,
   and `Relay` is the peer's home DERP region whether or not it is in use. So "empty
   `CurAddr` means relayed" mislabels idle peers. `PeerRelay` exists as a string field.
   Proposal for SPEC 8.2: direct if `CurAddr` is set; "peer relay" if `PeerRelay` is
   set; "relayed via DERP <Relay>" if `Active`; otherwise "idle". TailCopy's own sessions
   keep peers active, so connected peers show their real path.
3. **Node IDs.** `status` `Self.ID` / `Peer[].ID` is the stable string ID
   (`n…CNTRL`); `whois` `Node.ID` is a number and `Node.StableID` is the string. SPEC
   6.1 should name `StableID` (status `ID`, whois `Node.StableID`) so both sides compare
   the same value.
4. **LocalAPI permissions.** `/var/run/tailscale/tailscaled.sock` is mode 0666 and
   `status` and `whois` work as a normal user on Ubuntu 24.04 (WSL). Pending the same
   check on a desktop install.

## D-008 · Wayland window titles at creation · applied

A Wayland window's title may not be known when `window-created` fires. The extension
matches the picker by PID when it is created and checks the title when it is first
`shown`. Within SPEC 9.4.1's wording; noted for the production extension.

## D-007 · GTK main thread must not wait on GetContent · applied

While TailCopy owns the clipboard through the GTK focused write, the shell's
`GetContent` asks TailCopy's own GTK main loop for the data. A synchronous D-Bus call from
that thread would deadlock. The spike makes all D-Bus calls from tokio tasks; M1 must
do the same.

## D-006 · macOS watch interval · applied

`clipboard-rs` polls `changeCount` every 500 ms by default, but
`ClipboardWatcherContext::new_with_interval` takes 250 ms, so SPEC 9.3's own 250 ms
poll isn't needed.

## D-005 · shell-version 46 to 51 · applied

GNOME 51 has shipped (51.1 is current), so `metadata.json` lists 46 to 51 per SPEC
9.4.1. The extension's API use was checked against gnome-shell 46.10, 50.6 and 51.1
and mutter 46.8, 50.6 and 51.1.

## D-004 · M0 test build workflow · applied

`.github/workflows/test-builds.yml` ("TailCopy test builds") packages the M0 code as
installers: an NSIS setup for Windows, a universal ad-hoc-signed DMG for macOS and a
`.deb` for Ubuntu that also installs the GNOME extension system-wide. Each bundles
the `tailcopy-tailnet` test tool. The installers go on a draft release,
`m0-test-build`. It is separate from CI (SPEC 13 keeps spikes out of CI) and is
replaced by `release.yml` in M3. Tauri names the Debian package `tail-copy` (from the
product name, with no override); M3 packaging should fix that to `tailcopy`.

## D-003 · eslint-config-gnome from git · applied

GNOME Shell's eslint rules (`eslint-config-gnome`) aren't published to npm. The
extension pins the same git commit gnome-shell 51.1 uses
(`c479d059e8d9ea99c3b53c2ea43abf7fdb05eb51`).

## D-002 · TypeScript 6.0 · applied

TypeScript 7.0.2 is the latest release, but `svelte-check` 4.7 accepts `^5 || ^6`
and `typescript-eslint` 8.72 accepts `<6.1`. Pinned `~6.0.3` until they support 7.

## D-001 · Licence files · applied

The repo's initial `LICENSE` (MIT) became `LICENSE-MIT`, with `LICENSE-APACHE` added
for `MIT OR Apache-2.0` (SPEC 0.2). The extension carries GPL-2.0 text in
`extension/COPYING`.
