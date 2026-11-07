# Phone apps: one-time setup

What the owner sets up once so the phone apps can be signed and installed (SPEC 21.7).
None of it uses an Expo account or any Expo service.

## 1. Apple: Team ID and App Group

You need the paid Apple Developer Program membership (the one behind the macOS
Developer ID).

**Find your Team ID**
1. Sign in at <https://developer.apple.com/account>.
2. Open **Membership details**. Copy the 10-character **Team ID**.
3. Send it over, or set it as a repository variable:
   `gh variable set APPLE_TEAM_ID --body XXXXXXXXXX`.
   It isn't secret: it's printed inside every app you sign.

**Register the App Group and the four app IDs.** The easy way is to let Xcode do it the
first time you run on your iPhone:

1. On your Mac, clone the repo and check out the `mobile` branch.
2. `cd mobile && npm ci && npm run rust:ios`. This needs Rust with the
   `aarch64-apple-ios` and `aarch64-apple-ios-sim` targets, and Xcode 26 or later.
3. `APPLE_TEAM_ID=XXXXXXXXXX npx expo run:ios --device`, with your iPhone plugged in
   and Developer Mode on (Settings › Privacy & Security › Developer Mode).
4. If Xcode says it can't create a provisioning profile, open
   `mobile/ios/TailCopy.xcworkspace`. Then, for each of the four targets (TailCopy,
   share, action, widget), go to **Signing & Capabilities** and:
   - tick **Automatically manage signing**
   - pick your team
   - under **App Groups**, tick `group.io.github.moorew.tailcopy`, pressing **+** to
     create it the first time

   Run again.

**Or by hand** at <https://developer.apple.com/account/resources/identifiers/list>:
1. **+** › **App Groups** › `group.io.github.moorew.tailcopy`.
2. **+** › **App IDs** › **App**, four times:
   - `io.github.moorew.tailcopy`
   - `io.github.moorew.tailcopy.share`
   - `io.github.moorew.tailcopy.action`
   - `io.github.moorew.tailcopy.widget`

   On each, tick **App Groups**, then **Configure** › choose the group.

## 2. Android: an upload key

Without one, CI signs the APK with the public debug key. That's fine for testing. But a
phone with a debug-signed install can't take an update signed with a real key: you'd have
to uninstall first, which deletes the phone's history.

1. Create the key, and back up the file and passwords somewhere safe. Losing them means
   you can never ship an update to installs signed with it.
   ```sh
   keytool -genkeypair -v -keystore tailcopy-upload.jks -alias tailcopy \
     -keyalg RSA -keysize 4096 -validity 10000
   ```
2. Give it to CI:
   ```sh
   gh secret set ANDROID_KEYSTORE_BASE64 < <(base64 -w0 tailcopy-upload.jks)
   gh secret set ANDROID_KEYSTORE_PASSWORD
   gh secret set ANDROID_KEY_ALIAS --body tailcopy
   gh secret set ANDROID_KEY_PASSWORD
   ```
3. The next run of the **Mobile** workflow signs with it. Locally, set
   `TAILCOPY_KEYSTORE` (the file's path), `TAILCOPY_KEYSTORE_PASSWORD`,
   `TAILCOPY_KEY_ALIAS` and `TAILCOPY_KEY_PASSWORD` before `./gradlew assembleRelease`.

## 3. Installing the Android build

1. Open the latest **Mobile** run under Actions (or the PR's checks) and download the
   `tailcopy-android` artifact. It's a zip holding `tailcopy-<commit>.apk`.
2. Install it:
   - over USB: `adb install tailcopy-<commit>.apk`
   - or on the phone: copy the file across, open it, and allow your file manager to
     install unknown apps
3. The phone needs the Tailscale app, connected and signed in to the same account as your
   computer. The computer needs a desktop build from the `mobile` branch, which has the
   Pair a phone button.

## Later: TestFlight and signed iOS builds in CI

CI builds iOS only for the simulator, unsigned: a compile check. Building for TestFlight
needs an App Store Connect API key, held as repository secrets:
- `ASC_KEY_ID`
- `ASC_ISSUER_ID`
- `ASC_KEY_P8`, the key's .p8 file

You create it at App Store Connect › Users and Access › Integrations › App Store Connect
API, with the App Manager role. Set those up when M5's hardware tests pass, and the
workflow gets an upload job.
