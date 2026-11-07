/** @type {import('@bacons/apple-targets/app.plugin').ConfigFunction} */
module.exports = (config) => ({
  type: 'share',
  name: 'share',
  displayName: 'TailCopy',
  deploymentTarget: '26.0',
  bundleIdentifier: '.share',
  frameworks: ['UniformTypeIdentifiers'],
  entitlements: {
    'com.apple.security.application-groups': config.ios.entitlements['com.apple.security.application-groups'],
  },
});
