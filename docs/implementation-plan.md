# Implementation plan

Status: Phase 0 complete, verified locally on Linux and in [hosted CI](https://github.com/acramatte/foglio/actions/runs/34130532775) for commit `33a91e1`. Phase 1 and later are not started. See task evidence.

## Delivery strategy

Build vertical slices, not dormant future layers. The first usable milestone covers **Phase 0 and Phase 1 only**. The user narrowed this implementation request to **Phase 0 only** (D22); Phase 1 is not implicitly authorized. Do not create SQLite, watcher, Tauri, sync, encryption, or plugin scaffolding in that delivery.

Requirements: [product](specs/product.md). Contracts: [technical](specs/technical.md). Work items: [tasks](tasks.md). Evidence: [verification](specs/verification.md). Proposed decisions: [register](decisions.md).

## Phase gates

| Phase | Deliverable | Depends on | Completion gate | Explicitly excluded |
|---|---|---|---|---|
| 0 — Bootstrap | Workspace, core/CLI crates, toolchain, lint/test CI, project conventions | D09/D18 review | Core and real CLI compile; workspace tests, formatting and strict Clippy run in CI | Product subsystems |
| 1 — Filesystem core + CLI | Root selection, discovery/adoption, metadata, stable IDs, guarded CRUD/move/tags, lifecycle CLI | Phase 0; S01/S02 | Black-box lifecycle plus source §43's five invariants and destructive/stale-write safety pass on actual temporary files | DB, search, watcher, desktop |
| 2 — Index + search | SQLite migrations, transactional derived state, FTS, status/rescan/reindex, baseline measurements | Phase 1 | Remove cache, rebuild, compare files and searchable state; corrupted DB and multi-process cache tests pass | UI, background daemon |
| 3 — Live reconciliation | Recursive watcher, batching, subtree handling, in-process events, recovery | Phase 2 | A separate process creates/edits/moves/deletes notes; running core converges without event-order assumptions | Sync integrations |
| 4 — Read-only desktop | Tauri lifetime, selection, folders/notes/tags, search, sanitized preview | Phase 3 | Actual desktop navigates an existing root; safe renderer tests and command boundaries pass | Rich editor/autosave |
| 5 — Editing | Source/preview editor, autosave state machine, create/move/delete/tags, navigation protection | Phase 4; S03 | Real desktop creates/edits files; failed/stale saves preserve buffer; rendering and metadata round trips pass | Automatic merge |
| 6 — External-change UX | Clean reload, dirty conflict pause/choices, moved/deleted note handling | Phase 5 | Two-process edit during autosave cannot silently discard either observed version; deletion is not undone by autosave | History, CRDTs |
| 7 — Release hardening | Doctor, keyboard/accessibility/error polish, performance, packaging, external-sync docs | Phase 6 | Complete source §37 workflow and release/security/platform evidence, including fresh install | New roadmap features |

## Critical path and parallel work

```text
P0-01 → P0-02 → P0-03
       → P1-01 → P1-02 → P1-03 → P1-04 → P1-05 → P1-06 → P1-07
       → P2-01 → P2-02/P2-03 → P2-04
       → P3-01 → P3-02 → P3-03
       → P4-01 → P4-02/P4-03 → P4-04
       → P5-01 → P5-02 → P5-03/P5-04
       → P6-01 → P6-02 → P6-03
       → P7-01/P7-02/P7-03 → P7-04
```

The detailed task dependency table is authoritative. Arrows here group phases rather than permitting dependencies to be skipped. Within a phase, independent tests/docs and isolated UI work may proceed in parallel after interfaces are agreed. Keep one owner for file mutation and reconciliation invariants; do not let parallel agents implement divergent versions in different clients.

## Implementation loop for every task

1. Inspect current repository, status, and related task evidence. Do not assume this plan still describes an empty project.
2. Resolve only the decisions blocking this slice. Record evidence from bounded spikes; avoid reopening established architecture.
3. Define/execute a failing behavioral test where meaningful; prove it fails for the intended reason.
4. Implement the smallest complete behavior with real filesystem/database/process tests.
5. Run focused checks plus the workspace gate; run frontend/Tauri checks once those components exist.
6. Review file-safety/security implications and phase scope. No silent fallback that risks a note.
7. Reconcile specs when behavior intentionally changes. Record actual command results, limitations, and task status.

## Planned commands, not execution claims

Phase 0 establishes these gates:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Once desktop exists, add frontend type checking, renderer/editor tests, production build, Tauri backend tests, and an actual desktop smoke test. P4-01 selects the package manager/scripts and documents exact commands; no fictitious `npm` scripts are claimed today. Run core/CLI tests independently of Tauri native dependencies even if a workspace-wide job covers desktop on equipped runners.

Phase 1 demonstration uses an isolated temporary configuration and temporary library, builds and runs the actual `notes` executable, then checks files. Show generated full IDs from real execution rather than hard-coded illustrative ULIDs. No production notes are used for tests.

## Pull request boundaries

Prefer focused semantic commits and stacked PRs for dependent changes. Each PR should state rationale, scope/non-goals, affected task IDs, phase status, and an explicit **Tests** section containing commands and real results. Keep parser preservation, filesystem mutation safety, CLI integration, index/search, watcher, and editor/conflict changes reviewable as separate slices. Do not merge main into a feature branch; rebase dependent work when necessary.

Do not initialize Git, create remotes, publish issues, or push branches merely to record this plan. Those are separate actions during implementation. The Markdown backlog is the durable current work record; if a shared tracker is adopted, add links and designate one status authority instead of maintaining conflicting backlogs.

## Stop conditions

Stop the dependent task and report a blocker if YAML preservation cannot be guaranteed for the target input, safe filesystem primitives are unavailable, an editor silently loses Markdown, or the desktop cannot be exercised on a claimed target. Narrow the supported input/platform with an explicit decision, or change implementation. Do not replace failed validation with mocked success.

## Estimates and release policy

No calendar or story-point estimates are assigned before parser/filesystem/editor spikes. Phase 1 and Phase 6 contain the highest data-loss risks. A narrow supported-platform release is preferable to unverified portability. Performance budgets are proposals pending a measured baseline, not promises.

A release is done only when every required task and release scenario has evidence. Compile success, a screenshot, or a plan completed on paper is insufficient.
