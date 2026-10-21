#!/bin/sh
# Prints code=false when a pull request only touches Markdown or docs/, else code=true.
# Run on the PR's merge commit (fetch-depth 2): its first parent is the base it merges into.
set -eu
if [ "${EVENT:-}" != pull_request ]; then
  echo "code=true"
  exit 0
fi
others=$(git diff --name-only HEAD^1 HEAD | grep -v -E '(\.md$|^docs/)' || true)
if [ -n "$others" ]; then
  echo "code=true"
else
  echo "code=false"
fi
