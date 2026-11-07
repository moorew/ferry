// TailCopy for iOS and Android (SPEC 21). Names mirror crates/tailcopy-core/src/names.rs
// (a Rust test keeps them in step). No Expo cloud services: builds are local or CI.

import type { ExpoConfig } from 'expo/config';

const APP_ID = 'io.github.moorew.tailcopy';
const IOS_APP_GROUP = 'group.io.github.moorew.tailcopy';
const PAIR_URL_SCHEME = 'tailcopy';
const VERSION = '0.1.0';
// eslint-disable-next-line @typescript-eslint/no-require-imports
const tokens = require('../design/tokens/tokens.json');

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
    bundleIdentifier: APP_ID,
    supportsTablet: true,
    appleTeamId: process.env.APPLE_TEAM_ID,
    icon: {
      light: './assets/icon/ios-light.png',
      dark: './assets/icon/ios-dark.png',
      tinted: './assets/icon/ios-tinted.png',
    },
    entitlements: {
      'com.apple.security.application-groups': [IOS_APP_GROUP],
    },
    infoPlist: {
      TailCopyAppGroup: IOS_APP_GROUP,
      BGTaskSchedulerPermittedIdentifiers: [`${APP_ID}.refresh`],
      UIBackgroundModes: ['fetch'],
      ITSAppUsesNonExemptEncryption: false,
    },
  },
  android: {
    package: APP_ID,
    adaptiveIcon: {
      foregroundImage: './assets/icon/android-foreground.png',
      monochromeImage: './assets/icon/android-monochrome.png',
      backgroundColor: tokens.color.light.bg.$value,
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
