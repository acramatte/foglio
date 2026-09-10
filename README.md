> **✨ The origin story**
> My note-taking app’s subscription expired right as GPT6-Astra dropped.
> Coincidence? Maybe.
> But Since I was only using a subset of the paid features anyway, I told 
> myself that I might as well vibe-code my own.
> And here it is: **raw, un-reviewed** (but tested) and 100% mine. 🚀

# Foglio

A local-first Markdown document application for notes, repository docs and agent specs/plans with a headless Rust core, filesystem CLI, and Tauri desktop source editor with guarded autosave.

## Your notes stay yours

Foglio works with the Markdown files you already have. Open a folder of notes, repository documentation, or agent plans; Foglio never requires frontmatter, generated IDs, or a proprietary file format. Your files remain useful in any text editor.

- **Browse a real library** — navigate folders, filter exact tags, and search titles, paths, tags, and note text.
- **Write in Markdown** — create notes, edit source, and switch to a safe rendered preview. CommonMark and useful GFM, including tables, task lists, fenced code, links, and strikethrough, are supported.
- **Keep control of changes** — guarded autosave preserves frontmatter and refuses silent overwrites. Rename, move, tag, and permanently delete notes through explicit controls.
- **Use your normal tools too** — Foglio watches external edits. Clean notes refresh; if a note changes while you are editing, saving pauses and you can reload the file, discard local work deliberately, or save it as a new note.
- **Find and repair derived state** — Markdown is authoritative; SQLite is only a disposable local index. The read-only `doctor` command explains source and cache problems, while `reindex` rebuilds the index without rewriting your notes.
- **Work from the desktop or terminal** — the desktop app and `notes` command-line tool share the same local library and behavior. The scriptable CLI is practical for people and coding agents to search, create, and update Markdown through explicit local-file operations.

```text
Markdown files → local index and watcher → desktop navigation, source, and preview
Source edits → revision-guarded autosave → atomic Markdown write → refreshed index
```

## Desktop

Download the installer for your platform from [GitHub Releases](https://github.com/acramatte/foglio/releases). Current release `v0.1.1` includes:

| Platform | Download | SHA-256 |
|---|---|---|
| macOS (Apple Silicon) | [`Foglio_0.1.1_aarch64.dmg`](https://github.com/acramatte/foglio/releases/download/v0.1.1/Foglio_0.1.1_aarch64.dmg) | `ccc20877f663303e42abc40fb5cafa4bbc954ea21cfc49334cbb094e9814016e` |
| macOS (Intel) | [`Foglio_0.1.1_x64.dmg`](https://github.com/acramatte/foglio/releases/download/v0.1.1/Foglio_0.1.1_x64.dmg) | `d063ef51228419f93ddd84ebcbb93a6c350d5ebcab5c0d58bd195e8c72a846ab` |
| Debian/Ubuntu (x86_64) | [`Foglio_0.1.1_amd64.deb`](https://github.com/acramatte/foglio/releases/download/v0.1.1/Foglio_0.1.1_amd64.deb) | `bbf24be00f6dd60282fa6e0d6cf7c057feb50c0bf696e72f0333a3cf8a356655` |

Standalone `notes` CLI archives are also available:

| Platform | Download | SHA-256 |
|---|---|---|
| macOS (Apple Silicon) | [`foglio-notes-v0.1.1-darwin-aarch64.tar.gz`](https://github.com/acramatte/foglio/releases/download/v0.1.1/foglio-notes-v0.1.1-darwin-aarch64.tar.gz) | `fb20b6446a2c85464d73f311b63a7d21ea23b5b4399eda9e29a0313d14911dce` |
| Linux (x86_64) | [`foglio-notes-v0.1.1-linux-x86_64.tar.gz`](https://github.com/acramatte/foglio/releases/download/v0.1.1/foglio-notes-v0.1.1-linux-x86_64.tar.gz) | `c9e9371abf322081dc18672f0225737c4ca98100e18a189f5af0d367cf681546` |

Verify a download before installing it, for example: `sha256sum Foglio_0.1.1_amd64.deb` on Linux or `shasum -a 256 Foglio_0.1.1_aarch64.dmg` on macOS.

Enter an existing library path to browse folders, tags, and search results. Use **New note** and the **Source/Preview** controls to edit; Ctrl+E toggles the view and Ctrl+S flushes a save. Selection and `init` never modify Markdown. Paths identify documents, while hashes protect revisions. Existing `id` metadata is preserved as ordinary metadata. See the [identity contract](docs/path-identity.md) and [editing and recovery details](docs/phase6.md).

## Development

To run the desktop app from source, install the [native prerequisites](docs/phase4.md#build-and-run), then run `npm ci` and `npm run tauri -- dev` from `apps/desktop`.

**Status: Phases 1–6 and the local-hardening slice of Phase 7 are implemented and verified locally on Linux x86_64.** This includes diagnostics, keyboard/focus hardening, reproducible 1k/50k-note measurements, and Debian packaging. The v1 release gate remains open: hosted CI, agreed performance budgets, large-library UI behavior, physical-display/assistive-technology qualification, and an actual second-device transfer still need evidence. See [Phase 7 evidence and open release gates](docs/phase7.md).

The documented file-safety qualification currently covers Linux local filesystems. Foglio deliberately refuses risky replacements involving symlinks, hard links, ownership changes, and ACL/xattr-bearing targets. Cooperative locking cannot prevent arbitrary external-writer or hostile ancestor-swap races. Read the [filesystem safety boundary](docs/phase1.md#s02-filesystem-guarantees-and-limits) before relying on it.

## Build and test

Install [Rust using rustup](https://rustup.rs/). `rust-toolchain.toml` selects Rust 1.93.1 with rustfmt and Clippy. The core and CLI do not require the desktop dependencies.

```bash
cargo build --locked -p notes-core -p notes-cli
cargo run --locked -p notes-cli -- --help
cargo fmt --all -- --check
cargo clippy --locked -p notes-core -p notes-cli --all-targets --all-features -- -D warnings
cargo test --locked -p notes-core -p notes-cli --all-features
python3 tests/acceptance/phase1.py
python3 tests/acceptance/phase2.py
```

The product is **Foglio**; the executable is `notes`. Its commands cover library selection, note creation and lifecycle, tags, literal search, reconciliation, index rebuilding, read-only diagnostics, and status. Run `notes --help` for the full interface.

Tests use isolated temporary libraries and the compiled executable; personal notes are never used as fixtures. The full local qualification also covers the desktop app, native WebKit acceptance, Debian package construction, and staged-package execution. Run `bash scripts/package-linux.sh` to create a Debian/amd64 package. Detailed evidence, limits, and release blockers are in [Phase 7](docs/phase7.md).

## Project documents

- [Product specification](docs/specs/product.md): user-facing scope and requirements.
- [Technical specification](docs/specs/technical.md): domain and safety contracts.
- [Implementation plan](docs/implementation-plan.md): phase gates and delivery discipline.
- [Task backlog](docs/tasks.md): work items and execution evidence.
- [Verification specification](docs/specs/verification.md): acceptance scenarios.
- [Decision register](docs/decisions.md): established, accepted, and proposed choices.

Foglio intentionally does not include a sync engine, encryption, version history, plugins, attachments, semantic/vector search, or AI features. Synchronize the notes folder with the filesystem tool you trust; Foglio will treat its changes as normal external edits.

## License

[MIT](LICENSE)
