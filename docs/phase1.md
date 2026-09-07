# Phase 1: filesystem notes and CLI

Phase 1 is implemented and verified locally on Linux x86_64 with Rust 1.93.1. No Phase 1 hosted CI run is claimed: these changes have not been committed or pushed. The workflow includes the acceptance harness for the next hosted run.

## Commands and storage

Build with `cargo build --locked --workspace`; the executable is `target/debug/notes`.

```bash
notes init /absolute/path/to/notes
notes new "Architecture" --tag rust
notes new "Nested note" --path Projects/Nested.md --body "Plain Markdown body"
notes list
notes show path:Architecture.md
notes tag add path:Architecture.md design
notes tag remove path:Architecture.md rust
notes tags
notes move path:Architecture.md Projects/Architecture.md
notes delete path:Projects/Architecture.md --yes
```

These are usage examples, not commands run against a personal library. `init` selects/creates the root and adopts eligible missing-ID Markdown files. Repeat `init` to adopt later imports. Other commands scan current disk state without implicit adoption. Non-note files, unsupported paths, missing IDs, and invalid documents are diagnosed; scans with diagnostics exit 1, even if valid notes were successfully adopted/listed. Inspect the report before treating a partial scan as complete.

The selected canonical root is JSON in `$XDG_CONFIG_HOME/foglio/config.json`, or `$HOME/.config/foglio/config.json` when XDG_CONFIG_HOME is absent. Both bases must be absolute. `--library <root>` overrides selection for one command without changing it; it cannot be combined with `init <directory>`. There is no Phase 1 database or cache. Locks are outside the library under the same application config directory: `selection.lock` for selecting a root, and `locks/<SHA-256-of-canonical-root>.lock` for library mutations. No note body, ID authority, or tags live in app state.

Stop all Foglio processes before deleting app state. Reselect the original root with `init` afterward. Deleting configuration/cache/data does not delete or reconstruct any note file; tests compare the complete note manifest before and after reselection. Removing lock files while a process holds them defeats cooperative locking and is unsupported.

Default filenames replace ASCII spaces in the title with hyphens and append `.md`, preserving case and Unicode. Titles must be nonempty, single-line; separators are rejected in default filenames. Use `--path` for nested paths. Destinations must be complete relative lowercase `.md` filenames. Traversal, absolute paths, empty/dot components, backslashes, control characters and portable reserved names are rejected. Collisions fail without overwriting. Linux case-only renames are supported and tested.

Selectors are a canonical full uppercase 26-character ULID or relative `.md` path. `id:` validates the complete token strictly; `path:` explicitly requests path inspection. A duplicated ID has no winner: every successfully parsed member is marked ambiguous, ID lookup fails, and mutations through either ID or path fail. Path inspection remains available. Metadata/encoding/access diagnostics that prevent proving ID uniqueness conservatively block existing-note mutations, even when the invalid document is not the selected note. Repair such files manually; Foglio never silently regenerates an existing ID.

Deletion is permanent, with no undo/history. Noninteractive deletion requires `--yes`. With terminal stdin and stderr, Foglio asks for the exact answer `yes`; cancellation leaves bytes unchanged. The revision captured before prompting is checked after confirmation, so editing the file while the prompt is open prevents deletion.

## Core API

`Library::open(root, state_directory, create)` creates an isolated core handle; CLI and future desktop use `Library::resolve`/`Library::init` for shared selection. The root is immutable through the handle. `scan(adopt)` returns sorted entries, diagnostics, and `incomplete`; `Report::summaries` produces lightweight ID/path/title/tag/ambiguity summaries.

`create`, `get`, `update`, `move_note`, `delete`, and `tag` operate on real files. `tag(..., true)` adds; `tag(..., false)` removes, using exact case-sensitive equality and first-occurrence order. Body update is a core operation, not a Phase 1 CLI command. Mutations consume the opaque `Revision` returned with the document. `NoteId` and `LibraryRelativePath` validate identity and filesystem inputs. `ErrorCode` distinguishes domain failures without parsing prose.

Updates/moves/deletes/tags return `Commit { file_committed, durability_confirmed, path, revision }`. Paths in commits are absolute filesystem paths; entry/summary paths are library-relative. Creation returns its committed document. If creation/root selection committed but durability is uncertain, the error has code `committed` and a populated `commit` field. Do not blindly retry a committed mutation. Adoption retains successful entries and reports uncertain durability as a per-file diagnostic. No index outcome exists before Phase 2.

## JSON and exit codes

`--json` emits one object with exactly `result`, `diagnostics`, `incomplete`, and `error`. On success, `error` is null; on failure, `result` is null. Error objects contain `code`, `message`, and nullable `commit`.

- `init`: result contains `root` and `notes` summaries.
- `list`: result is an array of summaries (`id`, `path`, `title`, `tags`, `ambiguous`).
- `show`/`new`: result is the document entry, including complete `source`, `body`, `revision`, metadata and ambiguity flag.
- `tags`: sorted, deduplicated tag strings from unambiguous entries.
- `move`/`delete`/`tag`: commit result.

Human `show` emits exact complete Markdown bytes, including BOM/newlines. Human diagnostics go to stderr. JSON operational and argument errors use the same envelope on stdout, with no human stderr diagnostics. Help/version/no-command help remain human text even with `--json`. Interactive delete prompts still go to terminal stderr; scripts should pass `--yes`.

| Code | Meaning |
|---|---|
| 0 | Complete success |
| 1 | Operational failure, cancellation, incomplete scan, or unconfirmed durability |
| 2 | Usage, invalid path, or invalid selected metadata |
| 3 | Note not found |
| 4 | Stale revision, duplicate/ambiguous selector, occupied destination, or busy library |

Code 5 is reserved for later diagnostic commands; no doctor/index/search/sync stubs exist.

## S01: preservation strategy and supported YAML

The maintained `serde_yaml_ng` fork validates mappings/types/duplicate keys; `yaml-rust2` token scanning rejects anchors, aliases, custom tags, flow mappings and excessive nesting before deserialization. Whole-document YAML serialization is never used for imported notes. App-owned field spans are patched; unknown block mappings, scalar contents and Markdown remain unchanged. Top-level keys must use simple unquoted block-mapping syntax. Unsupported YAML is preserved and diagnosed, not normalized. Nested flow mappings, complex/quoted top-level keys and merges are deliberately unsupported in this release.

Tags accept flow and block string sequences, including indentationless block sequences, Unicode and literal punctuation. Changed tags are written as a JSON-compatible flow sequence. Comments in the replaced field are retained as standalone comments; tag-field formatting/blank lines may change. Unrelated fields and the body remain byte-preserved. Adding an existing tag or removing a missing tag keeps complete bytes unchanged. Missing-ID adoption inserts only the ID and necessary delimiters, preserving BOM and newline style. Existing valid ID-bearing files are never rewritten by scanning.

Frontmatter is only recognized at the beginning after an optional UTF-8 BOM, with `---` opening and `---` or `...` closing lines. LF and CRLF are tested. A closing delimiter at EOF is valid; adding a body inserts the required newline rather than corrupting it. Real Markdown parsing with `pulldown-cmark` derives the first H1, excluding fenced code; fallback is the filename stem.

Limits are 16 MiB per complete note (including generated metadata), 256 KiB frontmatter, indentation at most 64 bytes and container nesting at most 32. Invalid UTF-8, invalid/null IDs, wrong-type tags, duplicate nested/quoted-equivalent keys and unsupported/resource-limited YAML are rejected before writes. Every candidate mutation is reparsed before replacement.

Fixtures and tests: `crates/notes-core/tests/document.rs`, `tests/fixtures/preservation.md` relative to that crate, plus `tests/acceptance/phase1.py`. Red tests reproduced EOF-delimiter corruption and dropped tag comments before fixes; the final suite passes.

## S02: filesystem guarantees and limits

For replacement: acquire per-library nonblocking cooperative lock; validate components; read and check full-byte SHA-256 revision; create a random same-directory `.foglio-stage-*.tmp` at restrictive permissions; verify security metadata; write complete bytes; restore supported mode bits; fsync the file; recheck content/inode/mode/ownership and path; rename; fsync parent. Newly created directory entries are synced through their parents before note commit. Before-commit faults preserve the old note and clean owned staging files. Post-commit faults return committed/unconfirmed durability, not an untouched failure.

Create and move require Linux `renameat2(RENAME_NOREPLACE)` through safe `rustix` APIs, with no copy/delete or link/unlink fallback. Unsupported primitives and cross-device moves fail. Move preserves the existing inode/content/ID. Delete unlinks only one guarded note; empty directories remain. A killed staged writer leaves the old note complete and can leave a complete non-note staging file. Scans ignore identifiable staging files; no automatic stale-temp cleanup deletes user files.

Symlinks at root, ancestor or leaf are rejected; leaf reads use `O_NOFOLLOW | O_NONBLOCK`. Hard-linked files can be read, but mutations refuse them. Ordinary Unix mode bits, owner and group are preserved on replacement. If staging would change owner/group, or either inode has ACLs/xattrs/special mode bits, replacement is refused before publishing data. This intentionally excludes ACL-/security-label-bearing setups rather than weakening their access controls. Read-only missing-ID inputs remain unchanged; read-only valid-ID notes remain readable.

**Security boundary:** component checks plus leaf no-follow are not handle-relative protection against a hostile process swapping ancestors concurrently. Hash checks and cooperative locks are not an atomic compare-and-swap against external editors; an uncooperative write in the final check/rename window can still be overwritten. Do not place the library in an adversarial shared directory. External tools and different devices do not honor the local lock. Platform evidence covers ordinary local Linux filesystem behavior only, not arbitrary network/cloud filesystems or power-loss immunity. macOS/Windows remain unsupported.

## Verification evidence

Local environment: Linux x86_64, Rust/cargo 1.93.1, source workspace filesystem reported `ext2/ext3` by `stat -f`, with `/dev/shm` as a distinct device for the cross-device rejection test. Tests use only temporary directories; subprocess environments isolate HOME/XDG settings. PTYs exercise actual interactive prompts.

Commands executed successfully:

```text
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo build --locked --workspace
cargo test --locked --workspace --all-features
python3 tests/acceptance/phase1.py
/tmp/foglio-phase0-tools/bin/actionlint .github/workflows/ci.yml
git diff --check
```

Rust tests: 22 passed. The single ignored test is a subprocess fixture explicitly invoked by the passing killed-writer parent test, not an unexecuted acceptance case. Python acceptance: 12 passed, including actual ACL and differing supplementary-group refusal on this host, concurrent CLI no-clobber creation, JSON usage errors, interactive cancellation/stale confirmation, and state deletion/reselection.

Real generated IDs from the independently rerun acceptance suite: imported `01M1Y8EGYK9NGYD3W7G2R3Z23F`; created `01M1Y8EGYS281ZVY09YFKZMREQ`. They were generated in disposable fixtures, not hard-coded implementation results.

| Gate | Evidence |
|---|---|
| V01 | fmt/Clippy/build/tests; help/version/usage subprocesses preserve isolated state |
| V02, V04 | repeat init, hidden discovery, preservation adoption, read-only imports, invalid input diagnostics |
| V03 | parser tables and repository fixture, BOM/CRLF, aliases/limits, nested duplicate keys, real H1 |
| V05 | every duplicate member ambiguous; metadata errors cannot hide a collision; explicit path inspection |
| V06 | complete CLI/core lifecycle, body updates, tag idempotence, collision refusal, stable bytes/ID on move |
| V07 | traversal/reserved/non-UTF-8 paths, root/ancestor/leaf symlinks, hardlinks, ACL/group safety, case-only rename |
| V08 | before-write/sync/replace fault injection, after-commit uncertainty, actual killed staged subprocess, stale update/tag/move/delete, external edit during staging, cross-device rejection |
| V09 | actual executable commands, JSON/stdout/stderr/status codes, permanent deletion with noninteractive and PTY confirmation |
| V10 | complete manifest unchanged after deleting app state and reselecting root |

Source §43's five checks are automated: ordinary generated Markdown; lossless imported Markdown adoption; stable identity/bytes on move; unrelated YAML surviving edits; and authoritative notes surviving app-state deletion. Phase 2 onward and the desktop/v1 release gates remain open.
