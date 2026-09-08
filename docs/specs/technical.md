# Technical specification

Status: Phases 1–4 implemented and verified locally. [Phase 1](../phase1.md) records filesystem/parser contracts; [Phase 2](../phase2.md) records index/search APIs; [Phase 3](../phase3.md) records watcher lifetime, subscription ordering, shared-handle status, conservative full-content batching and recovery. [Phase 4](../phase4.md) records the implemented read-only desktop commands, lifetime, rendering policy and native evidence. Phase 5+ APIs below remain design contracts, not code. Normative requirements are in [product.md](product.md); choices are in the [decision register](../decisions.md).

## 1. Workspace and boundaries

```text
Cargo.toml
crates/notes-core/src/{lib,library,note,frontmatter,filesystem,error}.rs
crates/notes-core/tests/
crates/notes-cli/src/{main,commands,output}.rs
crates/notes-cli/tests/
# Phase 2 additions:
crates/notes-core/src/{index,search}.rs
crates/notes-core/migrations/
# Phase 3 additions:
crates/notes-core/src/{watcher,events}.rs
# Phase 4 onward:
apps/desktop/src-tauri/
apps/desktop/src/
```

Paths are suggested module ownership, not a request to create all modules immediately. Begin with the smallest vertical slice. Core depends on neither Tauri nor frontend types. CLI/Tauri validate transport shape and translate core results; they do not implement a second parser, file writer, identity resolver, or indexer.

Candidate dependencies to assess when needed: `clap`, `tempfile`, a maintained YAML parser meeting S01, `pulldown-cmark`, `rusqlite` with FTS5 availability tested, `notify`, `tracing`, and `thiserror`. These are not pinned selections. Pin actual toolchain/package versions and commit lockfiles during implementation. Avoid an async runtime until a concrete integration requires it.

## 2. Library, configuration, paths

One selected root is remembered in OS-native application configuration outside the notes tree. CLI and desktop resolve the same configuration. A per-command root override is useful for scripts/tests but does not introduce a library manager. `init` is repeatable and never empties a directory. Switching selected root must explicitly replace the selection.

Derived cache/database and cooperative locks live outside the root, namespaced by canonical root. Configuration is preference state, not note data; UI state may reset if deleted. A moved root can be selected again and its cache rebuilt. Never synchronize SQLite, its WAL/SHM files, or app locks as note content.

Discovery visits regular lowercase `.md` files recursively, including hidden directories under the proposed policy. Ignore own identifiable staging files (which must not have `.md` suffix); do not indiscriminately ignore unfamiliar Markdown. Symlinks are not followed. Unsupported names, unreadable files, and hard-linked mutation targets become diagnostics, not silent omission or coercion.

Use a validated `LibraryRelativePath`, separate from arbitrary `PathBuf`. Reject traversal components, absolute mutation destinations, path prefixes outside the root, symlink components, and unsafe platform names. For a nonexistent destination, validate the existing parent chain and create directories only within the root. Canonicalization alone is not sufficient: S02 must validate handle-relative/no-follow operations or explicitly document weaker guarantees. Do not claim security against a hostile concurrent ancestor swap without tests and supporting primitives.

## 3. Document and identity contract

- Document identity is its validated library-relative path. No embedded ID type, ID lookup, or compatibility mode remains.
- `NoteDocument`: library-relative path, tags, raw Markdown body, lossless source/frontmatter representation, and `Revision` derived from full file bytes.
- `NoteSummary`: relative path, derived display title, tags; no body duplication required in the domain summary.
- `Revision`: opaque full-byte content hash, used as a precondition, never written into frontmatter.
- `Diagnostic`: stable code, path(s), safe explanation, optional repair guidance. No note body in logs/errors by default.

Frontmatter is recognized only at the beginning after an optional UTF-8 BOM. Support conventional YAML opening/closing delimiters as established by S01 fixtures. A `---` inside a fenced block or later body is not frontmatter. Input must be UTF-8; invalid encoding is preserved and diagnosed. Parse a mapping without executing custom YAML tags or constructors. Restrict parser resource usage for pathological aliases/nesting.

App-owned fields:

```yaml
tags: [elixir, architecture]
```

`id` is ordinary unowned YAML, with any type supported by the parser; it is neither validated as an identifier nor stripped. `tags` is absent or a sequence of strings; scalar tags, duplicate YAML keys, malformed mappings, and unterminated frontmatter are errors. Unknown keys, nested values, and unsupported body syntax are not removed. If a safe metadata patch cannot preserve an input, refuse the mutation with a diagnostic.

Selection, init, scan, watch and indexing never modify Markdown. Supported read-only documents can be read/indexed without frontmatter. Creation writes plain Markdown unless tags are explicitly requested. Body/tag changes preserve BOM, newline style and unrelated metadata, including existing `id` fields.

Tag equality is exact/case-sensitive; adding an existing tag is idempotent, removing a missing tag is idempotent. Preserve spelling and first occurrence order; do not case-fold into a hierarchy. Derive title from the first actual parsed H1, not a regex matching a fenced code block; fall back to filename stem.

Distinct paths remain distinct documents even with identical hashes or metadata. Hashes are full-byte revision preconditions, not persistent identities. External moves are conservatively observed as old-path deletion and new-path creation; do not silently retarget selection by content equality. Future app moves explicitly update the open path after a successful move. Unreadable unrelated files do not prevent guarded mutation or search of healthy paths.

## 4. Core operation contracts

| Operation | Contract |
|---|---|
| `init_library(root)` | Validate/create/select root without modifying Markdown; return summary plus per-file diagnostics |
| `create_note(path/title, body, tags)` | Plain Markdown with optional tags; no-clobber creation; collision is a typed error, never implicit overwrite |
| `get_note(path)` | Return current disk document and revision, not cached canonical content |
| `list_notes(filters)` | Stable ordering by relative path; expose incomplete/stale state and diagnostics rather than claiming completeness |
| `update_note(path, expected_revision, body)` | Preserve unknown frontmatter; reject stale disk revision |
| `move_note(path, expected_revision, destination)` | Preserve complete bytes; reject occupied destination; whole destination filename is explicit |
| `delete_note(path, expected_revision)` | Explicit permanent removal of selected file only; reject stale revision; no recursive directory deletion |
| `add_tag/remove_tag(path, expected_revision, tag)` | Guarded metadata-only patch; same preservation constraints as body updates |
| `search(query, filters)` | Derived FTS results with relative paths; never use results as authority for writes |
| `rescan()` | Read-only full content reconciliation; structured report |
| `reindex()` | Rebuild derived state from complete filesystem reconciliation; preserve notes; diagnostics mean result is incomplete |
| `doctor()` | Read-only inspection by default; no source rewriting; optional explicit safe repair invokes known operations |
| `status()` | Actual root, discovered/indexed/invalid counts, cache state, current process watcher state |

Core error families: invalid path, not found, destination exists, invalid metadata, revision conflict, permission denied, unsupported filesystem, library busy, I/O failure, index degraded/corrupt. Avoid collapsing all failures into strings.

Return mutation outcome separately from index outcome: `file_committed` with revision/path plus `index_clean` or a warning requiring reconciliation. A caller must not retry a committed mutation as if the file write failed.

## 5. Filesystem consistency and safety

For update: acquire cooperative per-library mutation lock; resolve safe path; read and validate current bytes/revision; stage a same-directory non-note temp file with restricted permissions; write complete content; flush/sync; recheck precondition and identity; atomically replace using supported platform primitive; sync parent where available; publish committed result; update derived state. Preserve appropriate existing file permissions. Do not create a temporarily world-readable copy of a private note.

Create requires an atomic no-clobber primitive, not only `exists()` followed by overwrite. Move requires no-clobber semantics too, handles case-only rename explicitly, and does not silently copy-delete across filesystems. Reject cross-device moves in v1 if atomicity cannot be met. Delete checks revision and safe path first. Empty folders are not implicitly deleted.

Failure before replacement leaves old note intact; failure after replacement is reported as committed/uncertain durability rather than ordinary untouched failure. Temp files are not notes and cleanup must never remove unrelated user files. Test recovery behavior before adding automatic stale-temp cleanup.

Hash preconditions and local locks do **not** implement distributed locking or guarantee compare-and-swap against arbitrary external writes. Document and test the residual final check/rename race. File-save durability guarantees are limited to tested normal local filesystem semantics, not power-loss immunity on every device.

Multiple Foglio processes serialize mutations/rebuild via short-lived cooperative local locking; readers may coexist. Use bounded busy handling, no daemon. External tools ignore this lock. SQLite transaction safety alone does not protect two file writers.

## 6. Derived index and search (Phase 2)

Schema version 2 (known schema 1 caches are discarded under the root lock and rebuilt without touching Markdown; future versions are refused):

```text
files(path PRIMARY KEY, title, body, tags_json, content_hash, fingerprint, stale)
notes(path PRIMARY KEY, title NOT NULL, content_hash NOT NULL)
tags(note_path REFERENCES notes(path) ON DELETE CASCADE, tag,
     PRIMARY KEY(note_path, tag))
notes_fts(note_path UNINDEXED, title, body, path, tags)  # FTS5 virtual table
```

Use schema versioning, foreign keys, transactional metadata/tag/FTS updates, unique relative paths, and explicit tag collation. A database may cache body text for search; that text is always derived. WAL/busy timeout and connection ownership are implementation choices to validate with multi-process tests.

Initial/rebuild scan reads actual files and populates a complete new derived generation before publishing it. Unreadable subtrees are **unknown**, not evidence of deletion; preserve potentially valid cached records but mark them stale and exclude them from healthy claims until resolved. Confirmed absence removes records. Healthy paths remain searchable during partial scans; inaccessible paths remain excluded with an incomplete report.

Recover corruption by replacing/rebuilding only app-owned cache files under coordinated access; never delete Markdown. Missing DB triggers rebuild. If index work fails after file mutation, show degraded state and retry reconciliation rather than rolling back user content from SQLite.

Warm opening uses metadata candidates and cached hashes to avoid re-parsing unchanged bodies. Watcher-touched paths are content-checked even if size/mtime match. Full `rescan`/`reindex` hashes everything. Metadata shortcuts cannot prove no offline same-size/same-mtime edit occurred; expose/document that freshness limit.

Search defaults to safe literal token search, not raw user-supplied SQL/FTS syntax. Expose explicit phrase/prefix behavior with tests, escape FTS expressions, parameterize SQL, filter exact tags and component-aware folder prefixes, rank using FTS5 BM25 with deterministic path tie-break. Start with title weighted above body; tune only with fixtures. Return snippets as text and highlight safely, never HTML-inject snippets. Empty query returns a usage error. Invalid queries cannot corrupt state.

## 7. Watcher and reconciliation (Phase 3)

Recursive `notify` events are hints, not truth. Debounce dirty paths into bounded batches; inspect present state. Directory moves/deletes invalidate the subtree; watcher overflow, unsupported event detail, or dropped client events triggers full reconciliation. Provide a periodic safety reconciliation policy and explicit manual rescan; choose bounded intervals based on measurement, not an event-count assumption.

Start observing before startup reconciliation and drain accumulated events afterward to avoid a scan/watch gap. Reconcile self-generated atomic saves by content hash, not blanket timed suppression. Publish events only after committed state, with degraded-index information when needed.

Small in-process subscription contract: `NoteCreated`, `NoteChanged`, `NoteDeleted`, `DiagnosticsChanged`, `IndexStateChanged`, `RescanRequired`. Events carry paths/revisions, not full document bodies by default. Bounded queue overflow produces invalidation, not silent missed updates. Watcher shutdown releases resources deterministically. No external event bus and no fake persistent watcher for short-lived CLI commands.

## 8. CLI (Phase 1 onward)

Use shared root resolution; proposed global flags are `--library <root>` and `--json`. Human tables go to stdout, diagnostics to stderr; JSON returns one stable envelope with result/diagnostics and explicit incomplete status. Final schema is frozen with black-box tests at P1-06.

Commands accept literal library-relative `.md` paths only, without `id:` or `path:` selector prefixes. `show` emits the ordinary complete Markdown document in human/raw mode. `new <title>` uses a documented portable filename policy; invalid/empty names and collisions fail rather than overwrite. Optional `--path` bypasses title-to-name derivation but not safety validation. `move` takes a complete relative destination ending in `.md`, not an ambiguous directory-or-file argument.

Commands: source lifecycle commands plus `tags`, `tag add/remove`, `search`, `rescan`, `reindex`, `status`, `doctor`. `rescan` is an explicit maintenance addition corresponding to the source core API. No `sync`, daemon, or watch-service command. CLI mutation commands reconcile enough current disk state to detect stale data even without a running watcher.

Proposed exit codes: 0 complete success; 1 operational or partial failure; 2 usage/validation; 3 not found; 4 conflict/busy; 5 diagnostic checks found issues. Freeze command-specific mapping in help/tests. Never report watcher active merely because the desktop may be running: CLI status says inactive for this process unless it actually starts one.

## 9. Desktop and editor (Phases 4–6)

Tauri owns library/core lifetime and watcher subscriptions. Use typed command DTOs and domain-to-transport error mapping. Frontend invokes narrowly scoped commands, not arbitrary filesystem APIs. Blocking scan/index work must not block UI commands; validate execution boundaries under load.

Start with library selection, physical folder tree, note/tag lists, note opening, safe Markdown preview, and domain search. Then add source editing/autosave and mutations. UI frontmatter is managed separately from body editing to preserve unknown keys. No persisted proprietary editor document becomes canonical.

Editor state machine: `clean`, `dirty`, `saving`, `save_error`, `conflict`, `missing_on_disk`. Snapshot path, base revision, content, and buffer generation at dispatch. Serialize saves per note; an older acknowledgement cannot mark a newer buffer clean. Edits during a save schedule a later save with the acknowledged revision. Navigation/close must flush safely or explicitly preserve/cancel, never discard unsaved content silently.

On external event, compare fetched disk revision with base revision, not just event type. Clean buffers refresh; dirty/saving buffers with divergent disk state enter conflict and pause autosave. Same-content self-events do not cause a conflict. External moves leave the selected path missing unless reliable move evidence is implemented; current reconciliation never auto-follows. Never retarget by hash, especially with unsaved edits. Missing paths pause autosave and must not be recreated by an old save.

Conflict choices: reload current disk with explicit local-discard confirmation, or save local text as a new note at an explicitly chosen no-clobber path while leaving original disk content intact. If reload/save-copy sees another change, keep conflict state and refresh options. No automatic merging, force overwrite, or history store is required.

## 10. Rendering, security, diagnostics

Sanitize CommonMark/GFM output with a tested allowlist; disable raw HTML or sanitize it identically. No script execution, event attributes, dangerous URL schemes, remote image requests, or implicit local-file embedding. User-activated permitted `https`/`http`/`mailto` links go through controlled external opening. Relative `.md` links resolve through safe library paths; unsupported links show a safe non-executing fallback. Attachments/images stay unsupported and their Markdown is preserved.

Use restrictive Tauri capabilities and CSP, avoid broad shell/filesystem permissions, and test malicious Markdown in the actual webview. Log structured categories (`filesystem`, `watcher`, `parser`, `index`, `search`, `commands`), no full content or search queries by default.

Doctor reports readability/writability without modifying notes, parser errors, SQLite/schema health, stale/missing/orphan records, skipped paths and unsupported conditions. Writability probes are explicitly temporary and never note rewrites. Safe repairs are explicit reindex operations; source corrections remain manual.

Healthy watcher status is a quiet footer “Monitoring external changes”, not a save indicator. Failures stay visible/actionable. Future editor save status remains separate.
