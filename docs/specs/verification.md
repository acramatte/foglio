# Verification specification

Status: V01–V10 Phase 1 checks pass locally on Linux x86_64 with Rust 1.93.1; see [exact evidence and limitations](../phase1.md#verification-evidence). Hosted CI currently covers Phase 0 only. V11–V27 remain planned and unrun.

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
| V02 | Init absent/existing root, repeat init, root reselection after config deletion, nested/hidden/non-note discovery; existing notes unchanged except eligible adoption | Core/CLI, P1 |
| V03 | Plain Markdown, valid frontmatter, inline/list tags, unknown/nested keys, full valid/invalid ULIDs, wrong types, duplicate YAML keys, malformed/unterminated YAML, aliases/resource limits, UTF-8/BOM/CRLF, fenced `---`, actual H1 extraction | Parser, P1 |
| V04 | Missing-ID adoption adds valid ID and preserves body and unknown metadata; valid ID-bearing discovery does not rewrite; read-only/invalid/unstable input remains untouched or guarded/retried | Core real files, P1 |
| V05 | Two paths with same ID: both diagnosed, no scan-order winner/no automatic regeneration; explicit path inspection works; normal mutation fails; later index excludes whole group and recovers when collision removed | Core P1; index P2 |
| V06 | Create/read/update/move/rename/delete/tags; move preserves complete bytes/ID; body edits preserve metadata; tag changes preserve body; collision and ambiguous selector leave targets unchanged | Core real files, P1 |
| V07 | Traversal/absolute/escaping prefix, symlink root/ancestor/leaf, hard-linked target, non-UTF-8 path, reserved names and case-only move; no out-of-root reads/writes; unsupported cases explicit | Security/platform, P1 and P7 |
| V08 | Inject failure before write/flush/replace and after commit; kill process during staged save; result is old or complete new note, not truncation. Stale expected revisions reject update/move/delete/adoption; file modes preserved; committed/index-failure outcome is distinct once index exists | Filesystem P1; index extension P2 |
| V09 | Run actual CLI init/new/list/show/move/tags/tag add/tag remove/delete; inspect output/JSON/stderr/exit codes, selector ambiguity, filename collisions, noninteractive delete confirmation and invalid arguments | Black-box CLI, P1 |
| V10 | Record notes after adoption, remove app configuration/cache only, reselect root; all authoritative content/IDs/tags preserved. Phase 1 must not need a DB to pass | Core/CLI, P1 |
| V11 | Scan→index, edit→update, move→path change, delete→removal; compare full file manifest before/after DB removal and reindex. Corrupt cache rebuilt, no content restored from cache | SQLite/CLI, P2 |
| V12 | Metadata/tags/FTS change transactionally; indexing fault after durable save yields degraded status; unreadable subtree stays unknown, not deleted; partial scan/rebuild never claims complete health | SQLite/core, P2 |
| V13 | Search title/body/path/tags, phrase/prefix, punctuation/Unicode, exact case-preserving tag filters, folder `foo` versus `foobar`, ranked ties, empty/malformed/SQL-like queries, safe snippets | Search/CLI, P2 |
| V14 | Status uses actual totals and process watcher state; doctor read-only checks library access, IDs/YAML, DB/schema, stale/orphans; compare note bytes before/after; explicit reindex repair never resolves identity ambiguity | Status P2; doctor P7 |
| V15 | Two CLI processes and desktop-like core: simultaneous adoption/create/stale update/rebuild with readers; bounded lock/busy outcomes, no duplicate unintended files, DB integrity and no silent lost app-process update | Multi-process, P2 |
| V16 | Independent writer creates/edits/atomic-replaces/moves/deletes a note and whole directory while core watches; startup scan/watch boundary; final IDs/paths/content/index converge | Native watcher, P3 |
| V17 | Burst/self-save events, same-size/same-mtime touched file, dropped/overflow event hints, subscriber queue overflow, permission loss/recovery, watcher stop/restart; eventual rescan repairs state without event loop | Watcher/reconciliation, P3 |
| V18 | Real desktop selects library, browses folders/notes/tags, searches, opens current content, responds to changes, and closes without orphan watcher; scan work does not freeze interaction | Actual desktop, P4 |
| V19 | Required CommonMark/GFM rendering; scripts, raw HTML payloads, event handlers, dangerous schemes, remote images, escaping local links, malicious snippets blocked in actual webview; CSP/capabilities reviewed | Renderer/security, P4 and P7 |
| V20 | Source/preview round trip: tables/tasks/code/strikethrough/links plus unsupported wiki/image syntax preserved; no-op editing never silently rewrites unknown YAML; rich editor tested only if selected | Editor, P5 |
| V21 | Debounce, typing during save, out-of-order acknowledgements, note switch, window close, permission/disk error, stale revision: newest dirty generation never falsely shown saved; buffers retained on failure | State machine + desktop, P5 |
| V22 | Real desktop create/edit/autosave/move/rename/tags/delete updates ordinary files and stable IDs; destructive confirmation and error paths exercised | Desktop/file lifecycle, P5 |
| V23 | External edit of clean/dirty/saving note; external rename/delete/duplicate ID; repeated changes during conflict choices; clean reloads, dirty saves pause, save-copy has new ID, no resurrection or silent overwrite | Two-process desktop, P6 |
| V24 | External transfer to another device after adoption; delayed updates, deletion, and conflict file with duplicate ID; valid files discovered, ambiguous files preserved/diagnosed. Exercise actual selected sync tool for release, not just provider-shaped filenames | Simulation P3/P6; real transfer P7 |
| V25 | Fixed 1k/50k datasets; cold rebuild, warm startup, body parse count, search p50/p95, watcher convergence, memory; keep environment and distributions; full reindex catches same-metadata offline changes | Benchmark P2 baseline/P7 gate |
| V26 | Keyboard-only navigation/create/search/edit, focus behavior and labels, empty/error states; fresh packaged install/uninstall does not remove user library | Desktop/package, P7 |
| V27 | Full release workflow below using packaged build; all supported-platform gates evidenced; untested platforms/features labeled unsupported | Release, P7 |

## Source invariant traceability

| Source | Invariant | Scenarios |
|---|---|---|
| §35.1 and §43.5 | SQLite/app-state deletion cannot destroy authoritative notes | V10, V11, V27 |
| §35.2 and §43.3 | Renaming/moving preserves identity | V06, V16, V22 |
| §35.3 | Another process's edit eventually appears while running | V16, V18, V23 |
| §35.4 and §43.2 | Arbitrary Markdown can be adopted without losing content | V03, V04 |
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
2. Create/edit/tag notes through desktop. Inspect files in VS Code or another external text editor; record IDs and a file manifest.
3. Edit a clean note externally and verify live refresh. Repeat with a dirty buffer and verify explicit conflict handling without silent loss.
4. Move/rename a note through the app and externally; confirm ID stability and searchable new location.
5. Search body text, title, tags and folder; verify expected results.
6. Close the app. Transfer/synchronize the library using an actual external filesystem tool to another device. Do not copy app config, cache, SQLite WAL, or locks.
7. Open the transferred root there and verify note identity/content, plus a deliberate duplicate-ID conflict copy is preserved and diagnosed.
8. Close relevant processes, remove only app-owned SQLite/cache state, reopen, and verify full derived reconstruction against the manifest. No user Markdown may disappear.
9. Run doctor, packaged CLI checks, security suite, keyboard workflow and declared OS smoke gates.
10. Record build revision, platforms, tool versions, commands, outputs, residual limitations, and task/PR evidence. Mark v1 complete only if every required gate passes.

Synthetic transfer fixtures are adequate during development. They are not evidence that Syncthing/rclone or cross-device release acceptance was actually exercised. If another device, native webview, packaging credentials, or target OS is unavailable, record that release gate as blocked.
