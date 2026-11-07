// Native project changes Expo's own config doesn't cover (SPEC 21.5, 21.4):
// brand icons into the iOS asset catalog, and Android app shortcuts.

const fs = require('node:fs');
const path = require('node:path');
const {
  withAndroidManifest,
  withAppBuildGradle,
  withDangerousMod,
  withGradleProperties,
  AndroidConfig,
} = require('expo/config-plugins');

function withBrandIcons(config) {
  return withDangerousMod(config, [
    'ios',
    (cfg) => {
      const from = path.join(cfg.modRequest.projectRoot, 'native-assets', 'ios', 'Icons.xcassets');
      const to = path.join(cfg.modRequest.platformProjectRoot, cfg.modRequest.projectName, 'Images.xcassets');
      for (const set of fs.readdirSync(from).filter((f) => f.endsWith('.imageset'))) {
        fs.cpSync(path.join(from, set), path.join(to, set), { recursive: true });
      }
      return cfg;
    },
  ]);
}

function withShortcuts(config) {
  return withAndroidManifest(config, (cfg) => {
    const main = AndroidConfig.Manifest.getMainActivityOrThrow(cfg.modResults);
    main['meta-data'] = (main['meta-data'] ?? []).filter((m) => m.$['android:name'] !== 'android.app.shortcuts');
    main['meta-data'].push({
      $: { 'android:name': 'android.app.shortcuts', 'android:resource': '@xml/tc_shortcuts' },
    });
    return cfg;
  });
}

// The Rust core is built for these two (scripts/build-rust.sh); a build with more
// ABIs would install on a phone that can't load it.
function withAbis(config) {
  return withGradleProperties(config, (cfg) => {
    cfg.modResults = cfg.modResults.filter((p) => !(p.type === 'property' && p.key === 'reactNativeArchitectures'));
    cfg.modResults.push({ type: 'property', key: 'reactNativeArchitectures', value: 'arm64-v8a,x86_64' });
    return cfg;
  });
}

// Release builds are signed with the owner's upload key when TAILCOPY_KEYSTORE (a
// path) and its passwords are set in the environment, and with the template's debug
// key otherwise (test builds only: that key is public).
const releaseSigning = `
        release {
            if (System.getenv('TAILCOPY_KEYSTORE')) {
                storeFile file(System.getenv('TAILCOPY_KEYSTORE'))
                storePassword System.getenv('TAILCOPY_KEYSTORE_PASSWORD')
                keyAlias System.getenv('TAILCOPY_KEY_ALIAS')
                keyPassword System.getenv('TAILCOPY_KEY_PASSWORD')
            }
        }`;

function withReleaseSigning(config) {
  return withAppBuildGradle(config, (cfg) => {
    let gradle = cfg.modResults.contents;
    if (gradle.includes('TAILCOPY_KEYSTORE')) return cfg;
    const debugConfig = /signingConfigs \{\s*debug \{[^}]*\}/;
    const releaseUse = /(release \{[^}]*?)signingConfig signingConfigs\.debug/;
    if (!debugConfig.test(gradle) || !releaseUse.test(gradle)) {
      throw new Error('with-tailcopy: app/build.gradle no longer has the expected signing blocks');
    }
    gradle = gradle.replace(debugConfig, (m) => m + releaseSigning);
    gradle = gradle.replace(
      releaseUse,
      "$1signingConfig System.getenv('TAILCOPY_KEYSTORE') ? signingConfigs.release : signingConfigs.debug",
    );
    cfg.modResults.contents = gradle;
    return cfg;
  });
}

module.exports = (config) => withReleaseSigning(withAbis(withShortcuts(withBrandIcons(config))));
