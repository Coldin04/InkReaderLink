#!/usr/bin/env bash
set -euo pipefail

if ! command -v cargo-ndk >/dev/null 2>&1; then
    printf '%s\n' "cargo-ndk is required; install it with: cargo install cargo-ndk --locked" >&2
    exit 1
fi
if ! command -v uniffi-bindgen >/dev/null 2>&1; then
    printf '%s\n' "uniffi-bindgen is required; install version 0.32.2 with: cargo install uniffi --version 0.32.2 --features cli --bin uniffi-bindgen --locked" >&2
    exit 1
fi

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
generated_dir="$repo_root/android-sdk/build/generated"
rm -rf "$generated_dir/jniLibs" "$generated_dir/sources/uniffi"
mkdir -p "$generated_dir/jniLibs" "$generated_dir/sources/uniffi"

cd "$repo_root"
cargo ndk \
    -t arm64-v8a \
    -t armeabi-v7a \
    -t x86_64 \
    -o "$generated_dir/jniLibs" \
    build --release -p inkreaderlink-uniffi

for abi in arm64-v8a armeabi-v7a x86_64; do
    abi_dir="$generated_dir/jniLibs/$abi"
    if [[ ! -f "$abi_dir/libinkreaderlink_uniffi.so" ]]; then
        printf 'cargo-ndk did not produce the expected library for %s\n' "$abi" >&2
        exit 1
    fi
    mv "$abi_dir/libinkreaderlink_uniffi.so" "$abi_dir/libcold04_inkreaderlink.so"
done

uniffi-bindgen generate \
    --library "$repo_root/target/aarch64-linux-android/release/libinkreaderlink_uniffi.so" \
    --language kotlin \
    --out-dir "$generated_dir/sources/uniffi"
