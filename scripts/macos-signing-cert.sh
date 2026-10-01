#!/usr/bin/env bash
# One-time setup: create a self-signed "FluxChat Local" code-signing
# identity in your login keychain.
#
# macOS ties Keychain access (where FluxChat keeps your session and the key
# to its local data) and the microphone permission to the app's signature.
# Ad-hoc signatures change on every build, so macOS would ask again after
# each rebuild; builds signed with this identity keep the same one.
# build-macos.sh and `cargo run` (scripts/macos-run.sh) use it when present.
set -euo pipefail

NAME="FluxChat Local"
KEYCHAIN="$HOME/Library/Keychains/login.keychain-db"

if security find-identity -v -p codesigning | grep -q "\"$NAME\""; then
    echo "\"$NAME\" already exists."
    exit 0
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
cat > "$TMP/cert.cnf" <<CNF
[req]
distinguished_name = dn
x509_extensions = ext
prompt = no
[dn]
CN = $NAME
[ext]
basicConstraints = critical, CA:false
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, codeSigning
CNF

# The system's LibreSSL writes a PKCS#12 that `security import` accepts.
/usr/bin/openssl req -x509 -newkey rsa:2048 -nodes -days 3650 -config "$TMP/cert.cnf" \
    -keyout "$TMP/key.pem" -out "$TMP/cert.pem" 2>/dev/null
/usr/bin/openssl pkcs12 -export -inkey "$TMP/key.pem" -in "$TMP/cert.pem" \
    -name "$NAME" -out "$TMP/id.p12" -passout pass:fluxchat

security import "$TMP/id.p12" -k "$KEYCHAIN" -P fluxchat -T /usr/bin/codesign
echo "Trusting the certificate for code signing (macOS asks for your password)..."
security add-trusted-cert -r trustRoot -p codeSign -k "$KEYCHAIN" "$TMP/cert.pem"

if security find-identity -v -p codesigning | grep -q "\"$NAME\""; then
    echo "Created \"$NAME\". Rebuild with ./build-macos.sh."
    echo "macOS asks once more for Keychain access; choose Always Allow."
else
    echo "The identity was imported but isn't valid for code signing." >&2
    exit 1
fi
