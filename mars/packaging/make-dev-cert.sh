#!/bin/bash
# Creates a self-signed "Mars Local Development" code-signing identity in the login Keychain.
# A stable signing identity keeps Keychain "Always Allow" grants across rebuilds; ad-hoc
# signatures change with every build, so macOS asks for the password again each time.
# The private key never leaves the Keychain after import, and only codesign may use it.
set -euo pipefail

NAME="Mars Local Development"
KEYCHAIN="$HOME/Library/Keychains/login.keychain-db"

if security find-identity -v -p codesigning "$KEYCHAIN" | grep -q "\"$NAME\""; then
    echo "Identity \"$NAME\" already exists."
    exit 0
fi

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
chmod 700 "$WORK"

cat > "$WORK/cert.cnf" <<EOF
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
EOF

openssl req -x509 -newkey rsa:2048 -sha256 -days 3650 -nodes \
    -config "$WORK/cert.cnf" -keyout "$WORK/key.pem" -out "$WORK/cert.pem" 2>/dev/null

# macOS imports only legacy PKCS#12 encryption, so ask OpenSSL 3 for it explicitly.
PASS=$(openssl rand -hex 16)
openssl pkcs12 -export -legacy -inkey "$WORK/key.pem" -in "$WORK/cert.pem" \
    -name "$NAME" -passout "pass:$PASS" -out "$WORK/identity.p12" 2>/dev/null ||
    openssl pkcs12 -export -inkey "$WORK/key.pem" -in "$WORK/cert.pem" -name "$NAME" \
        -certpbe PBE-SHA1-3DES -keypbe PBE-SHA1-3DES -macalg sha1 \
        -passout "pass:$PASS" -out "$WORK/identity.p12"

security import "$WORK/identity.p12" -k "$KEYCHAIN" -P "$PASS" -T /usr/bin/codesign
# Trust only for code signing, only for this user. macOS asks for the login password once here.
security add-trusted-cert -r trustRoot -p codeSign -k "$KEYCHAIN" "$WORK/cert.pem"

security find-identity -v -p codesigning "$KEYCHAIN" | grep "\"$NAME\""
echo "Created \"$NAME\". build-app.sh signs with it automatically."
