#!/usr/bin/env bash
# Build a distributable release tarball for the notes CLI from this checkout.
# Output: target/<target>/release/foglio-notes-v<version>-<os>-<arch>.tar.gz
# The version comes from FOLGLIO_VERSION when set (CI release builds), else
# from the workspace Cargo.toml version.
# The target triple comes from FOLGLIO_TARGET when set (CI cross builds, e.g.
# the macOS Intel job running on an arm64 runner), else from the host
# toolchain. Artifact names follow the requested triple, never the build host,
# so a cross build cannot publish a host-arch binary under a foreign name.
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_TARGET_DIR="$PWD/target"

target="${FOLGLIO_TARGET:-$(rustc -vV | sed -n 's/^host: //p')}"
case "$target" in
  *-apple-darwin) os=darwin ;;
  *-linux-gnu | *-linux-musl) os=linux ;;
  *) printf '%s\n' "Unsupported release target: ${target:-<empty>}" >&2; exit 1 ;;
esac
case "$target" in
  x86_64-*) arch=x86_64 ;;
  aarch64-*) arch=aarch64 ;;
  *) printf '%s\n' "Unsupported release target: $target" >&2; exit 1 ;;
esac

cargo build --locked --release -p notes-cli --target "$target"
if [[ -n "${FOLGLIO_VERSION:-}" ]]; then
  version="$FOLGLIO_VERSION"
else
  version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
fi
out="target/${target}/release/foglio-notes-v${version}-${os}-${arch}.tar.gz"
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
install -m 0755 "target/${target}/release/notes" "$stage/notes"
install -m 0644 README.md LICENSE "$stage/"
# Reproducible metadata is Linux-only; macOS bsdtar gets a plain tar.
if [[ "$os" == linux ]]; then
  tar -C "$stage" --owner=0 --group=0 --sort=name --mtime='UTC 1970-01-01' \
    -czf "$PWD/$out" notes README.md LICENSE
else
  tar -C "$stage" -czf "$PWD/$out" notes README.md LICENSE
fi
printf '%s\n' "$out"
