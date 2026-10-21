#!/bin/sh
# Prints the CHANGELOG.md section for one version, for the GitHub release notes.
# Usage: sh dev/release-notes.sh 0.12.3 [CHANGELOG.md]
set -eu
version=$1
file=${2:-CHANGELOG.md}
notes=$(awk -v head="## [$version]" '
  index($0, head) == 1 { on = 1; next }
  on && /^## \[/ { exit }
  on { print }
' "$file" | sed -e '/./,$!d')
if [ -z "$notes" ]; then
  echo "no CHANGELOG section for $version" >&2
  exit 1
fi
printf '%s\n\nFull history: [CHANGELOG.md](https://github.com/mariadb-RupeshBiswas/local-stt/blob/main/CHANGELOG.md)\n' "$notes"
