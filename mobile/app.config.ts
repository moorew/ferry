// TailCopy for iOS and Android (SPEC 21). Names mirror crates/tailcopy-core/src/names.rs
// (a Rust test keeps them in step). No Expo cloud services: builds are local or CI.

import type { ExpoConfig } from 'expo/config';

const APP_ID = 'io.github.moorew.tailcopy';
const IOS_APP_GROUP = 'group.io.github.moorew.tailcopy';
const PAIR_URL_SCHEME = 'tailcopy';
// eslint-disable-next-line @typescript-eslint/no-require-imports
const tokens = require('../design/tokens/tokens.json');

// One version for every platform: the desktop's, which release tags must match
// (D-022). Android orders updates by versionCode, so 0.3.1 becomes 301 (D-036).
// eslint-disable-next-line @typescript-eslint/no-require-imports
const VERSION: string = require('../app/src-tauri/tauri.conf.json').version;
const [major, minor, patch] = VERSION.split('.').map(Number);
const VERSION_CODE = major * 10000 + minor * 100 + patch;

// Someone building the iOS app for themselves signs it under their own Apple team,
// which can't use our bundle id, so TAILCOPY_IOS_ID swaps in theirs (docs/BUILD-IOS.md).
const IOS_ID = process.env.TAILCOPY_IOS_ID || APP_ID;
const IOS_GROUP = process.env.TAILCOPY_IOS_ID ? `group.${IOS_ID}` : IOS_APP_GROUP;

const fonts = [
  '../design/fonts/static/BricolageGrotesque-ExtraBold.ttf',
  '../design/fonts/static/Figtree-Regular.ttf',
  '../design/fonts/static/Figtree-Medium.ttf',
  '../design/fonts/static/Figtree-SemiBold.ttf',
  '../design/fonts/static/Figtree-Bold.ttf',
  '../design/fonts/static/JetBrainsMono-Regular.ttf',
];

export default (): ExpoConfig => ({
  name: 'TailCopy',
  slug: 'tailcopy',
  scheme: PAIR_URL_SCHEME,
  version: VERSION,
  orientation: 'default',
  userInterfaceStyle: 'automatic',
  ios: {
    bundleIdentifier: IOS_ID,
    buildNumber: String(VERSION_CODE),
    supportsTablet: true,
    appleTeamId: process.env.APPLE_TEAM_ID,
    icon: {
      light: './assets/icon/ios-light.png',
      dark: './assets/icon/ios-dark.png',
      tinted: './assets/icon/ios-tinted.png',
    },
    entitlements: {
      'com.apple.security.application-groups': [IOS_GROUP],
    },
    infoPlist: {
      TailCopyAppGroup: IOS_GROUP,
      BGTaskSchedulerPermittedIdentifiers: [`${APP_ID}.refresh`],
      UIBackgroundModes: ['fetch'],
      ITSAppUsesNonExemptEncryption: false,
    },
  },
  android: {
    package: APP_ID,
    versionCode: VERSION_CODE,
    adaptiveIcon: {
      foregroundImage: './assets/icon/android-foreground.png',
      monochromeImage: './assets/icon/android-monochrome.png',
      backgroundColor: tokens.color.light.accent.$value,
    },
    predictiveBackGestureEnabled: true,
    blockedPermissions: ['android.permission.RECORD_AUDIO'],
  },
  plugins: [
    'expo-router',
    [
      'expo-camera',
      {
        cameraPermission: 'TailCopy uses the camera to scan the pairing code on your computer.',
        recordAudioAndroid: false,
      },
    ],
    ['expo-build-properties', { android: { minSdkVersion: 29 }, ios: { deploymentTarget: '26.0' } }],
    ['expo-font', { fonts }],
    '@bacons/apple-targets',
    './plugins/with-tailcopy.js',
  ],
  experiments: {
    typedRoutes: true,
    reactCompiler: true,
  },
});
