---
id: 01M20M73Q6C0R4R3WGC69H8FDD
---
# Foglio

A local-first Markdown document application for notes, repository docs and agent specs/plans with a headless Rust core, filesystem CLI, and Tauri desktop source editor with guarded autosave.

**Status: Phases 1–5 implemented and verified locally on Linux x86_64.** [Editing, autosave and verification](docs/phase5.md). [Desktop setup and native verification](docs/phase4.md). See [filesystem safety](docs/phase1.md), [index/search commands and benchmarks](docs/phase2.md), and [live reconciliation APIs, recovery and evidence](docs/phase3.md). Hosted CI for these changes is pending; the [previous hosted run](https://github.com/acramatte/foglio/actions/runs/34130532775) covers Phase 0 only.

Markdown files are authoritative. SQLite is a disposable index. External editing is supported by the design; synchronization belongs to external filesystem tools.

## Build and test

Install [Rust using rustup](https://rustup.rs/). `rust-toolchain.toml` selects Rust 1.93.1 with rustfmt and Clippy. System-packaged Rust must provide that toolchain and both components separately.

```bash
cargo build --locked -p notes-core -p notes-cli
cargo run --locked -p notes-cli -- --help
cargo fmt --all -- --check
cargo clippy --locked -p notes-core -p notes-cli --all-targets --all-features -- -D warnings
cargo test --locked -p notes-core -p notes-cli --all-features
python3 tests/acceptance/phase1.py
python3 tests/acceptance/phase2.py
```

The product is **Foglio**; packages remain `notes-core` and `notes-cli`, and the executable is `notes`. Commands: `init`, `new`, `list`, `show`, `move`, `delete`, `tags`, `tag add`, `tag remove`, `search`, `rescan`, `reindex`, `status`. Global flags: `--library <root>` and `--json`. No arguments prints help without opening state. Unsupported commands/flags exit 2.

Tests use actual temporary files and the compiled executable with isolated HOME/XDG state. They cover non-mutating initialization and selection, body/metadata preservation, guarded lifecycle operations, stale writes, no-clobber collisions, permissions/ACLs, fault injection and a killed staged writer. The Python harness also exercises interactive deletion through a PTY and configuration deletion/reselection. It requires Python 3 on Linux; the platform test requires writable `/dev/shm` on a different filesystem from the temporary library.

CI runs the locked build, Rust tests, Python acceptance harness, formatting, strict Clippy, and an isolated CLI help smoke on Ubuntu 24.04. Validate workflow syntax locally with `actionlint .github/workflows/ci.yml` (verified with actionlint 1.7.7). Core/CLI gates do not require Tauri or Node.

Linux local filesystems are the only supported Phase 1 target. ACL-/xattr-bearing replacement targets, ownership-changing replacements, symlinks and hard-link mutations are refused. Cooperative locks do not protect against arbitrary external-writer or hostile ancestor-swap races. Read the [safety boundary](docs/phase1.md#s02-filesystem-guarantees-and-limits) before use. macOS and Windows remain unsupported.

## Desktop

Install the [native prerequisites](docs/phase4.md#build-and-run), then run `npm ci` and `npm run tauri -- dev` from `apps/desktop`. The built binary is `target/debug/foglio-desktop`; production builds use `target/release/foglio-desktop`. Enter an existing library path to browse folders, tags and search results with safe Markdown preview. Selection and `init` never modify Markdown. Supported files need no frontmatter or IDs. Paths identify documents; hashes protect revisions. Existing `id` metadata is preserved without identity semantics. External moves leave the old selection unavailable rather than silently following matching content. See the [identity contract and verification](docs/path-identity.md).

```text
Markdown → core index/watcher → Tauri commands → navigation + source/preview
Source buffer → revision-guarded autosave → core atomic file write → derived index
```

Healthy monitoring is a quiet footer indicator, separate from save status. Use New note and the Source/Preview controls to edit, with Ctrl+E to toggle and Ctrl+S to flush. Autosave preserves frontmatter; tags, move/rename and permanent deletion have separate controls. Failed saves retain your source and block navigation/close. Stale or missing files pause autosave; Phase 6's reload/discard and save-copy choices remain open. Search behavior is unchanged.

## Scope and conventions

This delivery completes **Phase 5 source editing and autosave** locally. Phase 6 conflict-resolution UX and Phase 7 release hardening remain open. Rich editing was omitted after the preservation spike. No daemon, sync integration, encryption, history or plugin scaffold is included.

- Keep transport handling in CLI and domain behavior in the UI-independent core.
- Pin selected toolchains/dependencies and retain `Cargo.lock` in version control.
- Test only temporary libraries; never use personal notes as fixtures.
- Use focused semantic commits and rationale-rich PRs with affected task IDs, phase status, and explicit **Tests** evidence. Do not merge main into feature branches; rebase when necessary.
- The Markdown backlog is the durable task-status authority. Close tasks only against real execution evidence; hosted CI needs its own run evidence once a remote exists.

## Project documents and precedence

- [Product specification](docs/specs/product.md): scope and normative requirements.
- [Technical specification](docs/specs/technical.md): domain and safety contracts.
- [Implementation plan](docs/implementation-plan.md): phase gates and delivery discipline.
- [Task backlog](docs/tasks.md): work items and execution evidence.
- [Verification specification](docs/specs/verification.md): acceptance scenarios.
- [Decision register](docs/decisions.md): established, accepted, and proposed choices.

Source: the user-supplied **Headless Markdown Notes — v1 Specification & Implementation Plan.md**, sections 1–44, at `/home/alexis/Downloads/Headless Markdown Notes — v1 Specification & Implementation Plan.md`; it is not vendored here. Explicit user-approved amendments recorded in the decision register take precedence, followed by the original brief/product requirements, then technical contracts, then planning/task guidance. Proposed defaults are not accepted decisions. Surface contradictions instead of silently changing requirements.

## License

[MIT](LICENSE), selected by the project owner for Phase 0.
