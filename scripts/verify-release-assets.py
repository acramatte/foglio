#!/usr/bin/env python3
"""Refuse partial or misnamed platform assets before signing a release."""
import re
import sys
from pathlib import Path


def expected_assets(version):
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version):
        raise ValueError("expected a stable X.Y.Z version")
    return {
        f"Foglio_{version}_amd64.deb",
        f"Foglio_{version}_aarch64.dmg",
        f"Foglio_{version}_x64.dmg",
        f"foglio-notes-v{version}-linux-x86_64.tar.gz",
        f"foglio-notes-v{version}-darwin-aarch64.tar.gz",
        f"foglio-notes-v{version}-darwin-x86_64.tar.gz",
    }


def verify(directory, version):
    expected = expected_assets(version)
    entries = {path.name: path for path in directory.iterdir()}
    if set(entries) != expected:
        raise ValueError(
            f"missing assets: {sorted(expected - entries.keys())}; "
            f"unexpected assets: {sorted(entries.keys() - expected)}"
        )
    for path in entries.values():
        if path.is_symlink() or not path.is_file() or path.stat().st_size == 0:
            raise ValueError(f"asset must be a nonempty regular file: {path.name}")


if __name__ == "__main__":
    verify(Path(sys.argv[1]), sys.argv[2])
    print("PASS: complete six-artifact release set")
