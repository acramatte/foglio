# Product specification

Status: implementation draft derived from the supplied brief. Established requirements below preserve its scope; proposed clarifications are identified in the [decision register](../decisions.md).

## Purpose

Foglio manages one directory of ordinary Markdown notes. The same headless Rust core serves a first-class CLI and a thin Tauri/TypeScript desktop client. Users can stop using Foglio and continue using their notes in any text editor.

## Requirements

| ID | Requirement | Acceptance |
|---|---|---|
| R01 | Markdown files, not SQLite, are authoritative | Delete derived state, reopen, and reconstruct all valid notes without losing document bytes |
| R02 | Stable identity is independent of path | App and external moves preserve the frontmatter ID |
| R03 | Arbitrary Markdown can be adopted | A missing ID is inserted safely; body and unrelated frontmatter survive |
| R04 | Portable minimal metadata | App owns only `id` and optional `tags`; no operational metadata in files |
| R05 | One selected library, physical folders | CLI and desktop use the configured root; paths are real relative paths |
| R06 | Shared headless domain operations | CRUD, move, tags, search, maintenance live in Rust core, not frontend adapters |
| R07 | First-class CLI | Initialization, list/show/new/move/delete, tags, search, status, reindex, doctor work against real files |
| R08 | Lexical search | SQLite FTS5 searches title, body, tags, and path; complete rebuild is supported |
| R09 | Supported external modifications | Recursive reconciliation observes creation, edits, moves, and deletion without desktop restart |
| R10 | Conservative mutations | Crash-conscious writes, destination collision checks, stale-write detection, no silent ambiguous repairs |
| R11 | Useful desktop editing | Folder/note/tag browsing, search, create, edit, move/rename, delete, tag editing, rendering, autosave |
| R12 | External-change UX | Clean editor reloads; dirty editor pauses saving and requires an explicit resolution |
| R13 | External filesystem sync | No sync engine; delayed updates and conflict files are handled as filesystem inputs |
| R14 | Diagnostics | Status reflects the actual process/index; doctor reports malformed metadata, duplicates, stale/orphan records, and access problems |
| R15 | Secure Markdown boundary | Sanitize rendering, restrict links, prevent traversal and out-of-library file access, never execute document content |
| R16 | Personal-library performance | Measure cold/warm startup, reconciliation, and search at 1,000 and 50,000 notes; no unconditional warm full-body parse |
| R17 | Recoverable failure | File safety takes priority over indexing; failures clearly distinguish a committed file change from an index failure |

## User workflows

### Initialize and adopt

`notes init <directory>` creates or selects a library without clearing existing files. Valid Markdown without IDs is adopted. Malformed or ambiguous documents are preserved and diagnosed. Selection is stored outside the notes root. Deleting configuration requires selecting the root again, not recovering notes from a database.

### CLI note lifecycle

```text
notes init <directory>
notes new <title>
notes list
notes show <id-or-path>
notes move <id-or-path> <destination>
notes delete <id-or-path>
```

Phase 1 includes this workflow, tag operations, and core update operations. Search and index commands arrive in Phase 2; watcher support in Phase 3; full doctor in Phase 7. Commands not yet implemented must not pretend to succeed.

### Everyday desktop use

Select the library, browse a real folder or tag, open a note, edit Markdown, and see saving/saved/error state. Search title/body/path/tags. Rename or move without changing identity. Delete only after an explicit destructive action. Open the root in another editor and see ordinary Markdown.

### External editing and conflict

External modifications to a clean open note reload the editor while preserving navigation where practical. A dirty note that changes, moves ambiguously, or disappears on disk must stop autosaving. The UI must not silently discard the editor buffer or the disk version. Basic conflict choices are enough; a merge editor is not required.

### Recovery and synchronization

Close the application, synchronize only the notes directory through an external tool, and open the library on another device. Rebuild device-local state from files. A conflict copy with an existing ID is an identity collision, not permission to assign a new ID automatically.

## Definition of v1

The release gate is the complete workflow in source §37, plus the file-safety and security cases in [verification](verification.md): desktop creation/editing, external editor refresh, stable-ID move, text/tag search, cross-device external transfer, and recovery after SQLite deletion.

A phase being complete does not mean v1 is complete. A successful CLI prototype is specifically not the desktop release.

## Non-goals

No attachments or image management/rendering, proprietary document format, wiki links, semantic/vector search, embeddings, LLM/AI/MCP, sharing, collaboration, simultaneous document editing, CRDTs, custom sync server, provider APIs, built-in sync commands, libSQL/Turso, encryption, note history, multiple independent libraries, daemon, plugin framework, spreadsheets, authenticators, or mobile implementation.

Preserve unsupported Markdown syntax as text; exclusion of a feature is not permission to delete that syntax. Future relative attachments remain possible without adding an attachment abstraction now.

## Scope interpretation

- CommonMark plus useful GFM: tables, tasks, fenced code, links, strikethrough, and ordinary inline/block elements.
- Source editor plus sanitized preview is sufficient for v1. Milkdown/ProseMirror is a preferred rich-editor candidate, contingent on preservation testing, not an unconditional release dependency.
- Tags are case-preserving, non-hierarchical strings. Proposed exact-match filter semantics are in the technical spec.
- No operational timestamps, revision, device ID, hash, or provider fields are inserted into frontmatter.
- The user normally edits a given note on one device at a time. This assumption does not excuse silent overwrites detectable locally.
