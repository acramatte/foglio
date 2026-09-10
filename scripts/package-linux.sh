#!/usr/bin/env bash
# Build both executables from this checkout before bundling the Debian package.
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "$(uname -s)" != Linux || "$(uname -m)" != x86_64 ]]; then
  printf '%s\n' 'Only Linux x86_64 packaging is qualified by this workflow.' >&2
  exit 1
fi
# The explicit target directory matches the bundle file map, even when the
# caller normally uses a shared Cargo target directory.
export CARGO_TARGET_DIR="$PWD/target"
cargo build --locked --release -p notes-cli
cd apps/desktop
npm ci
npm run tauri -- build --bundles deb -- --locked
