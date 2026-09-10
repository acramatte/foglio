#!/usr/bin/env bash
# Build a distributable release tarball for the notes CLI from this checkout.
# Usage: scripts/package-release.sh   -> target/release/foglio-notes-v<version>-linux-x86_64.tar.gz
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "$(uname -s)" != Linux || "$(uname -m)" != x86_64 ]]; then
  printf '%s\n' 'Only Linux x86_64 packaging is qualified by this workflow.' >&2
  exit 1
fi
export CARGO_TARGET_DIR="$PWD/target"
cargo build --locked --release -p notes-cli
version=$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')
out="target/release/foglio-notes-v${version}-linux-x86_64.tar.gz"
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
install -m 0755 target/release/notes "$stage/notes"
install -m 0644 README.md LICENSE "$stage/"
tar -C "$stage" --owner=0 --group=0 --sort=name --mtime='UTC 1970-01-01' \
  -czf "$PWD/$out" notes README.md LICENSE
printf '%s\n' "$out"
