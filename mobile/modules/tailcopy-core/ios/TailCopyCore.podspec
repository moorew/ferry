# The bridge to the Rust core. `npm run rust:ios` (on a Mac) builds
# rust/TailCopyMobile.xcframework and generates rust/swift/tailcopy_mobile.swift.
Pod::Spec.new do |s|
  s.name           = 'TailCopyCore'
  s.version        = '0.1.0'
  s.summary        = 'TailCopy core: store, search and sync (SPEC 21.2)'
  s.license        = 'MIT OR Apache-2.0'
  s.author         = 'TailCopy'
  s.homepage       = 'https://github.com/moorew/tailcopy'
  s.platforms      = { :ios => '26.0' }
  s.swift_version  = '5.9'
  s.source         = { git: 'https://github.com/moorew/tailcopy.git' }
  s.static_framework = true

  s.dependency 'ExpoModulesCore'

  s.source_files = '*.swift', '../rust/swift/*.swift'
  s.vendored_frameworks = '../rust/TailCopyMobile.xcframework'
  s.frameworks = 'BackgroundTasks', 'UniformTypeIdentifiers'
  s.pod_target_xcconfig = {
    'DEFINES_MODULE' => 'YES',
    'SWIFT_COMPILATION_MODE' => 'wholemodule'
  }
end
