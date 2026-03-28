#!/bin/bash
set -euo pipefail

echo "Building fluxchat with Dioxus CLI (release)..."
cd crates/app
dx build --release
cd ../..

APP_PATH="target/dx/fluxchat/release/macos/FluxchatApp.app"

if [ ! -d "$APP_PATH" ]; then
    echo "ERROR: Build failed — app bundle not found at $APP_PATH"
    exit 1
fi

# Copy our custom icon into the bundle
cp assets/icon.icns "$APP_PATH/Contents/Resources/icon.icns"

echo ""
echo "Done! App bundle at: $APP_PATH"
echo "Run with: open $APP_PATH"
echo "Install: rm -rf /Applications/FluxChat.app; cp -r $APP_PATH /Applications/FluxChat.app"
