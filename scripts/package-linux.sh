#!/usr/bin/env bash
# Build both executables from this checkout before bundling the Debian package.
# FOLGLIO_VERSION (optional) overrides the bundle version, e.g. from a release tag.
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "$(uname -s)" != Linux || "$(uname -m)" != x86_64 ]]; then
  printf '%s\n' 'Only Linux x86_64 packaging is qualified by this workflow.' >&2
  exit 1
fi
# The explicit target directory matches the bundle file map, even when the
# caller normally uses a shared Cargo target directory.
export CARGO_TARGET_DIR="$PWD/target"
override=""
if [[ -n "${FOLGLIO_VERSION:-}" ]]; then
  # Version override lives under target/ so it never enters the source tree.
  # Tauri resolves --config relative to apps/desktop/src-tauri, hence ../..
  override="target/release-version.conf.json"
  mkdir -p target
  printf '{"version": "%s"}\n' "$FOLGLIO_VERSION" > "$override"
fi
cargo build --locked --release -p notes-cli
cd apps/desktop
npm ci
if [[ -n "$override" ]]; then
  npm run tauri -- build --bundles deb --config "../../$override" -- --locked
else
  npm run tauri -- build --bundles deb -- --locked
fi
