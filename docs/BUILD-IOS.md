# Building TailCopy for your iPhone

TailCopy isn't in the App Store (D-036). If you have a Mac, you can build it and
install it on your own iPhone. It takes about half an hour the first time.

## What you need

- A Mac with **Xcode 26** or later (free from the Mac App Store). Open it once so it
  can finish installing its components.
- An iPhone on **iOS 26** or later, and a USB cable.
- An **Apple ID**. A free one works. The paid Apple Developer Program ($99 a year)
  only makes it last longer (see "Free or paid" below).
- [Rust](https://rustup.rs) and [Node 22](https://nodejs.org).
- The **Tailscale** app on the iPhone, signed in to the same account as your
  computers.

## Free or paid Apple ID

| | Free Apple ID | Apple Developer Program |
| --- | --- | --- |
| App keeps opening for | 7 days, then reinstall it (step 6) | a year |
| Apps you can install this way | 3 per iPhone | no practical limit |
| New app IDs | 10 a week (TailCopy uses 4) | no practical limit |

Reinstalling over the top keeps your history. If you let the free install lapse,
TailCopy just won't open until you reinstall it; nothing is lost.

## 1. Add your Apple ID to Xcode

Open Xcode, then **Settings › Accounts › +** and sign in. That creates a "Personal
Team" for a free Apple ID.

With a paid membership, your **Team ID** is under **Membership details** at
<https://developer.apple.com/account>: 10 characters, like `A1B2C3D4E5`. With a free
Apple ID you can skip it. Leave out the `APPLE_TEAM_ID` line in step 5, and choose
your team in Xcode instead (step 5's "If Xcode says…" fix).

## 2. Pick a bundle id of your own

Apple won't let you sign with TailCopy's own id, so choose one that's yours, in
reverse-domain form. For example, `com.yourname.tailcopy`. Use only letters,
numbers, dots and hyphens. The App Group becomes `group.com.yourname.tailcopy`.

## 3. Get the code and build the Rust core

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
git clone https://github.com/moorew/tailcopy.git
cd tailcopy
git checkout v0.4.0     # or the latest release tag
cd mobile
npm ci
npm run rust:ios
```

## 4. Get the iPhone ready

1. Plug it in and tap **Trust** on the iPhone.
2. Turn on **Settings › Privacy & Security › Developer Mode**. The iPhone restarts;
   confirm when it asks.

## 5. Build and install

```sh
export TAILCOPY_IOS_ID=com.yourname.tailcopy
export APPLE_TEAM_ID=A1B2C3D4E5     # paid membership only; see step 1
export EXPO_NO_TELEMETRY=1
npx expo prebuild -p ios --clean
npx expo run:ios --device --configuration Release
```

Choose your iPhone when it asks. The Release build carries everything it needs, so it
runs without your Mac nearby.

**If Xcode says it can't create a provisioning profile,** open
`ios/TailCopy.xcworkspace` in Xcode. For each of the four targets (**TailCopy**,
**share**, **action** and **widget**), go to **Signing & Capabilities** and:
- tick **Automatically manage signing**
- choose your team
- under **App Groups**, make sure `group.com.yourname.tailcopy` is ticked; press
  **+** to add it if it isn't there

Then run the `npx expo run:ios` line again.

**The first time it opens,** iOS says the developer isn't trusted. Go to
**Settings › General › VPN & Device Management**, tap your Apple ID, then **Trust**.

## 6. Updating (and the 7-day reinstall)

```sh
cd tailcopy
git fetch --tags && git checkout v0.4.1     # the new release tag
cd mobile
npm ci
npm run rust:ios
npx expo prebuild -p ios --clean
npx expo run:ios --device --configuration Release
```

Keep the same `TAILCOPY_IOS_ID` and `APPLE_TEAM_ID` every time. A different id
installs a second, empty TailCopy instead of updating yours.

On a free Apple ID, run the last line again every 7 days, even if there's no new
version.

## 7. Pair it

Open TailCopy and follow the setup. On your computer, go to **TailCopy › Settings ›
Tailnet › Pair a phone** and scan the code.

## Troubleshooting

- **"Untrusted Developer"**: see the end of step 5.
- **"Maximum number of apps for free development profiles"**: remove another app you
  installed this way, or use a paid membership.
- **The share sheet saves nothing**: the App Group isn't set up the same on every
  target. Check all four targets in step 5's Xcode fix.
- **Build errors about Rust or `libtailcopy_mobile`**: run `npm run rust:ios` again
  after any update.
