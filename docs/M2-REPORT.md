# M2 report: tailnet sync

Status as of 3 November 2026. Branch `m2`, released as **v0.2.0** before the gate's
hardware runs, by owner decision (D-025). The gate (SPEC 17) needs smoke items 8 to 11
between Windows and Ubuntu, the harness green, a two-day soak with no lost clips and
the latency budgets met. The harness is green; the rest needs your machines and is
laid out below.

## What shipped

| Area | Where | Notes |
| --- | --- | --- |
| Merge rule fix (D-018) | `tailcopy-core` | Clears name what they saw and the pins they kept; strict convergence |
| Sync store API | `tailcopy-core/src/store/sync.rs` | Backlog in `(hlc, origin, seq)` order, device names (LWW, migration 0003), Send/Receive switches, peer records |
| Image sync limit | `tailcopy-core` config, capture | `sync.images`, `sync.max_image_bytes` (10 MB); over it stays local-only |
| Tailnet adapter | `tailcopy-tailnet` | CLI and LocalAPI backends, lenient parsing, candidates, paths, whois cache, the sync `Transport`, pinned relay address |
| Protocol and engine | `tailcopy-sync` | Frames (named MessagePack, 8 MiB cap), hello/vector/devices/backlog/`CaughtUp`/live, whois on every request, one session per peer, who-dials rules, backoff, heartbeats, inline and fetched blobs |
| Simulation harness | `tailcopy-testkit` | Fake tailnet, real sessions over loopback between in-process nodes |
| Relay | `tailcopy-relay` | Minimal: LocalAPI, accepts authorized devices, never dials. Not packaged (M3) |
| App | `app/src-tauri/src/sync.rs` and others | Engine on the store actor, live mirror, missing-blob fetch on paste, tray new dot, devices and toggles commands, diagnostics and `tailcopy status` |
| UI | Settings > Tailnet, picker, History | Device list with path and Send/Receive, peers without TailCopy, refused nodes, What syncs, arrival tint and badge, synced count |

Not in M2 though the handoff lists them: the "Found your tailnet" first-run screen,
tray sync animation frames, and the "too large" row state. They need a design pass
over states that only now exist; say if you want them before M3.

## Tests

- `cargo test --workspace` (Linux, in Docker with the CI packages): all pass. Clippy
  is clean on Linux, and cross-checked clean for `x86_64-pc-windows-msvc` and
  `aarch64-apple-darwin` (stub compilers, check only).
- **Convergence property test** (SPEC 14.1) now runs without the late-pin exception;
  3,000 cases passed (`PROPTEST_CASES=3000`).
- **Simulation harness**, 11 scenarios, real sessions over loopback (skipped on macOS,
  which only routes 127.0.0.1), three clean runs in a row:
  copy reaches the other device; ops travel through a third device; a partition with
  skew of +50 s and -40 s heals and converges (identical rows, vectors and consistency
  checks); a restarted device catches up 20 clips in order with no live events, then
  gets a live one; the higher id dials when the lower one can't be reached; one session
  per pair across restarts; an image over the inline limit is fetched, and a third
  device fetches it from the second after the origin went offline; other logins and an
  untrusted tagged node never sync; a restored backup takes a new device id and its new
  ops aren't dropped; Send and Receive switches; Tailscale stopping and starting.
- **Frames:** every variant round-trips, blobs are MessagePack bin, oversized frames are
  refused both ways, an old op without the new clear fields still decodes.
- **Tailnet adapter:** 23 tests on fabricated fixtures (D-024 item 5), plus a live run on
  `venus-wsl` through both backends.
- **Live:** the relay on `venus-wsl` listened only on the Tailscale IPv4 and IPv6
  addresses and answered an authorized `/v1/hello` over the tailnet.
- **UI:** typecheck, lint and token check pass; Settings > Tailnet and the picker's
  arrival tint checked in light and dark against the mock backend.

Not tested: the desktop app syncing between two real machines. That's the gate's job
and the steps are below.

## Measurements

| Metric | Result | Budget |
| --- | --- | --- |
| Copy on A to available on B, inline clip, loopback | p95 62 ms over 20 copies, including the test's 50 ms polling | 300 ms p95, direct path |
| Op sizes on the wire and in the log | `ClipAdded` 458 B on average (max 793), stub 221 B, `ClipUsed` 128 B, `PinSet` 134 B | SPEC 6.5 guessed about 100 B |
| Log growth | About 50 copies and 20 pastes a day comes to roughly 13 KB a day, 5 MB a year per device, before SQLite overhead | "a few MB per device per year" |

Stubs are about twice SPEC 6.5's guess because ops carry field names and UUID strings.
Still small; worth revisiting only if logs grow faster in the soak.

Real-network latency, idle CPU and memory with sync on need your machines (below).

## Open issues

- **D-024** (proposed): Tailscale details found while building the adapter.
- **Windows Firewall** (SPEC 8.3, M0 item 4 still open): binding the listener may show a
  Firewall prompt. If it's refused, Windows still syncs by dialling out after 10 s.
  Please note exactly what you see.
- Discovery probes every same-login tailnet machine; on a tailnet with many machines
  that don't run TailCopy that's about one refused connection each per minute, and only
  debug logging.
- Old device ids (after a restored backup) stay in the devices table and can show as
  an extra, offline device in Settings.

## Manual test steps (Windows venus ↔ Ubuntu Lenovo)

Install v0.2.0 on both from the release. Tailscale running and signed in to the same
account on both.

1. **Discovery.** Open Settings > Tailnet on each. Within about 10 s each lists the
   other under Devices running TailCopy with "Synced · direct" (or relayed). Note any
   Windows Firewall prompt (screenshot, which boxes were ticked, and the output of
   `Get-NetConnectionProfile` in PowerShell).
2. **Smoke 8: live.** Copy a line in Notepad on Windows. On Ubuntu the picker shows it
   at the top with "From venus", and Ctrl+V in Text Editor pastes it (live mirror).
   Then the other way round. Roughly how long did it take?
3. **Smoke 9: sleep.** Copy something on the Lenovo, then suspend it. Copy 20
   numbered lines on Windows. Wake the Lenovo: all 20 appear in its picker in order,
   and Ctrl+V still pastes what you copied there before suspending (catch-up never
   touches the clipboard).
4. **Smoke 10: Tailscale restart.** `sudo systemctl restart tailscaled` on the Lenovo
   (or quit and reopen Tailscale on Windows). Copy something on the other machine
   within a minute; it arrives without doing anything else.
5. **Smoke 11: delete and pin.** Delete a clip on Windows: gone on Ubuntu. Pin one on
   Ubuntu: pinned on Windows.
6. **Images.** Copy a screenshot on one; the thumbnail appears on the other, and
   pasting it from the picker works. A clip that's still arriving pastes after a short
   wait or says "Not available yet (source offline)".
7. **Switches.** Turn Receive off for venus on the Lenovo; copy on Windows: nothing new
   shows on the Lenovo. Turn it back on.
8. **Soak.** Use both normally for two days. Then send me Copy diagnostics from each
   and both logs (Settings > About > Open log folder). Logs never contain clip content.
   Anything missing on one side that's on the other is a bug I want to hear about, with
   roughly when it was copied.

If you leave `venus-wsl` running `tailcopy-relay` (I can start it), it joins as a third
peer, and you can test catch-up through the relay with both desktops taking turns
being off.

## What I need from you

1. The runs above, with diagnostics and logs from both machines.
2. Your call on D-024.
3. Whether the first-run "Found your tailnet" screen and tray sync frames should come
   before M3.
