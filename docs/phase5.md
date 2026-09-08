# Phase 5 — Source editing and autosave

P5-01–P5-04 implemented and verified locally on Linux, including review regressions. Hosted CI remains pending. No commits or pushes.

## Behavior

- Source-body editor and sanitized preview; Ctrl+E toggles in either direction, Ctrl+S flushes. Frontmatter is kept outside the editor and tags use core metadata patches. Preview overflow is contained in the note pane rather than scrolling the application window.
- Autosave debounces for 400 ms. A dispatch snapshots library session, path, revision, body and buffer generation. Saves serialize; edits during an in-flight save use its acknowledged revision for the next save, never falsely marking a newer buffer saved.
- Failed saves retain the buffer and pause automatic retries. Permission/I/O errors offer explicit retry. Stale and missing paths pause without overwrite or resurrection. Phase 6's reload/discard and save-copy choices are not implemented; copy retained source manually if needed.
- Create, move/rename, add/remove tags and explicitly confirmed permanent deletion invoke guarded core operations. New-note titles use the shared filename policy (ASCII spaces become hyphens and `.md` is appended); an optional library-relative folder such as `blog` or `blog/engineering` places the derived filename in that hierarchy. Creation remains no-clobber. Tag removal offers the selected note's existing tags and is unavailable when there are none.
- Note/library/link navigation and native close flush or remain on the current buffer. The editor is read-only during a guarded transition. Native WM_DELETE_WINDOW is intercepted; only a successful frontend flush authorizes backend watcher shutdown and exit. Process termination/crashes are not protected and there is no durable draft/history store.
- Monitoring is separate from save state. Committed file outcomes with index/durability warnings remain successful saves, never automatic retries.

## S03 / D17 evidence

The isolated Milkdown 7.22.1 CommonMark/GFM + ProseMirror model 1.25.11/state 1.4.4/view 1.42.3 spike rejected rich mode. Untouched and heading-edited round trips removed fenced-code extra metadata, escaped an inline directive, converted reference structure, and normalized whitespace/newlines. Tested footnotes and raw HTML survived; do not generalize that they were lost.

Actual `Document::parse`/`with_body` preserved targeted source edits and separately retained BOM/CRLF YAML. Existing sanitized preview preserved source but is intentionally non-reversible. Chrome textarea experiments confirmed CRLF/lone-CR normalization to LF. The native WebKit fixture verifies BOM, uniform CRLF, comments, arbitrary `id` metadata, nested YAML, GFM and unsupported wiki/image/directive/code syntax with an actual keyboard edit.

Spike artifacts and exact commands: `/tmp/foglio-s03-3c0qMK/EVIDENCE.md`, `spike.mjs`, `baseline.mjs`, `core-check`, and JSON results. Temporary paths are local evidence, not durable release artifacts. Rich mode is omitted rather than silently normalizing source.

### Review fixes and source mapping

Three independent-review regressions were reproduced as failing tests, fixed, and rerun successfully. Mutations retain their lock through follow-up opening; no overlapping operation can release another operation's lock and discard a newly edited buffer. Skipped watcher refreshes remain pending until the selected buffer becomes clean. Each textarea input delta maps against the current raw body, retaining the saved body for exact reversion and the original newline convention for inserted text. Sequential disjoint edits preserve untouched mixed newlines, verified in actual WebKit and on disk. A single replacement range uses the document's newline convention inside that range; the editor does not provide multi-cursor/global replace operations.

## Executed verification

Linux x86_64, Rust 1.93.1, Node 24.18.0, actual Tauri/WebKit under Xvfb, tauri-driver 2.0.6. All libraries and XDG state are temporary; no personal notes are fixtures.

Passed:

- `cargo fmt --all -- --check`
- `cargo test --locked --workspace --all-features`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `npm test` in `apps/desktop`: 37 passing tests
- `npm run typecheck` and `npm run build`
- `npm run tauri -- build --debug --no-bundle` and optimized `--no-bundle`
- `python3 tests/acceptance/phase1.py`: 12 passing scenarios
- `python3 tests/acceptance/phase2.py`: 4 passing scenarios
- `TAURI_DRIVER=/tmp/foglio-phase4-tools/bin/tauri-driver python3 tests/acceptance/phase4.py`
- `TAURI_DRIVER=/tmp/foglio-phase4-tools/bin/tauri-driver python3 tests/acceptance/phase5.py`: 9 passing native scenarios
- Both native harnesses repeated successfully with `FOGLIO_DESKTOP_BINARY=$PWD/target/release/foglio-desktop`.
- `git diff --check`

The native Phase 5 harness verifies ordinary on-disk lifecycle, collision and delete cancellation, metadata/source preservation, immediate navigation flush, actual permission failure/retry, blocked failed/conflicted native close, stale-save preservation, missing-path non-resurrection and dirty native close flushing before process exit. It explicitly terminates conflicted fixture processes only to reset tests, not as app-provided conflict resolution. Frontend tests cover debounce, typing during a save, revision/generation handling, stale watcher reads and failed transition retention. Rust mutation tests exercise real files, concurrent stale writes, malformed revisions, safety paths and postcommit index degradation.

### Harness pitfalls

Build the native binary via Tauri immediately before native acceptance: plain Cargo workspace tests can replace the debug binary with a dev-URL build. WebKit send-keys of multiline strings dropped newline keys in this environment; the preservation case uses real W3C key actions for a selected single-word edit. Other mutation cases dispatch input in the actual webview and inspect real files. Native close targets the exact `Foglio` X11 window, not hidden `foglio-desktop` helper windows.

## Limits

Hosted CI has not run. No physical-display manual/accessibility qualification, installers, other OS targets, cross-device sync, crash-draft recovery or large-corpus performance claim. Existing cooperative-lock/final external-writer race and supported-YAML restrictions remain. Phase 6 conflict choices and Phase 7 release hardening stay open.
