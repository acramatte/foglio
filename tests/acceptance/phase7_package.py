#!/usr/bin/env python3
"""Staged Debian payload smoke, NOT a fresh-OS package-manager install.

Extracts only into a temporary directory, exercises the shipped CLI and native
WebKit desktop, then removes the payload while preserving a library manifest.
Host WebKit/GTK dependencies are reused. No sudo or host package changes.
"""
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
PACKAGE = Path(os.environ.get(
    "FOGLIO_DEB", ROOT / "target/release/bundle/deb/Foglio_0.1.0_amd64.deb"))


def run(*args, env=None):
    return subprocess.run(args, check=True, text=True, capture_output=True, env=env).stdout


def manifest(root):
    return {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in root.rglob("*") if p.is_file()}


def main():
    assert PACKAGE.is_file(), f"Run bash scripts/package-linux.sh first: {PACKAGE}"
    with tempfile.TemporaryDirectory(prefix="foglio-package-") as temporary:
        base = Path(temporary)
        payload, control = base / "payload", base / "control"
        run("dpkg-deb", "--extract", str(PACKAGE), str(payload))
        run("dpkg-deb", "--control", str(PACKAGE), str(control))
        # Uninstallation must not contain hooks that could touch user notes.
        for name in ("preinst", "postinst", "prerm", "postrm"):
            assert not (control / name).exists(), name
        print(run("dpkg-deb", "--field", str(PACKAGE)), flush=True)
        cli = payload / "usr/bin/notes"
        desktop = payload / "usr/bin/foglio-desktop"
        assert cli.is_file() and desktop.is_file()
        entries = list((payload / "usr/share/applications").glob("*.desktop"))
        assert len(entries) == 1
        assert "Exec=foglio-desktop" in entries[0].read_text()
        library = base / "library"
        library.mkdir()
        (library / "ordinary.md").write_bytes(b"# Ordinary\n\nUnicode: caf\xc3\xa9\n")
        before = manifest(library)
        env = dict(os.environ)
        for key, directory in (("HOME", "home"), ("XDG_CONFIG_HOME", "config"),
                               ("XDG_CACHE_HOME", "cache"), ("XDG_DATA_HOME", "data")):
            path = base / directory
            path.mkdir()
            env[key] = str(path)
        assert "doctor" in run(str(cli), "--help", env=env)
        run(str(cli), "--library", str(library), "reindex", env=env)
        run(str(cli), "--library", str(library), "doctor", env=env)
        results = run(str(cli), "--library", str(library), "search", "Ordinary", env=env)
        assert "ordinary.md" in results
        assert manifest(library) == before
        # Only app-owned disposable cache is removed, never Markdown.
        shutil.rmtree(base / "cache")
        run(str(cli), "--library", str(library), "reindex", env=env)
        run(str(cli), "--library", str(library), "doctor", env=env)
        assert manifest(library) == before
        native_env = dict(os.environ, FOGLIO_DESKTOP_BINARY=str(desktop))
        for name in ("phase4.py", "phase5.py", "phase6.py", "phase7_keyboard.py"):
            harness = ROOT / "tests/acceptance" / name
            assert harness.is_file(), harness
            subprocess.run(["python3", str(harness)], check=True, env=native_env)
        shutil.rmtree(payload)
        assert manifest(library) == before
        print("PASS: staged Debian payload, CLI doctor/search/cache rebuild, native suites, payload removal preserves library")
        print("LIMIT: not a package-manager install/uninstall or fresh dependency environment; second-device acceptance not exercised")


if __name__ == "__main__":
    main()
