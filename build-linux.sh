#!/bin/bash
set -euo pipefail

# Check for required system dependencies
MISSING_DEPS=()
pkg-config --exists webkit2gtk-4.1 2>/dev/null || MISSING_DEPS+=("webkit2gtk-4.1")
pkg-config --exists gtk+-3.0 2>/dev/null || MISSING_DEPS+=("gtk3")
pkg-config --exists libsoup-3.0 2>/dev/null || MISSING_DEPS+=("libsoup3")
command -v xdotool >/dev/null 2>&1 || MISSING_DEPS+=("xdotool")

if [ ${#MISSING_DEPS[@]} -gt 0 ]; then
    echo "ERROR: Missing system dependencies: ${MISSING_DEPS[*]}"
    echo "Install them with your package manager, e.g.:"
    echo "  Arch:   pacman -S webkit2gtk-4.1 gtk3 libsoup3 xdotool"
    echo "  Debian: apt install libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev xdotool"
    echo "  Fedora: dnf install webkit2gtk4.1-devel gtk3-devel libsoup3-devel xdotool"
    exit 1
fi

# Ensure cargo bin is on PATH (for dx)
export PATH="$HOME/.cargo/bin:$PATH"

if ! command -v dx >/dev/null 2>&1; then
    echo "ERROR: Dioxus CLI (dx) not found. Install with: cargo install dioxus-cli"
    exit 1
fi

echo "Building fluxchat with Dioxus CLI (release)..."
cd crates/app
dx build --release
cd ../..

BIN_PATH="target/dx/fluxchat/release/linux/app/fluxchat"

if [ ! -f "$BIN_PATH" ]; then
    echo "ERROR: Build failed — binary not found at $BIN_PATH"
    exit 1
fi

# Create .desktop file and install structure
INSTALL_DIR="dist/linux"
rm -rf "$INSTALL_DIR"
mkdir -p "$INSTALL_DIR/bin"
mkdir -p "$INSTALL_DIR/share/applications"
mkdir -p "$INSTALL_DIR/share/icons/hicolor/256x256/apps"

cp "$BIN_PATH" "$INSTALL_DIR/bin/fluxchat"
cp assets/icons/icon_256x256.png "$INSTALL_DIR/share/icons/hicolor/256x256/apps/fluxchat.png"

cat > "$INSTALL_DIR/share/applications/fluxchat.desktop" << 'DESKTOP'
[Desktop Entry]
Name=FluxChat
Comment=Cross-platform Matrix chat client
Exec=fluxchat
Icon=fluxchat
Terminal=false
Type=Application
Categories=Network;InstantMessaging;Chat;
Keywords=matrix;chat;irc;messaging;
DESKTOP

echo ""
echo "Done! Binary at: $BIN_PATH"
echo "Install directory at: $INSTALL_DIR/"
echo ""
echo "Run directly:  ./$BIN_PATH"
echo "Install to /usr/local:"
echo "  sudo cp $INSTALL_DIR/bin/fluxchat /usr/local/bin/"
echo "  sudo cp $INSTALL_DIR/share/applications/fluxchat.desktop /usr/share/applications/"
echo "  sudo cp $INSTALL_DIR/share/icons/hicolor/256x256/apps/fluxchat.png /usr/share/icons/hicolor/256x256/apps/"
