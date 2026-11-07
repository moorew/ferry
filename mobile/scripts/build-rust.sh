#!/usr/bin/env bash
# Builds the Rust core (crates/tailcopy-mobile) for the phone apps and generates its
# uniffi bindings into modules/tailcopy-core/rust (SPEC 21.2).
#
#   scripts/build-rust.sh android   needs the Android NDK (ANDROID_NDK_HOME) and cargo-ndk
#   scripts/build-rust.sh ios       needs macOS with Xcode
set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
repo="$(cd "$here/.." && pwd)"
out="$here/modules/tailcopy-core/rust"
profile="${PROFILE:-release}"
flag=$([ "$profile" = release ] && echo --release || true)

bindgen() {
  # $1 language, $2 out dir. Release builds strip the uniffi metadata, so the
  # bindings come from an unstripped host build; the API is the same everywhere.
  (cd "$repo" && cargo build -q -p tailcopy-mobile --lib)
  local target="${CARGO_TARGET_DIR:-$repo/target}"
  local lib="$target/debug/libtailcopy_mobile.so"
  [ -f "$lib" ] || lib="$target/debug/libtailcopy_mobile.dylib"
  rm -rf "$2"
  cargo run -q --manifest-path "$repo/Cargo.toml" -p tailcopy-mobile --features bindgen \
    --bin uniffi-bindgen -- generate --library "$lib" --language "$1" --out-dir "$2" --no-format
}

case "${1:-}" in
  android)
    # Google Play requires 16 KB page alignment for native code.
    export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-Wl,-z,max-page-size=16384"
    rm -rf "$out/jniLibs"
    (cd "$repo" && cargo ndk -t arm64-v8a -t x86_64 --platform 29 -o "$out/jniLibs" \
      build $flag -p tailcopy-mobile --lib)
    bindgen kotlin "$out/kotlin"
    ;;
  ios)
    # Match the app's deployment target (app.config.ts), or the linker warns that
    # the C parts (SQLite, ring, blake3) were built for a newer iOS.
    export IPHONEOS_DEPLOYMENT_TARGET=26.0
    targets=(aarch64-apple-ios aarch64-apple-ios-sim)
    for t in "${targets[@]}"; do
      (cd "$repo" && cargo build $flag -p tailcopy-mobile --lib --target "$t")
    done
    lib() { echo "$repo/target/$1/$profile/libtailcopy_mobile.a"; }
    bindgen swift "$out/swift.tmp"
    mkdir -p "$out/swift" "$out/include"
    rm -f "$out/swift/"* "$out/include/"*
    mv "$out/swift.tmp/tailcopy_mobile.swift" "$out/swift/"
    mv "$out/swift.tmp/tailcopy_mobileFFI.h" "$out/include/"
    mv "$out/swift.tmp/tailcopy_mobileFFI.modulemap" "$out/include/module.modulemap"
    rm -rf "$out/swift.tmp" "$out/TailCopyMobile.xcframework"
    xcodebuild -create-xcframework \
      -library "$(lib aarch64-apple-ios)" -headers "$out/include" \
      -library "$(lib aarch64-apple-ios-sim)" -headers "$out/include" \
      -output "$out/TailCopyMobile.xcframework"
    # TailCopyCore.podspec links these slices by path.
    for slice in ios-arm64 ios-arm64-simulator; do
      test -f "$out/TailCopyMobile.xcframework/$slice/libtailcopy_mobile.a" \
        || { echo "missing xcframework slice $slice" >&2; exit 1; }
    done
    ;;
  *)
    echo "usage: $0 android|ios" >&2
    exit 2
    ;;
esac
