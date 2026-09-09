# Implementation tasks

**Status: Phase 0 verified locally and in hosted CI. P1-01–P1-07, P2-01–P2-04, P3-01–P3-03, P4-01–P4-04, P5-01–P5-04, and P6-01–P6-03 verified locally; hosted CI for Phases 1–6 pending. Phase 7 remains open.** Dependencies are prerequisite task IDs. Each row is a bounded work item, not an instruction to scaffold future phases. Proposed decisions must be resolved at their named gate.

## Completion protocol

Use `[ ]` open, `[-]` in progress, `[x]` verified complete, `[!]` blocked. To close a task, append evidence using the template below and satisfy its acceptance criterion plus the phase gate. All dependencies must be complete unless the plan explicitly changes.

```text
Task:
Status:
Decision changes:
Implementation/PR:
Tests: <exact command; real result; environment>
Acceptance evidence: <scenario IDs and artifacts>
Limitations/blockers:
```

Do not check a box just because code exists. The source brief's illustrative examples are not test output.

## Phase 0 — Repository/bootstrap

| Status | ID | Dependencies | Work / files | Acceptance |
|---|---|---|---|---|
| [x] | P0-01 | — | Confirm D09 naming and D18 supported OS matrix; establish root README conventions and planning/source precedence | Decisions recorded, crate/binary names consistent, first usable milestone explicitly Phase 0–1, current request Phase 0 only (D22); no speculative subsystems |
| [x] | P0-02 | P0-01 | Create root `Cargo.toml`, `rust-toolchain.toml`, `Cargo.lock`, `.gitignore`, `crates/notes-core`, `crates/notes-cli`; document selected license instead of guessing one | Both crates build; actual CLI `--help` works; core has no UI dependency; root test suite runs |
| [x] | P0-03 | P0-02 | Add `.github/workflows/ci.yml` and test instructions; format, strict Clippy, workspace tests; isolate test config/root | CI definition validated and commands run locally; CI run evidence required once hosted; no real home/library writes (V01) |

## Phase 0 execution evidence

- **Tasks:** P0-01/P0-02/P0-03 verified complete. Both the local checks and hosted CI execution satisfy the Phase 0 gate.
- **Decision changes:** user approved D09 naming, D18 Linux-first, D21 MIT, and D22 Phase-0-only request scope. README records conventions and source precedence.
- **Implementation/PR:** commit [`33a91e1`](https://github.com/acramatte/foglio/commit/33a91e1886a3edc6ec5eca8e182e99b88c4f74c7) on `main` in the private `acramatte/foglio` repository; no PR (direct-to-main delivery approved by the user). Workspace manifests, lockfile, pinned toolchain, dependency-free core, clap CLI, black-box tests, MIT license, and CI definition added.
- **Environment:** Linux x86_64; official Rust/cargo 1.93.1. System Rust lacked rustfmt/Clippy, so an isolated rustup installation was created under `/tmp/foglio-phase0-tools` without changing shell startup files or replacing system Rust. For this session's toolchain: `export PATH=/tmp/foglio-phase0-tools/cargo/bin:$PATH RUSTUP_HOME=/tmp/foglio-phase0-tools/rustup CARGO_HOME=/tmp/foglio-phase0-tools/cargo`. Other developers should use normal rustup setup from README.
- **Red/green:** `cargo test --workspace --all-features` initially failed all three CLI tests against an empty `main` (missing help/version and incorrect usage status); the implemented parser passes all three.
- **Tests:** `cargo fmt --all -- --check` passed; `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` passed; `cargo build --locked --workspace` passed; `cargo test --locked --workspace --all-features` passed (3 black-box tests; core and doc tests currently contain no cases); `/tmp/foglio-phase0-tools/bin/actionlint .github/workflows/ci.yml` passed with actionlint 1.7.7.
- **Acceptance evidence (V01):** actual executable tests cover `--help`, `-h`, no arguments, `--version`, unknown flag and unavailable commands. Each subprocess clears inherited environment and uses temporary HOME/config/cache/data/library directories; assertions confirm no application state or note changes. CLI path-depends on core; core has no dependencies or product scaffolding. Formatting and strict lint cover both crates.
- **Hosted CI evidence:** [run 34130532775](https://github.com/acramatte/foglio/actions/runs/34130532775), workflow `CI`, job `Core and CLI (Linux)`, completed successfully for `33a91e1886a3edc6ec5eca8e182e99b88c4f74c7`. Pinned toolchain setup, formatting, strict Clippy, core/CLI build, workspace tests, and isolated CLI help all passed. Verified through authenticated GitHub CLI run/job reads.
- **Limitations:** no Phase 1 operations, filesystem-save safety, macOS/Windows support, or later subsystems are implemented or verified.

## Phase 1 — Filesystem notes and CLI only

| Status | ID | Dependencies | Work / files | Acceptance |
|---|---|---|---|---|
| [x] | P1-01 | P0-03 | Implement library/config resolution, path types and recursive discovery in `library.rs`/`filesystem.rs`; resolve D10/D11 | Repeated init preserves contents; shared config resolves selected root; paths cannot escape; non-note/symlink/unsupported inputs diagnosed (V02, V07) |
| [x] | P1-02 | P1-01 | Run S01; implement frontmatter/body parser, title extraction, tag semantics; preservation fixtures under `crates/notes-core/tests/fixtures/` | Plain Markdown, arbitrary user `id` metadata and both tag forms parse; malformed/duplicate-key/wrong-type metadata stays byte-identical; unknown nested keys/comments/body/BOM/CRLF survive supported patches (V03, V04) |
| [x] | P1-03 | P1-02 | Run S02; implement revision preconditions, cooperative lock, staging/flush/replace, no-clobber create/move, explicit commit outcomes | Fault and stale-write tests inspect disk; occupied targets remain unchanged; permissions preserved; external-writer race/platform limits documented (V06, V07, V08) |
| [x] | P1-04 | P1-03 | Implement byte-preserving initialization/discovery and independent path identity in core (D26 supersedes adoption) | All existing Markdown bytes survive init; read-only inputs stay readable; identical content/metadata at different paths remains independent (V04, V05) |
| [x] | P1-05 | P1-04 | Implement create/get/list/update/move/delete/add-tag/remove-tag domain APIs | Actual temp-filesystem lifecycle, stable move identity, metadata-only tag edits, body updates, stale mutation rejection pass (V06, V08) |
| [x] | P1-06 | P1-05 | Implement `init/new/list/show/move/delete/tags/tag add/tag remove`, selectors, config override, JSON/errors/help; resolve D15/D20 | Black-box executable lifecycle works; delete confirmation/noninteractive behavior deterministic; stdout/stderr/exit codes tested; no sync/index stubs (V09) |
| [x] | P1-07 | P1-06 | Add first-delivery acceptance harness and user documentation; reconcile phase status | Source §43's five checks pass automatically, safety suites pass, deleting config leaves notes intact and root reselection works; Phase 0–1 full gate recorded (V01–V10) |

**Stop after P1-07 for the first implementation delivery.**

## Phase 1 execution evidence

- **Tasks/status:** P1-01–P1-07 verified locally; hosted CI remains pending.
- **Decisions:** user approved D10–D15 and D20, and explicitly authorized Phase 1. S01/S02 findings and supported-input/platform limits are recorded in [Phase 1](phase1.md).
- **Implementation/PR:** uncommitted working tree, no push or PR. Core, CLI, preservation/fault tests, acceptance harness and CI step implemented without Phase 2 scaffolding.
- **Tests:** `cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`, `cargo build --locked --workspace`, `cargo test --locked --workspace --all-features`, `python3 tests/acceptance/phase1.py`, actionlint and `git diff --check` pass on Linux x86_64/Rust 1.93.1. Rust: 22 passing tests plus a subprocess fixture invoked by its parent test; Python: 12 passing acceptance scenarios.
- **Acceptance:** source §43's five invariants and V01–V10 are mapped to actual tests in [verification evidence](phase1.md#verification-evidence), including killed staged writer, stale confirmation, duplicate IDs, YAML preservation and app-state deletion.
- **Limitations:** Linux local filesystems only, restricted losslessly patchable YAML, ACL/xattr or ownership-changing replacements refused, documented uncooperative-writer/ancestor-swap race. Phase 1 hosted CI has not run; old hosted evidence covers Phase 0 only.


## Phase 2 — Derived index and lexical search

| Status | ID | Dependencies | Work / files | Acceptance |
|---|---|---|---|---|
| [x] | P2-01 | P1-07 | Implement `index.rs`, migrations, FTS5 availability check, cache lifecycle, complete scan/transactional index updates | Scan/edit/move/delete reflected atomically; DB deletion/corruption rebuild preserves notes; inaccessible subtree is not interpreted as empty; healthy paths remain searchable during partial scans (V11, V12) |
| [x] | P2-02 | P2-01 | Implement `search.rs`, parameterized literal/phrase/prefix queries, ranking, tag/folder filters; resolve D16 | Every indexed field searchable; malformed/hostile query safe; deterministic ordering and component-aware paths (V13) |
| [x] | P2-03 | P2-02 | Add CLI `search/rescan/reindex/status`; distinguish file-commit/index-degraded results and short-lived watcher state | Reindex reconstructs complete healthy state or explicitly reports partial diagnostics; status counts reflect actual scan/index state; CLI status does not claim background watcher (V11, V12, V14) |
| [x] | P2-04 | P2-02, P2-03 | Validate multiple processes/locking and S05 initial fixture/benchmark harness; optimize warm-open only with evidence | Concurrent app writers/rebuild/readers have bounded deterministic outcomes; warm scan avoids unconditional reparsing; baseline at both corpus sizes saved with environment (V15, V25) |

## Phase 2 execution evidence

- **Tasks/status:** P2-01–P2-04 verified locally. User explicitly authorized Phase 2 and approved D16. No Phase 3 scaffolding.
- **Implementation/PR:** uncommitted working tree; no push or PR. Phase 1 was committed as `3a09508`; its earlier execution block is historical.
- **Tests:** locked workspace build/tests, formatting, strict Clippy, both Python acceptance harnesses, actionlint and diff checks passed. Actual evidence and limitations: [Phase 2](phase2.md).
- **Acceptance:** V11–V13, V14 status portion, V15 concurrent/bounded-lock behavior, and V25 initial 1k/50k benchmark completed. Raw measured distributions/environment: [baseline](phase2-baseline.json).
- **Decisions:** bundled rusqlite/FTS5; transactional schema/version 1; DELETE journal and connection-scoped cooperative locking; explicit post-commit errors; unknown identity suppresses search conservatively.
- **Limitations:** Linux only; tmpfs/page-cache-warm benchmark, not physical cold-storage evidence; metadata warm scans cannot prove arbitrary offline equality; snapshots hold a root lock; hosted CI pending. Doctor, watcher and desktop gates remain open.

## Phase 3 — Filesystem watcher

| Status | ID | Dependencies | Work / files | Acceptance |
|---|---|---|---|---|
| [x] | P3-01 | P2-04 | Implement recursive watcher, dirty-path batching, startup ordering, subtree invalidation, shutdown in `watcher.rs`; S04 | Separate process create/edit/rename/delete and directory operations converge; no exact low-level event-order assumptions (V16) |
| [x] | P3-02 | P3-01 | Implement domain subscriptions, committed revisions, bounded queues, overflow invalidation, periodic/manual recovery | Self-writes converge without loops; dropped events trigger recovery; clients can refetch and recover; diagnostics/index-degraded state observable (V17) |
| [x] | P3-03 | P3-02 | Add real watcher stress and cross-platform cases, plus external-editor/sync documentation notes | Atomic-save bursts, temporary disappearances, same-metadata touched files, duplicate conflict copies, pause/resume and directory changes pass on declared targets (V16, V17, V24) |

## Phase 3 execution evidence

- **Tasks/status:** P3-01–P3-03 verified locally. User explicitly authorized Phase 3; no Phase 4 scaffold or daemon command added.
- **Implementation/PR:** uncommitted working tree; no commit, push or PR. Core watcher/events, backend lifetime status, native and deterministic recovery tests, and [Phase 3 API/evidence](phase3.md).
- **Tests:** `cargo fmt --all -- --check`; `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`; `cargo test --locked --workspace --all-features`; `cargo build --locked --workspace`; `python3 tests/acceptance/phase1.py`; `python3 tests/acceptance/phase2.py`; `git diff --check` all passed on Linux x86_64/Rust 1.93.1. Existing Python suites: 12 and 4 passing scenarios. Native suites repeated with `cargo test --locked -p notes-core --test watcher --test watcher_recovery --test watcher_startup --test watcher_status`: all 10 repetitions passed (suite elapsed 2.733–2.772 seconds; not a convergence benchmark).
- **Acceptance:** V16/V17, shared-handle V14 watcher status, and V24 development simulation pass against actual temporary files and separate Python writers. Explicit dropped-hint injection proves periodic content recovery; positive change/move/delete assertions verify event payloads against disk revisions.
- **Review/red-green:** independent review identified subscription initialization ambiguity and inactive shared-handle status. The new status regression failed before the backend counter fix, then passed. Initial invalidation ordering, startup overlap and dropped-hint recovery have additional coverage.
- **Decisions/limitations:** S04 Linux/tmpfs slice resolved; full-library hashing per bounded batch prioritizes identity correctness over large-library throughput. Configurable scheduling defaults are not performance guarantees. No real OS suspend, other OS backend, remote filesystem or second-device sync evidence. Hosted CI and later release gates remain pending.

## Phase 4 — Read-only desktop

| Status | ID | Dependencies | Work / files | Acceptance |
|---|---|---|---|---|
| [x] | P4-01 | P3-03 | Create `apps/desktop` Tauri/TypeScript app; select frontend framework/package manager; shared core lifetime, narrow DTOs, capability/CSP setup, frontend CI | Production frontend/Tauri build; actual window opens; core tests remain independently runnable; commands do not block UI under scan load (V18) |
| [x] | P4-02 | P4-01 | Implement selected-library onboarding, physical folder tree, notes/tags, opening, search and empty/error states | Real fixture root navigable; selections identify notes by relative path, paths shown correctly; search/tag results open current disk content (V18) |
| [x] | P4-03 | P4-01 | Implement sanitized CommonMark/GFM preview, link policy, unsupported image fallback and security fixtures | Required Markdown renders; scripts/schemes/local traversal/remote image loads blocked in actual webview, original Markdown unchanged (V19) |
| [x] | P4-04 | P4-02, P4-03 | Wire watcher invalidation to lists/preview with error handling and lifecycle cleanup | External changes reflected while browsing; unreadable/deleted records surfaced; identical copies independently accessible; close/reopen releases watchers cleanly (V18, V19) |

## Phase 4 execution evidence

- **Tasks/status:** P4-01–P4-04 verified locally on Linux. User authorized Phase 4 and installed missing native dependencies. No editing/autosave work included.
- **Implementation/PR:** uncommitted working tree; no commit/push/PR. `apps/desktop` contains Tauri backend, typed TypeScript frontend, safe preview, tests and locked dependencies. Core adds non-adopting selection; CI separates native/headless gates.
- **Tests:** `cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`, `cargo test --locked --workspace --all-features`, scoped core/CLI build, both existing Python acceptance suites, `npm test` (16 passing), `npm run typecheck`, production frontend and debug/release Tauri builds, and diff checks passed. Desktop backend has six passing tests.
- **Acceptance:** `TAURI_DRIVER=/tmp/foglio-phase4-tools/bin/tauri-driver python3 tests/acceptance/phase4.py` passes against the actual debug application; the same harness with `FOGLIO_DESKTOP_BINARY=/home/alexis/Development/private/foglio/target/release/foglio-desktop` passes against the optimized production application. V18/V19 cover navigation, folders/tags/FTS, byte preservation, safe rendering/links, external edits/moves/deletion, ambiguity/unreadability/recovery, and close/reopen. Loopback image trap receives zero requests.
- **Review/decisions:** plain TypeScript/Vite/npm, Tauri 2, Marked/DOMPurify allowlist, narrow IPC and read-only selection. Fixed review findings for navigation during refresh and frontend/backend link-policy disagreement; added regression tests. [Detailed setup, architecture and evidence](phase4.md).
- **Limitations:** hosted CI pending; Linux/WebKit/Xvfb only, no manual accessibility or physical-display evidence, installers, other OSes, cross-device sync or large-corpus performance claims. Existing whole-library scan behavior remains visible and P7 performance work remains open.

## Ordinary-Markdown amendment (D26–D28)

- [x] Remove embedded-ID types/selectors/DTOs, adoption and duplicate-ID suppression; path-key the disposable schema-2 cache and preserve SHA-256 stale-write guards.
- [x] Browse/search/open supported plain Markdown and preserve arbitrary existing `id` metadata. Init/select/index/watch never rewrite source; untagged creation is plain Markdown.
- [x] Keep external moves as delete/create observations; never retarget selection using matching content. Move healthy monitoring to a quiet footer, preserving visible errors.
- **Evidence:** [path identity](path-identity.md). Workspace tests/strict Clippy, CLI acceptance (12 + 4), frontend tests (18), typecheck, optimized native build and actual WebKit acceptance passed locally. Nothing committed or pushed for this amendment.
- **Scope:** search UX remains unchanged. Desktop creation/editing/autosave remains Phase 5.

## Phase 5 — Editor and autosave

| Status | ID | Dependencies | Work / files | Acceptance |
|---|---|---|---|---|
| [x] | P5-01 | P4-04 | Run S03 editor round-trip spike; record D17; implement source/preview baseline and optional proven rich mode | Markdown fixtures preserve unsupported syntax and unknown frontmatter; rich mode omitted if preservation fails (V20) |
| [x] | P5-02 | P5-01 | Implement per-note debounced autosave state machine, generation/revision tracking, save error UI and stale-save pause | Edits during in-flight saves not marked saved prematurely; stale core write cannot overwrite newer disk; unsaved buffer survives save failure (V21) |
| [x] | P5-03 | P5-02 | Add desktop create/move/rename/delete/tag controls using core; explicit permanent-delete confirmation | End-to-end desktop file lifecycle and explicit selected-path updates pass; no frontend filesystem reimplementation (V22) |
| [x] | P5-04 | P5-02 | Protect switching notes, navigation and window close with pending saves; keyboard source/preview affordances | Navigation flushes or stays/cancels with explicit choice; close cannot silently discard known dirty buffer; manual smoke plus automated state tests (V20, V21) |

## Phase 5 execution evidence

- **Tasks/status:** P5-01–P5-04 verified locally on Linux. User explicitly authorized Phase 5 and the three review corrections. Hosted CI pending.
- **Decision changes:** S03 rejects default Milkdown 7.22.1 rich serialization; D17 resolves to source body + sanitized preview. Unknown supported YAML stays outside the editor. Native close flushes or stays; no discard/force-write path added.
- **Implementation/PR:** uncommitted working tree, no push or PR. Source editor/state machine, typed guarded Tauri mutation commands, lifecycle dialogs, close handshake, narrow event capabilities and CI acceptance step. [Detailed evidence and limitations](phase5.md).
- **Tests:** workspace fmt/tests/strict Clippy; frontend tests (37 passing), typecheck and build; debug/optimized native builds; Phase 1/2 CLI acceptance (12 + 4); Phase 4 native regression and Phase 5 native acceptance (9 scenarios) pass. Exact commands in Phase 5 evidence.
- **Acceptance:** V20/V21/V22 real file lifecycle, contained preview scrolling, bidirectional Ctrl+E toggling, title-derived portable filenames in the library root or an optional nested folder, existing-tag removal choices, actual keyboard source edit, supported metadata/BOM/CRLF and mixed-newline preservation, error/retry, stale/missing buffers and real WM_DELETE_WINDOW flush/block behavior. Independent-review overlap/deferred-refresh/newline regressions failed before fixes and pass afterward. Scripted native interaction replaces a separate manual physical-display smoke for this local gate; physical-display/accessibility review remains P7.
- **Limitations:** source-only editor, no Phase 6 reload/save-copy choices, no durable crash drafts/history, no hosted CI/installer/other-OS/large-corpus claim. Existing filesystem races and supported-YAML limits remain.

## Phase 6 — External-change experience

| Status | ID | Dependencies | Work / files | Acceptance |
|---|---|---|---|---|
| [x] | P6-01 | P5-03, P5-04 | Define/implement clean refresh, dirty/saving conflict and missing-note states; resolve D19 | External update reloads clean note; dirty/in-flight edit pauses; external moves leave the old path missing unless reliably observed; no hash retargeting or implicit recreation (V23) |
| [x] | P6-02 | P6-01 | Implement explicit reload/discard and save-local-as-new conflict choices; repeated-change guards | Disk and local versions never silently discarded; save-copy uses a new no-clobber path; repeated external changes remain guarded (V23) |
| [x] | P6-03 | P6-02 | Run two-process end-to-end conflict and externally synchronized conflict-copy scenarios | Dirty edit + external write/move/delete and identical conflict copies preserve independent content without hash-based retargeting in running desktop (V23, V24) |

### Phase 6 completion evidence

- **Tasks/status:** P6-01–P6-03 verified locally on Linux; user explicitly authorized Phase 6. Hosted CI pending.
- **Decision:** D19 resolved to explicit discard/reload or literal new-path no-clobber copy preserving original metadata. Dirty/saving observations and save readback guard buffers; missing paths never auto-follow or resurrect.
- **Implementation:** frontend conflict choices/state machine, core source guards and no-clobber copy, typed Tauri command, unit/integration/native regressions and CI step. No commit/push/PR. [Evidence and limits](phase6.md).
- **Tests:** workspace formatting/tests/strict Clippy; 60 frontend tests and typecheck/build; CLI acceptance 12 + 4; native Phase 6 (9 scenarios) and Phase 5 (10 scenarios) on debug and optimized binaries; Phase 4 debug regression pass.
- **Acceptance:** V23 clean/dirty/in-flight external edits, delayed acknowledgement with and without newer typing, native close protection, explicit discard/cancel, repeated writes/reappearance during choices, original metadata/body preservation, occupied/new destination guards, external move/delete and independent identical conflict copies. V24 remains a filesystem simulation; real provider/device transfer is P7.
- **Limits:** supported-YAML and final external-writer race limits, explicit retry on transient cooperative-lock Busy, no crash drafts/history/installer/other-OS/hosted-CI claim. Current malformed/unreadable disk observations can block UI recovery until repaired.

## Phase 7 — Diagnostics, polish, release

| Status | ID | Dependencies | Work / files | Acceptance |
|---|---|---|---|---|
| [ ] | P7-01 | P6-03 | Implement full read-only `doctor`, safe explicit repairs, actionable CLI/UI diagnostics | Read-only run leaves bytes unchanged; malformed YAML, access errors, schema corruption, stale/orphans found; no source rewriting (V14) |
| [ ] | P7-02 | P6-03 | Keyboard shortcuts, focus/accessibility, fast navigation/search, empty/error polish; command palette only if justified | Keyboard-only create/find/edit/move workflow; focus and screen-reader labels checked; no new product scope (V26) |
| [ ] | P7-03 | P6-03 | Benchmark 1k/50k corpora, warm/cold behavior and watcher convergence; finish platform fault/security matrix | Reproducible distributions meet agreed budgets or limitations block target claim; no fabricated timing or untested OS promises (V07, V15, V19, V25) |
| [ ] | P7-04 | P7-01, P7-02, P7-03 | Package chosen OS targets; install smoke, README/help, `docs/sync.md`, recovery/limitations docs; final v1 acceptance | Fresh install and complete release workflow pass, including actual external-tool transfer to another device and DB deletion/rebuild; all required evidence linked (V24, V26, V27) |

### P7-02 contention follow-up

- [ ] Coordinate same-process watcher scans and desktop mutations to reduce spurious `busy` outcomes without weakening bounded external-process locks, stale revision checks or committed-outcome semantics. Phase 6 native regression observed safe retained-buffer failures requiring explicit Retry save; navigation acceptance exercises this recovery. Regress concurrent watcher/lifecycle/navigation activity before claiming contention-free UX.

## Deferred search UX

- [ ] SEARCH-UX: evaluate interactive prefix matching, explicit phrase controls and separately scoped typo tolerance/ranking. Keep current literal behavior until explicitly authorized; add core/UI/native regression criteria before changing it.

## Backlog exclusions

Future features from source §40 remain outside these tasks. Do not open implementation tasks for attachments, AI/MCP, encryption, cloud APIs, daemon IPC, CRDTs, history, or mobile without a scope amendment.
