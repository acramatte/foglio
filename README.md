# Foglio

A local-first Markdown notes application with a headless Rust core and a working filesystem CLI. The Tauri desktop client is planned, not implemented.

**Status: Phase 1 implemented and verified locally on Linux x86_64.** See [commands, safety limits and test evidence](docs/phase1.md). Phase 1 hosted CI is pending; the [previous hosted run](https://github.com/acramatte/foglio/actions/runs/34130532775) covers Phase 0 only.

Markdown files are authoritative. SQLite will be a disposable index. External editing is supported by the design; synchronization belongs to external filesystem tools.

## Build and test

Install [Rust using rustup](https://rustup.rs/). `rust-toolchain.toml` selects Rust 1.93.1 with rustfmt and Clippy. System-packaged Rust must provide that toolchain and both components separately.

```bash
cargo build --locked --workspace
cargo run --locked -p notes-cli -- --help
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
python3 tests/acceptance/phase1.py
```

The product is **Foglio**; packages remain `notes-core` and `notes-cli`, and the executable is `notes`. Commands: `init`, `new`, `list`, `show`, `move`, `delete`, `tags`, `tag add`, `tag remove`. Global flags: `--library <root>` and `--json`. No arguments prints help without opening state. Unsupported commands/flags exit 2.

Tests use actual temporary files and the compiled executable with isolated HOME/XDG state. They cover adoption, body/metadata preservation, guarded lifecycle operations, stale writes, no-clobber collisions, permissions/ACLs, fault injection and a killed staged writer. The Python harness also exercises interactive deletion through a PTY and configuration deletion/reselection. It requires Python 3 on Linux; the platform test requires writable `/dev/shm` on a different filesystem from the temporary library.

CI runs the locked build, Rust tests, Python acceptance harness, formatting, strict Clippy, and an isolated CLI help smoke on Ubuntu 24.04. Validate workflow syntax locally with `actionlint .github/workflows/ci.yml` (verified with actionlint 1.7.7). Core/CLI gates do not require Tauri or Node.

Linux local filesystems are the only supported Phase 1 target. ACL-/xattr-bearing replacement targets, ownership-changing replacements, symlinks and hard-link mutations are refused. Cooperative locks do not protect against arbitrary external-writer or hostile ancestor-swap races. Read the [safety boundary](docs/phase1.md#s02-filesystem-guarantees-and-limits) before use. macOS and Windows remain unsupported.

## Scope and conventions

This delivery completes the **Phase 0–1 filesystem/CLI milestone** locally. Phase 2 onward remains open. There is no SQLite, search, watcher, Tauri, sync, encryption, or plugin scaffold.

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
