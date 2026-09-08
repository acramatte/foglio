# Product specification

Status: implementation draft derived from the supplied brief. Established requirements below preserve its scope; proposed clarifications are identified in the [decision register](../decisions.md).

## Purpose

Foglio browses and edits one directory of ordinary Markdown documents, including repository documentation, agent specs/plans, and personal notes. The same headless Rust core serves a first-class CLI and a thin Tauri/TypeScript desktop client. Users can stop using Foglio and continue using their notes in any text editor.

## Requirements

| ID | Requirement | Acceptance |
|---|---|---|
| R01 | Markdown files, not SQLite, are authoritative | Delete derived state, reopen, and reconstruct all valid notes without losing document bytes |
| R02 | Paths identify documents; hashes identify revisions | App moves preserve bytes and explicitly update the selected path; external moves never retarget by hash alone |
| R03 | Supported Markdown needs no preparation | Plain documents browse/search/open without IDs or frontmatter; selection/init/indexing preserve source bytes |
| R04 | Portable optional metadata | App owns only optional `tags`; existing `id` is ordinary user metadata, preserved without identity semantics |
| R05 | One selected library, physical folders | CLI and desktop use the configured root; paths are real relative paths |
| R06 | Shared headless domain operations | CRUD, move, tags, search, maintenance live in Rust core, not frontend adapters |
| R07 | First-class CLI | Initialization, list/show/new/move/delete, tags, search, status, reindex, doctor work against real files |
| R08 | Lexical search | SQLite FTS5 searches title, body, tags, and path; complete rebuild is supported |
| R09 | Supported external modifications | Recursive reconciliation observes creation, edits, moves, and deletion without desktop restart |
| R10 | Conservative mutations | Crash-conscious writes, destination collision checks, stale-write detection, no silent ambiguous repairs |
| R11 | Useful desktop editing | Folder/note/tag browsing, search, create, edit, move/rename, delete, tag editing, rendering, autosave |
| R12 | External-change UX | Clean editor reloads; dirty editor pauses saving and requires an explicit resolution |
| R13 | External filesystem sync | No sync engine; delayed updates and conflict files are handled as filesystem inputs |
| R14 | Diagnostics | Status reflects the actual process/index; doctor reports malformed metadata, stale/orphan records, and access problems |
| R15 | Secure Markdown boundary | Sanitize rendering, restrict links, prevent traversal and out-of-library file access, never execute document content |
| R16 | Personal-library performance | Measure cold/warm startup, reconciliation, and search at 1,000 and 50,000 notes; no unconditional warm full-body parse |
| R17 | Recoverable failure | File safety takes priority over indexing; failures clearly distinguish a committed file change from an index failure |

## User workflows

### Select a Markdown directory

`notes init <directory>` creates or selects a library without clearing existing files. Existing Markdown is never modified. Supported documents need no IDs or frontmatter; malformed or unsupported input is preserved and diagnosed. Selection is stored outside the notes root. Deleting configuration requires selecting the root again, not recovering notes from a database.

### CLI note lifecycle

```text
notes init <directory>
notes new <title>
notes list
notes show <relative-path>
notes move <relative-path> <destination>
notes delete <relative-path>
```

Phase 1 includes this workflow, tag operations, and core update operations. Search and index commands arrive in Phase 2; watcher support in Phase 3; full doctor in Phase 7. Commands not yet implemented must not pretend to succeed.

### Everyday desktop use

Select the library, browse a real folder or tag, open a note, edit Markdown, and see saving/saved/error state. Search title/body/path/tags. Rename or move with complete bytes preserved and the selected path explicitly updated. Delete only after an explicit destructive action. Open the root in another editor and see ordinary Markdown.

### External editing and conflict

External modifications to a clean open note reload the editor while preserving navigation where practical. A dirty note that changes, moves ambiguously, or disappears on disk must stop autosaving. The UI must not silently discard the editor buffer or the disk version. Basic conflict choices are enough; a merge editor is not required.

### Recovery and synchronization

Close the application, synchronize only the notes directory through an external tool, and open the library on another device. Rebuild device-local state from files. A conflict copy is a separate path, even when its content and metadata match another document. Never merge or retarget documents by matching hashes alone.

## Definition of v1

The release gate is the complete workflow in source §37, plus the file-safety and security cases in [verification](verification.md): desktop creation/editing, external editor refresh, byte-preserving move, text/tag search, cross-device external transfer, and recovery after SQLite deletion.

A phase being complete does not mean v1 is complete. A successful CLI prototype is specifically not the desktop release.

## Non-goals

No attachments or image management/rendering, proprietary document format, wiki links, semantic/vector search, embeddings, LLM/AI/MCP, sharing, collaboration, simultaneous document editing, CRDTs, custom sync server, provider APIs, built-in sync commands, libSQL/Turso, encryption, note history, multiple independent libraries, daemon, plugin framework, spreadsheets, authenticators, or mobile implementation.

Preserve unsupported Markdown syntax as text; exclusion of a feature is not permission to delete that syntax. Future relative attachments remain possible without adding an attachment abstraction now.

## Scope interpretation

- Desktop creation, source editing and guarded autosave are implemented in Phase 5. Rich mode is omitted after S03 preservation failure. Phase 6 reload/discard and save-copy conflict choices remain open. Search UX improvements are deferred; current literal search is unchanged.
- Healthy watcher status belongs in a quiet footer: “Monitoring external changes”. It is not a save indicator; monitoring failures stay visible and actionable. Future save status is separate.

- CommonMark plus useful GFM: tables, tasks, fenced code, links, strikethrough, and ordinary inline/block elements.
- Source editor plus sanitized preview is sufficient for v1. Milkdown/ProseMirror is a preferred rich-editor candidate, contingent on preservation testing, not an unconditional release dependency.
- Tags are case-preserving, non-hierarchical strings. Proposed exact-match filter semantics are in the technical spec.
- No operational timestamps, revision, device ID, hash, or provider fields are inserted into frontmatter.
- The user normally edits a given note on one device at a time. This assumption does not excuse silent overwrites detectable locally.
