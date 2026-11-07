/** @type {import('@bacons/apple-targets/app.plugin').ConfigFunction} */
module.exports = (config) => ({
  type: 'action',
  name: 'action',
  displayName: 'Save to TailCopy',
  deploymentTarget: '26.0',
  bundleIdentifier: '.action',
  frameworks: ['UniformTypeIdentifiers'],
  entitlements: {
    'com.apple.security.application-groups': config.ios.entitlements['com.apple.security.application-groups'],
  },
});
