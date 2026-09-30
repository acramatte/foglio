# Agent guide

## Product and layout

Foglio is a local-first Markdown application for notes, repository documentation,
and agent specs/plans. Two interfaces operate on the same library:

- `notes`: a scriptable CLI for people and coding agents to search, create, update,
  and maintain Markdown through explicit local-file operations. It also provides
  reconciliation, index rebuilding, read-only diagnostics, and status. See the
  [README](README.md#build-and-test) and `notes --help` for its interface.
- `foglio-desktop`: the Tauri app for interactive browsing, source editing, and
  rendered preview, with guarded autosave and external-change handling.

Both use `notes-core`; keep domain behavior and filesystem guarantees there rather
than duplicating them in clients.

| Path | Responsibility | Cargo package / executable |
| --- | --- | --- |
| `crates/notes-core` | Domain, filesystem operations, SQLite index, watcher | `notes-core` |
| `crates/notes-cli` | Terminal interface | `notes-cli` / `notes` |
| `apps/desktop/src-tauri` | Tauri backend | `notes-desktop` / `foglio-desktop` |
| `apps/desktop/src` | TypeScript frontend | Private npm package `foglio-desktop` |

Rust crates are not published to crates.io. Distribution uses release installers
and CLI archives, not registry packages.

## Contracts and conventions

- Markdown files are authoritative. Preserve user frontmatter and unedited source
  bytes; never inject operational timestamps, IDs, or app-owned metadata.
- Paths identify documents; revision hashes guard writes. Preserve atomic-write,
  conflict, and unsafe-target checks. Do not weaken them to make tests pass. Read
  [path identity](docs/path-identity.md) and the
  [filesystem safety boundary](docs/phase1.md#s02-filesystem-guarantees-and-limits).
- SQLite is a disposable derived index, not a separate source of truth. Preserving
  non-derivable state requires an explicit documented decision.
- Report absence and errors honestly; do not fabricate metadata or silently
  substitute misleading values.
- Match neighboring Rust/TypeScript style and existing UI helpers. Update Rust/TS
  DTOs, command registration, API mocks, and fixtures together when contracts change.
- Keep dependencies pinned and lockfiles current; avoid unrelated upgrades. Use
  the repository-pinned toolchain and `npm ci` for reproducible installs.
- Keep changes focused. Read definitions and usages before editing. Tests use
  isolated temporary libraries, never personal notes.
- Use Conventional Commits (`feat`, `fix`, `docs`, etc.); git-cliff derives release
  notes from them. PRs explain why/how and include explicit `Tests:` evidence.
- Use an isolated branch/worktree when other work is present. Stage explicit paths,
  preserve unrelated changes, and rebase rather than merge main into a feature
  branch. Commit/push only when requested; do not merge PRs without approval.

## Verification

Run from the repository root unless noted. [CI](.github/workflows/ci.yml) defines
hosted gates. Never claim checks passed unless actually run.

Core and CLI (no desktop prerequisites):

```sh
cargo fmt --all -- --check
cargo build --locked -p notes-core -p notes-cli
cargo clippy --locked -p notes-core -p notes-cli --all-targets --all-features -- -D warnings
cargo test --locked -p notes-core -p notes-cli --all-features
python3 tests/acceptance/phase1.py
python3 tests/acceptance/phase2.py
```

Frontend (from `apps/desktop`):

```sh
npm ci
npm run typecheck
npm test
npm run build
```

Desktop backend (requires [native prerequisites](docs/phase4.md#build-and-run)):

```sh
cargo test --locked -p notes-desktop
cargo clippy --locked -p notes-desktop --all-targets -- -D warnings
```

For editor, watcher, or Tauri-boundary changes, run relevant native suites under
`tests/acceptance` against a freshly built binary. Build with
`npm run tauri -- build --debug --no-bundle` from `apps/desktop`; desktop harnesses
accept `FOGLIO_DESKTOP_BINARY` and `TAURI_DRIVER`. DOM-only tests do not qualify
real WebKit layout or filesystem behavior. Packaging changes also need staged
acceptance (`python3 tests/acceptance/phase7_package.py`) after packaging, with
native test prerequisites installed; see CI for setup.

For prose-only changes, check `git diff --check` and local documentation links;
state that runtime tests were not run instead of claiming runtime qualification.

## Versions, packaging, and releases

- Coordinate authorized version bumps across root `Cargo.toml` (inherited by
  crates), `Cargo.lock`, `apps/desktop/package.json`,
  `apps/desktop/package-lock.json`, and `apps/desktop/src-tauri/tauri.conf.json`.
- Reuse [Linux packaging](scripts/package-linux.sh) and
  [CLI archive packaging](scripts/package-release.sh). Debian packages include
  both executables; CLI archives contain `notes`, README, and LICENSE.
- `v*` tags trigger the [release workflow](.github/workflows/release.yml). Tagging,
  publishing releases, and changing signing configuration require explicit
  approval. Scripts use `FOLGLIO_VERSION` and `FOLGLIO_TARGET` (spelling intentional).
- Assets are Linux x86_64 `.deb` and CLI `.tar.gz`, plus experimental macOS
  arm64/x86_64 `.dmg` and CLI `.tar.gz`. A build does not qualify a platform:
  Linux local filesystems are the documented safety qualification; macOS is
  experimental and Windows is unsupported.
- Preserve signed `SHA256SUMS`, the signed `release.txt` update manifest, and build
  provenance attestations. Never put signing keys or credentials in the repository.
- Follow [git-cliff configuration](cliff.toml) and
  [release caveats](.github/release-notes.md). Preview pending changes with
  `git cliff --unreleased` before tagging; `--latest` renders the tagged range.
- Build success alone is not release qualification. Consult
  [Phase 7 evidence and open gates](docs/phase7.md).

## Source-of-truth documents

Read relevant contracts before changing behavior:

- [Product specification](docs/specs/product.md)
- [Technical specification](docs/specs/technical.md)
- [Verification specification](docs/specs/verification.md)
- [Decision register](docs/decisions.md): distinguish accepted choices from proposals.
- [Implementation plan](docs/implementation-plan.md) and [task backlog](docs/tasks.md)

Keep this guide operational; put detailed architecture, acceptance evidence, and
release procedures in the linked documents.
