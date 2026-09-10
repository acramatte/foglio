#!/usr/bin/env bash
# Build a distributable release tarball for the notes CLI from this checkout.
# Output: target/release/foglio-notes-v<version>-<os>-<arch>.tar.gz
set -euo pipefail
cd "$(dirname "$0")/.."
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) os=linux; arch=x86_64 ;;
  Linux-aarch64|Linux-arm64) os=linux; arch=aarch64 ;;
  Darwin-arm64) os=darwin; arch=aarch64 ;;
  Darwin-x86_64) os=darwin; arch=x86_64 ;;
  *) printf '%s\n' "Unsupported build host: $(uname -s)-$(uname -m)" >&2; exit 1 ;;
esac
export CARGO_TARGET_DIR="$PWD/target"
cargo build --locked --release -p notes-cli
version=$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')
out="target/release/foglio-notes-v${version}-${os}-${arch}.tar.gz"
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
install -m 0755 target/release/notes "$stage/notes"
install -m 0644 README.md LICENSE "$stage/"
# Reproducible metadata is Linux-only; macOS bsdtar gets a plain tar.
if [[ "$os" == linux ]]; then
  tar -C "$stage" --owner=0 --group=0 --sort=name --mtime='UTC 1970-01-01' \
    -czf "$PWD/$out" notes README.md LICENSE
else
  tar -C "$stage" -czf "$PWD/$out" notes README.md LICENSE
fi
printf '%s\n' "$out"
