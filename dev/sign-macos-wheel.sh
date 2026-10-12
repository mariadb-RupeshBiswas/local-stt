#!/bin/bash
# Signs the app inside a macOS wheel with a certificate, so macOS keeps permissions across updates.
# Usage: P12_PASSWORD=... dev/sign-macos-wheel.sh <wheel> <p12> [expected certificate SHA-1]
set -euo pipefail

wheel="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
p12="$2"
expected="$(echo "${3:-}" | tr '[:upper:]' '[:lower:]')"
: "${P12_PASSWORD:?P12_PASSWORD is not set}"
[ -s "$p12" ] || { echo "error: signing certificate $p12 is missing or empty" >&2; exit 1; }

work="$(mktemp -d)"
keychain="$work/sign.keychain-db"
trap 'security delete-keychain "$keychain" 2>/dev/null || true; rm -rf "$work"' EXIT

# A keychain of its own, outside the search list, so nothing else on the machine can use the key.
keychain_password="$(openssl rand -hex 16)"
security create-keychain -p "$keychain_password" "$keychain"
security set-keychain-settings -lut 3600 "$keychain"
security unlock-keychain -p "$keychain_password" "$keychain"
security import "$p12" -k "$keychain" -P "$P12_PASSWORD" -T /usr/bin/codesign > /dev/null
security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$keychain_password" "$keychain" > /dev/null
identity="$(security find-identity -p codesigning "$keychain" | awk '/ 1\) / { print $2; exit }')"
[ -n "$identity" ] || { echo "error: no signing identity in $p12" >&2; exit 1; }
leaf="$(echo "$identity" | tr '[:upper:]' '[:lower:]')"
if [ -n "$expected" ] && [ "$leaf" != "$expected" ]; then
  echo "error: certificate $leaf is not the release certificate $expected" >&2
  exit 1
fi

uvx --from wheel==0.48.0 wheel unpack "$wheel" -d "$work/unpacked" > /dev/null
root="$(echo "$work"/unpacked/*)"
scripts="$(echo "$root"/*.data/scripts)"

# The app's own install builds the bundle, so it matches what users get file for file.
HOME="$work/build-home" LOCAL_STT_DATA_DIR="$work/build-data" "$scripts/local-stt" install > /dev/null
app="$work/build-home/Applications/local-stt.app"
rm -f "$app/Contents/Resources/local-stt.source"
codesign --force --sign "$identity" --keychain "$keychain" --timestamp=none "$app"
codesign --verify --deep --strict "$app"

cp "$app/Contents/MacOS/local-stt" "$scripts/local-stt"
seal_dir="$(dirname "$scripts")/data/share/local-stt"
mkdir -p "$seal_dir"
cp "$app/Contents/_CodeSignature/CodeResources" "$seal_dir/CodeResources"
rm "$wheel"
uvx --from wheel==0.48.0 wheel pack "$root" -d "$(dirname "$wheel")" > /dev/null
[ -f "$wheel" ] || { echo "error: the repacked wheel has another name than $wheel" >&2; exit 1; }

# Installs the signed wheel as a user would and checks the app keeps the certificate signature.
HOME="$work/user-home" LOCAL_STT_DATA_DIR="$work/user-data" uvx --no-cache --from "$wheel" local-stt install > /dev/null
installed="$work/user-home/Applications/local-stt.app"
codesign --verify --deep --strict "$installed"
requirement="$(codesign -d -r- "$installed" 2>/dev/null)"
case "$requirement" in
  *"certificate leaf = H\"$leaf\""*) ;;
  *) echo "error: installed app is not signed by $leaf: $requirement" >&2; exit 1 ;;
esac
[ ! -e "$installed/Contents/Resources/local-stt.source" ] || { echo "error: installed app carries a build stamp" >&2; exit 1; }
shipped="$(unzip -p "$wheel" '*.data/scripts/local-stt' | shasum -a 256 | cut -d' ' -f1)"
copied="$(shasum -a 256 "$installed/Contents/MacOS/local-stt" | cut -d' ' -f1)"
[ "$shipped" = "$copied" ] || { echo "error: install changed the signed binary" >&2; exit 1; }
echo "Signed $(basename "$wheel") with $leaf; an install keeps that signature."
