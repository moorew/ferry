# Phone apps: one-time setup

What the owner sets up once so the phone apps can be released (SPEC 21.7, D-036).
None of it uses an Expo account, an app store or any other outside service.

## 1. Android: the release key

Every Android release is signed with this key, and Android only installs an update
signed with the same key as the installed app. There's no app store holding a copy:
**if the key or its passwords are lost, every install has to be removed, with its
history, before it can take another update.** Keep two backups in different places,
such as a password manager and an offline drive.

1. Create the key:
   ```sh
   keytool -genkeypair -v -keystore tailcopy-release.jks -alias tailcopy \
     -keyalg RSA -keysize 4096 -validity 10000
   ```
2. Give it to CI:
   ```sh
   gh secret set ANDROID_KEYSTORE_BASE64 < <(base64 -w0 tailcopy-release.jks)
   gh secret set ANDROID_KEYSTORE_PASSWORD
   gh secret set ANDROID_KEY_ALIAS --body tailcopy
   gh secret set ANDROID_KEY_PASSWORD
   ```
3. Note its fingerprint. Each release's notes print it, and users can check it in
   Obtainium or AppVerifier:
   ```sh
   keytool -list -v -keystore tailcopy-release.jks -alias tailcopy | grep SHA256
   ```

The release workflow refuses to publish without the key, and refuses a debug-signed
APK. The **Mobile** workflow signs its test builds with the key too, when the secrets
are set. Locally, set `TAILCOPY_KEYSTORE` (the file's path),
`TAILCOPY_KEYSTORE_PASSWORD`, `TAILCOPY_KEY_ALIAS` and `TAILCOPY_KEY_PASSWORD` before
`./gradlew assembleRelease`.

An install signed with the public debug key (any test build made before the secrets
existed) can't take a release. Uninstall it once, which clears its history, then
install the release.

## 2. Releasing

The phone shares the desktop's version, so a normal release (a `v*` tag matching
`app/src-tauri/tauri.conf.json`) also builds `TailCopy-<version>-android-arm64.apk`
and attaches it. Nothing else to do.

## 3. Installing a test build

1. Open a **Mobile** run under Actions (or a PR's checks) and download the
   `tailcopy-android` artifact. It's a zip holding `tailcopy-<commit>.apk`.
2. Install it with `adb install -r tailcopy-<commit>.apk`, or copy it to the phone and
   open it there.

A test build has the same version as the last release, so Obtainium won't treat it as
newer. It updates again at the next release.

## iOS

Nobody gets an iOS build from us. The owner, like anyone else, builds it from source
with [BUILD-IOS.md](BUILD-IOS.md), under his own bundle id. For a paid membership,
that can be TailCopy's own id, `io.github.moorew.tailcopy`, after registering it and
`group.io.github.moorew.tailcopy` in the developer account; otherwise use a new id
as the guide says. The CI iOS job only checks that it compiles (simulator, unsigned).
