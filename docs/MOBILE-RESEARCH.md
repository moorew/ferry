# TailCopy for iOS and Android: research and proposal

Status: **research only, nothing built.** SPEC 2.3 puts phone apps (and embedded
Tailscale) out of scope for v1, so this needs Will's go-ahead and a spec change before
any code. Proposal logged as D-031 in `docs/DECISIONS.md`.

Researched 2026-11-06 by four parallel agents (tailnet and Rust reuse, iOS, Android,
React Native stack) against current vendor docs. Version numbers are as reported on that
date; re-verify each at build time (CLAUDE.md rule 5). Items marked *(unverified)* were
not confirmed by a primary source.

---

## 1. The short version

- **It's feasible.** React Native with Expo makes sense for the app's screens. The surfaces
  that run outside the app (extensions, keyboards, widgets, tiles) must be pure Swift or
  Kotlin. One Rust crate built on `tailcopy-core` and `tailcopy-sync` serves all of them
  through uniffi.
- **The phone is not a desktop.** Neither OS lets an app read the clipboard in the
  background. The only exception is Android, when TailCopy is your default keyboard. On a
  phone, TailCopy is "browse and copy out, save in on purpose". Desktop-style automatic
  capture isn't possible. Other clipboard apps work the same way (Paste, Copied, KDE
  Connect, Clipper), and the top complaint in their reviews is apps that promise more.
- **Two spec rules can't hold on phones as written:**
  1. SPEC 8.5 `whois`. Other apps can't use the Tailscale app's LocalAPI on iOS or
     Android, so the phone can't verify peers the way desktops do.
  2. SPEC 9.7 "secrets are never stored". iOS has no documented concealed-type marker,
     so this is best-effort on iOS.
- **The phone only dials out and never listens.** iOS suspends it in the background and
  Android's Doze cuts its network. It catches up when it opens; a relay that is always on
  makes this work well. Waking it without a cloud service is impossible on iOS (APNs
  needs a server holding Apple's key).

---

## 2. Networking and trust (the decision that blocks everything)

| Option | How | For | Against |
| --- | --- | --- | --- |
| **A. Official Tailscale app** (recommended) | The phone runs the Tailscale app as usual; TailCopy opens TCP to peers' 100.x addresses | No second login, no Go runtime, no VPN entitlement, no store-policy risk, zero battery cost on our side | No `whois`/`status` on the phone, so it needs pairing (below). A work VPN or split tunnelling silently breaks it. MagicDNS sometimes fails in third-party apps, so store IPs |
| **B. Embedded userspace node** (libtailscale/TailscaleKit, Go tsnet) | TailCopy is its own tailnet node inside the app | Gets its own LocalAPI, so SPEC 8.5 works unchanged. Coexists with the Tailscale app | A second node and a second login per phone. Go runtime next to Rust. Node keys expire (180 d). Seconds to reconnect on every launch. Thin upstream maintenance (no tags, quiet since 2026-02 *(unverified)*). App Store precedent exists (NovaScale) |
| C. Own VPN | NetworkExtension / VpnService | none | Displaces Tailscale; rejected |
| tailscale-rs | Rust tsnet | Would fit the `Transport` trait | Pre-alpha 0.7; iOS and Android are listed as unsupported. Not for 2026 |

### Proposed trust model for option A: pairing with pinned keys

1. On a desktop, Settings > Devices > **Add phone** shows a QR code: desktop `device_id`,
   Tailscale IPs, MagicDNS name, static public key, and a single-use secret (2 min).
2. The phone generates a key pair (Keychain or Keystore). It dials the desktop and runs a
   mutual handshake (Noise XX or IK, e.g. the `snow` crate) that proves it holds the
   secret.
3. The desktop admits the phone only if **whois says same login (unchanged rule 1) and**
   the pairing proof is valid. It then binds the phone's key to its Tailscale stable ID.
4. Every later session: desktop-side whois as today, plus pinned keys both ways.
5. **Vouching:** a paired desktop sends a signed device list (an extension of the
   `Devices` frame) with each peer's key and addresses. The phone learns candidates from
   it instead of `tailscale status`, so pairing with one desktop is enough.

The tailnet stays the outer trust boundary, because desktops still enforce same-login
whois. Pinned keys stop a phone from pushing history to whatever node now holds an IP.
Costs: new security code, which needs its own threat model and tests; a pairing screen
with camera permission; and an unpair/revoke action.

Option B keeps the spec's model intact with no new crypto. That is a real argument for
it, so it's listed as the alternative in D-031. The `Transport` trait leaves room to add
B later either way.

### Protocol changes either way (SPEC 7.7, 6.2)

- New role `mobile` (in `Role` and in the `devices.role` CHECK constraint). A mobile
  device always dials and never binds a listener. Desktops and relays never dial it,
  otherwise they would keep probing a dead port whenever the Tailscale app shows the
  phone `Online`.
- Catch-up on foreground uses the existing version-vector flow unchanged. Catch-up ops
  never touch the clipboard (7.6 already says so). Live mirror happens only while the app
  is open, and is off by default on Android because each write shows Android 13+'s
  clipboard overlay.
- The relay matters more. Strict mode rejects tagged relays, so either document "run the
  relay untagged as the owner", or pair the relay the same way as phones.

### Background execution

- **iOS:** sync on foreground; `BGAppRefreshTask` gives ~30 s at times the OS chooses
  (hours apart); `beginBackgroundTask` finishes a sync that's already running. Tear
  sessions down when the app backgrounds. The tokio runtime freezes during suspend.
- **Android:** WorkManager periodic catch-up (15 min minimum) by default. An opt-in
  "Keep connected" mode runs a `connectedDevice` foreground service (`dataSync` is capped
  at 6 h/day on API 35+). It needs Play's foreground-service declaration and has a
  persistent notification. Clipt's permanent notification was its most-hated feature, so
  keep it opt-in.

---

## 3. Capture and integrations

### iOS

Rules: reading the clipboard shows "Allow Paste" unless the user tapped `UIPasteControl`
or set Settings > TailCopy > Paste from Other Apps to **Allow**. `changeCount`,
`detectPatterns` and `hasStrings` don't read content *(the `hasStrings` banner behaviour is
unverified; test on a device)*. The app gets no reads in the background. iOS 27's own
keyboard paste suggestion shows only the latest item, so the history is the
differentiator.

| Priority | Integration | Notes |
| --- | --- | --- |
| v1 | Foreground capture + picker | Compare `changeCount` on foreground and read only if it changed. Onboarding asks once for "Allow" with a clear reason |
| v1 | `UIPasteControl` "Add clipboard" button | One read, no prompt |
| v1 | Share and Action extensions | Capture from any app with no prompt. Write to the inbox (see storage below) |
| v1 | App Intents (Shortcuts, Siri, Action button, Spotlight snippets) | "Save clipboard", "Copy latest from &lt;device&gt;", "Copy pinned &lt;label&gt;". Whether an intent can write the clipboard in the background is unverified, so fall back to `openAppWhenRun` |
| v1 | Control Center control (iOS 18+) | "Capture now": opens, captures, dismisses. The closest thing to the desktop hotkey |
| v1 | Pinned-items widget (interactive, Lock Screen) | Cheap once intents exist |
| v1 | iPad: split view, drag and drop, ⌘F / arrows / Return | Dropping in is a capture route with no prompt |
| later | Pick-only keyboard extension | Needs Full Access to reach the App Group, which brings the scary "could transmit everything you type" warning. Memory ceiling ~30 to 60 MB *(unverified)*. Guideline 4.4.1 requires typing and a next-keyboard key. Must be Swift |
| later | Spotlight `IndexedEntity` | Pinned and labelled items only, never all history |
| skip | Live Activities, Watch, Handoff | Live Activities would break "quiet"; Universal Clipboard already covers Apple-to-Apple |

Universal Clipboard: de-duplicate echoes by hash (`group_key`), and write mirrored clips
with `.localOnly` so they don't go back out over Continuity.

Secrets (best-effort): skip items whose types include `org.nspasteboard.ConcealedType` or
`TransientType`, and skip values that look like one-time codes. Say plainly in Settings
that iOS doesn't let TailCopy guarantee this.

### Android

Rules: on Android 10+ only the default keyboard or the focused app can read the
clipboard. Android 12+ shows a "pasted from your clipboard" toast on content reads but not
on `getPrimaryClipDescription()`. Android 13+ has its own copy overlay and
`EXTRA_IS_SENSITIVE`. A 2026 API adds `isRemoteClipboard()` and
`setExcludedFromRemoteClipboard()` *(which builds have them is unverified)*. Treat
"excluded" like "save but don't sync". Accessibility-service capture is out: it breaks Play
policy (tightened 29 Oct 2026), Android 17 Advanced Protection revokes it, and it would
undermine the privacy promise.

| Priority | Integration | Notes |
| --- | --- | --- |
| v1 | Read on picker focus | Check the metadata first and read only if the clip is new and not sensitive. Explain the toast once |
| v1 | `ACTION_PROCESS_TEXT`: "Save to TailCopy" in the selection menu | The strongest capture path. Also "Paste from TailCopy" to replace a selection in editable fields. Some host apps don't show it (package visibility) |
| v1 | Share target (`ACTION_SEND` text, URL, image) | Saves silently and closes. Grant URI permissions explicitly |
| v1 | Quick Settings tile "Save clipboard" | Launches an invisible focus activity (the KDE Connect pattern) with `startActivityAndCollapse(PendingIntent)`. Test on Samsung, OnePlus and Xiaomi |
| v1 | App shortcuts | Search, Save clipboard, Pinned |
| v1.x | Picker keyboard (`InputMethodService`) | The only way to paste from history into any app, and the only passive-capture path (when default). Never capture in password fields (`EditorInfo` flags). A full typing keyboard would mean a HeliBoard fork (GPL-3.0 *(unverified)*), which is a licensing call |
| v1.x | Glance widget | Pinned and recent items, small thumbnails (Android 17 image memory cap) |
| opt-in | "Keep connected" foreground service | See section 2 |
| later | F-Droid "power mode" via adb or Shizuku `READ_LOGS` | Clip Stack/Join approach; whether it still works on 16/17 is unverified. Never in the Play build |

---

## 4. UI per platform

Shared principles from HANDOFF carry over: the picker is the home screen; tapping a row
copies (haptic, gone); items from other devices arrive with device colour **and** name;
reduced motion means fades only, up to 120 ms. Rows grow with Dynamic Type or font scale
up to 200% rather than truncating at the desktop's 56 pt.

**iOS 26/27 (design for 27, minimum 26).** Liquid Glass is mandatory in Xcode 27 builds.
- **Tabs:** History, Pinned, Devices, plus a Search tab (iOS 27 puts search back in the
  tab bar). Settings opens as a sheet from a toolbar button.
- **Rows:** swipe right to pin, swipe left to delete with undo. Long-press opens a context
  menu with a full preview and Copy, Copy as plain text, Pin, Label, Share, Delete.
- **Colour:** leave the system glass bars alone. Lilac paper and ink go on content
  surfaces only. Violet becomes the app `tintColor`, the single accent. Device colours are
  dynamic (light and dark values) and never sit on glass.
- **Icons:** SF Symbols for system actions (share, trash, pin, search, more), so they
  match glass toolbars and scale with Dynamic Type. The brand's 24-grid set covers brand
  concepts (type tiles, device glyphs, trail, password skipped), exported as template
  symbol images. Proposed as a scoped exception to "don't mix icon sets"; Will's call.
- **iPad:** three columns (sidebar, list, detail).
- **Test with:** Tinted glass, Reduce Transparency, Increase Contrast.

**Android (Material 3 Expressive, targetSdk 36).** Edge-to-edge and predictive back are
mandatory.
- **Phone:** no bottom bar, so the picker stays the product. Expressive search app bar,
  filter chips (All, Pinned, This phone, one per device, Images, Links). Devices and
  Settings sit in the overflow menu. Detail opens in a bottom sheet.
- **Quick picker:** a translucent bottom-sheet activity opened from the tile, widget and
  shortcuts, which closes after a pick.
- **600 dp and wider:** navigation rail plus list and detail side by side. Hardware-keyboard
  shortcuts for ChromeOS.
- **Colour:** brand Lilac by default, with an optional "Use wallpaper colours" switch.
  Device colours are never harmonised to the wallpaper.
- **App icon:** adaptive, with a monochrome layer (the trail mark).
- **Rows:** swipe to delete (Undo snackbar), swipe the other way to pin. Long-press for
  multi-select.

**Both:** onboarding is check Tailscale, pair with a desktop (QR), name this device, then
set up capture (honest about the limits). Settings has a Capture section with setup guides
for each entry point.

---

## 5. Stack

| Concern | Choice | Reported version (2026-11-06) |
| --- | --- | --- |
| Framework | Expo, Continuous Native Generation (`ios/` and `android/` generated, gitignored), dev client, **no EAS** (local and GitHub Actions builds), `EXPO_NO_TELEMETRY=1` | SDK 58 (RN 0.88.1, React 19.3, Hermes V1, New Architecture only) |
| Navigation | Expo Router native tabs + native stack (real `UITabBar` with Liquid Glass; Material bottom nav) | expo-router 58, react-native-screens 4.30 |
| Native controls | `@expo/ui` (SwiftUI and Jetpack Compose; menus, sheets, Settings forms); `.ios.tsx` / `.android.tsx` only for whole-screen differences | 58.x (breaking changes each SDK) |
| Lists | LegendList (FlashList is in fixes-only mode since Shopify left RN) | 3.7 |
| Motion | Reanimated 4 + `ReduceMotion.System`, expo-haptics | 4.7 |
| iOS targets | `@bacons/apple-targets` for share, widget/control and keyboard; App Intents in Swift (`expo-app-intents` is alpha) | 5.0 |
| Android surfaces | A local Expo Module in Kotlin: tile, `PROCESS_TEXT`/`SEND` activities, IME, Glance, service | |
| Rust | New `crates/tailcopy-mobile` facade: core + the sync **client** (axum server behind a feature flag, off), its own tokio runtime, uniffi Swift and Kotlin bindings. The RN app reaches it through a thin Expo Module over the same bindings | uniffi 0.31.x pinned |
| Tokens | Style Dictionary emits TS, Swift and Kotlin (and could replace hand-synced `tokens.css`). `tokens.json` needs `$type` fields first, which is a token change | 5.8 |
| Icons | SVGR `--native` + react-native-svg; template images (iOS); VectorDrawables (Android); one script | |
| Tests | Jest + jest-expo + RNTL; Maestro locally (no Maestro Cloud); cargo tests for the facade | |

**Why not uniffi-bindgen-react-native as the base:** its docs still say not for
production, and it's pinned to uniffi 0.31. The extensions need Swift and Kotlin bindings
anyway, so one binding style serves app, extensions and services. Revisit when it reaches
1.0.

**Rust on mobile:**
- `aarch64-apple-ios` and `aarch64-linux-android` are fine for rusqlite (bundled, FTS5),
  blake3, tokio and rmp-serde *(not yet test-built here)*.
- Ship **one** dynamic xcframework that the extensions load from the app bundle, rather
  than three static copies.
- Android: NDK r28+ or `max-page-size=16384` for the 16 KB page rule.
- Don't add a JS SQLite library (two SQLite copies); JS reaches the store only through
  Rust.

**Shared storage on iOS:**
- Holding a SQLite lock in the App Group container while suspended gets the app killed
  (`0xdead10cc`; only visible in TestFlight). A clipboard-sync engine hit this in 2026.
- Rule: only the app writes. Extensions append to an inbox (atomic JSON files or a small
  separate database) that the app ingests. The keyboard and widget open the store
  read-only and briefly.
- Close connections on resign-active.
- Spike this first.

**Layout:**

```text
crates/tailcopy-mobile/       uniffi facade (workspace member)
mobile/                       Expo app (own package-lock, like app/)
  app.config.ts               config plugins, App Group, entitlements
  src/app/                    Expo Router routes
  src/ui/ src/theme/          components; generated tokens.ts, icons/
  modules/tailcopy-core/      Expo Module bridging the uniffi bindings (Swift + Kotlin)
  modules/tailcopy-android/   tile, PROCESS_TEXT, SEND, IME, Glance, service (Kotlin)
  targets/share|widget|keyboard/  Swift (apple-targets)
  e2e/                        Maestro flows
design/tokens/build/          Style Dictionary config
```

Bundle IDs and the App Group name go in `names.rs` with the other names (CLAUDE.md
rule 7).

**CI and release:**
- **Android:** builds on `ubuntu-latest` (and in Docker locally).
- **iOS:** needs a macOS runner with Xcode 27; fastlane signs every target and uploads
  to TestFlight.
- **Play:** fastlane `supply`.
- **F-Droid:** Expo's prebuilt AARs and Hermes may fail its build-from-source rules, so
  start with IzzyOnDroid or GitHub APKs and treat F-Droid main as a stretch goal.

---

## 6. Proposed plan (if Will says go)

The mobile work would follow M3 (v1.0) unless Will wants it earlier. It mirrors M0 then M1/M2.

**M5.0 spikes (throwaway, in `spikes/`):**
1. Cross-compile a `tailcopy-mobile` facade for iOS and Android. Call it from Swift, Kotlin
   and RN via the Expo Module. Measure binary size.
2. iOS App Group: app plus share extension plus widget on one store under suspend/resume
   in TestFlight, with no `0xdead10cc`.
3. Phone dials a desktop through the Tailscale app: `/v1/hello` and the pairing handshake,
   with the desktop's whois of the phone.
4. Android capture paths: `PROCESS_TEXT` in Chrome, Gmail and Keep; tile plus focus
   activity on Pixel, Samsung, OnePlus; toast behaviour.
5. iOS paste behaviour: the `changeCount` and `hasStrings` prompts, the "Allow" setting,
   whether an App Intent can write the clipboard in the background.
6. Memory of a Swift keyboard and a widget linking the Rust framework.

**M5:** History, Pinned, Devices, Settings, onboarding with pairing; foreground capture;
share, Action extension and `PROCESS_TEXT`; tile and Control Center control; App
Intents; dial-only sync with catch-up; TestFlight and Play internal track.

**M6:** pinned widgets, picker keyboard (both platforms), iPad and large-screen layouts,
optional "Keep connected", Spotlight, store releases.

---

## 7. Found along the way, affects desktop

- **macOS clipboard-read alerts.** Apple announced in May 2025 that macOS will alert when
  an app reads the pasteboard programmatically. It also added `NSPasteboard` `detect`
  methods and `accessBehavior`
  ([MacRumors](https://www.macrumors.com/2025/05/12/apple-mac-apps-clipboard-change/)).
  Check the current macOS capture path on macOS 26/27 (D-032).

## 8. Key sources

- Tailscale: [libtailscale](https://github.com/tailscale/libtailscale),
  [tailscale-rs](https://github.com/tailscale/tailscale-rs/),
  [tunnelless](https://github.com/indiagrams/tunnelless),
  [other VPNs FAQ](https://tailscale.com/docs/reference/faq/other-vpns)
- iOS: [UIPasteboard detectPatterns](https://developer.apple.com/documentation/uikit/uipasteboard/detectpatterns(for:completionhandler:)),
  [App Review Guidelines](https://developer.apple.com/app-store/review/guidelines/),
  [SQLite in App Group containers](https://ryanashcraft.com/sqlite-databases-in-app-group-containers/),
  [Paste help](https://pasteapp.io/help/paste-on-iphone),
  [WWDC25-275 App Intents](https://developer.apple.com/videos/play/wwdc2025/275/)
- Android: [Copy and paste](https://developer.android.com/develop/ui/views/touch-and-input/copy-paste),
  [FGS timeouts](https://developer.android.com/develop/background-work/services/fgs/timeout),
  [FGS types](https://developer.android.com/develop/background-work/services/fgs/service-types),
  [Accessibility policy](https://support.google.com/googleplay/android-developer/answer/16558241),
  [Play target API](https://support.google.com/googleplay/android-developer/answer/11926878),
  [KDE Connect focus activity](https://invent.kde.org/network/kdeconnect-android/-/merge_requests/127)
- React Native: [Expo SDK 58](https://expo.dev/changelog/sdk-58),
  [RN 0.88](https://reactnative.dev/blog/2026/10/19/react-native-0.88),
  [Expo native tabs](https://docs.expo.dev/router/advanced/native-tabs/),
  [@expo/ui stable](https://expo.dev/blog/expo-ui-stable-sdk-56),
  [apple-targets](https://github.com/EvanBacon/expo-apple-targets),
  [uniffi-bindgen-react-native](https://jhugman.github.io/uniffi-bindgen-react-native/),
  [Style Dictionary](https://github.com/style-dictionary/style-dictionary/releases)
