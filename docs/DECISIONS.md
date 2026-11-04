# Decisions

Deviations from SPEC.md and findings that change it, newest first. Each entry says
whether it is **proposed** (needs the owner's approval before SPEC.md changes),
**applied** (a default we picked within the spec's latitude) or **landed** (agreed and
written into SPEC.md).

---

## D-025 · v0.2.0 ships sync before the M2 gate · landed

Owner decision (3 November): release tailnet sync as **v0.2.0**, a full release like
v0.1.0, before the M2 gate's hardware runs (smoke items 8 to 11 between Windows and
Ubuntu, and the two-day soak). Those runs happen on the released build. The relay
binary exists (`tailcopy-relay`, SPEC 12 flags) but isn't packaged or released; that
stays in M3.

## D-024 · Tailscale details found while building the adapter · proposed

1. **Offline peers have no path.** Status can report `Online: false` with
   `Active: true`; D-009's rule would call that "relayed via DERP". The Devices view
   shows offline peers as offline and only gives a path for online ones. Proposal: add
   that to D-009 rule 2 and SPEC 8.2.
2. **Nulls inside maps.** A `CapMap` capability's value can itself be `null`; an
   untagged `Self` has no `Tags` key at all. Both parse as empty.
3. **Tagged nodes share a placeholder user.** Every tagged node reports the same
   "tagged-devices" user, so a tagged device has no login of its own and the same-user
   rule can't match through it. SPEC 8.5 rule 1 should say a tagged *self* never
   matches either.
4. **Windows CLI path.** The adapter tries `C:\Program Files\Tailscale\tailscale.exe`
   before `PATH` (the installer doesn't always add it to `PATH` for the user). SPEC 8.1
   says `PATH` only.
5. **Fixtures are fabricated.** SPEC 14.1 asks for redacted fixtures from the owner's
   machines; the committed ones are hand-written in the shape of real output with every
   value made up. Real Windows and macOS ones would still help.
6. **`PeerRelay` format unverified.** No peer on the test tailnet uses a peer relay, so
   the format the Devices view shows for it is a guess.

## D-023 · What Send and Receive mean · applied

The per-device switches (D-021, item 7) sit on top of full anti-entropy (SPEC 7.4),
which needs every device to hold a gapless log from every origin. So neither switch
withholds ops; they withhold content:

- **Receive off for device X:** `ClipAdded` ops that X made are stored as stubs on
  arrival (SPEC 6.5), whichever peer they come from. The log stays complete, pins and
  deletes still apply, nothing is fetched or mirrored, and the clip never shows.
- **Send off for device X:** this device sends X a stub of each clip it made, and
  refuses X's blob requests. Clips from other origins pass as usual.
- Both apply to new clips only, and a stub is final for that op (SPEC 6.5), so turning a
  switch back on doesn't bring old clips across.
- Receive off is a firm guarantee; Send off isn't, because a third device or a relay
  with both switches on can still forward the clip to X. Settings says so in one line.

Also chosen within the spec's latitude, in `tailcopy-sync`:

- Devices dial each other from their own Tailscale address, so the peer's whois sees
  the right node. Blob fetches use the same source address.
- A peer that dialled us is reached for blobs on the configured sync port (SPEC 0.2
  makes it the same everywhere).
- Live ops for the mirror are `ClipAdded` and `ClipUsed` from another origin, after the
  peer's `CaughtUp`, within 10 s of local time; the newest one in a batch wins.
- Discovery probes every candidate with exponential backoff to 60 s, so a tailnet with
  many machines that don't run TailCopy costs about one refused connection per machine
  per minute.
- The image sync limit is `sync.images` and `sync.max_image_bytes` (default 10 MB);
  over it, an image is kept local-only like an over-cap clip.

## D-022 · v0.1.0 is a public release, before sync · landed

Owner decision (3 November): tag the M1 app with the redesign (D-021) as **v0.1.0**, a
full GitHub release with notes, screenshots and a README, and hide the pre-release
test builds. SPEC 17 puts releases and the README in M3; this pulls a first one
forward. What the v0.1.0 release has, against SPEC 13:

- `release.yml` runs on `v*` tags and publishes straight away (not a draft), with the
  same build steps the M1 test builds used rather than `tauri-action`, plus a
  `SHA256SUMS` file. Notes come from `docs/releases/<tag>.md`.
- Not yet: the Windows `.msi`, the signed updater manifest, relay binaries and image,
  and the extension zip for EGO. They stay in M3.
- `builds.yml` (the pre-release workflow) is gone; `m0-test-build` and `m1-test-build`
  are turned into drafts, which hides them without deleting them.

## D-021 · The "Lilac utility" redesign changes SPEC 10 · landed

Owner decision (3 November): every recommendation below accepted; logo Option A;
items arriving from other devices never toast (tray dot only). SPEC 10.1 to 10.5 and
M2's scope updated.

The design handoff (`design/HANDOFF.md`, 3 November) replaces the look in SPEC 10.4 and
D-016 and touches the keyboard map and picker. Recon: `docs/DESIGN-PHASE0.md`.
Changes the spec needs, with what I recommend where the two disagree:

1. **Look (10.4, D-016):** lilac/aubergine palette, one violet accent, six device
   colours, Bricolage Grotesque, Figtree and JetBrains Mono, logo Option A. Supersedes
   D-016's black and white and Commissioner. Apply as designed.
2. **Picker window (10.1, 10.4):** 960 × 640, resizable, remembers its size, 46 px
   rows, radius 22, 140 ms open, instead of 640 × 420 fixed, 36 px rows, radius 12,
   80 to 120 ms. Apply as designed, unless you want it smaller than the manager.
3. **Keyboard (10.3):** the handoff's map differs from the spec in four places.
   Recommend keeping the spec on three of them:
   - Picker shortcut **Ctrl+1**: browsers, editors and terminals all use Ctrl+1 to
     switch tabs, so a global binding takes that over everywhere. Keep Ctrl+Alt+V /
     Cmd+Shift+V.
   - Paste a row with bare **1 to 9**: focus sits in the search field, so a digit has
     to type a digit. Keep Ctrl/Cmd+1 to 9.
   - **Delete** to delete: Delete deletes forward in the search field. Keep
     Ctrl/Cmd+Shift+Backspace.
   - **Esc clears the search, then closes**: adopt.
4. **"Pasted into {app}" toast:** the picker hides before pasting, so a success toast
   needs an OS notification (SPEC 1.1 allows those only when something is wrong) or a
   new overlay window. Recommend no success toast; keep notifications for failures
   ("Copied. Press Ctrl V" when paste can't happen). In-window toasts in the manager
   (Delete with Undo, Copied) are fine.
5. **Tray "new" dot and the arrival badge (10.4 "Never: badges"):** the spec's ban
   was aimed at nags. A violet dot that clears when the picker opens is the quiet sync
   signal the design is built around. Recommend allowing it and rewording 10.4 to ban
   promotional badges.
6. **"Password skipped" rows:** SPEC 9.7 says secrets are never stored. Even a
   contentless row records that you copied a password, from which app and when.
   Recommend dropping this row state.
7. **Per-device Send and Receive toggles, and an image sync limit (default 10 MB):**
   new sync behaviour not in SPEC 7 or 11. Both fit M2. Recommend adding them to M2's
   scope; the sync limit would make over-limit images local-only (6.3).
8. **Extra kinds (code, link, colour):** derived for display and filtering only; the
   stored and synced kinds stay text, rich and image. "File" is out of scope (2.3).
9. **Library sidebar (10.5):** adds Code and Links to All, Pinned, Images, Formatted.
   Apply.

## D-020 · GNOME capture waits for the copy to settle, and fails closed on the hint · applied

Found in the end-to-end test of the installed app on GNOME 46 and 50:

- One copy in another app can arrive as several `owner-changed` events, and the first
  can list fewer types than the last (a password manager's
  `x-kde-passwordManagerHint` arrived in the second, 12 ms later). TailCopy now waits
  150 ms after a change and reads only the last one. This also stops one copy being
  stored twice. It stays well inside the 300 ms budget for a copy to appear.
- When the hint is offered but its value can't be read (an empty transfer), TailCopy
  treats the copy as secret. SPEC 9.4.3 says to skip when the value is `secret`; this
  skips in that case too, so a broken or slow source never leaks a password into the
  history.

## D-019 · Core data layer choices · applied

Choices made where SPEC 6 is silent, in `tailcopy-core`:

- Purged tombstones (after 30 days) are the one allowed difference between the
  `clips` table and a full recomputation from `ops`; the consistency check knows this.
- The version vector stores the highest *contiguous* seq per origin. Same as SPEC 6.1
  under gapless delivery, and still correct if ops arrive out of order.
- A label on a grouped row applies to every member, so a new copy of the same content
  keeps it.
- Clears from other devices also remove local-only clips, so "clear history" clears
  everything.
- Plain text derived from HTML or RTF is stored as a `text/plain` format, so plain
  paste works for it on every device. `source_app` stays local unless sync is opted in.
- `before_hlc` for a clear is the larger of the local HLC and the newest `ClipAdded`
  held, so clips stamped ahead by a skewed clock are cleared too.
- Retention counts clips, not display rows. A bad config value is an error, shown in
  the UI, and the app falls back to defaults. `from:` is a case-insensitive prefix match
  on display name or Tailscale hostname. `today`/`yesterday` use the last-used time and
  local midnight. The database file is `tailcopy.sqlite3`.
- Measured: search over 10,000 clips, p95 5.9 ms (budget 16 ms); building the warm
  index for 10,000 rows takes about 195 ms, once at startup.

## D-018 · A late pin can resurrect a cleared clip · landed

**Owner decision (3 November): apply the fix.** As built, `HistoryCleared` carries two
new fields: `spared` (the pinned clips the clear kept) and `seen` (the clearing
device's version vector). A clip is deleted by a clear only if the clearing device had
seen it (`seq <= seen[origin]`), it was created at or before `before_hlc`, and it isn't
in `spared` (unless `include_pinned`). `seen` goes one step past the proposal below:
with `spared` alone, a clip pinned on an offline laptop that the clearing device never
saw would be cleared. Clears written before this (no `seen`) keep the old rule, so
existing local data materializes the same. The convergence property test no longer
needs its late-pin exception. SPEC 6.3 and 6.4 rule 4 updated.

**Finding.** SPEC 6.4 rule 4 decides whether a `HistoryCleared` spared a clip from its
pin state "as of `C.hlc`". If device A pins a clip at hlc 5 while offline and device B
clears without `include_pinned` at hlc 10, B deletes the clip and stubs its content;
when A's pin arrives later, rule 4 says the clip was never deleted. Nodes that already
stubbed it keep it hidden (content is gone), so A shows the clip and B doesn't. This
breaks convergence and is the only case where "no deleted clip ever reappears" and rule
4 disagree. The current code follows rule 4 as written; a test documents the case.

**Proposal.** `HistoryCleared` carries the ids of the pinned clips it spared, computed by
the clearing device when it clears. Rule 4 becomes "deleted if some clear doesn't list
it in `spared`". A later pin can't change the outcome, every node agrees, and the op
stays small (pinned clips are few). It is a wire format change, so it should land before
M2 ships sync.

## D-017 · No commands after install · landed

Owner decision: on every platform, installing and clicking through first run is the
whole setup. The installers put everything in place (the `.deb` installs the GNOME
extension and its schema system-wide). First run enables the extension over D-Bus,
offers a Log out now button when GNOME hasn't loaded it yet, asks for macOS
Accessibility with a button that opens the right System Settings pane, and handles
start at login. macOS builds are signed and notarized with the owner's Developer ID,
so no quarantine command. Unsigned Windows installers keep the SmartScreen "Run
anyway" click, which the owner accepts. The tailnet test tool leaves the installers;
its job moves to the Devices view in M2. SPEC 9.4.6 and 17 updated.

## D-016 · Commissioner, black and white · superseded by D-021

Owner decision: the app UI and the wordmark use Commissioner (Google Fonts, SIL Open
Font License), bundled with the app so nothing is fetched at runtime (SPEC 16). The
icon and UI are black and white with neutral greys; colour only signals state. The
icon is an original design (the Noun Project reference needs a licence or credit).
SPEC 10.4 updated.

## D-015 · Strict "my login only" sync · landed

Owner decision: a `sync.strict_same_login` setting, on by default, limits sync to
untagged devices signed in to the same Tailscale login and refuses the app-capability
and relay-tag paths. Turning it off is needed only for a tagged relay. The owner's
tailnet policy is allow-all, so the README doesn't need the first grant in SPEC 8.6;
other users' devices on an allow-all tailnet are refused by the identity check. SPEC
8.5, 11 and 12 updated.

## D-014 · M1 covers macOS · landed

Owner decision: M1 builds the GUI app on Windows, Ubuntu GNOME and macOS together,
before the M0 hardware tests, which now run on the M1 builds. macOS moves from M3 to
M1. SPEC 17 updated.

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
the `tailcopy-tailnet` test tool. The installers are published as the
pre-release `m0-test-build`, marked as an unsigned test build. It is separate from CI (SPEC 13 keeps spikes out of CI) and is
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
