# Verification specification

Status: V01–V10 Phase 1 checks and V11–V15 Phase 2 checks (V14 status portion only), plus V25 initial baseline, pass locally on Linux x86_64 with Rust 1.93.1; see [Phase 1](../phase1.md#verification-evidence) and [Phase 2 evidence/limitations](../phase2.md). V16/V17 and the V24 development simulation now pass: [Phase 3 evidence](../phase3.md). Hosted CI currently covers Phase 0 only. V18/V19 Phase 4 pass in actual Tauri/WebKit under Xvfb; see [desktop evidence and limits](../phase4.md). V20–V22 Phase 5 source editing/autosave/lifecycle pass locally, including actual WebKit keyboard edits and native-close protection; see [Phase 5 evidence](../phase5.md). Doctor, full conflict choices, actual cross-device sync and release checks remain planned and unrun.

## Test discipline

Use temporary libraries and isolated config/cache directories. Assert actual file bytes and executable behavior; mocks alone do not validate filesystem preservation. Core tests do not need Tauri. Watcher tests use an independent writer process and bounded eventual-state assertions, not exact event counts/order or blind fixed sleeps. Include deterministic reconciliation tests separately from native watcher tests.

Fault injection is appropriate at filesystem commit boundaries, backed by actual on-disk assertions; process-kill tests add realism but do not prove arbitrary power-loss durability. Record OS/filesystem/toolchain for platform-sensitive results. Never test destructive operations on the user's real notes.

Suggested test locations:

- `crates/notes-core/tests/{document,library,filesystem,index,search,watcher}_*.rs`
- `crates/notes-core/tests/fixtures/` for preservation and hostile inputs.
- `crates/notes-cli/tests/` for black-box compiled CLI tests.
- `apps/desktop/src/` colocated editor/renderer state tests and `apps/desktop/tests/` for actual desktop integration.
- `tests/acceptance/` for release orchestration and reproducible fixture generation once needed.

## Scenario catalog

| ID | Scenario and observable pass condition | Layer / earliest gate |
|---|---|---|
| V01 | Workspace fmt/strict Clippy/tests pass; CLI help executes; test HOME/config/cache isolation cannot touch normal user state | Build/CLI, P0 |
| V02 | Init absent/existing root, repeat init, root reselection after config deletion, nested/hidden/non-note discovery; all existing Markdown bytes unchanged | Core/CLI, P1 |
| V03 | Plain Markdown, valid frontmatter, inline/list tags, unknown/nested keys, arbitrary supported `id` metadata types, wrong types, duplicate YAML keys, malformed/unterminated YAML, aliases/resource limits, UTF-8/BOM/CRLF, fenced `---`, actual H1 extraction | Parser, P1 |
| V04 | Plain Markdown and arbitrary `id` metadata are discoverable without preparation; init/select/scan/index/watch preserve complete bytes, including read-only input; new untagged files have no generated frontmatter | Core real files, P1 |
| V05 | Two paths with identical content or `id` metadata are independently readable, searchable and mutable; changing/deleting one preserves the other; no hash-based retargeting | Core P1; index P2 |
| V06 | Create/read/update/move/rename/delete/tags; move preserves complete bytes; body edits preserve metadata; tag changes preserve body; collision and unsafe path leave targets unchanged | Core real files, P1 |
| V07 | Traversal/absolute/escaping prefix, symlink root/ancestor/leaf, hard-linked target, non-UTF-8 path, reserved names and case-only move; no out-of-root reads/writes; unsupported cases explicit | Security/platform, P1 and P7 |
| V08 | Inject failure before write/flush/replace and after commit; kill process during staged save; result is old or complete new note, not truncation. Stale expected revisions reject update/move/delete/tag; file modes preserved; committed/index-failure outcome is distinct once index exists | Filesystem P1; index extension P2 |
| V09 | Run actual CLI init/new/list/show/move/tags/tag add/tag remove/delete; inspect output/JSON/stderr/exit codes, literal path arguments, filename collisions, noninteractive delete confirmation and invalid arguments | Black-box CLI, P1 |
| V10 | Record ordinary notes, remove app configuration/cache only, reselect root; all authoritative content/tags preserved. Phase 1 must not need a DB to pass | Core/CLI, P1 |
| V11 | Scan→index, edit→update, move→path change, delete→removal; compare full file manifest before/after DB removal and reindex. Corrupt cache rebuilt, no content restored from cache | SQLite/CLI, P2 |
| V12 | Metadata/tags/FTS change transactionally; indexing fault after durable save yields degraded status; unreadable subtree stays unknown, not deleted; partial scan/rebuild never claims complete health | SQLite/core, P2 |
| V13 | Search title/body/path/tags, phrase/prefix, punctuation/Unicode, exact case-preserving tag filters, folder `foo` versus `foobar`, ranked ties, empty/malformed/SQL-like queries, safe snippets | Search/CLI, P2 |
| V14 | Status uses actual totals and process watcher state; doctor read-only checks library access, YAML, DB/schema, stale/orphans; compare note bytes before/after; explicit reindex repair never rewrites source documents | Status P2; doctor P7 |
| V15 | Two CLI processes and desktop-like core: simultaneous selection/create/stale update/rebuild with readers; bounded lock/busy outcomes, no duplicate unintended files, DB integrity and no silent lost app-process update | Multi-process, P2 |
| V16 | Independent writer creates/edits/atomic-replaces/moves/deletes a note and whole directory while core watches; startup scan/watch boundary; final paths/content/index converge | Native watcher, P3 |
| V17 | Burst/self-save events, same-size/same-mtime touched file, dropped/overflow event hints, subscriber queue overflow, permission loss/recovery, watcher stop/restart; eventual rescan repairs state without event loop | Watcher/reconciliation, P3 |
| V18 | Real desktop selects library, browses folders/notes/tags, searches, opens current content, responds to changes, and closes without orphan watcher; scan work does not freeze interaction | Actual desktop, P4 |
| V19 | Required CommonMark/GFM rendering; scripts, raw HTML payloads, event handlers, dangerous schemes, remote images, escaping local links, malicious snippets blocked in actual webview; CSP/capabilities reviewed | Renderer/security, P4 and P7 |
| V20 | Source/preview round trip: tables/tasks/code/strikethrough/links plus unsupported wiki/image syntax preserved; no-op editing never silently rewrites unknown YAML; rich editor tested only if selected | Editor, P5 |
| V21 | Debounce, typing during save, out-of-order acknowledgements, note switch, window close, permission/disk error, stale revision: newest dirty generation never falsely shown saved; buffers retained on failure | State machine + desktop, P5 |
| V22 | Real desktop create/edit/autosave/move/rename/tags/delete updates ordinary files and explicit path selection; destructive confirmation and error paths exercised | Desktop/file lifecycle, P5 |
| V23 | External edit of clean/dirty/saving note; external rename/delete/identical copy; repeated changes during conflict choices; clean reloads, dirty saves pause, save-copy has a new no-clobber path, no resurrection or silent overwrite | Two-process desktop, P6 |
| V24 | External transfer to another device without preparation; delayed updates, deletion, and identical conflict copy; valid files discovered as distinct paths and preserved. Exercise actual selected sync tool for release, not just provider-shaped filenames | Simulation P3/P6; real transfer P7 |
| V25 | Fixed 1k/50k datasets; cold rebuild, warm startup, body parse count, search p50/p95, watcher convergence, memory; keep environment and distributions; full reindex catches same-metadata offline changes | Benchmark P2 baseline/P7 gate |
| V26 | Keyboard-only navigation/create/search/edit, focus behavior and labels, empty/error states; fresh packaged install/uninstall does not remove user library | Desktop/package, P7 |
| V27 | Full release workflow below using packaged build; all supported-platform gates evidenced; untested platforms/features labeled unsupported | Release, P7 |

## Source invariant traceability

| Source | Invariant | Scenarios |
|---|---|---|
| §35.1 and §43.5 | SQLite/app-state deletion cannot destroy authoritative notes | V10, V11, V27 |
| §35.2 and §43.3 | Renaming/moving preserves bytes and explicitly changes path (D26 supersedes original stable-ID contract) | V06, V16, V22 |
| §35.3 | Another process's edit eventually appears while running | V16, V18, V23 |
| §35.4 and §43.2 | Supported Markdown can be opened without changing content | V03, V04 |
| §35.5 and §43.4 | Unknown frontmatter survives edits | V03, V06, V20 |
| §35.6 | Crash-conscious save avoids truncated note | V08 |
| §43.1 | Newly created note is ordinary valid Markdown | V06, V09 |

## Product requirement coverage

| Requirement | Scenarios |
|---|---|
| R01 | V10, V11, V27 |
| R02 | V06, V16, V22 |
| R03 | V03, V04 |
| R04 | V03, V06, V20 |
| R05 | V02, V07, V18 |
| R06 | V01, V06, V09, V18, architectural dependency review |
| R07 | V09, V13, V14 |
| R08 | V11, V12, V13 |
| R09 | V16, V17, V18 |
| R10 | V05, V06, V07, V08, V15 |
| R11 | V18, V20, V21, V22, V26 |
| R12 | V21, V23 |
| R13 | V05, V24, V27 |
| R14 | V12, V14 |
| R15 | V07, V19 |
| R16 | V17, V25 |
| R17 | V08, V12, V21, V23 |

## Proposed performance budgets

Agree these at S05 against recorded hardware and corpus characteristics before treating them as release thresholds:

- Warm search p95 at or below 100 ms at 50,000 notes, measured at the core API, excluding UI rendering.
- A settled ordinary external single-file edit reflected within 2 seconds at p95 on an idle local filesystem; editor UI update measured separately.
- Warm startup must not parse every unchanged Markdown body; measure time to usable UI separately from background metadata reconciliation.
- Cold rebuild is a measured baseline, not an invented timing guarantee. Capture total corpus bytes, file-size distribution, storage medium, OS, CPU/RAM, and warm/cold cache conditions.

Fixtures are synthetic **test data**, labeled as such, never fabricated real usage or benchmark results. Include representative tags, Unicode, folder depth, and Markdown constructs, plus pathological large-file/parser inputs in a separate robustness corpus. Publish regressions rather than dropping slow runs.

## Final release workflow

1. Install packaged build in a fresh target environment and initialize a disposable library.
2. Create/edit/tag notes through desktop. Inspect files in VS Code or another external text editor; record paths, revisions and a file manifest.
3. Edit a clean note externally and verify live refresh. Repeat with a dirty buffer and verify explicit conflict handling without silent loss.
4. Move/rename a note through the app and externally; confirm byte preservation, searchable new path, and no automatic external-move retargeting.
5. Search body text, title, tags and folder; verify expected results.
6. Close the app. Transfer/synchronize the library using an actual external filesystem tool to another device. Do not copy app config, cache, SQLite WAL, or locks.
7. Open the transferred root there and verify paths/content, plus a deliberate identical conflict copy is independently accessible and preserved.
8. Close relevant processes, remove only app-owned SQLite/cache state, reopen, and verify full derived reconstruction against the manifest. No user Markdown may disappear.
9. Run doctor, packaged CLI checks, security suite, keyboard workflow and declared OS smoke gates.
10. Record build revision, platforms, tool versions, commands, outputs, residual limitations, and task/PR evidence. Mark v1 complete only if every required gate passes.

Synthetic transfer fixtures are adequate during development. They are not evidence that Syncthing/rclone or cross-device release acceptance was actually exercised. If another device, native webview, packaging credentials, or target OS is unavailable, record that release gate as blocked.
