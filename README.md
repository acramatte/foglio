# Foglio

A local-first Markdown notes application with a headless Rust core. CLI note operations and a Tauri desktop client are planned, not implemented.

**Status: Phase 0 complete.** Bootstrap checks passed locally on Linux x86_64 and in [hosted CI](https://github.com/acramatte/foglio/actions/runs/34130532775) for commit `33a91e1`. Phase 1 has not started.

Markdown files are authoritative. SQLite will be a disposable index. External editing is supported by the design; synchronization belongs to external filesystem tools.

## Build and test

Install [Rust using rustup](https://rustup.rs/). `rust-toolchain.toml` selects Rust 1.93.1 with rustfmt and Clippy. System-packaged Rust must provide that toolchain and both components separately.

```bash
cargo build --locked --workspace
cargo run --locked -p notes-cli -- --help
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
```

The product is **Foglio**; packages remain `notes-core` and `notes-cli`, and the executable is `notes`. Currently `notes` offers help and version only; no arguments prints help. Unsupported commands/flags exit with code 2. No library or application configuration is opened or written.

Black-box tests run the compiled executable with a cleared environment, temporary HOME/XDG config/cache/data directories, and a temporary library. They assert that these directories and the existing note remain unchanged. Core has no dependencies, UI code, or placeholder domain API. The CLI's path dependency establishes the shared-core boundary for Phase 1.

CI runs the locked build, tests, formatting, strict Clippy, and an isolated CLI help smoke on Ubuntu 24.04. Validate workflow syntax locally with `actionlint .github/workflows/ci.yml` (verified with actionlint 1.7.7). Core/CLI gates do not require Tauri or Node.

Linux is the first implementation target. macOS and Windows are not claimed supported until their filesystem and packaging gates pass. Bootstrap checks do not establish note-save safety.

## Scope and conventions

This request delivers **Phase 0 only**. The first usable filesystem/CLI milestone remains Phase 0–1; Phase 1 requires a separate implementation request. Do not scaffold SQLite, watchers, Tauri, sync, encryption, or plugins ahead of their phases.

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
