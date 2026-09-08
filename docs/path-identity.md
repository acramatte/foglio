# Ordinary Markdown and path identity

D26–D28 supersede the embedded-ID assumptions in the original brief. Foglio supports repository documentation, agent specs/plans and personal notes without converting them into app-owned documents.

## Contract

- A document is located by its safe library-relative `.md` path. Core, CLI, search results, watcher payloads and desktop commands use paths, not IDs.
- A full-byte SHA-256 hash is an opaque revision/stale-write precondition. It is not persistent identity: edits change it and distinct documents can share it.
- Supported Markdown needs no frontmatter. Existing `id` fields are ordinary user metadata, neither validated as identifiers nor removed. General YAML safety restrictions still apply, including duplicate-key and unsupported-construct diagnostics.
- `init` creates/selects a root and reports discovery without modifying existing Markdown. Desktop selection, scan, watch, search and index maintenance also preserve source bytes.
- Untagged creation writes plain Markdown. Explicit tags create optional `tags` frontmatter. Body/tag updates preserve unrelated metadata.
- Equal content/metadata at separate paths remains independently readable, searchable and mutable. Unreadable unrelated files do not suppress healthy search results or block a revision-guarded operation on another path. Reports still expose incomplete coverage.
- Moves preserve complete bytes. CLI arguments are literal relative paths, without `id:`/`path:` prefixes. Example: `notes move Projects/plan.md Archive/plan.md`.
- Watcher reconciliation observes moves as old-path deletion and new-path creation, not `NoteMoved`. The read-only desktop leaves the old selected path unavailable until the user selects the new path. It never follows equal hashes. Future app-initiated moves must explicitly update the open path after success; future dirty buffers must not be retargeted by content equality or recreate missing files.

## Cache transition

Schema 2 keys `files`/`notes` by path, with path foreign keys in `tags` and `notes_fts`. Opening known schema 1 discards only the app-owned cache under the existing root lock and rebuilds from Markdown. Unknown future versions are refused, not downgraded. Migration 001 remains as a historical fixture for migration tests, not a runtime compatibility model. No Markdown migration or metadata cleanup is performed.

## Monitoring and scope

Healthy monitoring is shown quietly in the footer as **Monitoring external changes**, with an explanatory tooltip. Monitoring failure remains visible in the alert area. This is not save status. Desktop creation, source editing and autosave remain Phase 5; future save state is separate. Search matching/ranking is unchanged, and search UX improvements remain deferred in the backlog.

## Local verification

The following passed on the Linux workspace using isolated temporary libraries:

- `cargo test --locked --workspace --all-features`, including parser preservation, plain creation/init, path-independent copies, revision guards, schema-1 rebuild and future-schema refusal, watcher recovery and six desktop backend tests.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`.
- `cargo build --locked -p notes-cli`; `python3 tests/acceptance/phase1.py` (12 scenarios); `python3 tests/acceptance/phase2.py` (4 scenarios).
- From `apps/desktop`: `npm test` (18 tests), `npm run typecheck`, `npm run tauri -- build --no-bundle`.
- `FOGLIO_DESKTOP_BINARY="$PWD/target/release/foglio-desktop" TAURI_DRIVER=/tmp/foglio-phase4-tools/bin/tauri-driver python3 tests/acceptance/phase4.py` passed against the actual optimized Tauri application in WebKit/Xvfb: plain Markdown, untouched source, footer status, folder/tag/search navigation, safe preview/links, external move without retargeting, independent identical copy, same-path refresh, permission recovery, deletion and close/reopen.

Independent review follow-up: fixed the containment test's leftover symlink fixture; added a red/green regression for future-schema database-byte and WAL-mode preservation, with version probing before writable journal configuration; added a regression proving update/tag/move/delete at a missing old path cannot retarget either surviving identical document. Full workspace tests, strict Clippy and both CLI acceptance suites passed again after these changes.

Hosted CI, other operating systems, packaging, manual accessibility, large-corpus performance and actual cross-device sync remain unverified for this amendment. Historical phase benchmark results have not been rerun under schema 2. Existing filesystem final-check/rename and ancestor-swap limitations still apply; see [Phase 1](phase1.md#s02-filesystem-guarantees-and-limits).
