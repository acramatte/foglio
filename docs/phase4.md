# Phase 4 — Read-only desktop

The Linux Tauri client lives in `apps/desktop`. It browses an existing library without editing or deleting Markdown. Editing/autosave remains Phase 5. Native packaging/installers and other operating systems remain release work.

## Build and run

Use Rust 1.93.1, Node 24.18.0 and npm with the committed lockfiles. On Ubuntu, install:

```sh
sudo apt-get install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev patchelf xvfb xauth webkit2gtk-driver
cd apps/desktop
npm ci
npm run typecheck
npm test
npm run tauri -- build --debug --no-bundle
../../target/debug/foglio-desktop
```

`npm run tauri -- dev` runs the Vite development server on port 1420. `npm run tauri -- build --no-bundle` builds the optimized application with embedded production frontend. Bundling is deliberately disabled until release packaging. An app icon is generated from `app-icon.svg` using the pinned Tauri CLI; only the Linux-required `src-tauri/icons/icon.png` is retained.

The workspace defaults to core/CLI. On machines without GTK/WebKit, use `cargo test --locked -p notes-core -p notes-cli` and similarly scoped build/Clippy commands. Full workspace commands include native dependencies; build the frontend first so Tauri can embed it. CI has separate headless and desktop jobs; the desktop job runs actual WebKit acceptance under Xvfb.

## Behavior and architecture

Enter an existing notes directory in **Library folder** and choose **Open library**. Selection uses the same configuration and root format as the CLI through `Library::select_existing()`. Neither selection nor `init` changes Markdown. Supported files need no frontmatter or IDs.

The three-column view shows physical folders (including empty folders), exact tags, notes/search results and a read-only preview with path and tags. Folder filters include descendants. Note opening and link navigation use safe relative paths; identical content and metadata do not disable documents. Diagnostics remain visible. Search is the existing core literal FTS search, capped at the core default of 50 hits. The UI currently displays the returned count, not an exhaustive-match total.

```text
Tauri commands → blocking worker → shared Library → Markdown / disposable SQLite
                                          ↑
recursive watcher → bounded subscription → cached session/generation state
                                          ↓
750 ms frontend poll → refetch lists / current note by path → sanitized preview
```

Slow selection, scans, search, current-file reads, link resolution and shutdown joins run off the UI thread. Selections and core reads serialize on a backend mutex, but `desktop_state` uses a separate short-held state lock and remains available while core work waits. Session tokens reject requests for a previous selection; frontend request generations prevent old responses overwriting newer views. Selection persists only after the new watcher starts, and failed selection keeps the previous session. Window exit waits for watcher shutdown; reopening reloads the shared configuration.

A watcher invalidation is consumed before refetching its snapshot. Same-path edits reload the open document. External moves/deletes leave the old selected path unavailable; choosing the new path is explicit. Identical hashes never retarget selection. Unreadable documents also show an unavailable state. Recovery refetches it again. There is no editor buffer, autosave or conflict resolution in this phase.

Healthy watcher status lives in the footer as **Monitoring external changes**, with an explanation that this is external monitoring, not saving. Failures stay visible in the alert area. Future save state remains separate.

## Security boundary

- Marked renders CommonMark/GFM tables, tasks, fenced code, strikethrough and normal text; raw HTML is escaped. DOMPurify applies an explicit tag/attribute allowlist.
- Images, including reference images, are textual placeholders. No local-file embedding, image fetching, inline event handlers, frames, scripts or document-supplied styles are permitted.
- Links never navigate the webview directly. User-activated HTTP/HTTPS/mailto goes through a backend URL allowlist and controlled OS opening; unsupported schemes are inert. Actual external application launching is not exercised by acceptance, to avoid touching user apps.
- Relative Markdown links resolve through the core's safe path checks, with contained parents allowed, traversal outside the root rejected, and symlink checks. Fragments open the destination note but do not scroll to an anchor.
- The main-window capability has no filesystem/shell/opener-plugin permissions. Only narrow application commands exist. Production CSP denies images, frames, objects, remote scripts and remote connections; frontend IPC is explicitly allowed.

## Verification evidence

Environment: Ubuntu 26.04.1 Linux x86_64, unprivileged user, Rust 1.93.1, Node 24.18.0/npm 11.16.0, GTK 3.24.52, WebKitGTK 2.52.6. Tests use isolated temporary HOME/XDG directories and libraries, never personal notes.

- **Backend:** six real-filesystem tests pass. Covers byte-preserving selection/browse, plain Markdown and arbitrary user `id` metadata, empty folders, controlled links and URL schemes, duplicates, watcher updates, stale sessions, failed selection, concurrent selections, responsive cached state while a core lock is held, shutdown/reopen.
- **Frontend:** 18 Vitest/jsdom tests pass. Covers GFM and hostile HTML/URL/image fixtures; stale note/search/library responses; generation refresh; moved/deleted notes; navigation clicks during pending refresh; controlled links. These tests are not claimed as native evidence.
- **Native V18/V19:** `TAURI_DRIVER=/tmp/foglio-phase4-tools/bin/tauri-driver python3 tests/acceptance/phase4.py` passes against `target/debug/foglio-desktop` using the real WebKit driver and Xvfb. The optimized production build (`npm run tauri -- build --no-bundle`) also passes the same native suite with `FOGLIO_DESKTOP_BINARY=/home/alexis/Development/private/foglio/target/release/foglio-desktop`. Exercises onboarding, unchanged source bytes, empty folder/tag filtering, actual FTS, safe GFM DOM, contained-parent links and traversal/scheme rejection through actual IPC, external-process edit/move, duplicate conflict copies, permission loss/recovery, deletion and close/reopen. A loopback HTTP trap receives zero image requests.
- **Regression:** full workspace tests and strict Clippy, formatting, core/CLI build, both Python acceptance suites (12 and 4 scenarios), and diff checks pass.
- **Review:** independent review found a navigation refresh null dereference and frontend/backend relative-link mismatch; both fixed with regression tests. Initial frontend tests also exposed double-encoded path classification inconsistency, now rejected before invoking the backend.

## Limitations

Hosted CI has not run for this delivery. Linux is the only validated platform; Xvfb/native WebKit is actual application execution but not a manual physical-display/accessibility review. Large-library UI/search latency and memory budgets remain P7. The inherited watcher hashes the full library per batch; browse scans files and requests serialize, so no large-corpus responsiveness guarantee is claimed. The always-visible path-entry onboarding is not a native directory picker. No rich editor, mutations, sync integration, daemon, installers or release guarantee are included.
