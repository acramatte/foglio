# Technical specification

Status: implementation draft. Normative source requirements are in [product.md](product.md); accepted naming/platform/license/scope choices and remaining proposed defaults are tracked in the [decision register](../decisions.md). API shapes below are contracts to implement, not existing code.

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

Candidate dependencies to assess when needed: `clap`, `ulid`, `tempfile`, a maintained YAML parser meeting S01, `pulldown-cmark`, `rusqlite` with FTS5 availability tested, `notify`, `tracing`, and `thiserror`. These are not pinned selections. Pin actual toolchain/package versions and commit lockfiles during implementation. Avoid an async runtime until a concrete integration requires it.

## 2. Library, configuration, paths

One selected root is remembered in OS-native application configuration outside the notes tree. CLI and desktop resolve the same configuration. A per-command root override is useful for scripts/tests but does not introduce a library manager. `init` is repeatable and never empties a directory. Switching selected root must explicitly replace the selection.

Derived cache/database and cooperative locks live outside the root, namespaced by canonical root. Configuration is preference state, not note data; UI state may reset if deleted. A moved root can be selected again and its cache rebuilt. Never synchronize SQLite, its WAL/SHM files, or app locks as note content.

Discovery visits regular lowercase `.md` files recursively, including hidden directories under the proposed policy. Ignore own identifiable staging files (which must not have `.md` suffix); do not indiscriminately ignore unfamiliar Markdown. Symlinks are not followed. Unsupported names, unreadable files, and hard-linked mutation targets become diagnostics, not silent omission or coercion.

Use a validated `LibraryRelativePath`, separate from arbitrary `PathBuf`. Reject traversal components, absolute mutation destinations, path prefixes outside the root, symlink components, and unsafe platform names. For a nonexistent destination, validate the existing parent chain and create directories only within the root. Canonicalization alone is not sufficient: S02 must validate handle-relative/no-follow operations or explicitly document weaker guarantees. Do not claim security against a hostile concurrent ancestor swap without tests and supporting primitives.

## 3. Document and identity contract

- `NoteId`: validated full 26-character ULID, canonical serialized representation. Reject malformed existing IDs; do not substitute for them. The shortened examples in the brief are not test fixtures.
- `NoteDocument`: identity, library-relative path, tags, raw Markdown body, lossless source/frontmatter representation, and `Revision` derived from full file bytes.
- `NoteSummary`: identity, relative path, derived display title, tags; no body duplication required in the domain summary.
- `Revision`: opaque full-byte content hash, used as a precondition, never written into frontmatter.
- `Diagnostic`: stable code, path(s), safe explanation, optional repair guidance. No note body in logs/errors by default.

Frontmatter is recognized only at the beginning after an optional UTF-8 BOM. Support conventional YAML opening/closing delimiters as established by S01 fixtures. A `---` inside a fenced block or later body is not frontmatter. Input must be UTF-8; invalid encoding is preserved and diagnosed. Parse a mapping without executing custom YAML tags or constructors. Restrict parser resource usage for pathological aliases/nesting.

App-owned fields:

```yaml
id: <valid ULID>
tags: [elixir, architecture]
```

`id` is required after successful adoption. `tags` is absent or a sequence of strings; scalar tags, null/wrong-type ID, duplicate YAML keys, malformed mappings, and unterminated frontmatter are errors. Unknown keys, nested values, and unsupported body syntax are not removed. If a safe metadata patch cannot preserve an input, refuse the mutation with a diagnostic.

Missing-ID adoption uses the same guarded write pipeline as edits. Insert only the new ID/minimal delimiters, leaving body bytes and unknown keys intact. Preserve BOM and newline style. Do not rewrite valid ID-bearing files on discovery. Existing read-only files can be read/indexed when valid; missing-ID read-only files stay unadopted with a diagnostic. Re-read unstable inputs rather than writing a stale parsed snapshot.

Tag equality is exact/case-sensitive; adding an existing tag is idempotent, removing a missing tag is idempotent. Preserve spelling and first occurrence order; do not case-fold into a hierarchy. Derive title from the first actual parsed H1, not a regex matching a fenced code block; fall back to filename stem.

Duplicate IDs are a library-wide diagnostic. Every member is excluded from unambiguous ID resolution and normal search, even if one was indexed first. Read raw content by explicit path for inspection; ordinary app mutation refuses an ambiguous group. Never regenerate IDs silently. Full scans collect the complete identity map before publishing healthy entries.

## 4. Core operation contracts

| Operation | Contract |
|---|---|
| `init_library(root)` | Validate/create/select root; adopt eligible Markdown; return summary plus per-file diagnostics |
| `create_note(path/title, body, tags)` | Generate new ID; no-clobber creation; collision is a typed error, never implicit overwrite |
| `get_note(selector)` | Return current disk document and revision, not cached canonical content |
| `list_notes(filters)` | Stable ordering by relative path; expose incomplete/stale state and diagnostics rather than claiming completeness |
| `update_note(id, expected_revision, body)` | Preserve ID/unknown frontmatter; reject stale disk revision |
| `move_note(id, expected_revision, destination)` | Preserve complete bytes and ID; reject occupied destination; whole destination filename is explicit |
| `delete_note(id, expected_revision)` | Explicit permanent removal of selected file only; reject stale revision; no recursive directory deletion |
| `add_tag/remove_tag(id, expected_revision, tag)` | Guarded metadata-only patch; same preservation constraints as body updates |
| `search(query, filters)` | Derived FTS results with unambiguous IDs; never use results as authority for writes |
| `rescan()` | Full content reconciliation and optional eligible missing-ID adoption; structured report |
| `reindex()` | Rebuild derived state from complete filesystem reconciliation; preserve notes; diagnostics mean result is incomplete |
| `doctor()` | Read-only inspection by default; no adoption or ID repair; optional explicit safe repair invokes known operations |
| `status()` | Actual root, discovered/indexed/invalid counts, cache state, current process watcher state |

Core error families: invalid selector/path, not found, destination exists, invalid metadata, ambiguous ID, revision conflict, permission denied, unsupported filesystem, library busy, I/O failure, index degraded/corrupt. Avoid collapsing all failures into strings.

Return mutation outcome separately from index outcome: `file_committed` with revision/path plus `index_clean` or a warning requiring reconciliation. A caller must not retry a committed mutation as if the file write failed.

## 5. Filesystem consistency and safety

For update/adoption: acquire cooperative per-library mutation lock; resolve safe path; read and validate current bytes/revision; stage a same-directory non-note temp file with restricted permissions; write complete content; flush/sync; recheck precondition and identity; atomically replace using supported platform primitive; sync parent where available; publish committed result; update derived state. Preserve appropriate existing file permissions. Do not create a temporarily world-readable copy of a private note.

Create requires an atomic no-clobber primitive, not only `exists()` followed by overwrite. Move requires no-clobber semantics too, handles case-only rename explicitly, and does not silently copy-delete across filesystems. Reject cross-device moves in v1 if atomicity cannot be met. Delete checks revision and unambiguous identity first. Empty folders are not implicitly deleted.

Failure before replacement leaves old note intact; failure after replacement is reported as committed/uncertain durability rather than ordinary untouched failure. Temp files are not notes and cleanup must never remove unrelated user files. Test recovery behavior before adding automatic stale-temp cleanup.

Hash preconditions and local locks do **not** implement distributed locking or guarantee compare-and-swap against arbitrary external writes. Document and test the residual final check/rename race. File-save durability guarantees are limited to tested normal local filesystem semantics, not power-loss immunity on every device.

Multiple Foglio processes serialize mutations/adoption/rebuild via short-lived cooperative local locking; readers may coexist. Use bounded busy handling, no daemon. External tools ignore this lock. SQLite transaction safety alone does not protect two file writers.

## 6. Derived index and search (Phase 2)

Proposed schema:

```text
notes(id PRIMARY KEY, path UNIQUE NOT NULL, title NOT NULL,
      content_hash NOT NULL, modified_at, size_bytes)
tags(note_id REFERENCES notes ON DELETE CASCADE, tag,
     PRIMARY KEY(note_id, tag))
notes_fts(note_id UNINDEXED, title, body, path, tags)  # FTS5 virtual table
```

Use schema versioning, foreign keys, transactional metadata/tag/FTS updates, unique relative paths, and explicit tag collation. A database may cache body text for search; that text is always derived. WAL/busy timeout and connection ownership are implementation choices to validate with multi-process tests.

Initial/rebuild scan reads actual files, identifies ambiguity, and populates a complete new derived generation before publishing it. Unreadable subtrees are **unknown**, not evidence of deletion; preserve potentially valid cached records but mark them stale and exclude them from healthy claims until resolved. Confirmed absence removes records. Removed duplicate groups restore eligibility for the remaining valid file on reconciliation.

Recover corruption by replacing/rebuilding only app-owned cache files under coordinated access; never delete Markdown. Missing DB triggers rebuild. If index work fails after file mutation, show degraded state and retry reconciliation rather than rolling back user content from SQLite.

Warm opening uses metadata candidates and cached hashes to avoid re-parsing unchanged bodies. Watcher-touched paths are content-checked even if size/mtime match. Full `rescan`/`reindex` hashes everything. Metadata shortcuts cannot prove no offline same-size/same-mtime edit occurred; expose/document that freshness limit.

Search defaults to safe literal token search, not raw user-supplied SQL/FTS syntax. Expose explicit phrase/prefix behavior with tests, escape FTS expressions, parameterize SQL, filter exact tags and component-aware folder prefixes, rank using FTS5 BM25 with deterministic path tie-break. Start with title weighted above body; tune only with fixtures. Return snippets as text and highlight safely, never HTML-inject snippets. Empty query returns a usage error. Invalid queries cannot corrupt state.

## 7. Watcher and reconciliation (Phase 3)

Recursive `notify` events are hints, not truth. Debounce dirty paths into bounded batches; inspect present state. Directory moves/deletes invalidate the subtree; watcher overflow, unsupported event detail, or dropped client events triggers full reconciliation. Provide a periodic safety reconciliation policy and explicit manual rescan; choose bounded intervals based on measurement, not an event-count assumption.

Start observing before startup reconciliation and drain accumulated events afterward to avoid a scan/watch gap. Reconcile self-generated atomic saves by content hash, not blanket timed suppression. Publish events only after committed state, with degraded-index information when needed.

Small in-process subscription contract: `NoteCreated`, `NoteChanged`, `NoteMoved`, `NoteDeleted`, `DiagnosticsChanged`, `IndexStateChanged`, `RescanRequired`. Events carry IDs/paths/revisions, not full document bodies by default. Bounded queue overflow produces invalidation, not silent missed updates. Watcher shutdown releases resources deterministically. No external event bus and no fake persistent watcher for short-lived CLI commands.

## 8. CLI (Phase 1 onward)

Use shared root resolution; proposed global flags are `--library <root>` and `--json`. Human tables go to stdout, diagnostics to stderr; JSON returns one stable envelope with result/diagnostics and explicit incomplete status. Final schema is frozen with black-box tests at P1-06.

Selectors accept bare ID or relative path; if ambiguous, reject and request `id:<ulid>` or `path:<relative-path>`. `show` emits the ordinary complete Markdown document in human/raw mode. `new <title>` uses a documented portable filename policy; invalid/empty names and collisions fail rather than overwrite. Optional `--path` bypasses title-to-name derivation but not safety validation. `move` takes a complete relative destination ending in `.md`, not an ambiguous directory-or-file argument.

Commands: source lifecycle commands plus `tags`, `tag add/remove`, `search`, `rescan`, `reindex`, `status`, `doctor`. `rescan` is an explicit maintenance addition corresponding to the source core API. No `sync`, daemon, or watch-service command. CLI mutation commands reconcile enough current disk state to detect stale data and ID ambiguity even without a running watcher.

Proposed exit codes: 0 complete success; 1 operational or partial failure; 2 usage/validation; 3 not found; 4 conflict/ambiguity/busy; 5 diagnostic checks found issues. Freeze command-specific mapping in help/tests. Never report watcher active merely because the desktop may be running: CLI status says inactive for this process unless it actually starts one.

## 9. Desktop and editor (Phases 4–6)

Tauri owns library/core lifetime and watcher subscriptions. Use typed command DTOs and domain-to-transport error mapping. Frontend invokes narrowly scoped commands, not arbitrary filesystem APIs. Blocking scan/index work must not block UI commands; validate execution boundaries under load.

Start with library selection, physical folder tree, note/tag lists, note opening, safe Markdown preview, and domain search. Then add source editing/autosave and mutations. UI frontmatter is managed separately from body editing to preserve unknown keys and immutable IDs. No persisted proprietary editor document becomes canonical.

Editor state machine: `clean`, `dirty`, `saving`, `save_error`, `conflict`, `missing_on_disk`. Snapshot ID, base revision, content, and buffer generation at dispatch. Serialize saves per note; an older acknowledgement cannot mark a newer buffer clean. Edits during a save schedule a later save with the acknowledged revision. Navigation/close must flush safely or explicitly preserve/cancel, never discard unsaved content silently.

On external event, compare fetched disk revision with base revision, not just event type. Clean buffers refresh; dirty/saving buffers with divergent disk state enter conflict and pause autosave. Same-content self-events do not cause a conflict. External move preserves open-note identity; deletion/duplicate identity prevents recreation by an old autosave.

Conflict choices: reload current disk with explicit local-discard confirmation, or save local text as a new note/new ID while leaving original disk content intact. If reload/save-copy sees another change, keep conflict state and refresh options. No automatic merging, force overwrite, or history store is required.

## 10. Rendering, security, diagnostics

Sanitize CommonMark/GFM output with a tested allowlist; disable raw HTML or sanitize it identically. No script execution, event attributes, dangerous URL schemes, remote image requests, or implicit local-file embedding. User-activated permitted `https`/`http`/`mailto` links go through controlled external opening. Relative `.md` links resolve through safe library paths; unsupported links show a safe non-executing fallback. Attachments/images stay unsupported and their Markdown is preserved.

Use restrictive Tauri capabilities and CSP, avoid broad shell/filesystem permissions, and test malicious Markdown in the actual webview. Log structured categories (`filesystem`, `watcher`, `parser`, `index`, `search`, `commands`), no full content or search queries by default.

Doctor reports readability/writability without modifying notes, invalid/missing/duplicate IDs, parser errors, SQLite/schema health, stale/missing/orphan records, skipped paths and unsupported conditions. Writability probes, if used, are explicitly temporary and never note rewrites. Read-only doctor cannot adopt missing IDs by opening a mutating library handle. Safe repairs are explicit reindex/adoption operations; ambiguous identity repair stays manual.
