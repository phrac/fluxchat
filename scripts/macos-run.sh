#!/usr/bin/env bash
# Cargo runner on macOS (.cargo/config.toml): signs the fluxchat binary with
# the "FluxChat Local" identity (scripts/macos-signing-cert.sh), as the app
# bundle is, so `cargo run` builds can use the Keychain item without a
# prompt after every rebuild. Other binaries (tests) run unchanged.
BIN="$1"
if [ "$(basename "$BIN")" = "fluxchat" ] \
    && security find-identity -v -p codesigning 2>/dev/null | grep -q '"FluxChat Local"'; then
    codesign --force --sign "FluxChat Local" --identifier com.fluxchat.app "$BIN" 2>/dev/null \
        || echo "macos-run: couldn't sign $BIN" >&2
fi
exec "$@"
