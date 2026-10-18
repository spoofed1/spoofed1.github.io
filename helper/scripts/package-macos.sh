#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
: "${MACOSX_DEPLOYMENT_TARGET:=12.0}"
export MACOSX_DEPLOYMENT_TARGET
cargo build --release --locked --target aarch64-apple-darwin
cargo build --release --locked --target x86_64-apple-darwin
mkdir -p 'dist/Spoofed Helper.app/Contents/MacOS' 'dist/Spoofed Helper.app/Contents/Resources'
lipo -create target/aarch64-apple-darwin/release/spoofed-helper target/x86_64-apple-darwin/release/spoofed-helper -output 'dist/Spoofed Helper.app/Contents/MacOS/spoofed-helper'
cp resources/Info.plist 'dist/Spoofed Helper.app/Contents/Info.plist'
cp resources/THIRD_PARTY_NOTICES.txt 'dist/Spoofed Helper.app/Contents/Resources/'
# Cloud-synced working folders can attach Finder metadata to generated bundles.
xattr -d com.apple.FinderInfo 'dist/Spoofed Helper.app' 2>/dev/null || true
codesign --force --deep --sign - 'dist/Spoofed Helper.app'
ditto -c -k --sequesterRsrc --keepParent 'dist/Spoofed Helper.app' dist/Spoofed-Helper-macOS.zip
