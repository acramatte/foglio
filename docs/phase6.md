# Phase 6 — External-change experience

P6-01–P6-03 are verified locally on Linux. D19 resolves to explicit discard/reload or a guarded new-path copy; no force-overwrite, automatic merge, history or hash-based retargeting.

## Behavior and boundaries

- A watcher refresh reads the selected path even when its editor is dirty. Pending autosaves wait for that observation; observations during a save wait for its acknowledgement. Equal revisions do not create self-event conflicts. Clean refresh retains selection/caret/scroll where practical; dirty divergence and missing paths retain the local body and pause autosave.
- Saves read back their committed revision before becoming clean or allowing navigation/native close. This catches an external replacement during delayed acknowledgement, even without newer typing. A verification I/O failure retains the buffer; explicit retry checks the disk without replaying the already committed mutation. There is still no cross-process atomic compare-and-swap.
- **Reload / discard local** reads the current disk state before displaying an explicit destructive confirmation, reads again afterward, and discards only when those observations agree. Confirmed absence clears selection without recreating a file. Cancel retains the buffer. Another edit/delete/reappearance requires a fresh choice.
- **Save local as new note** takes a literal library-relative `.md` destination, distinct from the original path (including case-only variants), and preserves the retained original frontmatter/BOM plus the current raw local body. It does not borrow metadata from the conflicting disk version, synthesize an empty-body heading, silently suffix collisions, or overwrite an existing destination.
- The backend receives `{session,path,observedRevision,destination,baseSource,body}`. A null observation means confirmed absence, never an unchecked write. `Library::save_copy` holds one cooperative lock and checks raw source bytes/absence before staging and immediately before the destination's atomic no-clobber replacement. Permission/other I/O failures are not treated as absence. The untouched source need not be writable.
- Committed copy/index/durability warnings remain committed outcomes. If the copy is replaced, removed or unreadable before its follow-up open, the UI reports the committed path and retains the original local buffer. A successful copy becomes the selected independent note and resumes ordinary guarded editing.
- Healthy external monitoring remains separate from save status. Search semantics and narrow Tauri capabilities are unchanged.

```text
Watcher generation → wait for current save → disk observation → clean refresh / retained conflict
Save acknowledgement → revision readback → saved / retained conflict or error
Conflict choice → observe → explicit confirmation/path → reobserve
  reload → install confirmed snapshot / clear missing selection
  copy → core source guards + atomic no-clobber create → verify copy → select new path
```

## Verification

Environment: Linux `7.0.0-31-generic`, Rust `1.93.1`, Node `24.18.0`, Python `3.14.7`; native fixtures on `/tmp` tmpfs, Tauri/WebKit under Xvfb. Only disposable libraries and isolated HOME/XDG state were used.

```sh
cargo fmt --all -- --check
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo build --locked -p notes-cli
python3 tests/acceptance/phase1.py
python3 tests/acceptance/phase2.py

cd apps/desktop
npm test
npm run tauri -- build --debug --no-bundle
cd ../..
python3 tests/acceptance/phase6.py
python3 tests/acceptance/phase5.py
python3 tests/acceptance/phase4.py

cd apps/desktop
npm run tauri -- build --no-bundle
cd ../..
FOGLIO_DESKTOP_BINARY="$PWD/target/release/foglio-desktop" python3 tests/acceptance/phase6.py
FOGLIO_DESKTOP_BINARY="$PWD/target/release/foglio-desktop" python3 tests/acceptance/phase5.py
```

All listed final checks pass. Frontend: **60 tests**, typecheck and production assets pass. CLI acceptance: **12 + 4** tests. Native Phase 6: **9 scenarios** on debug and optimized binaries; Phase 5: **10 scenarios** on both, plus Phase 4 debug regression. The CI workflow now includes Phase 6 native acceptance; hosted CI has not run.

Core/backend tests cover late source change/removal/reappearance during staging, a racing destination, held cooperative lock, no-clobber/same-path guards, original metadata and exact body/empty-body preservation, stale sessions, raw malformed-current-source guards and committed index failures. Frontend regression tests initially failed for missing observation behavior and delayed-save verification before implementation.

The native harness uses separate Python processes for actual writes/moves/deletes and identical conflict copies. Its in-flight scenarios withhold the real `fetch` IPC response after the real save commits; they never replace the command result. They verify both newer typing and the no-newer-typing/native-close case against file bytes. V24 is an external-filesystem simulation, not a selected sync-provider or second-device transfer.

## Limits and recovery

- Cooperative locks cannot prevent an arbitrary external writer from changing/reverting a path between observations or in the final check-to-operation window. Reload/readback are also snapshots, not a lock on other applications. No force-write or original-path resurrection fallback is provided.
- Native regression exposed transient `busy` outcomes from watcher/mutation contention. These are non-committing failures: source remains retained, navigation/close stays blocked, and **Retry save** is explicit. Phase 5 navigation acceptance now exercises this user-visible recovery when needed, not hidden IPC retries. Same-process contention polish is recorded under P7-02; no contention-free claim is made.
- Current desktop conflict observations require a parseable current note (or typed absence). If an external edit makes YAML unsupported/malformed or the path unreadable, the buffer stays retained and resolution may remain blocked until the disk issue is repaired. Core save-copy accepts raw current revisions, but no raw-revision recovery UI is claimed.
- Supported-YAML restrictions remain; the native preservation fixture uses the supported block nested mapping rather than unsupported flow mappings. An initial fixture using a flow mapping was rejected without source changes.
- An initial workspace doctest run encountered a stale `yaml-rust2` build artifact. `cargo clean -p yaml-rust2` followed by the complete workspace tests and strict Clippy passed; no dependency change was needed.
- No physical-display visual/accessibility qualification, installers, other OS targets, crash drafts/history, large-corpus performance or actual cross-device sync claim. Phase 7 remains open. No commits, pushes or PRs were created.
