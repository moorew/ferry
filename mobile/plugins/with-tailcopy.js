// Native project changes Expo's own config doesn't cover (SPEC 21.5, 21.4):
// brand icons into the iOS asset catalog, and Android app shortcuts.

const fs = require('node:fs');
const path = require('node:path');
const { withDangerousMod, withAndroidManifest, AndroidConfig } = require('expo/config-plugins');

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

module.exports = (config) => withShortcuts(withBrandIcons(config));
