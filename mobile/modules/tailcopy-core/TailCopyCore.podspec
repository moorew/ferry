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

  # CocoaPods only takes files under the podspec's directory, so it lives here.
  s.source_files = 'ios/*.swift', 'rust/swift/*.swift'
  s.frameworks = 'BackgroundTasks', 'UniformTypeIdentifiers'
  s.pod_target_xcconfig = {
    'DEFINES_MODULE' => 'YES',
    'EXCLUDED_ARCHS[sdk=iphonesimulator*]' => 'x86_64',
    # The bindings import the C module described by rust/include/module.modulemap.
    'SWIFT_INCLUDE_PATHS' => '$(PODS_TARGET_SRCROOT)/rust/include',
    'SWIFT_COMPILATION_MODE' => 'wholemodule'
  }
  # The pod's Swift module mentions the FFI module, so targets that import the pod
  # need to find it too, and the app links the Rust library's slice for its SDK
  # directly (CocoaPods' copy step for static xcframeworks didn't put it on the
  # link line). Paths go from mobile/ios/Pods to mobile/modules/tailcopy-core.
  rust = '$(PODS_ROOT)/../../modules/tailcopy-core/rust'
  s.user_target_xcconfig = {
    'SWIFT_INCLUDE_PATHS' => "#{rust}/include",
    # The Rust core is built for arm64 only (devices, Apple silicon simulators).
    'EXCLUDED_ARCHS[sdk=iphonesimulator*]' => 'x86_64',
    'OTHER_LDFLAGS[sdk=iphoneos*]' => "$(inherited) \"#{rust}/TailCopyMobile.xcframework/ios-arm64/libtailcopy_mobile.a\"",
    'OTHER_LDFLAGS[sdk=iphonesimulator*]' => "$(inherited) \"#{rust}/TailCopyMobile.xcframework/ios-arm64-simulator/libtailcopy_mobile.a\""
  }
end
