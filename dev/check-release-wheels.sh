#!/bin/sh
# Fails unless <dir> holds exactly one macOS and one Windows wheel, and the macOS one is the signed one.
set -eu
dir="$1"
total="$(ls "$dir"/*.whl 2>/dev/null | wc -l)"
mac="$(ls "$dir"/*macosx*.whl 2>/dev/null | wc -l)"
win="$(ls "$dir"/*win_amd64.whl 2>/dev/null | wc -l)"
if [ "$total" -ne 2 ] || [ "$mac" -ne 1 ] || [ "$win" -ne 1 ]; then
  echo "expected one macOS and one Windows wheel in $dir, found:" >&2
  ls "$dir" >&2
  exit 1
fi
# The sign job adds the bundle's seal; a wheel without it is a plain build and would reset permissions.
if ! unzip -l "$dir"/*macosx*.whl | grep -q '\.data/data/share/local-stt/CodeResources$'; then
  echo "the macOS wheel in $dir is not the signed one" >&2
  exit 1
fi
echo "release wheels ok"
