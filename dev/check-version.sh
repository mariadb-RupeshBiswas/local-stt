#!/bin/sh
# Prints the release version once Cargo.toml, Cargo.lock and pyproject.toml agree on a plain X.Y.Z.
# CI runs it on every pull request; release.yml runs it before publishing.
set -eu
cd "$(dirname "$0")/.."
cargo_version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
py_version="$(sed -n 's/^version = "\(.*\)"/\1/p' pyproject.toml | head -n 1)"
lock_version="$(awk '/^name = "local-stt"$/ { getline; print; exit }' Cargo.lock | sed -n 's/^version = "\(.*\)"/\1/p')"
[ "$cargo_version" = "$py_version" ] || { echo "Cargo.toml says $cargo_version, pyproject.toml says $py_version" >&2; exit 1; }
[ "$cargo_version" = "$lock_version" ] || { echo "Cargo.lock says $lock_version, Cargo.toml says $cargo_version: run cargo check" >&2; exit 1; }
# Pre-release suffixes read differently in Cargo and on PyPI, so releases use plain versions only.
echo "$cargo_version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' || { echo "not a plain X.Y.Z version: $cargo_version" >&2; exit 1; }
echo "$cargo_version"
