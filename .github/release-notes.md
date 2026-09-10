Local-first Markdown document application: headless Rust core, filesystem CLI (`notes`), and Tauri desktop editor with guarded autosave.

**Supported platform: Linux x86_64 only** (Debian/amd64 package qualified on Ubuntu 24.04/26.04). macOS and Windows are unsupported.

## Artifacts

- `foglio-notes-vX.Y.Z-linux-x86_64.tar.gz` — CLI-only (`notes`) plus README and LICENSE. Extract and copy `notes` to a directory on your `PATH`.
- `Foglio_X.Y.Z_amd64.deb` — Debian package installing `/usr/bin/notes`, `/usr/bin/foglio-desktop` (Tauri desktop app), icon and desktop entry. Install with `sudo apt install ./Foglio_X.Y.Z_amd64.deb`.

## Known limitations

The v1 release gate is not complete. See the [Phase 7 evidence and open release gates](docs/phase7.md) for details:

- Performance budgets are undecided; measurements are available but not budgeted.
- Physical-display and assistive-technology qualification has not been performed.
- Actual second-device transfer acceptance is explicitly deferred/blocked.
- Local filesystems only; ACL-/xattr-bearing targets, ownership-changing replacements, symlinks and hard-link mutations are refused. Cooperative locks do not protect against arbitrary external-writer races.
- Markdown files are authoritative; the SQLite index is disposable and can be rebuilt with `notes reindex`.

## Verification

Each release build runs the locked Rust/frontend checks, constructs the Debian package, and runs the staged native WebKit acceptance harness in hosted CI.
