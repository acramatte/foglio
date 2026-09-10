# Decision register

Status meanings: **Established** comes from the supplied brief; **Proposed** is a concrete planning default that needs acceptance before implementation; **Spike** needs technical evidence. Phase 0 bootstrap execution is recorded in [tasks](tasks.md); S01/S02, the S04 Linux watcher slice, and the S05 baseline now have evidence. Later editor/release gates remain open.

## Accepted ordinary-Markdown amendment

The user approved removing IDs after trying Phase 4. There is no legacy-user compatibility requirement. These choices supersede the original ID/adoption portions of D03, D04, D06, D14 and D15 below; those rows record history, not current contracts.

| ID | Decision | Rationale |
|---|---|---|
| D26 | Library-relative paths identify documents; full-byte SHA-256 hashes identify revisions. Remove embedded-ID APIs, adoption and duplicate-ID ambiguity. Preserve arbitrary existing `id` metadata. | Ordinary repository specs/plans and notes must work without modifying source. Equal hashes do not establish identity. Known schema-1 cache is disposable; rebuild as schema 2, refuse future schemas. External moves are delete/create observations; no hash-only selection retargeting. |
| D27 | Quiet footer “Monitoring external changes”; visible monitoring failures; separate future editor save state | Watcher activity is not autosave. |
| D28 | Defer search UX changes and leave desktop creation/editing in Phase 5 | This amendment changes identity and monitoring presentation only. |

Current behavior and verification: [path identity](path-identity.md).

## Phase 7 local execution choices

The user authorized Phase 7 implementation on the established Linux-first scope, chose **measure first, then decide budgets**, and explicitly left actual second-device release acceptance blocked. The local package is Debian/amd64 version 0.1.0, with staged payload verification, not a completed fresh-OS/v1 release. `doctor` is strictly read-only; existing explicit `reindex` is the cache repair, never automatic source rewriting. See [Phase 7 evidence](phase7.md) and [benchmarks](phase7-benchmarks.md). Proposed performance numbers below remain proposals.

## Established decisions (historical where superseded)

| ID | Decision | Rationale / source |
|---|---|---|
| D01 | Local Markdown files are authoritative; SQLite/FTS5 is disposable | Portability and rebuild, §§2, 18–20 |
| D02 | Rust headless core shared directly by CLI and Tauri/TypeScript | Reuse without daemon/IPC, §§4–5 |
| D03 | One library, physical directories, minimal YAML `id` and `tags` | §§6–11 |
| D04 | ULID IDs independent of path | Suggested in §9, required by first implementation task §43; examples in brief are illustrative, not valid fixtures |
| D05 | External filesystem sync and external editing | No distributed sync machinery, §§27–30 |
| D06 | Conservative file preservation and no silent duplicate-ID repair | §§23, 32 |
| D07 | Incremental phases; first delivery is Phase 0–1 only | §§36, 42–43 |
| D08 | No speculative sync/encryption/daemon/plugin abstractions | §§3, 42 |

## Accepted Phase 0 decisions

The user approved these choices before implementation:

| ID | Decision | Rationale / gate |
|---|---|---|
| D09 | Product Foglio; crates `notes-core` / `notes-cli`; binary `notes` | Preserve source command contracts; P0-01 accepted |
| D18 | Linux first; macOS/Windows support only after their filesystem and packaging gates pass | Avoid unverified platform claims; P0-01 accepted |
| D21 | MIT license | Explicit owner selection; root LICENSE and Cargo manifests |
| D22 | This request implements Phase 0 only; first usable milestone remains Phase 0–1 | Explicit request scope; do not start Phase 1 implicitly |

## Accepted Phase 1 decisions

The user explicitly requested Phase 1 and approved the proposed defaults before implementation. D22 remains the historical scope of the earlier bootstrap request; it does not constrain this delivery. [Phase 1 evidence](phase1.md) resolves S01/S02 on the tested Linux filesystem.

| ID | Accepted choice | Rationale / evidence |
|---|---|---|
| D10 | XDG config (HOME fallback) outside the library; selected canonical root and root-keyed cooperative locks; no cache until needed | Shared selection and app-state deletion/reselection tests; Phase 1 has no derived DB |
| D11 | Recursive hidden-inclusive lowercase `.md` discovery; no symlink following; no hard-link writes; portable UTF-8 mutation paths | V02/V07 tests; unsupported entries remain unchanged and diagnosed |
| D12 | Validate with pinned `serde_yaml_ng` and `yaml-rust2`; patch owned fields, never reserialize imported YAML | S01 fixture tests; unsupported YAML/resource limits explicitly described in Phase 1 notes |
| D13 | Opaque SHA-256 revisions, cooperative nonblocking locks, same-directory staged/synced replacement, atomic Linux no-replace create/move | S02 stale/fault/process-kill tests; final external-writer/ancestor race remains documented |
| D14 | Every duplicate ID member ambiguous; no silent repair; explicit path inspection; incomplete identity maps block existing-note mutation | V05 plus malformed-metadata duplicate regression |
| D15 | `--library`, `--json`, `id:`/`path:` selectors; permanent deletion needs `--yes` or terminal confirmation | Stable envelope/exit codes and real executable/PTY tests; code 5 reserved for later doctor |
| D20 | No undo/history; confirmed permanent deletion of one note | Stale confirmation and deletion tests; empty directories preserved |
| D23 | Current request authorizes Phase 1 only, after completed Phase 0 | Stop after P1-07; no Phase 2+ scaffolding |
| D24 | Refuse replacements with ACLs/xattrs/special mode bits or changed owner/group; cap complete notes at 16 MiB | Do not silently broaden access or commit notes the reader cannot reopen; actual ACL/group/size tests |

## Accepted Phase 2 decisions

The user explicitly authorized Phase 2 and approved D16 before implementation. Earlier D22/D23 request scopes are historical.

| ID | Accepted choice | Evidence |
|---|---|---|
| D16 | Literal tokens by default, explicit phrase/prefix modes, exact case-sensitive tags and component-aware folders | Core/CLI hostile and Unicode query fixtures; SQLite unicode61 tokenization |
| D25 | Phase 2 only; bundled SQLite/FTS5, transactional cache, root-lock-scoped connections with DELETE journaling | Cache deletion/corruption and concurrent process tests; [Phase 2](phase2.md) |
| S05 baseline | Record real 1k/50k measurements; 100 ms search p95 remains provisional, not a release guarantee | [Raw baseline and environment](phase2-baseline.json); tmpfs and warm OS-cache limits explicit |

## Phase 3 implementation choices

The user explicitly authorized Phase 3. S04 is verified on Linux/tmpfs with independent writers, native atomic-save bursts, startup overlap, subtree changes, permission recovery, duplicate conflict copies, pause/restart and injected dropped hints. See [evidence and limitations](phase3.md). Actual OS suspend and cross-device/provider transfer remain release gates.

- Use pinned `notify` 8.2.0 with nonblocking bounded hints and bounded subscriber queues; access events are ignored to avoid scanner feedback.
- Reconcile full content per dirty batch and periodically, preserving complete identity/unknown-coverage invariants. Default 75 ms debounce, 500 ms maximum batching delay and 30-second safety interval are tunable policies, not benchmark guarantees. Large-library watcher measurements remain P7.
- Publish committed, body-free revisions; consume initial/overflow invalidation before refetching. Track native backend lifetimes on the shared library handle; do not imply daemon or cross-process status.

## Phase 4 implementation choices

User authorized the read-only desktop and installed native prerequisites. P4 uses Tauri 2 with plain TypeScript/Vite and npm lockfiles, avoiding an unnecessary frontend framework for this view. Marked plus DOMPurify provides escaped raw HTML and an explicit safe GFM allowlist. Native links use narrow Rust commands, not broad filesystem/shell/opener plugin capabilities.

Selection is non-adopting and persists the existing core configuration. A managed backend owns watcher subscriptions; blocking workers serialize core operations while cached state stays independent. The frontend polls session/generation state and discards stale responses. Production native WebKit/Xvfb acceptance is recorded in [Phase 4](phase4.md); editing, native installers and other OS targets remain excluded.

## Phase 5 implementation choices

The user authorized Phase 5 and review corrections. **D17 resolved:** source-body editor plus existing sanitized preview; no rich mode. S03's real Milkdown 7.22.1 parser/model-transaction/serializer spike lost fence metadata and changed unsupported directives/reference structure. Source updates preserve frontmatter outside the editor, map sequential textarea changes against current raw source and keep exact saved bytes for no-op/reversion. See [evidence](phase5.md).

Autosave uses a 400 ms debounce with per-selected-note serialized revision/generation snapshots. Native close is a frontend flush handshake followed by blocking watcher shutdown. Failed operations retain buffers; stale/missing paths never force overwrite/recreate. Lifecycle operations retain their navigation lock through follow-up reads. D19 conflict choices remain Phase 6. No new rich-editor dependencies or broad filesystem/shell permissions were added.

## Phase 6 implementation choices

User authorized Phase 6. **D19 resolved:** explicit reload/local-discard or save local as a new note at a distinct literal no-clobber path. Both choices reobserve the original path after confirmation; core copy checks raw revision/absence under one lock before staging and just before replacement. Original frontmatter/BOM are retained separately from local body edits. No force-write, hash retargeting or implicit missing-path recreation.

Watcher reads pause queued autosaves and wait for current acknowledgements. Saves verify their committed revision before clearing the buffer or allowing navigation/close, including delayed acknowledgements without newer typing. A committed write whose readback fails is not replayed by Retry save. Native two-process and simulated sync-copy evidence, cooperative-lock contention recovery, supported-YAML limitations and final external-writer race limits are recorded in [Phase 6](phase6.md).

## Proposed implementation defaults for later phases

| ID | Proposal | Why / decision gate |
|---|---|---|
| D10 | OS-native config/cache directories outside library; config selects root, cache keyed by canonical root | No synced database/lock files; moving root may rebuild; resolve in P1-01 |
| D11 | Lowercase `.md` regular files only; do not follow symlinks; reject symlink root and mutation paths; refuse writes to hard-linked notes | Tight portable security boundary; non-UTF-8 names diagnosed and not mutated; resolve in P1-01 |
| D12 | Preserve body bytes, BOM/newlines, unrelated frontmatter text where practical; malformed YAML, duplicate keys, invalid IDs/tags are non-mutating diagnostics | General YAML serialization can destroy comments or values; resolve parser strategy in P1-02 |
| D13 | Hash-based revision preconditions for updates/moves/deletes plus local app-process serialization | Protect against detectable stale writes without storing revision metadata in notes; resolve in P1-03 |
| D14 | Duplicate ID makes every member ambiguous; exclude the whole group from ID lookup/search until manually repaired | Never let scan order choose a winner; path-based diagnostic viewing stays available; resolve in P1-04 |
| D15 | CLI human output plus stable `--json`; explicit `id:` / `path:` selectors disambiguate; destructive CLI delete requires `--yes` or terminal confirmation | Scriptability and deliberate destruction; resolve in P1-06 |
| D16 | Literal text search by default, explicit phrase/prefix modes, exact case-sensitive tag filter, component-aware folder filter | Avoid exposing raw FTS syntax accidentally; resolve in P2-02 |
| D17 | Source editor and sanitized preview first; rich mode only if round-trip spike passes | Meet v1 without sacrificing Markdown preservation; resolve in P5-01 |
| D19 | Dirty conflict resolution offers reload (explicit discard) or save local buffer as a new note at a new no-clobber path; no blind force-overwrite | Preserve both versions without introducing history; resolved in Phase 6 above |
| D20 | No delete undo/history in v1; delete is a confirmed permanent filesystem removal | Source requires deletion but excludes history; make limitation visible; resolve in P1-06 |

## Evidence-required spikes

| ID | Question | Evidence / bound | Gate |
|---|---|---|---|
| S01 | Can chosen parser safely edit only app-owned YAML fields? | Fixtures with unknown nested values, comments, anchors/aliases, duplicate keys, delimiters, CRLF/BOM, malformed input. Select maintained parsing/token tooling; explicitly refuse unrepresentable mutations rather than round-trip lossy YAML. One focused spike, not a parser framework. | P1-02 |
| S02 | Which safe-replace/no-clobber primitives work on declared targets? | Actual temp-directory fault tests for flush/replace, parent durability, collision, symlink/hard-link checks, move interruption; document unsupported filesystems and external-writer race window. | P1-03 |
| S03 | Which editor preserves the required Markdown? | Compare source/preview baseline with Milkdown/ProseMirror using untouched round trips and edited fixtures; reject silent loss. No rich mode if evidence fails. | P5-01 |
| S04 | Watcher and shared cache under real filesystem behavior? | Multiple processes, atomic-save external editor, directory moves, overflow recovery, permissions, sleeping/reopening; verify on declared OS/filesystem matrix. | P3-01 through P3-03 |
| S05 | What performance thresholds are reproducible? | Fixed hardware, fixture distribution, cold/warm cache definitions; adopt numeric targets before optimization, report distributions, never invent benchmark results. | P2-04 and P7-03 |

## Risks to keep visible

1. Atomic replacement prevents partial files but is not an atomic compare-and-swap against arbitrary external editors. Hash checks narrow, not eliminate, the final check/write race. Cooperative locks protect Foglio processes only.
2. Unsupported/malformed/unstable input must remain untouched; report diagnostics without destructive fallback.
3. Paths are not permanent identity across external moves. Without reliable move evidence, show the old path missing.
4. Equal content hashes or metadata do not prove identity. Never retarget a dirty buffer or merge conflict copies using equality alone.
5. Filesystem and SQLite cannot share a transaction. Durable file commit must not be reported as if nothing happened merely because indexing failed.
6. Metadata-only warm scans can miss same-size/same-mtime edits while offline. Running watcher paths require content checks; full rescan/reindex is the definitive recovery. Document freshness limits rather than promising impossible cheap certainty.
7. Mounts/network/cloud placeholder files may not support tested atomic/durability semantics. Detect/report unsupported behavior; do not claim all remote filesystems safe.

Decisions may be amended only with explicit rationale and updated affected specs/tests/tasks. Established choices are not reopened just to explore alternative stacks.
