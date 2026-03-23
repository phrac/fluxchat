#!/bin/bash
set -e

echo "Building fluxchat with Dioxus CLI (release)..."
cd crates/app
dx build --release
cd ../..

APP_PATH="target/dx/fluxchat-app/release/macos/FluxchatApp.app"

# Copy our custom icon into the bundle
cp assets/fluxchat.icns "$APP_PATH/Contents/Resources/fluxchat.icns"

echo ""
echo "Done! App bundle at: $APP_PATH"
echo "Run with: open $APP_PATH"
echo "Install: cp -r $APP_PATH /Applications/FluxChat.app"
