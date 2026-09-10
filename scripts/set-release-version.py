#!/usr/bin/env python3
"""Write a Tauri config override file setting the release version.

Usage: set-release-version.py <version> <output.json>

The override is merged by `tauri build --config <file>` so bundle names use
the released tag version regardless of the version baked into tauri.conf.json.
"""
import json
import sys


def main() -> None:
    if len(sys.argv) != 3:
        sys.exit(f"usage: {sys.argv[0]} <version> <output.json>")
    version, out_path = sys.argv[1], sys.argv[2]
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump({"version": version}, f)
        f.write("\n")


if __name__ == "__main__":
    main()
