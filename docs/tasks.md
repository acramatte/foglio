# Implementation tasks

**Status: every task is open; no implementation or application tests have run.** Dependencies are prerequisite task IDs. Each row is a bounded work item, not an instruction to scaffold future phases. Proposed decisions must be resolved at their named gate.

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
| [ ] | P0-01 | — | Confirm D09 naming and D18 supported OS matrix; establish root README conventions and planning/source precedence | Decisions recorded, crate/binary names consistent, first delivery explicitly Phase 0–1; no speculative subsystems |
| [ ] | P0-02 | P0-01 | Create root `Cargo.toml`, `rust-toolchain.toml`, `Cargo.lock`, `.gitignore`, `crates/notes-core`, `crates/notes-cli`; document selected license instead of guessing one | Both crates build; actual CLI `--help` works; core has no UI dependency; root test suite runs |
| [ ] | P0-03 | P0-02 | Add `.github/workflows/ci.yml` and test instructions; format, strict Clippy, workspace tests; isolate test config/root | CI definition validated and commands run locally; CI run evidence required once hosted; no real home/library writes (V01) |

## Phase 1 — Filesystem notes and CLI only

| Status | ID | Dependencies | Work / files | Acceptance |
|---|---|---|---|---|
| [ ] | P1-01 | P0-03 | Implement library/config resolution, path types and recursive discovery in `library.rs`/`filesystem.rs`; resolve D10/D11 | Repeated init preserves contents; shared config resolves selected root; paths cannot escape; non-note/symlink/unsupported inputs diagnosed (V02, V07) |
| [ ] | P1-02 | P1-01 | Run S01; implement validated `NoteId`, frontmatter/body parser, title extraction, tag semantics; preservation fixtures under `crates/notes-core/tests/fixtures/` | Valid ULIDs and both tag forms parse; malformed/duplicate-key/wrong-type metadata stays byte-identical; unknown nested keys/comments/body/BOM/CRLF survive supported patches (V03, V04) |
| [ ] | P1-03 | P1-02 | Run S02; implement revision preconditions, cooperative lock, staging/flush/replace, no-clobber create/move, explicit commit outcomes | Fault and stale-write tests inspect disk; occupied targets remain unchanged; permissions preserved; external-writer race/platform limits documented (V06, V07, V08) |
| [ ] | P1-04 | P1-03 | Implement safe missing-ID adoption, full discovery diagnostics, duplicate-group policy in core | Imported body survives; read-only/unstable inputs not overwritten; every duplicate member is ambiguous; no silent ID regeneration (V04, V05) |
| [ ] | P1-05 | P1-04 | Implement create/get/list/update/move/delete/add-tag/remove-tag domain APIs | Actual temp-filesystem lifecycle, stable move identity, metadata-only tag edits, body updates, stale mutation rejection pass (V06, V08) |
| [ ] | P1-06 | P1-05 | Implement `init/new/list/show/move/delete/tags/tag add/tag remove`, selectors, config override, JSON/errors/help; resolve D15/D20 | Black-box executable lifecycle works; delete confirmation/noninteractive behavior deterministic; stdout/stderr/exit codes tested; no sync/index stubs (V09) |
| [ ] | P1-07 | P1-06 | Add first-delivery acceptance harness and user documentation; reconcile phase status | Source §43's five checks pass automatically, safety suites pass, deleting config leaves notes intact and root reselection works; Phase 0–1 full gate recorded (V01–V10) |

**Stop after P1-07 for the first implementation delivery.**

## Phase 2 — Derived index and lexical search

| Status | ID | Dependencies | Work / files | Acceptance |
|---|---|---|---|---|
| [ ] | P2-01 | P1-07 | Implement `index.rs`, migrations, FTS5 availability check, cache lifecycle, complete scan/transactional index updates | Scan/edit/move/delete reflected atomically; DB deletion/corruption rebuild preserves notes; inaccessible subtree is not interpreted as empty; duplicates excluded (V11, V12) |
| [ ] | P2-02 | P2-01 | Implement `search.rs`, parameterized literal/phrase/prefix queries, ranking, tag/folder filters; resolve D16 | Every indexed field searchable; malformed/hostile query safe; deterministic ordering and component-aware paths (V13) |
| [ ] | P2-03 | P2-02 | Add CLI `search/rescan/reindex/status`; distinguish file-commit/index-degraded results and short-lived watcher state | Reindex reconstructs complete healthy state or explicitly reports partial diagnostics; status counts reflect actual scan/index state; CLI status does not claim background watcher (V11, V12, V14) |
| [ ] | P2-04 | P2-02, P2-03 | Validate multiple processes/locking and S05 initial fixture/benchmark harness; optimize warm-open only with evidence | Concurrent app writers/rebuild/readers have bounded deterministic outcomes; warm scan avoids unconditional reparsing; baseline at both corpus sizes saved with environment (V15, V25) |

## Phase 3 — Filesystem watcher

| Status | ID | Dependencies | Work / files | Acceptance |
|---|---|---|---|---|
| [ ] | P3-01 | P2-04 | Implement recursive watcher, dirty-path batching, startup ordering, subtree invalidation, shutdown in `watcher.rs`; S04 | Separate process create/edit/rename/delete and directory operations converge; no exact low-level event-order assumptions (V16) |
| [ ] | P3-02 | P3-01 | Implement domain subscriptions, committed revisions, bounded queues, overflow invalidation, periodic/manual recovery | Self-writes converge without loops; dropped events trigger recovery; clients can refetch and recover; diagnostics/index-degraded state observable (V17) |
| [ ] | P3-03 | P3-02 | Add real watcher stress and cross-platform cases, plus external-editor/sync documentation notes | Atomic-save bursts, temporary disappearances, same-metadata touched files, duplicate conflict copies, pause/resume and directory changes pass on declared targets (V16, V17, V24) |

## Phase 4 — Read-only desktop

| Status | ID | Dependencies | Work / files | Acceptance |
|---|---|---|---|---|
| [ ] | P4-01 | P3-03 | Create `apps/desktop` Tauri/TypeScript app; select frontend framework/package manager; shared core lifetime, narrow DTOs, capability/CSP setup, frontend CI | Production frontend/Tauri build; actual window opens; core tests remain independently runnable; commands do not block UI under scan load (V18) |
| [ ] | P4-02 | P4-01 | Implement selected-library onboarding, physical folder tree, notes/tags, opening, search and empty/error states | Real fixture root navigable; selections identify notes by ID, paths shown correctly; search/tag results open current disk content (V18) |
| [ ] | P4-03 | P4-01 | Implement sanitized CommonMark/GFM preview, link policy, unsupported image fallback and security fixtures | Required Markdown renders; scripts/schemes/local traversal/remote image loads blocked in actual webview, original Markdown unchanged (V19) |
| [ ] | P4-04 | P4-02, P4-03 | Wire watcher invalidation to lists/preview with error handling and lifecycle cleanup | External changes reflected while browsing; duplicate/unreadable/deleted records surfaced; close/reopen releases watchers cleanly (V18, V19) |

## Phase 5 — Editor and autosave

| Status | ID | Dependencies | Work / files | Acceptance |
|---|---|---|---|---|
| [ ] | P5-01 | P4-04 | Run S03 editor round-trip spike; record D17; implement source/preview baseline and optional proven rich mode | Markdown fixtures preserve unsupported syntax and unknown frontmatter; rich mode omitted if preservation fails (V20) |
| [ ] | P5-02 | P5-01 | Implement per-note debounced autosave state machine, generation/revision tracking, save error UI and stale-save pause | Edits during in-flight saves not marked saved prematurely; stale core write cannot overwrite newer disk; unsaved buffer survives save failure (V21) |
| [ ] | P5-03 | P5-02 | Add desktop create/move/rename/delete/tag controls using core; explicit permanent-delete confirmation | End-to-end desktop file lifecycle and stable identity pass; no frontend filesystem reimplementation (V22) |
| [ ] | P5-04 | P5-02 | Protect switching notes, navigation and window close with pending saves; keyboard source/preview affordances | Navigation flushes or stays/cancels with explicit choice; close cannot silently discard known dirty buffer; manual smoke plus automated state tests (V20, V21) |

## Phase 6 — External-change experience

| Status | ID | Dependencies | Work / files | Acceptance |
|---|---|---|---|---|
| [ ] | P6-01 | P5-03, P5-04 | Define/implement clean refresh, dirty/saving conflict and missing-note states; resolve D19 | External update reloads clean note; dirty/in-flight edit pauses; external move keeps identity and deletion does not trigger recreation (V23) |
| [ ] | P6-02 | P6-01 | Implement explicit reload/discard and save-local-as-new conflict choices; repeated-change guards | Disk and local versions never silently discarded; save-copy uses new ID; repeated external changes remain guarded (V23) |
| [ ] | P6-03 | P6-02 | Run two-process end-to-end conflict and externally synchronized conflict-copy scenarios | Dirty edit + external write/move/delete and copied duplicate IDs preserve content and surface ambiguity in running desktop (V23, V24) |

## Phase 7 — Diagnostics, polish, release

| Status | ID | Dependencies | Work / files | Acceptance |
|---|---|---|---|---|
| [ ] | P7-01 | P6-03 | Implement full read-only `doctor`, safe explicit repairs, actionable CLI/UI diagnostics | Read-only run leaves bytes unchanged; malformed/missing/duplicate IDs, access errors, schema corruption, stale/orphans found; ambiguous repairs refused (V14) |
| [ ] | P7-02 | P6-03 | Keyboard shortcuts, focus/accessibility, fast navigation/search, empty/error polish; command palette only if justified | Keyboard-only create/find/edit/move workflow; focus and screen-reader labels checked; no new product scope (V26) |
| [ ] | P7-03 | P6-03 | Benchmark 1k/50k corpora, warm/cold behavior and watcher convergence; finish platform fault/security matrix | Reproducible distributions meet agreed budgets or limitations block target claim; no fabricated timing or untested OS promises (V07, V15, V19, V25) |
| [ ] | P7-04 | P7-01, P7-02, P7-03 | Package chosen OS targets; install smoke, README/help, `docs/sync.md`, recovery/limitations docs; final v1 acceptance | Fresh install and complete release workflow pass, including actual external-tool transfer to another device and DB deletion/rebuild; all required evidence linked (V24, V26, V27) |

## Backlog exclusions

Future features from source §40 remain outside these tasks. Do not open implementation tasks for attachments, AI/MCP, encryption, cloud APIs, daemon IPC, CRDTs, history, or mobile without a scope amendment.
