# M5 report: phone apps v1 (in progress, before the gate)

Branch `mobile`. Spec: SPEC section 21, milestones M5 and M6, D-031 (owner decisions),
D-033 (build choices). Research: `docs/MOBILE-RESEARCH.md`.

The M5 gate needs hardware: an iPhone and an Android phone against a Windows or Ubuntu
desktop and a relay (SPEC 21.9). Nothing here has run on a phone yet.

## What was built

**Protocol and desktop (Rust, proto 1.1)**
- Role `mobile` (schema 4) and the `peer_keys` table.
- Desktops and relays hold an Ed25519 identity key (`identity.key`, mode 0600) and
  publish it in `/v1/hello` and `Hello`.
- A phone sends a challenge and checks the listener's signature against the key it
  paired with. Desktops still check the phone with whois.
- A desktop or relay sends a phone a `Directory` of the other desktops and relays it
  knows, so pairing once is enough.
- Nobody dials a phone. Tailscale peers on iOS and Android are no longer candidates,
  and blobs are never fetched from a phone.
- Devices frames say `device` instead of `mobile` to builds that predate it.
- Desktop: Settings > Tailnet > Phones > **Pair a phone** shows the QR code.
- Relay: `tailcopy-relay pair --data-dir DIR` prints it in the terminal.

**Phone core (`crates/tailcopy-mobile`)**
- One uniffi object holds the store actor (search index kept warm) and the dial-only
  sync engine.
- It handles capture (sensitive clips are never stored), the iOS extension inbox, search,
  content (with blob fetch), pin, label, delete and clear, devices, pairing and settings.
- On iOS it closes the database before suspension. Sync jobs never reopen it, so a
  suspended app holds no SQLite lock (the `0xdead10cc` risk).

**App (`mobile/`, Expo SDK 58, React Native 0.88)**
- Screens: History, Pinned, Devices, Search, clip detail, Settings, pairing (camera or
  `tailcopy://pair` link), onboarding, and the routes behind the control and Shortcuts.
- iOS: system tab bar (Liquid Glass) with brand icons, large titles, the system paste
  control, a native context menu with preview, and swipe to pin or delete.
- Android: no bottom bar; History with search and filter chips is home.
- The native side reads and writes the clipboard with every format. JavaScript never
  sees clip bytes.
- iOS extensions: share and action extensions (write the inbox), and a widget extension
  with the Save clipboard control and two App Intents (Save clipboard, Copy latest).
- Android integrations: "Save to TailCopy" in the text-selection menu and share sheet,
  the Save clipboard tile, app shortcuts, and a 15-minute WorkManager catch-up plus an
  immediate send after a save.
- Design: tokens, icon shapes, Android drawables and the iOS asset catalog are generated
  from `design/` (`npm run design`, checked in CI). App icons are rendered from the mark.

## Verified here

- **Rust:** 154 tests pass across core, sync, tailnet, relay, testkit and mobile; clippy
  and fmt are clean. New tests cover:
  - the schema 4 migration
  - `peer_keys`
  - the proof (wrong challenge, wrong ids and wrong key all fail)
  - pairing links (round trip and refusals)
  - four phone simulations over loopback:
    - a paired phone finds the relay through the Directory and syncs both ways
    - nobody dials the phone
    - an unpaired phone reaches nobody
    - a node holding the paired address with the wrong key is refused
    - a phone on another login is refused by the desktop
  - the facade: capture, search, copy, pin, the inbox, suspend and reopen, and settings
- **Desktop app:** clippy in the `ferry-dev` Docker image, and the front-end checks.
- **Relay:** `tailcopy-relay pair` against the live tailnet on venus-wsl prints a valid
  link and QR code; `identity.key` is mode 0600.
- **Phone app:** typecheck, lint, Jest and the design check pass.
- **Android:** the release APK builds in Docker (`mobile/docker/android.Dockerfile`).
  - It passes 16 KB page alignment (`zipalign -c -P 16`), with minSdk 29 and target 36.
  - The merged manifest has the selection-menu action, share target, tile, FileProvider
    and shortcuts.
  - It ships arm64-v8a and x86_64 only.
  - It's 103 MB as a universal APK, because the native libraries are stored
    uncompressed; Play's per-ABI split roughly halves that.
  - It has never launched: no emulator here (no KVM).
- **Swift:** the uniffi Swift and Kotlin bindings generate, and the generated Kotlin
  compiles in the Android build. The Swift code itself hasn't compiled anywhere yet.

## Not verified (needs hardware or a Mac)

- Any Swift: this machine can't compile it. The Mobile workflow's iOS job builds for the
  simulator, unsigned; it will be the first compile.
- Everything in SPEC 21.9 on real phones.
- iOS paste prompts: whether checking `types` and `changeCount` stays silent, and how
  "Allow" behaves.
- `if-addrs` finding the Tailscale address on iOS (utun) and Android (VPN interface).
  If it can't, the app says Tailscale is off when it isn't. The fallback is asking the
  platform: `NWPathMonitor` on iOS, `ConnectivityManager` on Android.
- The tile and focus activity on Samsung, OnePlus and Xiaomi.
- Background refresh timing on iOS.

## Needed from Will

1. **Apple Team ID** (`APPLE_TEAM_ID` for `app.config.ts`) and the App Group
   `group.io.github.moorew.tailcopy` registered for the app and its three extensions.
2. **Go-ahead to merge** the protocol change (proto 1.1, schema 4) into main, or keep
   it on `mobile` until the phones pass. Desktops that are updated still sync with
   0.2.x desktops. Phones need updated desktops.
3. **The CI run** on this branch: the iOS job is the first Swift compile.

## Test steps

Android (Linux, Docker):

```sh
docker build -t tailcopy-android -f mobile/docker/android.Dockerfile mobile/docker
docker run --rm -v "$PWD":/src -w /src/mobile -e CARGO_TARGET_DIR=/src/target/docker-android \
  tailcopy-android bash -c 'npm ci && npm run rust:android && \
  npx expo prebuild -p android --clean --no-install && cd android && ./gradlew assembleRelease'
adb install mobile/android/app/build/outputs/apk/release/app-release.apk
```

iOS (on a Mac with Xcode 26 or later, Rust and CocoaPods):

```sh
cd mobile && npm ci && APPLE_TEAM_ID=XXXXXXXXXX npm run rust:ios
APPLE_TEAM_ID=XXXXXXXXXX npx expo run:ios --device
```

Then, with the desktop built from this branch, the relay optional:

1. On the desktop: Settings > Tailnet > Phones > Pair a phone.
2. On the phone, with Tailscale connected: open TailCopy and scan the code during
   onboarding.
3. Run SPEC 21.9 items 1 to 12, noting the phone model and OS version for each.
4. Collect the phone's log from Settings or the app container (`TailCopy/logs`); it never
   contains clip content.
