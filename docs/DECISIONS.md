# Decisions

Deviations from SPEC.md and findings that change it, newest first. Each entry says
whether it is **proposed** (needs the owner's approval before SPEC.md changes),
**applied** (a default we picked within the spec's latitude) or **landed** (agreed and
written into SPEC.md).

---

## D-036 · No app stores: an APK on GitHub Releases, iOS built from source · landed

Owner decision (8 November): the phone apps don't go to the App Store or Google Play.
Android users get a signed APK from GitHub Releases, which Obtainium and similar
tools update from; iPhone users who want it build it themselves. SPEC 21.7, 21.8 and
the M5 and M6 scopes change:

- **One version.** The phone takes the desktop's version from
  `app/src-tauri/tauri.conf.json`; `versionCode` is `major × 10000 + minor × 100 +
  patch`. Obtainium compares the release tag with the installed `versionName`, so
  the release job fails if they differ.
- **Every release has the APK**, `TailCopy-<version>-android-arm64.apk`, so the
  latest release always has one. The job requires the release key, refuses a
  debug-signed APK, and appends the signing certificate's SHA-256 to the notes.
- **arm64 only.** Phones are arm64; dropping x86_64 (emulators) roughly halves the
  APK. `TAILCOPY_ANDROID_ABIS=arm64-v8a,x86_64` builds both.
- **iOS from source.** `TAILCOPY_IOS_ID` swaps in the builder's own bundle id (ours is
  registered to the owner's team); the App Group follows as `group.<id>`. The share and
  action extensions now derive the group from their own bundle id instead of reading
  it from Info.plist. Apple's capabilities table lists App Groups for free personal
  teams, so a free Apple ID works, with its limits: the app must be reinstalled every
  7 days, at most 3 such apps per device, and 10 new app IDs a week (TailCopy uses 4).
  `docs/BUILD-IOS.md` has the steps.
- **Not needed any more:** App Store Connect keys, TestFlight, Play Console, store
  privacy forms. The release key matters more: with no store holding a copy, losing it
  strands every Android install.

## D-035 · Pairing status, link opening, picker settings button, phone icon · landed

Owner requests after the first phone pairing test (8 November):

- **Pairing status.** The phone's pairing screen no longer closes as soon as the code is
  accepted: it follows the connection (trusted, connecting, connected, syncing, up to
  date) and, after 15 seconds without a connection, says what to check. The desktop's
  Pair a phone panel numbers the steps and shows the phone arriving. SPEC 21.5.
- **Pairing instructions** are spelled out as three numbered steps on both sides.
- **How saving works** on the phone: one list of the ways in for the platform, in
  onboarding and again from Settings › Capture. It's a single page shown as part of
  onboarding's existing capture step, not a carousel, so 10.4's "no onboarding
  carousels" still holds.
- **Picker settings button:** a quiet cog at the end of the search bar. SPEC 10.2.
- **Opening links:** Ctrl+click or Ctrl+Enter (Cmd on macOS) opens a link item; the
  manager's detail pane has Open link. Only `http`, `https` and `mailto` open, through
  `tauri-plugin-opener` from the clip's stored text, so a clip can't launch a file or
  another scheme. SPEC 10.2, 10.3.
- **Phone app icon:** violet field, white mark (was the colour mark on lilac paper).
  The desktop icons are unchanged. SPEC 21.5.

## D-034 · npm overrides for vulnerable phone build tooling · applied

Dependabot flagged 16 advisories in `mobile/package-lock.json` (7 November). All are in
build and test tooling (Expo prebuild, `@bacons/apple-targets`, jest, eslint); none ship
in the app bundle. `mobile/package.json` now forces patched releases with npm
`overrides`, keyed by version so that older majors on separate code paths stay put
(glob 7 and minimatch 3 have different APIs and are outside the advisories):

- `@xmldom/xmldom` 0.7.x → 0.8.15 (pulled in by `@bacons/xcode` through `@expo/plist` 0.0.18)
- `uuid` 8 → 11.1.1 (`@bacons/xcode`)
- `glob` 10/13 → 14.0.x and `minimatch` 9/10 → 11.0.x (Expo CLI, jest, typescript-eslint)

Verified with a clean `npm ci`, the mobile checks and an `expo prebuild` for both
platforms (all three iOS extension targets generated, plists valid). Drop each override
once upstream depends on a fixed release.

`deepmerge` (GHSA-gcqx-w48m-w2h4, low) has no fixed release. It only merges our own
config files (jest-config, `@expo/config`, `@sveltejs/vite-plugin-svelte`), never
untrusted input, so the alert is dismissed as tolerable risk.

## D-033 · Phone app build choices (M5) · applied

Choices within SPEC 21's latitude while building M5 (6 November):

- **One system symbol.** iOS Control Center controls can only show SF Symbols or a
  custom symbol exported from the SF Symbols app. The Save clipboard control uses the
  system `list.clipboard` symbol until the brand `paste` icon is exported as a custom
  symbol (needs a Mac). Everything else, including the iOS tab bar, uses the brand set,
  compiled into the asset catalog from `design/assets/icons`.
- **New design assets:** `phone` and `scan` icons (same grid and stroke), `qr-ink` and
  `qr-paper` tokens for pairing codes (dark on light in both themes), and phone app
  icons rendered from the mark by `mobile/scripts/app-icons.mjs`.
- **Intents open the app.** "Save clipboard" and "Copy latest" run through
  `tailcopy://capture` and `tailcopy://copy-latest`, because an intent writing the
  pasteboard in the background is unverified. "Copy latest from <device>" and "Copy
  pinned <label>" move to M6 with the widget.
- **Bindings:** uniffi 0.32 Swift and Kotlin bindings behind a local Expo Module (no
  uniffi-bindgen-react-native, which still says it isn't for production).
- **Android minimum is API 29**, the first release with today's clipboard rules.
- **No Expo cloud services:** builds are local, in `mobile/docker/android.Dockerfile`,
  or in the Mobile workflow; `EXPO_NO_TELEMETRY=1` everywhere.

## D-032 · Check macOS capture against the new pasteboard read alerts · proposed

Found while researching mobile (6 November): Apple announced in May 2025 that macOS
alerts the user when an app reads the pasteboard programmatically, and added
`NSPasteboard` `detect` methods and an `accessBehavior` property. SPEC 9.3's capture path
may trigger that alert on macOS 26/27. Proposed: test v0.2.x on current macOS, and if it
prompts, change SPEC 9.3 to check `changeCount` and types before reading and document the
permission in first run.

## D-031 · Phone apps (iOS and Android) · landed

Owner request and decisions (6 November): native iOS and Android apps in React Native,
designed per platform with integrations; **the phone rides the official Tailscale app and
trusts peers through QR pairing**; **Lilac colours only** (no Material You dynamic
colour); **straight to the full build, no spike milestone**. Brand icons only (the
owner didn't take up the SF Symbols exception; revisit if a system control needs one).
Research: `docs/MOBILE-RESEARCH.md`. SPEC now has section 21, milestones M5 and M6, and
these changes:

- **2.1, 2.3:** phones are a platform group gated in M5 and M6; "Phone apps" is no longer
  out of scope. Embedded Tailscale stays out, on phones too.
- **0.2:** phone app id, iOS App Group and the `tailcopy` URL scheme.
- **6.2:** `role` accepts `mobile` (schema 4), and a `peer_keys` table.
- **7.2, 7.3:** proto minor 1: `key` in `/v1/hello`; `key`, `challenge` and `proof` in
  `Hello`; a `Directory` frame for phones. Nodes never send role `mobile` to minor-0 peers.
- **7.7, 8.5:** phones always dial and are never dialled; they verify peers by pinned
  identity keys instead of whois. Desktops authorize phones by whois as before.
- **9.7:** "secrets are never stored" is best effort on iOS (no documented marker).

**Change from the research proposal:** the QR code carries no one-time secret. Desktops
already authenticate the phone by whois (rule 1), so pairing only needs to carry the
desktop's public key to the phone over a channel nobody else controls (the camera).
Pairing is one-way and the desktop stores nothing new for the phone.

## D-030 · X11 backend (tier 2) · applied

Owner decision (6 November): build the X11 backend now, because the owner's Zorin OS 17
desktop (GNOME 43, X11 on NVIDIA) showed "This desktop isn't supported yet" and no
shortcut worked. GNOME 43 can't load the extension either (the extension format changed
in GNOME 45), so the GNOME path can't serve that machine. SPEC 9.5 now describes the
backend. Choices within the spec's latitude:

- **XTest through `x11rb` instead of `enigo` for the paste keystroke.** enigo 0.6
  maps `Key::Unicode('v')` onto a spare keycode, which makes every X client reload its
  keymap, and some apps match shortcuts by keycode. TailCopy looks up the keycodes for
  `Control_L`, `Shift_L`, `v` and `Insert` in the current keymap (first group first)
  and sends them with XTest, like `xdotool key`. enigo stays on Windows and macOS.
- **Focus with EWMH.** The previous window is `_NET_ACTIVE_WINDOW` when the shortcut
  fires. Activation requests carry a real server timestamp (from a property change on a
  scratch window), because window managers refuse a timestamp-less request once there
  has been keyboard input. Each request ends with a round trip, because closing the
  connection straight after a request can drop it. Both were found by the Xvfb smoke
  test (`crates/tailcopy-platform/tests/x11_smoke.rs`), not guessed.
- **Writes reuse the GNOME backend's GTK multi-format offer** (now in
  `tailcopy_platform::linux`), which on X11 works without focus.
- **Source app:** `_GTK_APPLICATION_ID`, then `WM_CLASS`, then the process name, so the
  Linux paste-keystroke defaults (desktop ids such as `org.gnome.Terminal`) keep working.
- **Detection:** X11 only when `DISPLAY` is set and `WAYLAND_DISPLAY` isn't. A Wayland
  desktop other than GNOME stays degraded, and the banner now says which desktop and
  what is supported.
- **Shortcut conflicts are reported.** When the global shortcut plugin can't register a
  shortcut (on any platform), TailCopy now shows a notification instead of only logging.
- **First-show centring.** GTK reports no size for a never-shown window, so the picker
  opened off centre the first time; it now falls back to the saved picker size.

Tested in Docker (Xvfb, openbox) with the release build: capture, the picker shortcut,
Enter pasting into the previous window, and the plain-paste shortcut. Not yet tested on
real hardware or under mutter on X11.

## D-029 · A device is named after its tailnet machine name · applied

Found 6 November: the owner's `blackbox` showed up in Devices as "sempa". Its node is
called `blackbox` on the tailnet (MagicDNS), but tailscaled reports the OS hostname
`sempa`, and an empty `general.device_name` used `Self.HostName`. The OS hostname can
differ from the name the owner sees in the admin console (a node renamed there, or
tailscaled running in a container), and two machines can share it, while the MagicDNS
label is unique. The default name is now the MagicDNS label, keeping the OS hostname's
capitalisation when the two match apart from case (`Venus` stays `Venus`). It falls back
to the OS hostname when MagicDNS is off. A device renames itself on its next start and
the new name reaches the others with the next sync. SPEC 11 now says "Tailscale machine
name".

## D-028 · A daily update check, on by default · landed

Owner decision (4 November): tell users when a new release is out, per platform.
SPEC said update checks were opt-in; the owner chose on by default so everyone hears
about fixes like 0.2.1. Once a day (first a minute after start) TailCopy asks
`api.github.com/repos/moorew/tailcopy/releases/latest`, with only a `TailCopy/<version>`
user agent. If the latest non-prerelease is newer, it notifies once, adds "Download
TailCopy x.y.z…" to the top of the tray menu, and shows it in the sidebar and in
Settings > About. Download opens this platform's installer from the release (only
`github.com/moorew/tailcopy` URLs are accepted). It doesn't install anything itself;
signed in-app updates (the Tauri updater) stay in M3. `general.check_updates = false`,
or the switch in Settings > General, turns it off.

## D-027 · History belongs to the Tailscale login it was synced under · landed

Owner decision (4 November), from the audit's open lead: if the machine's Tailscale
login changes, sync loses access and needs approving again. The first login seen is
recorded with the history (`meta.sync_login`). While the machine is signed in as a
different one, whois treats nobody as "same user", open sessions end, and TailCopy
says so with a notification and a Settings > Tailnet banner ("Allow syncing as …").
Approving records the new login. Switching back to the recorded login resumes on its
own. The history stays on the device either way. The relay has no guard (it is tagged
and has no login).

## D-026 · Security fixes from the first audit (v0.2.1) · landed

The first security audit (4 November; source and sandboxed local runs only) confirmed
one high and seven low findings. v0.2.1 fixes them; SPEC 5.2, 6.5, 8.5 and 11 now say
so:

- **Own address (high).** Whois of this node's own Tailscale address returns this node,
  which passed "same user", so any local process could sync the full history. The
  server now refuses connections from its own addresses before whois.
- **Send and Receive follow the tailnet node.** The switches were keyed on the device
  id a peer claims, so a new id (a restored backup, a cloned profile, a modified
  client) escaped them. A new id from a node inherits that node's switches, changing a
  switch changes it for every id from that node, and Send, Receive and blob requests
  are all checked against the node.
- **Strict mode** now ends open sessions when it is turned on.
- **Blob size.** A fetched blob is capped at the smaller of its declared size and
  `max_fetch_bytes` while it downloads, and `put_blob` refuses one larger than declared.
- **Stubs and tombstones** no longer keep BLAKE3 of the deleted text; older stubs are
  rewritten once at startup and on arrival from older peers.
- **CLI socket** is owner-only (see SPEC 5.2). On Linux it was an abstract socket any
  local user could reach or claim first.
- **Unreadable config.toml** fails safe instead of falling back to defaults silently.

Also from the audit's leads: a navigation guard keeps both windows on TailCopy's own
pages; local clipboard changes count even while paused, so a live mirror can't
overwrite them; the Windows `CF_BITMAP` uses a 40-byte header; CI and release pin
every action to a commit SHA with read-only tokens, and nothing is built or signed
unless the tag is on `main`.

Still open, for the owner: whether a Tailscale **login or tailnet switch** should hold
back history captured under the previous login (today it syncs to the new login's
devices), and the remaining leads that need a real Windows, macOS or GNOME machine.

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
