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
  # $1 language, $2 built library, $3 out dir
  rm -rf "$3"
  cargo run -q --manifest-path "$repo/Cargo.toml" -p tailcopy-mobile --features bindgen \
    --bin uniffi-bindgen -- generate --library "$2" --language "$1" --out-dir "$3" --no-format
}

case "${1:-}" in
  android)
    # Google Play requires 16 KB page alignment for native code.
    export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-Wl,-z,max-page-size=16384"
    rm -rf "$out/jniLibs"
    (cd "$repo" && cargo ndk -t arm64-v8a -t x86_64 --platform 29 -o "$out/jniLibs" \
      build $flag -p tailcopy-mobile --lib)
    bindgen kotlin "$out/jniLibs/arm64-v8a/libtailcopy_mobile.so" "$out/kotlin"
    ;;
  ios)
    targets=(aarch64-apple-ios aarch64-apple-ios-sim)
    for t in "${targets[@]}"; do
      (cd "$repo" && cargo build $flag -p tailcopy-mobile --lib --target "$t")
    done
    lib() { echo "$repo/target/$1/$profile/libtailcopy_mobile.a"; }
    bindgen swift "$(lib aarch64-apple-ios)" "$out/swift.tmp"
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
    ;;
  *)
    echo "usage: $0 android|ios" >&2
    exit 2
    ;;
esac
