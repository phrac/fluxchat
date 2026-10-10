#!/usr/bin/env bash
set -euo pipefail

APP_NAME="FluxChat"
BUNDLE_ID="com.fluxchat.app"
BINARY_NAME="fluxchat"
ICON="assets/icon.icns"

# The bundle version follows the crate (the update check compares the release
# tag against CARGO_PKG_VERSION; Info.plist shouldn't tell a different story).
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' crates/app/Cargo.toml | head -1)

echo "Building $APP_NAME for macOS..."
cargo build -p fluxchat --release

BINARY="target/release/$BINARY_NAME"
if [ ! -f "$BINARY" ]; then
    echo "Build failed: binary not found"
    exit 1
fi

echo "Creating macOS app bundle..."
APP_DIR="target/release/$APP_NAME.app"
CONTENTS="$APP_DIR/Contents"
rm -rf "$APP_DIR"
mkdir -p "$CONTENTS/MacOS" "$CONTENTS/Resources"

# Copy binary
cp "$BINARY" "$CONTENTS/MacOS/$BINARY_NAME"

# Copy icon
if [ -f "$ICON" ]; then
    cp "$ICON" "$CONTENTS/Resources/icon.icns"
fi

# Create Info.plist
cat > "$CONTENTS/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>$APP_NAME</string>
    <key>CFBundleDisplayName</key>
    <string>$APP_NAME</string>
    <key>CFBundleIdentifier</key>
    <string>$BUNDLE_ID</string>
    <key>CFBundleVersion</key>
    <string>$VERSION</string>
    <key>CFBundleShortVersionString</key>
    <string>$VERSION</string>
    <key>CFBundleExecutable</key>
    <string>$BINARY_NAME</string>
    <key>CFBundleIconFile</key>
    <string>icon</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <!-- Without these macOS never asks for access and the microphone
         delivers silence (mic test, voice messages, calls). -->
    <key>NSMicrophoneUsageDescription</key>
    <string>FluxChat uses the microphone for voice calls and voice messages.</string>
    <key>NSCameraUsageDescription</key>
    <string>FluxChat uses the camera for video calls.</string>
</dict>
</plist>
EOF

# Sign the bundle so macOS ties the Info.plist, the microphone permission
# and Keychain access (the saved session) to this app. With the "FluxChat
# Local" identity (scripts/macos-signing-cert.sh) the signature stays the
# same across rebuilds; an ad-hoc one changes every build, and macOS asks
# again for Keychain and microphone access after each rebuild.
SIGN_ID="FluxChat Local"
if security find-identity -v -p codesigning 2>/dev/null | grep -q "\"$SIGN_ID\""; then
    codesign --force --deep --sign "$SIGN_ID" "$APP_DIR"
else
    echo "note: signing ad-hoc. Run scripts/macos-signing-cert.sh once to stop"
    echo "      macOS asking for Keychain access after every rebuild."
    codesign --force --deep --sign - "$APP_DIR"
fi

echo ""
echo "Build successful!"
echo "  App bundle: $APP_DIR"
ls -lh "$BINARY"
echo ""
echo "To install, copy to /Applications:"
echo "  cp -r \"$APP_DIR\" /Applications/"
echo ""
echo "Or open the bundle directly:"
echo "  open \"$APP_DIR\""
