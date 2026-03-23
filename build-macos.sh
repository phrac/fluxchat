#!/bin/bash
set -e

echo "Building fluxchat (release)..."
cargo build -p fluxchat-app --release

echo "Packaging FluxChat.app..."
mkdir -p FluxChat.app/Contents/{MacOS,Resources}

# Copy binary
cp target/release/fluxchat-app FluxChat.app/Contents/MacOS/fluxchat-app

# Copy icon
cp assets/fluxchat.icns FluxChat.app/Contents/Resources/fluxchat.icns

# Copy Info.plist (already in place, but ensure it's there)
cat > FluxChat.app/Contents/Info.plist << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>FluxChat</string>
    <key>CFBundleDisplayName</key>
    <string>FluxChat</string>
    <key>CFBundleIdentifier</key>
    <string>com.fluxchat.app</string>
    <key>CFBundleVersion</key>
    <string>0.1.0</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleExecutable</key>
    <string>fluxchat-app</string>
    <key>CFBundleIconFile</key>
    <string>fluxchat</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF

echo ""
echo "Done! FluxChat.app is ready."
echo "Run with: open FluxChat.app"
echo "Or install: cp -r FluxChat.app /Applications/"
