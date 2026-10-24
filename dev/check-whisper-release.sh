#!/bin/sh
# Opens one issue when whisper.cpp has a release newer than vendor/whisper.cpp.
# DRY_RUN=1 prints instead of opening; PINNED=<sha> overrides the submodule commit.
set -eu

pinned="${PINNED:-$(git ls-tree HEAD vendor/whisper.cpp | awk '{print $3}')}"
latest="$(gh api repos/ggml-org/whisper.cpp/releases/latest --jq .tag_name)"
# the tag comes from another repo, so only a plain version goes any further
echo "$latest" | grep -Eqx 'v[0-9]+\.[0-9]+\.[0-9]+' || { echo "unexpected tag: $latest"; exit 1; }
latest_sha="$(gh api "repos/ggml-org/whisper.cpp/commits/$latest" --jq .sha)"

if [ "$pinned" = "$latest_sha" ]; then
  echo "whisper.cpp $latest is the pinned release"
  exit 0
fi

title="whisper.cpp $latest is out: bump whisper.cpp and llama.cpp together"
seen="$(gh issue list --state all --limit 200 --json title --jq "[.[] | select(.title == \"$title\")] | length")"
if [ "$seen" != "0" ]; then
  echo "already raised: $title"
  exit 0
fi

body="whisper.cpp $latest is newer than the pinned \`vendor/whisper.cpp\` ($pinned).

1. Move \`vendor/whisper.cpp\` to $latest.
2. Move \`vendor/llama.cpp\` to the newest commit whose ggml matches whisper.cpp's (AGENTS.md).
3. Run local CI (AGENTS.md, Commands), then open a PR.

Opened by the weekly security workflow."

if [ "${DRY_RUN:-}" = "1" ]; then
  printf 'would open: %s\n\n%s\n' "$title" "$body"
  exit 0
fi
gh issue create --title "$title" --body "$body"
